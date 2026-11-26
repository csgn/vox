//! Rendering the editor into a [`Surface`]. Pure: reads editor state only.

use vox_view::{display_col, visible_slice};

use crate::editor::{Editor, Mode};
use crate::surface::{Color, CursorShape, Rect, Style, Surface};
use crate::ui::{Focus, NotifyPosition, View};

/// Widest a notification box gets.
const NOTIFY_WIDTH: u16 = 48;
/// Most text lines in one notification.
const NOTIFY_LINES: usize = 3;

impl Editor {
    /// Render the whole screen at the size last given to [`Editor::resize`].
    pub fn render(&self) -> Surface {
        let (width, height) = self.size;
        let mut s = Surface::new(width, height);
        let layout = self.layout();
        let (side, bottom, modal) = self.views();
        self.render_text(&mut s, layout.editor);
        let focus = self.focused();
        for (rect, view, focused) in [
            (layout.side, side, focus == Focus::Side),
            (layout.bottom, bottom, focus == Focus::Bottom),
        ] {
            if let (Some(rect), Some(view)) = (rect, view) {
                self.render_frame(&mut s, rect, view, focused && modal.is_none());
            }
        }
        if height >= 2 {
            self.render_status(&mut s, height - 2);
        }
        if height >= 1 {
            self.render_cmdline(&mut s, height - 1);
        }
        if let (Some(rect), Some(view)) = (layout.modal, modal) {
            self.render_frame(&mut s, rect, view, true);
        }
        self.render_notifications(&mut s, layout.body);
        self.place_cursor(
            &mut s,
            layout.editor,
            &[
                (layout.modal, modal),
                (layout.side.filter(|_| focus == Focus::Side), side),
                (layout.bottom.filter(|_| focus == Focus::Bottom), bottom),
            ],
        );
        s
    }

    /// Display width of `s`.
    fn str_width(&self, s: &str) -> usize {
        display_col(self.width.as_ref(), s, s.chars().count())
    }

    /// Border (bold blue when focused) plus the view inside it.
    fn render_frame(&self, s: &mut Surface, rect: Rect, view: &dyn View, focused: bool) {
        let border = if focused {
            Style::BOLD.fg(Color::Blue)
        } else {
            Style::DIM
        };
        s.draw_box(rect, &view.title(), border, self.width.as_ref());
        view.render(rect.inner(), s, self.width.as_ref());
    }

    /// Put the terminal cursor in the prompt, the first view in `views` that has focus,
    /// or the text. A focused view without a cursor hides it.
    fn place_cursor(
        &self,
        s: &mut Surface,
        editor: Rect,
        views: &[(Option<Rect>, Option<&dyn View>)],
    ) {
        s.cursor_shape = match self.mode {
            Mode::Insert => CursorShape::Bar,
            _ => CursorShape::Block,
        };
        if let Mode::Command(input) | Mode::Search(input) = &self.mode {
            let row = self.size.1.saturating_sub(1);
            s.cursor = (1 + self.str_width(input) as u16, row);
            return;
        }
        for (rect, view) in views {
            if let (Some(rect), Some(view)) = (rect, view) {
                match view.cursor(rect.inner()) {
                    Some(pos) => s.cursor = pos,
                    None => s.cursor_visible = false,
                }
                return;
            }
        }
        let x = self.gutter_width() + self.cursor_dcol().saturating_sub(self.view.left);
        let y = self.cursor.line.saturating_sub(self.view.top);
        s.cursor = (editor.x + x as u16, editor.y + y as u16);
    }

    /// Draw the text rows in `area`: line numbers, visible part of each line, `~` past the end.
    fn render_text(&self, s: &mut Surface, area: Rect) {
        let w = self.width.as_ref();
        let gutter = self.gutter_width();
        let text_width = (area.w as usize).saturating_sub(gutter);
        for row in 0..area.h {
            let idx = self.view.top + row as usize;
            let line = Rect::new(area.x, area.y + row, area.w, 1);
            match self.doc.line(idx) {
                Some(text) => {
                    let num = format!("{:>w$} ", idx + 1, w = gutter - 1);
                    s.put_in(line, 0, &num, Style::DIM, w);
                    let visible = visible_slice(w, &text, self.view.left, text_width);
                    let rest = Rect::new(area.x + gutter as u16, line.y, text_width as u16, 1);
                    if gutter < area.w as usize {
                        s.put_in(rest, 0, &visible, Style::default(), w);
                    }
                }
                None => s.put_in(line, 0, "~", Style::DIM, w),
            }
        }
        self.render_selection(s, area, gutter, text_width);
    }

    /// Reverse-video the visual selection.
    fn render_selection(&self, s: &mut Surface, area: Rect, gutter: usize, text_width: usize) {
        let Some((a, b, linewise)) = self.selection() else {
            return;
        };
        let w = self.width.as_ref();
        for row in 0..area.h {
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
                let x = area.x + (gutter + x0) as u16;
                s.set_style(x, area.y + row, (x1 - x0) as u16, Style::REVERSED);
            }
        }
    }

    /// Draw the status line: mode, file name, `[+]` if modified, cursor position.
    fn render_status(&self, s: &mut Surface, row: u16) {
        let mode = match (&self.mode, self.focused()) {
            (Mode::Normal, Focus::Side | Focus::Bottom) => "PANEL",
            (Mode::Normal | Mode::Search(_), _) => "NORMAL",
            (Mode::Insert, _) => "INSERT",
            (Mode::Visual { line: false }, _) => "VISUAL",
            (Mode::Visual { line: true }, _) => "V-LINE",
            (Mode::Command(_), _) => "COMMAND",
        };
        let name = self
            .doc
            .path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "[No Name]".into());
        let modified = if self.doc.is_modified() { " [+]" } else { "" };
        let pos = format!("{}:{} ", self.cursor.line + 1, self.cursor.col + 1);
        let dev = if self.developer_mode() { " DEV " } else { "" };
        let left = format!(" {mode}{dev}  {name}{modified}");
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

    /// Draw notifications stacked in a corner of `body`, oldest first.
    fn render_notifications(&self, s: &mut Surface, body: Rect) {
        let nw = NOTIFY_WIDTH.min(body.w.saturating_sub(2));
        if nw < 10 {
            return;
        }
        let x = body.x + body.w - nw - 1;
        let bottom_up = self.ui_config().notify_position == NotifyPosition::BottomRight;
        let mut used = 0u16;
        for n in self.notifications() {
            let lines = wrap(&n.text, (nw - 2) as usize, NOTIFY_LINES);
            let h = lines.len() as u16 + 2;
            if used + h > body.h {
                break;
            }
            let y = if bottom_up {
                body.y + body.h - used - h
            } else {
                body.y + used
            };
            let rect = Rect::new(x, y, nw, h);
            s.draw_box(
                rect,
                n.level.name(),
                Style::BOLD.fg(n.level.color()),
                self.width.as_ref(),
            );
            for (i, line) in lines.iter().enumerate() {
                s.put_in(
                    rect.inner(),
                    i as u16,
                    line,
                    Style::default(),
                    self.width.as_ref(),
                );
            }
            used += h;
        }
    }
}

/// Split `text` into at most `max` lines of up to `width` chars at spaces; the last line
/// ends with `…` if text was cut.
fn wrap(text: &str, width: usize, max: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        loop {
            let len = cur.chars().count();
            let wl = word.chars().count();
            if len > 0 && len + 1 + wl <= width {
                cur.push(' ');
                cur.push_str(&word);
                break;
            }
            if len == 0 && wl <= width {
                cur = word;
                break;
            }
            if len > 0 {
                lines.push(std::mem::take(&mut cur));
                continue;
            }
            // A word longer than the width: hard split.
            let head: String = word.chars().take(width).collect();
            word = word.chars().skip(width).collect();
            lines.push(head);
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    if lines.len() > max {
        lines.truncate(max);
        let last = &mut lines[max - 1];
        let keep: String = last.chars().take(width.saturating_sub(1)).collect();
        *last = format!("{keep}…");
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::wrap;

    #[test]
    fn wrap_cases() {
        assert_eq!(wrap("a bb ccc", 4, 3), ["a bb", "ccc"]);
        assert_eq!(wrap("abcdefgh", 3, 3), ["abc", "def", "gh"]);
        assert_eq!(wrap("one two three four", 4, 2), ["one", "two…"]);
        assert_eq!(wrap("", 5, 3), [""]);
    }
}
