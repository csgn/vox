//! Rendering the editor into a [`Surface`]. Pure: reads editor state only.

use vox_view::{display_col, visible_slice};

use crate::editor::{Editor, Mode};
use crate::surface::{Color, CursorShape, Rect, Style, Surface};
use crate::ui::{Focus, NotifyPosition, PromptPlace, View};

/// Widest a notification box gets.
const NOTIFY_WIDTH: u16 = 48;
/// Most text lines in one notification.
const NOTIFY_LINES: usize = 3;
/// Most completion entries shown at once.
const MENU_ROWS: usize = 10;
/// Widest the statusline completion box gets.
const MENU_WIDTH: u16 = 64;

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
            if self.modal_depth() > 0 && rect.w > 18 {
                // Hint on the bottom border: this dialog was opened from another one.
                let hint = " Esc: back ";
                let x = rect.x + rect.w - hint.chars().count() as u16 - 2;
                s.put_str(
                    x,
                    rect.y + rect.h - 1,
                    hint,
                    Style::BOLD.fg(Color::Blue),
                    self.width.as_ref(),
                );
            }
        }
        self.render_notifications(&mut s, layout.body);
        let prompt_cursor = self.render_prompt(&mut s);
        self.place_cursor(
            &mut s,
            prompt_cursor,
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
        prompt_cursor: Option<(u16, u16)>,
        editor: Rect,
        views: &[(Option<Rect>, Option<&dyn View>)],
    ) {
        s.cursor_shape = match self.mode {
            Mode::Insert => CursorShape::Bar,
            _ => CursorShape::Block,
        };
        if let Some(pos) = prompt_cursor {
            s.cursor = pos;
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
                    // Outside the focused unit, text is dimmed.
                    let style = match self.focused_unit() {
                        Some(u) if !u.contains(idx) => Style::DIM,
                        _ => Style::default(),
                    };
                    if gutter < area.w as usize {
                        s.put_in(rest, 0, &visible, style, w);
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
        // Which buffer, when more than one is open: `[2/3]`.
        let count = self.parked.len() + 1;
        let which = if count > 1 {
            let buffers = self.buffers();
            let n = buffers.iter().position(|b| b.active).unwrap_or(0) + 1;
            format!(" [{n}/{count}]")
        } else {
            String::new()
        };
        let pos = format!("{}:{} ", self.cursor.line + 1, self.cursor.col + 1);
        let dev = if self.developer_mode() { " DEV " } else { "" };
        let agent = match self.editor_mode() {
            crate::agent::EditorMode::Agent => " AGENT ",
            crate::agent::EditorMode::Text => "",
        };
        let focus = self
            .focused_unit()
            .map_or(String::new(), |u| format!("  FOCUS {}", u.label()));
        let scope = match self.scope().kind {
            crate::scope::ScopeKind::Global => "",
            crate::scope::ScopeKind::Local => " LOCAL ",
            crate::scope::ScopeKind::Vanilla => " VANILLA ",
        };
        let left = format!(" {mode}{agent}{dev}{scope}  {name}{modified}{which}{focus}");
        // Keep at least one space before the position: cut the left part if needed.
        let room = (s.width() as usize).saturating_sub(self.str_width(&pos) + 1);
        let mut left = left;
        while self.str_width(&left) > room {
            left.pop();
        }
        let pad = (s.width() as usize).saturating_sub(self.str_width(&left) + self.str_width(&pos));
        let text = format!("{left}{}{pos}", " ".repeat(pad));
        s.fill_row(row, Style::REVERSED);
        s.put_str(0, row, &text, Style::REVERSED, self.width.as_ref());
    }

    /// Draw the bottom line: the `:`/`/` prompt being typed, or the last message.
    fn render_cmdline(&self, s: &mut Surface, row: u16) {
        let on_line = |p: PromptPlace| p == PromptPlace::Statusline;
        let text = match (&self.mode, &self.message) {
            (Mode::Command(input), _) if on_line(self.ui_config().cmdline) => format!(":{input}"),
            (Mode::Search(input), _) if on_line(self.ui_config().searchline) => {
                format!("/{input}")
            }
            (Mode::Command(_) | Mode::Search(_), _) => String::new(),
            (_, Some(msg)) => msg.clone(),
            _ => String::new(),
        };
        s.put_str(0, row, &text, Style::default(), self.width.as_ref());
    }

    /// Draw the open prompt where `[ui] cmdline` / `searchline` say, with the completion
    /// menu. Returns where the terminal cursor goes.
    fn render_prompt(&self, s: &mut Surface) -> Option<(u16, u16)> {
        let (prefix, input, place) = match &self.mode {
            Mode::Command(i) => (':', i, self.ui_config().cmdline),
            Mode::Search(i) => ('/', i, self.ui_config().searchline),
            _ => return None,
        };
        let (w, h) = self.size;
        let menu = self.prompt_menu();
        let line = format!("{prefix}{input}");
        let line_w = self.str_width(&line) as u16;
        let title = if prefix == ':' { "Command" } else { "Search" };
        let accent = Style::BOLD.fg(Color::Blue);
        match place {
            PromptPlace::Statusline => {
                let row = h.saturating_sub(1);
                if let Some(menu) = &menu {
                    let start_x =
                        1 + self.str_width(&input[..menu.completion.start.min(input.len())]) as u16;
                    let rows = menu.completion.items.len().min(MENU_ROWS) as u16;
                    let bh = rows + 2;
                    let bw = self.menu_width(menu, w);
                    if h >= bh + 2 && bw >= 10 {
                        let x = start_x.saturating_sub(1).min(w - bw);
                        let rect = Rect::new(x, h - 2 - bh, bw, bh);
                        self.draw_menu(s, rect, menu, accent);
                    }
                }
                Some((line_w.min(w.saturating_sub(1)), row))
            }
            PromptPlace::Float | PromptPlace::Modal => {
                let modal = place == PromptPlace::Modal;
                let area = if modal {
                    crate::ui::compute(self.size, self.ui_config(), false, false, true).modal?
                } else {
                    let fw = (w * 6 / 10).max(40.min(w));
                    let rows = menu
                        .as_ref()
                        .map_or(0, |m| m.completion.items.len().min(MENU_ROWS) as u16);
                    let fh = if menu.is_some() { rows + 4 } else { 3 };
                    Rect::new((w - fw) / 2, h / 5, fw, fh.min(h.saturating_sub(h / 5)))
                };
                if area.w < 6 || area.h < 3 {
                    return None;
                }
                let box_title = menu
                    .as_ref()
                    .and_then(|m| m.completion.usage.as_ref())
                    .map_or(title.to_string(), |u| format!(":{u}"));
                s.draw_box(area, &box_title, accent, self.width.as_ref());
                if let Some(m) = &menu {
                    self.highlight_param(s, area, m);
                }
                let inner = area.inner();
                s.put_in(inner, 0, &line, Style::default(), self.width.as_ref());
                if let Some(m) = &menu
                    && inner.h > 2
                {
                    s.put_in(
                        inner,
                        1,
                        &"─".repeat(inner.w as usize),
                        Style::DIM,
                        self.width.as_ref(),
                    );
                    let hint_rows = u16::from(modal);
                    let list = Rect::new(inner.x, inner.y + 2, inner.w, inner.h - 2 - hint_rows);
                    self.draw_menu_items(s, list, m);
                }
                if modal {
                    let hint = "Tab complete · ↑/↓ select · Enter run · Esc close";
                    s.put_in(inner, inner.h - 1, hint, accent, self.width.as_ref());
                }
                Some((inner.x + line_w.min(inner.w.saturating_sub(1)), inner.y))
            }
        }
    }

    /// Width of the statusline completion box: widest entry plus its detail, within `w`.
    fn menu_width(&self, menu: &crate::prompt::Menu, w: u16) -> u16 {
        let items = &menu.completion.items;
        let text = items
            .iter()
            .map(|c| self.str_width(&c.text))
            .max()
            .unwrap_or(0);
        let detail = items
            .iter()
            .map(|c| self.str_width(&c.detail))
            .max()
            .unwrap_or(0);
        let usage = menu
            .completion
            .usage
            .as_ref()
            .map_or(0, |u| self.str_width(u) + 8);
        let want = (text + if detail > 0 { detail + 3 } else { 0 } + 4)
            .max(usage)
            .max(20);
        (want as u16).min(w.min(MENU_WIDTH))
    }

    /// A box with the completion entries; its title is the usage of the command.
    fn draw_menu(&self, s: &mut Surface, rect: Rect, menu: &crate::prompt::Menu, style: Style) {
        let title = menu
            .completion
            .usage
            .as_ref()
            .map(|u| format!(":{u}"))
            .unwrap_or_default();
        s.draw_box(rect, &title, style, self.width.as_ref());
        self.highlight_param(s, rect, menu);
        self.draw_menu_items(s, rect.inner(), menu);
    }

    /// Show the parameter being typed in the usage title (`SOURCE` in `:plugin add SOURCE`).
    fn highlight_param(&self, s: &mut Surface, rect: Rect, menu: &crate::prompt::Menu) {
        let (Some(usage), Some(param)) = (&menu.completion.usage, &menu.completion.param) else {
            return;
        };
        let title = format!(":{usage}");
        // The parameter after the command name: `[LEVEL]` and `LEVEL...` contain it too.
        let name_end = title.find(' ').unwrap_or(title.len());
        let Some(at) = title[name_end..].find(param.as_str()).map(|i| i + name_end) else {
            return;
        };
        // The title starts after `┌ ` and a space (see `Surface::draw_box`).
        let x = rect.x + 3 + self.str_width(&title[..at]) as u16;
        let len = self.str_width(param) as u16;
        if x + len < rect.x + rect.w - 1 {
            s.set_style(x, rect.y, len, Style::REVERSED.fg(Color::Blue));
        }
    }

    /// Completion entries in `area`: text, then the dimmed detail; the selected one reversed.
    fn draw_menu_items(&self, s: &mut Surface, area: Rect, menu: &crate::prompt::Menu) {
        let items = &menu.completion.items;
        if area.h == 0 {
            return;
        }
        let h = area.h as usize;
        let top = menu.selected.map_or(0, |i| i.saturating_sub(h - 1));
        let text_w = items
            .iter()
            .map(|c| self.str_width(&c.text))
            .max()
            .unwrap_or(0)
            .min(area.w as usize / 2);
        for (row, (i, c)) in items.iter().enumerate().skip(top).take(h).enumerate() {
            let line = Rect::new(area.x, area.y + row as u16, area.w, 1);
            let selected = menu.selected == Some(i);
            let style = if selected {
                Style::REVERSED
            } else {
                Style::default()
            };
            if selected {
                s.clear(line, style);
            }
            s.put_in(line, 0, &format!(" {}", c.text), style, self.width.as_ref());
            let dx = text_w as u16 + 3;
            if !c.detail.is_empty() && dx < area.w {
                let rest = Rect::new(area.x + dx, line.y, area.w - dx, 1);
                let dim = Style {
                    dim: !selected,
                    ..style
                };
                s.put_in(rest, 0, &c.detail, dim, self.width.as_ref());
            }
        }
        if items.len() > h {
            let more = format!(" {}/{} ", menu.selected.map_or(0, |i| i + 1), items.len());
            let x = area.x + area.w.saturating_sub(self.str_width(&more) as u16);
            s.put_str(
                x,
                area.y + area.h - 1,
                &more,
                Style::DIM,
                self.width.as_ref(),
            );
        }
    }

    /// Draw running jobs, then notifications, stacked in a corner of `body`, oldest first.
    /// A job's box shows a spinner and the time so far, its title, and its last progress.
    fn render_notifications(&self, s: &mut Surface, body: Rect) {
        let nw = NOTIFY_WIDTH.min(body.w.saturating_sub(2));
        if nw < 10 {
            return;
        }
        let x = body.x + body.w - nw - 1;
        let bottom_up = self.ui_config().notify_position == NotifyPosition::BottomRight;
        let width = (nw - 2) as usize;
        let now = self.now();
        let jobs = self.job_status().iter().map(|j| {
            let elapsed = now.saturating_duration_since(j.started);
            let frame = SPINNER[(elapsed.as_millis() / 100) as usize % SPINNER.len()];
            let mut lines = wrap(&j.title, width, 1);
            if let Some(d) = &j.detail {
                lines.extend(wrap(d, width, NOTIFY_LINES - 1));
            }
            (
                format!("{frame} {}", duration(elapsed.as_secs())),
                Color::Blue,
                lines,
            )
        });
        let notes = self.notifications().iter().map(|n| {
            let lines = wrap(&n.text, width, NOTIFY_LINES);
            (n.level.name().to_string(), n.level.color(), lines)
        });
        let mut used = 0u16;
        for (title, color, lines) in jobs.chain(notes) {
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
            s.draw_box(rect, &title, Style::BOLD.fg(color), self.width.as_ref());
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

/// Frames of the spinner shown on running jobs (one per 100 ms).
const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Seconds as `12s` or `3m 05s`.
fn duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m {:02}s", secs / 60, secs % 60)
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
