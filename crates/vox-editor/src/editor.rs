//! Editor state and key handling.

use vox_core::storage::{SimpleStorage, StorageFactory};
use vox_core::{Document, Position, motion, search};
use vox_view::{CharWidth, Monospace, Viewport, display_col};

use crate::keys::{Key, KeyCode};

use crate::normal::{self, Action, Cmd, InsertAt, Motion, Operator, Parse, Scroll, Target};
use crate::register::{self, Clipboard, NoClipboard, Reg, Registers};

/// Rows reserved below the text area: statusline + command line.
pub(crate) const CHROME_ROWS: u16 = 2;

const E37: &str = "E37: No write since last change (add ! to override)";

/// Editor input mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Navigation and commands.
    Normal,
    /// Text entry.
    Insert,
    /// Selection from the anchor to the cursor; `line` selects whole lines.
    Visual {
        /// Linewise (`V`) instead of charwise (`v`).
        line: bool,
    },
    /// `:` command line; holds the text typed so far.
    Command(String),
    /// `/` search prompt; holds the text typed so far.
    Search(String),
}

/// How a motion's range is interpreted by an operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Exclusive,
    Inclusive,
    Linewise,
}

/// Text an operator acts on. For linewise ranges only the lines of `start`/`end` matter.
#[derive(Debug, Clone, Copy)]
struct Range {
    start: Position,
    /// Exclusive end (charwise) or last line (linewise).
    end: Position,
    linewise: bool,
}

/// Platform services the editor uses. Adapters provide real implementations; the
/// defaults are std-only stand-ins suitable for tests.
pub struct Services {
    /// Creates text storage for documents opened with `:e`.
    pub storage: StorageFactory,
    /// Terminal cell width of chars.
    pub width: Box<dyn CharWidth>,
    /// Backing store of the `"+` / `"*` registers.
    pub clipboard: Box<dyn Clipboard>,
}

impl Default for Services {
    fn default() -> Self {
        Self {
            storage: SimpleStorage::boxed,
            width: Box::new(Monospace),
            clipboard: Box::new(NoClipboard),
        }
    }
}

/// Top-level editor state.
pub struct Editor {
    pub(crate) doc: Document,
    pub(crate) view: Viewport,
    pub(crate) cursor: Position,
    /// Column `j`/`k` try to keep.
    want_col: usize,
    pub(crate) mode: Mode,
    /// Visual mode anchor.
    anchor: Position,
    /// Terminal size (width, height), updated each frame.
    pub(crate) size: (u16, u16),
    /// Keys of an incomplete normal/visual command.
    pending: Vec<Key>,
    registers: Registers,
    last_search: Option<String>,
    pub(crate) message: Option<String>,
    quit: bool,
    storage: StorageFactory,
    pub(crate) width: Box<dyn CharWidth>,
}

impl std::fmt::Debug for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor")
            .field("doc", &self.doc)
            .field("cursor", &self.cursor)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl Editor {
    /// Create an editor for `doc` with default (std-only) [`Services`].
    pub fn new(doc: Document) -> Self {
        Self::with_services(doc, Services::default())
    }

    /// Create an editor for `doc` using `services`.
    pub fn with_services(doc: Document, services: Services) -> Self {
        let mut registers = Registers::default();
        registers.clipboard = services.clipboard;
        Self {
            doc,
            view: Viewport::default(),
            cursor: Position::default(),
            want_col: 0,
            mode: Mode::Normal,
            anchor: Position::default(),
            size: (80, 24),
            pending: Vec::new(),
            registers,
            last_search: None,
            message: None,
            quit: false,
            storage: services.storage,
            width: services.width,
        }
    }

    /// Replace the clipboard used by the `"+` and `"*` registers.
    pub fn set_clipboard(&mut self, clipboard: Box<dyn Clipboard>) {
        self.registers.clipboard = clipboard;
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

    /// Visual selection as (start, end inclusive, linewise), if in visual mode.
    pub fn selection(&self) -> Option<(Position, Position, bool)> {
        let Mode::Visual { line } = self.mode else {
            return None;
        };
        let (a, b) = order(self.anchor, self.cursor);
        Some((a, b, line))
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
            self.width.as_ref(),
            &self.doc.line(self.cursor.line).unwrap_or_default(),
            self.cursor.col,
        )
    }

    /// Scroll the viewport just enough to keep the cursor on screen.
    fn follow_cursor(&mut self) {
        let (w, h) = self.text_area();
        self.view.follow(self.cursor.line, self.cursor_dcol(), w, h);
    }

    /// Handle one key press.
    pub fn handle_key(&mut self, key: Key) {
        match self.mode {
            Mode::Normal | Mode::Visual { .. } => self.handle_normal(key),
            Mode::Insert => self.handle_insert(key),
            Mode::Command(_) | Mode::Search(_) => self.handle_prompt(key),
        }
        self.follow_cursor();
    }

    /// Index of the last line.
    fn last_line(&self) -> usize {
        self.doc.line_count() - 1
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
        let line = pos.line.min(self.last_line());
        self.cursor = Position::new(line, pos.col.min(self.max_col(line)));
        self.want_col = self.cursor.col;
    }

    /// Move `delta` lines, keeping the wanted column.
    fn move_lines(&mut self, delta: isize) {
        let line = self
            .cursor
            .line
            .saturating_add_signed(delta)
            .min(self.last_line());
        self.cursor = Position::new(line, self.want_col.min(self.max_col(line)));
    }

    /// Switch to insert mode with the cursor at `pos`.
    fn enter_insert(&mut self, pos: Position) {
        self.mode = Mode::Insert;
        self.set_cursor(pos);
    }

    /// Normal/visual mode: collect keys until they form a complete command, then run it.
    fn handle_normal(&mut self, key: Key) {
        if key.code == KeyCode::Esc {
            self.pending.clear();
            if matches!(self.mode, Mode::Visual { .. }) {
                self.mode = Mode::Normal;
            }
            return;
        }
        self.pending.push(key);
        let visual = matches!(self.mode, Mode::Visual { .. });
        match normal::parse(&self.pending, visual) {
            Parse::Incomplete => {}
            Parse::Invalid => self.pending.clear(),
            Parse::Done(cmd) => {
                self.pending.clear();
                self.exec(cmd);
            }
        }
    }

    /// Run one parsed normal/visual mode command (motion, operator, insert, paste, undo, ...).
    fn exec(&mut self, cmd: Cmd) {
        if let Some(r) = cmd.register
            && !register::is_valid(r)
        {
            self.message = Some(format!("E354: Invalid register name: '{r}'"));
            return;
        }
        let n = cmd.count.unwrap_or(1);
        let c = self.cursor;
        match cmd.action {
            Action::Move(Motion::Up) => self.move_lines(-(n as isize)),
            Action::Move(Motion::Down) => self.move_lines(n as isize),
            Action::Move(m) => {
                if let Some((pos, _)) = self.motion_target(m, cmd.count, false) {
                    self.set_cursor(pos);
                    if m == Motion::LineEnd {
                        self.want_col = usize::MAX;
                    }
                }
            }
            Action::Operate(op, target) => self.operate(op, target, cmd.count, cmd.register),
            Action::Insert(at) => {
                let pos = match at {
                    InsertAt::Before => c,
                    InsertAt::After => {
                        Position::new(c.line, (c.col + 1).min(self.doc.line_len(c.line)))
                    }
                    InsertAt::LineStart => motion::first_non_blank(&self.doc, c.line),
                    InsertAt::LineEnd => Position::new(c.line, self.doc.line_len(c.line)),
                    InsertAt::Below => {
                        let eol = Position::new(c.line, self.doc.line_len(c.line));
                        self.doc.insert(eol, self.doc.line_ending())
                    }
                    InsertAt::Above => {
                        self.doc
                            .insert(Position::new(c.line, 0), self.doc.line_ending());
                        Position::new(c.line, 0)
                    }
                };
                self.enter_insert(pos);
            }
            Action::Paste { before } => self.paste(before, n, cmd.register),
            Action::Undo => (0..n).for_each(|_| self.undo()),
            Action::Redo => (0..n).for_each(|_| self.redo()),
            Action::Scroll(s) => {
                let (_, height) = self.text_area();
                let half = (height / 2).max(1) as isize;
                let delta = match s {
                    Scroll::HalfDown => half,
                    Scroll::HalfUp => -half,
                    Scroll::PageDown => height as isize,
                    Scroll::PageUp => -(height as isize),
                };
                self.view.scroll_by(delta, self.doc.line_count());
                self.move_lines(delta);
            }
            Action::Visual { line } => match self.mode {
                Mode::Visual { line: cur } if cur == line => self.mode = Mode::Normal,
                Mode::Visual { .. } => self.mode = Mode::Visual { line },
                _ => {
                    self.anchor = c;
                    self.mode = Mode::Visual { line };
                }
            },
            Action::CommandLine => {
                self.message = None;
                self.mode = Mode::Command(String::new());
            }
            Action::Search => {
                self.message = None;
                self.mode = Mode::Search(String::new());
            }
        }
    }

    /// Where motion `m` (repeated `count` times) leads from the cursor, and its kind.
    /// `for_op` enables operator-only rules (`dw` stops at end of line).
    fn motion_target(
        &mut self,
        m: Motion,
        count: Option<usize>,
        for_op: bool,
    ) -> Option<(Position, Kind)> {
        let c = self.cursor;
        let n = count.unwrap_or(1);
        let doc = &self.doc;
        let repeat = |f: fn(&Document, Position) -> Position| (0..n).fold(c, |p, _| f(doc, p));
        let line_at = |l: usize| l.min(doc.line_count() - 1);
        Some(match m {
            Motion::Left => (
                Position::new(c.line, c.col.saturating_sub(n)),
                Kind::Exclusive,
            ),
            Motion::Right => (
                Position::new(c.line, (c.col + n).min(doc.line_len(c.line))),
                Kind::Exclusive,
            ),
            Motion::Up => (
                Position::new(c.line.saturating_sub(n), self.want_col),
                Kind::Linewise,
            ),
            Motion::Down => (
                Position::new(line_at(c.line + n), self.want_col),
                Kind::Linewise,
            ),
            Motion::WordForward => {
                let mut p = c;
                for step in 0..n {
                    let next = match motion::next_word_start(doc, p) {
                        Some(next) => next,
                        // No further word: an operator reaches the end of the document.
                        None if for_op => {
                            let last = doc.line_count() - 1;
                            p = Position::new(last, doc.line_len(last));
                            break;
                        }
                        None => motion::word_forward(doc, p),
                    };
                    if for_op && step == n - 1 && next.line > p.line {
                        // `dw` on the last word of a line stops at the line end.
                        p = Position::new(p.line, doc.line_len(p.line));
                        break;
                    }
                    p = next;
                }
                (p, Kind::Exclusive)
            }
            Motion::WordBackward => (repeat(motion::word_backward), Kind::Exclusive),
            Motion::WordEnd => (repeat(motion::word_end), Kind::Inclusive),
            Motion::LineStart => (motion::line_start(c), Kind::Exclusive),
            Motion::FirstNonBlank => (motion::first_non_blank(doc, c.line), Kind::Exclusive),
            Motion::LineEnd => {
                let line = line_at(c.line + n - 1);
                (
                    motion::line_end(doc, Position::new(line, 0)),
                    Kind::Inclusive,
                )
            }
            Motion::FileStart => (
                motion::first_non_blank(doc, count.map_or(0, |l| line_at(l.saturating_sub(1)))),
                Kind::Linewise,
            ),
            Motion::FileEnd => (
                motion::first_non_blank(
                    doc,
                    count.map_or(doc.line_count() - 1, |l| line_at(l.saturating_sub(1))),
                ),
                Kind::Linewise,
            ),
            Motion::SearchNext | Motion::SearchPrev => {
                let pos = self.search_target(m == Motion::SearchNext, n)?;
                (pos, Kind::Exclusive)
            }
        })
    }

    /// Repeat the last search `n` times. Sets messages for wrap / not found.
    fn search_target(&mut self, forward: bool, n: usize) -> Option<Position> {
        let Some(pattern) = self.last_search.clone() else {
            self.message = Some("E35: No previous regular expression".into());
            return None;
        };
        let mut pos = self.cursor;
        let mut wrapped = false;
        for _ in 0..n {
            match search::find(&self.doc, pos, &pattern, forward) {
                Some(m) => {
                    pos = m.pos;
                    wrapped |= m.wrapped;
                }
                None => {
                    self.message = Some(format!("E486: Pattern not found: {pattern}"));
                    return None;
                }
            }
        }
        self.message = if wrapped {
            Some(
                if forward {
                    "search hit BOTTOM, continuing at TOP"
                } else {
                    "search hit TOP, continuing at BOTTOM"
                }
                .into(),
            )
        } else {
            Some(format!("/{pattern}"))
        };
        Some(pos)
    }

    /// Work out the text range an operator acts on (from a motion, `dd`-style lines, or the visual selection), then apply it.
    fn operate(&mut self, op: Operator, target: Target, count: Option<usize>, reg: Option<char>) {
        let c = self.cursor;
        let n = count.unwrap_or(1);
        let range = match target {
            Target::Lines => Range {
                start: Position::new(c.line, 0),
                end: Position::new((c.line + n - 1).min(self.last_line()), 0),
                linewise: true,
            },
            Target::Selection => {
                let Some((a, b, line)) = self.selection() else {
                    return;
                };
                self.mode = Mode::Normal;
                Range {
                    start: a,
                    end: Position::new(b.line, b.col + 1),
                    linewise: line,
                }
            }
            Target::Motion(Motion::WordForward)
                if op == Operator::Change
                    && self
                        .doc
                        .line(c.line)
                        .and_then(|l| l.chars().nth(c.col))
                        .is_some_and(|ch| !ch.is_whitespace()) =>
            {
                // `cw` on a word changes to the end of the word, like `ce`.
                let first = motion::current_word_end(&self.doc, c);
                let end = (1..n).fold(first, |p, _| motion::word_end(&self.doc, p));
                Range {
                    start: c,
                    end: Position::new(end.line, end.col + 1),
                    linewise: false,
                }
            }
            Target::Motion(m) => {
                let Some((t, kind)) = self.motion_target(m, count, true) else {
                    return;
                };
                let (a, b) = order(c, t);
                match kind {
                    Kind::Linewise => Range {
                        start: Position::new(a.line, 0),
                        end: Position::new(b.line, 0),
                        linewise: true,
                    },
                    Kind::Inclusive => Range {
                        start: a,
                        end: Position::new(b.line, b.col + 1),
                        linewise: false,
                    },
                    Kind::Exclusive => Range {
                        start: a,
                        end: b,
                        linewise: false,
                    },
                }
            }
        };
        self.apply(op, range, reg);
    }

    /// Apply operator `op` to range `r`: save the text into the register, then delete, change, or yank.
    fn apply(&mut self, op: Operator, r: Range, reg: Option<char>) {
        let (a, b) = (r.start.line, r.end.line);
        let text = if r.linewise {
            let body = self
                .doc
                .slice(Position::new(a, 0), Position::new(b, self.doc.line_len(b)));
            format!("{body}\n")
        } else {
            self.doc.slice(r.start, r.end)
        };
        let reg_value = Reg {
            text: text.replace("\r\n", "\n"),
            linewise: r.linewise,
        };
        if let Err(e) = self.registers.set(reg, reg_value) {
            self.message = Some(format!("Clipboard error: {e}"));
        }
        match op {
            Operator::Yank => {
                if r.linewise {
                    self.set_cursor(Position::new(a, self.cursor.col));
                } else {
                    self.set_cursor(r.start);
                }
            }
            Operator::Delete => {
                if r.linewise {
                    self.delete_lines(a, b);
                    self.set_cursor(motion::first_non_blank(&self.doc, a.min(self.last_line())));
                } else {
                    self.doc.delete(r.start, r.end);
                    self.set_cursor(r.start);
                }
                self.doc.commit();
            }
            Operator::Change => {
                if r.linewise {
                    let end = Position::new(b, self.doc.line_len(b));
                    self.doc.delete(Position::new(a, 0), end);
                    self.enter_insert(Position::new(a, 0));
                } else {
                    self.doc.delete(r.start, r.end);
                    self.enter_insert(r.start);
                }
            }
        }
    }

    /// Delete lines `a..=b` including their line breaks.
    fn delete_lines(&mut self, a: usize, b: usize) {
        let len = |l| self.doc.line_len(l);
        if b < self.last_line() {
            self.doc
                .delete(Position::new(a, 0), Position::new(b + 1, 0));
        } else if a > 0 {
            let (from, to) = (Position::new(a - 1, len(a - 1)), Position::new(b, len(b)));
            self.doc.delete(from, to);
        } else {
            let to = Position::new(b, len(b));
            self.doc.delete(Position::new(0, 0), to);
        }
    }

    /// `p` / `P`: insert register text `n` times after or before the cursor (below or above for whole lines).
    fn paste(&mut self, before: bool, n: usize, reg: Option<char>) {
        let reg = match self.registers.get(reg) {
            Ok(r) => r,
            Err(e) => {
                self.message = Some(e);
                return;
            }
        };
        let le = self.doc.line_ending();
        let content = reg.text.repeat(n);
        let c = self.cursor;
        if reg.linewise {
            let body = content
                .strip_suffix('\n')
                .unwrap_or(&content)
                .replace('\n', le);
            let line = if before {
                self.doc
                    .insert(Position::new(c.line, 0), &format!("{body}{le}"));
                c.line
            } else {
                let eol = Position::new(c.line, self.doc.line_len(c.line));
                self.doc.insert(eol, &format!("{le}{body}"));
                c.line + 1
            };
            self.set_cursor(motion::first_non_blank(&self.doc, line));
        } else {
            let body = content.replace('\n', le);
            let at = if before || self.doc.line_len(c.line) == 0 {
                c
            } else {
                Position::new(c.line, c.col + 1)
            };
            let end = self.doc.insert(at, &body);
            if body.contains('\n') {
                self.set_cursor(at);
            } else {
                self.set_cursor(Position::new(end.line, end.col.saturating_sub(1)));
            }
        }
        self.doc.commit();
    }

    /// Insert mode: type text, new lines, backspace, arrows; `Esc` returns to normal mode.
    fn handle_insert(&mut self, key: Key) {
        let c = self.cursor;
        match key.code {
            KeyCode::Esc => {
                self.doc.commit();
                self.mode = Mode::Normal;
                self.set_cursor(Position::new(c.line, c.col.saturating_sub(1)));
            }
            KeyCode::Char(ch) if !key.mods.ctrl => {
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

    /// `u`: undo one step and move the cursor to the change.
    fn undo(&mut self) {
        match self.doc.undo() {
            Some(pos) => self.set_cursor(pos),
            None => self.message = Some("Already at oldest change".into()),
        }
    }

    /// `Ctrl-r`: redo one step and move the cursor to the change.
    fn redo(&mut self) {
        match self.doc.redo() {
            Some(pos) => self.set_cursor(pos),
            None => self.message = Some("Already at newest change".into()),
        }
    }

    /// Key handling for the `:` and `/` prompts.
    fn handle_prompt(&mut self, key: Key) {
        let (Mode::Command(input) | Mode::Search(input)) = &mut self.mode else {
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
                let input = std::mem::take(input);
                let mode = std::mem::replace(&mut self.mode, Mode::Normal);
                match mode {
                    Mode::Search(_) => self.search(input),
                    _ => self.execute(input.trim()),
                }
            }
            KeyCode::Char(c) => input.push(c),
            _ => {}
        }
    }

    /// Run a `/` search for `pattern` (empty = repeat the last pattern).
    fn search(&mut self, pattern: String) {
        if !pattern.is_empty() {
            self.last_search = Some(pattern);
        }
        if let Some(pos) = self.search_target(true, 1) {
            self.set_cursor(pos);
        }
    }

    /// Run a `:` command line (`w`, `q`, `wq`, `x`, `e`, with optional `!` and argument).
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
        match Document::open(&path, self.storage) {
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

/// Return `a` and `b` in document order.
fn order(a: Position, b: Position) -> (Position, Position) {
    if b < a { (b, a) } else { (a, b) }
}
