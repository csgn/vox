//! Rendering the editor into a [`Surface`]. Pure: reads editor state only.

use vox_view::{display_col, visible_slice};

use crate::editor::{CHROME_ROWS, Editor, Mode};
use crate::surface::{CursorShape, Style, Surface};

impl Editor {
    /// Render the whole screen at the size last given to [`Editor::resize`].
    pub fn render(&self) -> Surface {
        let (width, height) = self.size;
        let mut s = Surface::new(width, height);
        let text_rows = height.saturating_sub(CHROME_ROWS);
        self.render_text(&mut s, text_rows);
        if height >= 2 {
            self.render_status(&mut s, height - 2);
        }
        if height >= 1 {
            self.render_cmdline(&mut s, height - 1);
        }
        s.cursor = self.cursor_screen_position();
        s.cursor_shape = match self.mode {
            Mode::Insert => CursorShape::Bar,
            _ => CursorShape::Block,
        };
        s
    }

    /// Display width of `s`.
    fn str_width(&self, s: &str) -> usize {
        display_col(self.width.as_ref(), s, s.chars().count())
    }

    /// Screen position (column, row) of the cursor.
    fn cursor_screen_position(&self) -> (u16, u16) {
        if let Mode::Command(input) | Mode::Search(input) = &self.mode {
            let row = self.size.1.saturating_sub(1);
            return (1 + self.str_width(input) as u16, row);
        }
        let x = self.gutter_width() + self.cursor_dcol().saturating_sub(self.view.left);
        let y = self.cursor.line.saturating_sub(self.view.top);
        (x as u16, y as u16)
    }

    /// Draw the text rows: line numbers, visible part of each line, `~` past the end.
    fn render_text(&self, s: &mut Surface, rows: u16) {
        let w = self.width.as_ref();
        let gutter = self.gutter_width();
        let text_width = (s.width() as usize).saturating_sub(gutter);
        for row in 0..rows {
            let idx = self.view.top + row as usize;
            match self.doc.line(idx) {
                Some(text) => {
                    let num = format!("{:>w$} ", idx + 1, w = gutter - 1);
                    let x = s.put_str(0, row, &num, Style::DIM, w);
                    let visible = visible_slice(w, &text, self.view.left, text_width);
                    s.put_str(x, row, &visible, Style::default(), w);
                }
                None => {
                    s.put_str(0, row, "~", Style::DIM, w);
                }
            }
        }
        self.render_selection(s, rows, gutter, text_width);
    }

    /// Reverse-video the visual selection.
    fn render_selection(&self, s: &mut Surface, rows: u16, gutter: usize, text_width: usize) {
        let Some((a, b, linewise)) = self.selection() else {
            return;
        };
        let w = self.width.as_ref();
        for row in 0..rows {
            let idx = self.view.top + row as usize;
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
            let x0 = display_col(w, &text, from.min(len)) + from.saturating_sub(len);
            let x1 = display_col(w, &text, to.min(len)) + to.saturating_sub(len);
            let x0 = x0.max(self.view.left) - self.view.left;
            let x1 = x1.saturating_sub(self.view.left).min(text_width);
            if x1 > x0 {
                s.set_style((gutter + x0) as u16, row, (x1 - x0) as u16, Style::REVERSED);
            }
        }
    }

    /// Draw the status line: mode, file name, `[+]` if modified, cursor position.
    fn render_status(&self, s: &mut Surface, row: u16) {
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
        let pad = (s.width() as usize).saturating_sub(self.str_width(&left) + self.str_width(&pos));
        let text = format!("{left}{}{pos}", " ".repeat(pad));
        s.fill_row(row, Style::REVERSED);
        s.put_str(0, row, &text, Style::REVERSED, self.width.as_ref());
    }

    /// Draw the bottom line: the `:`/`/` prompt being typed, or the last message.
    fn render_cmdline(&self, s: &mut Surface, row: u16) {
        let text = match (&self.mode, &self.message) {
            (Mode::Command(input), _) => format!(":{input}"),
            (Mode::Search(input), _) => format!("/{input}"),
            (_, Some(msg)) => msg.clone(),
            _ => String::new(),
        };
        s.put_str(0, row, &text, Style::default(), self.width.as_ref());
    }
}
