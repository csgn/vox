//! Terminal UI for vox: application state, key handling, rendering, and the run loop.

mod app;
mod keys;
mod normal;
mod register;

use std::io;

use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::crossterm::event::{self, Event};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;
use vox_view::{display_col, visible_slice};

pub use app::{App, Mode};
pub use keys::parse_keys;
pub use register::{Clipboard, SystemClipboard};

impl App {
    /// Screen position (column, row) of the cursor inside `area`.
    pub fn cursor_screen_position(&self, area: Rect) -> (u16, u16) {
        if let Mode::Command(input) | Mode::Search(input) = &self.mode {
            let row = area.bottom().saturating_sub(1);
            return (area.x + 1 + input.width() as u16, row);
        }
        let x = self.gutter_width() + self.cursor_dcol().saturating_sub(self.view.left);
        let y = self.cursor.line.saturating_sub(self.view.top);
        (area.x + x as u16, area.y + y as u16)
    }

    fn render_text(&self, area: Rect, buf: &mut Buffer) {
        let gutter = self.gutter_width();
        let text_width = (area.width as usize).saturating_sub(gutter);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let rows: Vec<Line> = (0..area.height as usize)
            .map(|row| {
                let idx = self.view.top + row;
                match self.doc.line(idx) {
                    Some(text) => Line::from(vec![
                        Span::styled(format!("{:>w$} ", idx + 1, w = gutter - 1), dim),
                        Span::raw(visible_slice(&text, self.view.left, text_width)),
                    ]),
                    None => Line::styled("~", dim),
                }
            })
            .collect();
        Paragraph::new(rows).render(area, buf);
        self.render_selection(area, buf, gutter, text_width);
    }

    /// Reverse-video the visual selection.
    fn render_selection(&self, area: Rect, buf: &mut Buffer, gutter: usize, text_width: usize) {
        let Some((a, b, linewise)) = self.selection() else {
            return;
        };
        let style = Style::default().add_modifier(Modifier::REVERSED);
        for row in 0..area.height as usize {
            let idx = self.view.top + row;
            if idx < a.line || idx > b.line {
                continue;
            }
            let text = self.doc.line(idx).unwrap_or_default();
            let len = text.chars().count();
            let (from, to) = if linewise {
                (0, len.max(1))
            } else {
                let from = if idx == a.line { a.col } else { 0 };
                let to = if idx == b.line { b.col + 1 } else { len + 1 };
                (from, to)
            };
            // Columns past the text (empty line, line break) show as one cell.
            let x0 = display_col(&text, from.min(len)) + from.saturating_sub(len);
            let x1 = display_col(&text, to.min(len)) + to.saturating_sub(len);
            let x0 = x0.max(self.view.left) - self.view.left;
            let x1 = x1.saturating_sub(self.view.left).min(text_width);
            if x1 > x0 {
                let rect = Rect::new(
                    area.x + (gutter + x0) as u16,
                    area.y + row as u16,
                    (x1 - x0) as u16,
                    1,
                );
                buf.set_style(rect, style);
            }
        }
    }

    fn render_status(&self, area: Rect, buf: &mut Buffer) {
        let mode = match self.mode {
            Mode::Normal | Mode::Search(_) => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Visual { line: false } => "VISUAL",
            Mode::Visual { line: true } => "V-LINE",
            Mode::Command(_) => "COMMAND",
        };
        let name = self
            .doc
            .path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "[No Name]".into());
        let modified = if self.doc.is_modified() { " [+]" } else { "" };
        let pos = format!("{}:{} ", self.cursor.line + 1, self.cursor.col + 1);
        let left = format!(" {mode}  {name}{modified}");
        let pad = (area.width as usize).saturating_sub(left.width() + pos.width());
        let text = format!("{left}{}{pos}", " ".repeat(pad));
        Paragraph::new(text)
            .style(Style::default().add_modifier(Modifier::REVERSED))
            .render(area, buf);
    }

    fn render_cmdline(&self, area: Rect, buf: &mut Buffer) {
        let text = match (&self.mode, &self.message) {
            (Mode::Command(input), _) => format!(":{input}"),
            (Mode::Search(input), _) => format!("/{input}"),
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
            let area = frame.area();
            app.resize(area.width, area.height);
            frame.render_widget(&*app, area);
            frame.set_cursor_position(app.cursor_screen_position(area));
        })?;
        let style = match app.mode() {
            Mode::Insert => SetCursorStyle::SteadyBar,
            _ => SetCursorStyle::SteadyBlock,
        };
        execute!(io::stdout(), style)?;
        if let Event::Key(key) = event::read()? {
            app.handle_key(key);
        }
    }
    execute!(io::stdout(), SetCursorStyle::DefaultUserShape)
}
