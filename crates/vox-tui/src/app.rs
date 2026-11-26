//! Editor state and key handling.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use vox_core::{Document, Position, motion};
use vox_view::{Viewport, display_col};

/// Rows reserved below the text area: statusline + command line.
pub(crate) const CHROME_ROWS: u16 = 2;

/// Editor input mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Navigation and commands.
    Normal,
    /// Text entry.
    Insert,
    /// `:` command line; holds the text typed so far.
    Command(String),
}

/// Top-level editor state.
#[derive(Debug)]
pub struct App {
    pub(crate) doc: Document,
    pub(crate) view: Viewport,
    pub(crate) cursor: Position,
    /// Column `j`/`k` try to keep.
    want_col: usize,
    pub(crate) mode: Mode,
    /// Terminal size (width, height), updated each frame.
    size: (u16, u16),
    /// Pending `g` prefix for `gg`.
    pending_g: bool,
    pub(crate) message: Option<String>,
    quit: bool,
}

impl App {
    /// Create an app editing `doc`.
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            view: Viewport::default(),
            cursor: Position::default(),
            want_col: 0,
            mode: Mode::Normal,
            size: (80, 24),
            pending_g: false,
            message: None,
            quit: false,
        }
    }

    /// Whether the app has been asked to quit.
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Current mode.
    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    /// The document being edited.
    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// Cursor position.
    pub fn cursor(&self) -> Position {
        self.cursor
    }

    /// Current viewport.
    pub fn viewport(&self) -> Viewport {
        self.view
    }

    /// Last message shown on the command line, if any.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Update the terminal size and keep the cursor visible.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.size = (width, height);
        self.follow_cursor();
    }

    /// Width of the line-number gutter including its trailing space.
    pub(crate) fn gutter_width(&self) -> usize {
        self.doc.line_count().to_string().len().max(3) + 1
    }

    /// Text area size (width, height) in cells.
    pub(crate) fn text_area(&self) -> (usize, usize) {
        let width = (self.size.0 as usize)
            .saturating_sub(self.gutter_width())
            .max(1);
        let height = self.size.1.saturating_sub(CHROME_ROWS).max(1) as usize;
        (width, height)
    }

    /// Display column of the cursor within its line.
    pub(crate) fn cursor_dcol(&self) -> usize {
        display_col(
            &self.doc.line(self.cursor.line).unwrap_or_default(),
            self.cursor.col,
        )
    }

    fn follow_cursor(&mut self) {
        let (w, h) = self.text_area();
        self.view.follow(self.cursor.line, self.cursor_dcol(), w, h);
    }

    /// Handle one key press.
    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::Insert => self.handle_insert(key),
            Mode::Command(_) => self.handle_command(key),
        }
        self.follow_cursor();
    }

    /// Highest column the cursor may occupy on `line` in the current mode.
    fn max_col(&self, line: usize) -> usize {
        let len = self.doc.line_len(line);
        match self.mode {
            Mode::Insert => len,
            _ => len.saturating_sub(1),
        }
    }

    /// Move to `pos` (clamped) and remember its column for vertical moves.
    fn set_cursor(&mut self, pos: Position) {
        let line = pos.line.min(self.doc.line_count() - 1);
        self.cursor = Position::new(line, pos.col.min(self.max_col(line)));
        self.want_col = self.cursor.col;
    }

    /// Move `delta` lines, keeping the wanted column.
    fn move_lines(&mut self, delta: isize) {
        let line = self
            .cursor
            .line
            .saturating_add_signed(delta)
            .min(self.doc.line_count() - 1);
        self.cursor = Position::new(line, self.want_col.min(self.max_col(line)));
    }

    fn enter_insert(&mut self, pos: Position) {
        self.mode = Mode::Insert;
        self.set_cursor(pos);
    }

    fn handle_normal(&mut self, key: KeyEvent) {
        let (_, height) = self.text_area();
        let half = (height / 2).max(1) as isize;
        let page = height as isize;
        let lines = self.doc.line_count();
        let pending_g = std::mem::take(&mut self.pending_g);
        let c = self.cursor;

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            let delta = match key.code {
                KeyCode::Char('d') => half,
                KeyCode::Char('u') => -half,
                KeyCode::Char('f') => page,
                KeyCode::Char('b') => -page,
                KeyCode::Char('r') => {
                    self.redo();
                    return;
                }
                _ => return,
            };
            self.view.scroll_by(delta, lines);
            self.move_lines(delta);
            return;
        }

        match key.code {
            KeyCode::Char('h') | KeyCode::Left => {
                self.set_cursor(Position::new(c.line, c.col.saturating_sub(1)))
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.set_cursor(Position::new(c.line, c.col + 1))
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_lines(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_lines(-1),
            KeyCode::PageDown => {
                self.view.scroll_by(page, lines);
                self.move_lines(page);
            }
            KeyCode::PageUp => {
                self.view.scroll_by(-page, lines);
                self.move_lines(-page);
            }
            KeyCode::Char('w') => self.set_cursor(motion::word_forward(&self.doc, c)),
            KeyCode::Char('b') => self.set_cursor(motion::word_backward(&self.doc, c)),
            KeyCode::Char('0') | KeyCode::Home => self.set_cursor(motion::line_start(c)),
            KeyCode::Char('$') | KeyCode::End => {
                self.set_cursor(motion::line_end(&self.doc, c));
                self.want_col = usize::MAX;
            }
            KeyCode::Char('g') if pending_g => self.set_cursor(motion::doc_start(&self.doc)),
            KeyCode::Char('g') => self.pending_g = true,
            KeyCode::Char('G') => self.set_cursor(motion::doc_end(&self.doc)),
            KeyCode::Char('i') => self.enter_insert(c),
            KeyCode::Char('a') => {
                let col = (c.col + 1).min(self.doc.line_len(c.line));
                self.enter_insert(Position::new(c.line, col));
            }
            KeyCode::Char('o') => {
                let eol = Position::new(c.line, self.doc.line_len(c.line));
                let end = self.doc.insert(eol, self.doc.line_ending());
                self.enter_insert(end);
            }
            KeyCode::Char('O') => {
                self.doc
                    .insert(Position::new(c.line, 0), self.doc.line_ending());
                self.enter_insert(Position::new(c.line, 0));
            }
            KeyCode::Char('x') => {
                if self.doc.line_len(c.line) > 0 {
                    self.doc.delete(c, Position::new(c.line, c.col + 1));
                    self.doc.commit();
                    self.set_cursor(c);
                }
            }
            KeyCode::Char('u') => self.undo(),
            KeyCode::Char(':') => {
                self.message = None;
                self.mode = Mode::Command(String::new());
            }
            _ => {}
        }
    }

    fn handle_insert(&mut self, key: KeyEvent) {
        let c = self.cursor;
        match key.code {
            KeyCode::Esc => {
                self.doc.commit();
                self.mode = Mode::Normal;
                self.set_cursor(Position::new(c.line, c.col.saturating_sub(1)));
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                let end = self.doc.insert(c, ch.encode_utf8(&mut [0; 4]));
                self.set_cursor(end);
            }
            KeyCode::Tab => {
                let end = self.doc.insert(c, "\t");
                self.set_cursor(end);
            }
            KeyCode::Enter => {
                let end = self.doc.insert(c, self.doc.line_ending());
                self.set_cursor(end);
            }
            KeyCode::Backspace => {
                if c.col > 0 {
                    let prev = Position::new(c.line, c.col - 1);
                    self.doc.delete(prev, c);
                    self.set_cursor(prev);
                } else if c.line > 0 {
                    let prev = Position::new(c.line - 1, self.doc.line_len(c.line - 1));
                    self.doc.delete(prev, c);
                    self.set_cursor(prev);
                }
            }
            KeyCode::Left => self.set_cursor(Position::new(c.line, c.col.saturating_sub(1))),
            KeyCode::Right => self.set_cursor(Position::new(c.line, c.col + 1)),
            KeyCode::Up => self.move_lines(-1),
            KeyCode::Down => self.move_lines(1),
            _ => {}
        }
    }

    fn undo(&mut self) {
        match self.doc.undo() {
            Some(pos) => self.set_cursor(pos),
            None => self.message = Some("Already at oldest change".into()),
        }
    }

    fn redo(&mut self) {
        match self.doc.redo() {
            Some(pos) => self.set_cursor(pos),
            None => self.message = Some("Already at newest change".into()),
        }
    }

    fn handle_command(&mut self, key: KeyEvent) {
        let Mode::Command(input) = &mut self.mode else {
            return;
        };
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Backspace => {
                if input.pop().is_none() {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Enter => {
                let cmd = std::mem::take(input);
                self.mode = Mode::Normal;
                self.execute(cmd.trim());
            }
            KeyCode::Char(c) => input.push(c),
            _ => {}
        }
    }

    fn execute(&mut self, cmd: &str) {
        let (name, arg) = match cmd.split_once(char::is_whitespace) {
            Some((n, a)) => (n, Some(a.trim()).filter(|a| !a.is_empty())),
            None => (cmd, None),
        };
        let (name, force) = match name.strip_suffix('!') {
            Some(n) => (n, true),
            None => (name, false),
        };
        match name {
            "" => {}
            "q" | "quit" | "qa" | "qall" => {
                if self.doc.is_modified() && !force {
                    self.message = Some(E37.into());
                } else {
                    self.quit = true;
                }
            }
            "w" | "write" => {
                self.write(arg);
            }
            "wq" | "x" | "xit" => {
                if (name != "wq" && !self.doc.is_modified()) || self.write(arg) {
                    self.quit = true;
                }
            }
            "e" | "edit" => self.edit(arg, force),
            _ => self.message = Some(format!("E492: Not an editor command: {cmd}")),
        }
    }

    /// Save, optionally to a new path. Returns whether the write succeeded.
    fn write(&mut self, path: Option<&str>) -> bool {
        if path.is_none() && self.doc.path().is_none() {
            self.message = Some("E32: No file name".into());
            return false;
        }
        let result = match path {
            Some(p) => self.doc.save_as(p),
            None => self.doc.save(),
        };
        let name = self
            .doc
            .path()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        match result {
            Ok(bytes) => {
                self.message = Some(format!(
                    "\"{name}\" {}L, {bytes}B written",
                    self.doc.line_count()
                ));
                true
            }
            Err(e) => {
                self.message = Some(format!("E212: Can't open file for writing: {name}: {e}"));
                false
            }
        }
    }

    /// Open `path`, or reload the current file when `path` is `None`.
    fn edit(&mut self, path: Option<&str>, force: bool) {
        if self.doc.is_modified() && !force {
            self.message = Some(E37.into());
            return;
        }
        let path = match path.map(std::path::PathBuf::from) {
            Some(p) => p,
            None => match self.doc.path() {
                Some(p) => p.to_path_buf(),
                None => {
                    self.message = Some("E32: No file name".into());
                    return;
                }
            },
        };
        match Document::open(&path) {
            Ok(doc) => {
                self.message = Some(format!("\"{}\" {}L", path.display(), doc.line_count()));
                self.doc = doc;
                self.view = Viewport::default();
                self.set_cursor(Position::default());
            }
            Err(e) => self.message = Some(format!("E484: Can't open file {}: {e}", path.display())),
        }
    }
}

const E37: &str = "E37: No write since last change (add ! to override)";
