//! Terminal UI for vox: application state, key handling, rendering, and the run loop.

use std::io;

use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use vox_core::Document;
use vox_view::Viewport;

/// Rows reserved below the text area: statusline + command line.
const CHROME_ROWS: u16 = 2;

/// Editor input mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Default mode: navigation keys.
    Normal,
    /// `:` command line; holds the text typed so far.
    Command(String),
}

/// Top-level editor state.
#[derive(Debug)]
pub struct App {
    doc: Document,
    view: Viewport,
    mode: Mode,
    /// Text area height in rows, updated each frame.
    height: usize,
    /// Pending `g` prefix for `gg`.
    pending_g: bool,
    message: Option<String>,
    quit: bool,
}

impl App {
    /// Create an app editing `doc`.
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            view: Viewport::default(),
            mode: Mode::Normal,
            height: 1,
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

    /// Current viewport.
    pub fn viewport(&self) -> Viewport {
        self.view
    }

    /// Update the text area height from the full terminal area height.
    pub fn resize(&mut self, terminal_height: u16) {
        self.height = terminal_height.saturating_sub(CHROME_ROWS).max(1) as usize;
    }

    /// Handle one key press.
    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match &mut self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::Command(_) => self.handle_command(key),
        }
    }

    fn handle_normal(&mut self, key: KeyEvent) {
        let lines = self.doc.line_count();
        let half = (self.height / 2).max(1) as isize;
        let page = self.height.max(1) as isize;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let pending_g = std::mem::take(&mut self.pending_g);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.view.scroll_by(1, lines),
            KeyCode::Char('k') | KeyCode::Up => self.view.scroll_by(-1, lines),
            KeyCode::Char('d') if ctrl => self.view.scroll_by(half, lines),
            KeyCode::Char('u') if ctrl => self.view.scroll_by(-half, lines),
            KeyCode::Char('f') if ctrl => self.view.scroll_by(page, lines),
            KeyCode::Char('b') if ctrl => self.view.scroll_by(-page, lines),
            KeyCode::PageDown => self.view.scroll_by(page, lines),
            KeyCode::PageUp => self.view.scroll_by(-page, lines),
            KeyCode::Char('G') => self.view.scroll_to_bottom(lines, self.height),
            KeyCode::Char('g') if pending_g => self.view.scroll_to_top(),
            KeyCode::Char('g') => self.pending_g = true,
            KeyCode::Char(':') => {
                self.message = None;
                self.mode = Mode::Command(String::new());
            }
            _ => {}
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
        match cmd {
            "" => {}
            "q" | "q!" | "quit" | "quit!" | "qa" | "qa!" => self.quit = true,
            other => self.message = Some(format!("E492: Not an editor command: {other}")),
        }
    }

    fn render_text(&self, area: Rect, buf: &mut Buffer) {
        let lines = self.doc.line_count();
        let gutter = lines.to_string().len().max(3);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let rows: Vec<Line> = (0..area.height as usize)
            .map(|row| {
                let idx = self.view.top + row;
                match self.doc.line(idx) {
                    Some(text) => Line::from(vec![
                        Span::styled(format!("{:>gutter$} ", idx + 1), dim),
                        Span::raw(text.replace('\t', "    ")),
                    ]),
                    None => Line::styled("~", dim),
                }
            })
            .collect();
        Paragraph::new(rows).render(area, buf);
    }

    fn render_status(&self, area: Rect, buf: &mut Buffer) {
        let mode = match self.mode {
            Mode::Normal => "NORMAL",
            Mode::Command(_) => "COMMAND",
        };
        let name = self
            .doc
            .path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "[No Name]".into());
        let pos = format!("{}/{} ", self.view.top + 1, self.doc.line_count());
        let left = format!(" {mode}  {name}");
        let pad = (area.width as usize).saturating_sub(left.chars().count() + pos.chars().count());
        let text = format!("{left}{}{pos}", " ".repeat(pad));
        Paragraph::new(text)
            .style(Style::default().add_modifier(Modifier::REVERSED))
            .render(area, buf);
    }

    fn render_cmdline(&self, area: Rect, buf: &mut Buffer) {
        let text = match (&self.mode, &self.message) {
            (Mode::Command(input), _) => format!(":{input}"),
            (_, Some(msg)) => msg.clone(),
            _ => String::new(),
        };
        Paragraph::new(text).render(area, buf);
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [text, status, cmdline] = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);
        self.render_text(text, buf);
        self.render_status(status, buf);
        self.render_cmdline(cmdline, buf);
    }
}

/// Run the event loop on `terminal` until the app quits.
pub fn run(app: &mut App, terminal: &mut DefaultTerminal) -> io::Result<()> {
    while !app.should_quit() {
        terminal.draw(|frame| {
            app.resize(frame.area().height);
            frame.render_widget(&*app, frame.area());
            if let Mode::Command(input) = app.mode() {
                let row = frame.area().height.saturating_sub(1);
                frame.set_cursor_position((1 + input.chars().count() as u16, row));
            }
        })?;
        if let Event::Key(key) = event::read()? {
            app.handle_key(key);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn type_str(app: &mut App, s: &str) {
        for c in s.chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
    }

    fn app_with_lines(n: usize) -> App {
        let text: String = (1..=n).map(|i| format!("line {i}\n")).collect();
        let mut app = App::new(Document::from_text(&text));
        app.resize(12); // text height 10
        app
    }

    #[test]
    fn scroll_keys() {
        let mut app = app_with_lines(100);
        type_str(&mut app, "jjj");
        assert_eq!(app.viewport().top, 3);
        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.viewport().top, 2);
        app.handle_key(ctrl('d'));
        assert_eq!(app.viewport().top, 7);
        app.handle_key(ctrl('u'));
        assert_eq!(app.viewport().top, 2);
        app.handle_key(ctrl('f'));
        assert_eq!(app.viewport().top, 12);
        type_str(&mut app, "G");
        assert_eq!(app.viewport().top, 90);
        type_str(&mut app, "gg");
        assert_eq!(app.viewport().top, 0);
    }

    #[test]
    fn single_g_does_not_jump() {
        let mut app = app_with_lines(100);
        type_str(&mut app, "jjgj");
        assert_eq!(app.viewport().top, 3);
    }

    #[test]
    fn quit_command() {
        let mut app = app_with_lines(3);
        type_str(&mut app, ":q");
        assert_eq!(app.mode(), &Mode::Command("q".into()));
        app.handle_key(key(KeyCode::Enter));
        assert!(app.should_quit());
    }

    #[test]
    fn unknown_command_sets_message() {
        let mut app = app_with_lines(3);
        type_str(&mut app, ":foo");
        app.handle_key(key(KeyCode::Enter));
        assert!(!app.should_quit());
        assert_eq!(app.mode(), &Mode::Normal);
        assert_eq!(
            app.message.as_deref(),
            Some("E492: Not an editor command: foo")
        );
    }

    #[test]
    fn escape_and_backspace_leave_command_mode() {
        let mut app = app_with_lines(3);
        type_str(&mut app, ":x");
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode(), &Mode::Normal);
        type_str(&mut app, ":");
        app.handle_key(key(KeyCode::Backspace));
        assert_eq!(app.mode(), &Mode::Normal);
    }
}
