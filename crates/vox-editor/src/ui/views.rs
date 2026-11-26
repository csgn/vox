//! Built-in views: list, text, confirm dialog, input prompt.

use vox_view::CharWidth;

use super::{Outcome, View};
use crate::keys::{Key, KeyCode};
use crate::surface::{Color, Rect, Style, Surface};

/// Keep `selected` visible in a window of `height` rows starting at `scroll`.
fn keep_visible(selected: usize, scroll: usize, height: usize) -> usize {
    if selected < scroll {
        selected
    } else if height > 0 && selected >= scroll + height {
        selected + 1 - height
    } else {
        scroll
    }
}

/// One row of a [`ListView`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    /// Main text.
    pub label: String,
    /// Secondary text, shown dimmed after the label.
    pub detail: String,
    /// Command line run when the row is chosen with `Enter`.
    pub command: Option<String>,
}

impl ListItem {
    /// A row with a label only.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: String::new(),
            command: None,
        }
    }

    /// Set the dimmed detail text.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// Set the command run on `Enter`.
    pub fn command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }
}

/// A selectable list. Keys: `j`/`k`/arrows move, `g`/`G` first/last, `Enter` runs the row's
/// command, `q`/`Esc` close.
#[derive(Debug, Clone)]
pub struct ListView {
    title: String,
    items: Vec<ListItem>,
    selected: usize,
    scroll: std::cell::Cell<usize>,
    /// Close the view when a row's command runs.
    pub close_on_enter: bool,
}

impl ListView {
    /// A list with `items`.
    pub fn new(title: impl Into<String>, items: Vec<ListItem>) -> Self {
        Self {
            title: title.into(),
            items,
            selected: 0,
            scroll: std::cell::Cell::new(0),
            close_on_enter: true,
        }
    }

    /// Index of the selected row.
    pub fn selected(&self) -> usize {
        self.selected
    }
}

impl View for ListView {
    fn title(&self) -> String {
        self.title.clone()
    }

    fn render(&self, area: Rect, s: &mut Surface, width: &dyn CharWidth) {
        if self.items.is_empty() {
            s.put_in(area, 0, "(empty)", Style::DIM, width);
            return;
        }
        let scroll = keep_visible(self.selected, self.scroll.get(), area.h as usize);
        self.scroll.set(scroll);
        for (row, item) in self
            .items
            .iter()
            .enumerate()
            .skip(scroll)
            .take(area.h as usize)
        {
            let y = (row - scroll) as u16;
            let selected = row == self.selected;
            let style = if selected {
                Style::REVERSED
            } else {
                Style::default()
            };
            let line = Rect::new(area.x, area.y + y, area.w, 1);
            s.clear(line, style);
            let label = format!(" {} ", item.label);
            s.put_in(line, 0, &label, style, width);
            if !item.detail.is_empty() {
                let used = vox_view::display_col(width, &label, label.chars().count()) as u16;
                if used < area.w {
                    let rest = Rect::new(area.x + used, area.y + y, area.w - used, 1);
                    let dim = Style {
                        dim: !selected,
                        ..style
                    };
                    s.put_in(rest, 0, &item.detail, dim, width);
                }
            }
        }
    }

    fn handle_key(&mut self, key: Key) -> Outcome {
        let last = self.items.len().saturating_sub(1);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.selected = (self.selected + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => self.selected = last,
            KeyCode::Enter => {
                return match self
                    .items
                    .get(self.selected)
                    .and_then(|i| i.command.clone())
                {
                    Some(cmd) if self.close_on_enter => Outcome::RunAndClose(cmd),
                    Some(cmd) => Outcome::Run(cmd),
                    None => Outcome::Handled,
                };
            }
            KeyCode::Char('q') | KeyCode::Esc => return Outcome::Close,
            _ => return Outcome::Ignored,
        }
        Outcome::Handled
    }
}

/// A list of checkboxes. Keys: `j`/`k` move, `Space` toggles, `a` toggles all, `Enter`
/// runs `template` with `{}` replaced by the checked values (space-separated), `q`/`Esc`
/// cancel.
#[derive(Debug, Clone)]
pub struct CheckList {
    title: String,
    /// (label, value, checked).
    items: Vec<(String, String, bool)>,
    selected: usize,
    template: String,
}

impl CheckList {
    /// Rows of (label, value); all unchecked.
    pub fn new(
        title: impl Into<String>,
        items: Vec<(String, String)>,
        template: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            items: items.into_iter().map(|(l, v)| (l, v, false)).collect(),
            selected: 0,
            template: template.into(),
        }
    }

    /// Values of checked rows, in order.
    pub fn checked(&self) -> Vec<&str> {
        self.items
            .iter()
            .filter(|i| i.2)
            .map(|i| i.1.as_str())
            .collect()
    }
}

impl View for CheckList {
    fn title(&self) -> String {
        format!("{} — Space toggle, a all, Enter confirm", self.title)
    }

    fn render(&self, area: Rect, s: &mut Surface, width: &dyn CharWidth) {
        let scroll = keep_visible(self.selected, 0, area.h as usize);
        for (row, (label, _, checked)) in self
            .items
            .iter()
            .enumerate()
            .skip(scroll)
            .take(area.h as usize)
        {
            let style = if row == self.selected {
                Style::REVERSED
            } else {
                Style::default()
            };
            let line = Rect::new(area.x, area.y + (row - scroll) as u16, area.w, 1);
            s.clear(line, style);
            let mark = if *checked { "[x]" } else { "[ ]" };
            s.put_in(line, 0, &format!(" {mark} {label}"), style, width);
        }
    }

    fn handle_key(&mut self, key: Key) -> Outcome {
        let last = self.items.len().saturating_sub(1);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.selected = (self.selected + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char(' ') => {
                if let Some(item) = self.items.get_mut(self.selected) {
                    item.2 = !item.2;
                }
            }
            KeyCode::Char('a') => {
                let all = self.items.iter().all(|i| i.2);
                self.items.iter_mut().for_each(|i| i.2 = !all);
            }
            KeyCode::Enter => {
                let values = self.checked().join(" ");
                return Outcome::RunAndClose(self.template.replace("{}", &values));
            }
            KeyCode::Char('q') | KeyCode::Esc => return Outcome::Close,
            _ => return Outcome::Ignored,
        }
        Outcome::Handled
    }
}

/// Scrollable read-only text. Keys: `j`/`k` scroll, `g`/`G` top/bottom, `q`/`Esc` close.
#[derive(Debug, Clone)]
pub struct TextView {
    title: String,
    lines: Vec<(Style, String)>,
    /// First visible line; `usize::MAX` means "at the end".
    top: usize,
}

impl TextView {
    /// Text starting at the top.
    pub fn new(title: impl Into<String>, lines: Vec<(Style, String)>) -> Self {
        Self {
            title: title.into(),
            lines,
            top: 0,
        }
    }

    /// Text scrolled to the end (newest lines visible).
    pub fn at_end(mut self) -> Self {
        self.top = usize::MAX;
        self
    }
}

impl View for TextView {
    fn title(&self) -> String {
        self.title.clone()
    }

    fn render(&self, area: Rect, s: &mut Surface, width: &dyn CharWidth) {
        let max_top = self.lines.len().saturating_sub(area.h as usize);
        let top = self.top.min(max_top);
        for (i, (style, text)) in self
            .lines
            .iter()
            .skip(top)
            .take(area.h as usize)
            .enumerate()
        {
            s.put_in(area, i as u16, text, *style, width);
        }
    }

    fn handle_key(&mut self, key: Key) -> Outcome {
        let max_top = self.lines.len().saturating_sub(1);
        let top = self.top.min(max_top);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.top = (top + 1).min(max_top),
            KeyCode::Char('k') | KeyCode::Up => self.top = top.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => self.top = 0,
            KeyCode::Char('G') | KeyCode::End => self.top = usize::MAX,
            KeyCode::Char('q') | KeyCode::Esc => return Outcome::Close,
            _ => return Outcome::Ignored,
        }
        Outcome::Handled
    }
}

/// A yes/no question. `y`/`Enter` runs the command, `n`/`q`/`Esc` cancels.
#[derive(Debug, Clone)]
pub struct Confirm {
    title: String,
    message: String,
    command: String,
}

impl Confirm {
    /// Ask `message`; on yes, run `command`.
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            command: command.into(),
        }
    }
}

impl View for Confirm {
    fn title(&self) -> String {
        self.title.clone()
    }

    fn render(&self, area: Rect, s: &mut Surface, width: &dyn CharWidth) {
        s.put_in(area, 0, &self.message, Style::default(), width);
        let hint = Style::BOLD.fg(Color::Blue);
        s.put_in(
            area,
            area.h.saturating_sub(1).clamp(1, 2),
            "[y] yes   [n] no",
            hint,
            width,
        );
    }

    fn handle_key(&mut self, key: Key) -> Outcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => Outcome::RunAndClose(self.command.clone()),
            KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc => Outcome::Close,
            _ => Outcome::Handled,
        }
    }
}

/// A one-line text prompt. `Enter` runs `template` with `{}` replaced by the input.
#[derive(Debug, Clone)]
pub struct Input {
    title: String,
    prompt: String,
    text: String,
    template: String,
}

impl Input {
    /// Ask with `prompt`; on `Enter`, run `template` (e.g. `"e {}"`).
    pub fn new(
        title: impl Into<String>,
        prompt: impl Into<String>,
        template: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            prompt: prompt.into(),
            text: String::new(),
            template: template.into(),
        }
    }

    /// Start with `text` already typed.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    /// Prompt plus typed text.
    fn line(&self) -> String {
        format!("{}{}", self.prompt, self.text)
    }
}

impl View for Input {
    fn title(&self) -> String {
        self.title.clone()
    }

    fn render(&self, area: Rect, s: &mut Surface, width: &dyn CharWidth) {
        s.put_in(area, 0, &self.line(), Style::default(), width);
    }

    fn handle_key(&mut self, key: Key) -> Outcome {
        match key.code {
            KeyCode::Enter => Outcome::RunAndClose(self.template.replace("{}", &self.text)),
            KeyCode::Esc => Outcome::Close,
            KeyCode::Backspace => {
                self.text.pop();
                Outcome::Handled
            }
            KeyCode::Char(c) if !key.mods.ctrl => {
                self.text.push(c);
                Outcome::Handled
            }
            _ => Outcome::Handled,
        }
    }

    /// Cursor after the typed text.
    fn cursor(&self, area: Rect) -> Option<(u16, u16)> {
        let col = self.line().chars().count() as u16;
        Some((area.x + col.min(area.w.saturating_sub(1)), area.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::parse_keys;
    use vox_view::Monospace;

    fn press(v: &mut dyn View, keys: &str) -> Vec<Outcome> {
        parse_keys(keys)
            .into_iter()
            .map(|k| v.handle_key(k))
            .collect()
    }

    fn draw(v: &dyn View, w: u16, h: u16) -> String {
        let mut s = Surface::new(w, h);
        v.render(Rect::new(0, 0, w, h), &mut s, &Monospace);
        s.to_string()
    }

    #[test]
    fn list_moves_scrolls_and_runs() {
        let items = (1..=5)
            .map(|i| ListItem::new(format!("item{i}")).command(format!("cmd{i}")))
            .collect();
        let mut v = ListView::new("L", items);
        press(&mut v, "jjj");
        assert_eq!(v.selected(), 3);
        assert_eq!(draw(&v, 10, 2), "\" item3    \"\n\" item4    \"\n");
        assert_eq!(press(&mut v, "<CR>"), [Outcome::RunAndClose("cmd4".into())]);
        v.close_on_enter = false;
        assert_eq!(
            press(&mut v, "G<CR>"),
            [Outcome::Handled, Outcome::Run("cmd5".into())]
        );
        assert_eq!(
            press(&mut v, "gx<Esc>"),
            [Outcome::Handled, Outcome::Ignored, Outcome::Close]
        );
        assert_eq!(v.selected(), 0);
    }

    #[test]
    fn list_detail_and_empty() {
        let v = ListView::new("L", vec![ListItem::new("a").detail("info")]);
        assert_eq!(draw(&v, 12, 1), "\" a info     \"\n");
        let empty = ListView::new("L", vec![]);
        assert_eq!(draw(&empty, 8, 1), "\"(empty) \"\n");
    }

    #[test]
    fn text_view_scrolls() {
        let lines = (1..=5)
            .map(|i| (Style::default(), format!("l{i}")))
            .collect();
        let mut v = TextView::new("T", lines).at_end();
        assert_eq!(draw(&v, 3, 2), "\"l4 \"\n\"l5 \"\n");
        press(&mut v, "gj");
        assert_eq!(draw(&v, 3, 2), "\"l2 \"\n\"l3 \"\n");
        assert_eq!(press(&mut v, "q"), [Outcome::Close]);
    }

    #[test]
    fn checklist_toggles_and_confirms() {
        let items = vec![
            ("Config".into(), "config".into()),
            ("Plugin a".into(), "a".into()),
        ];
        let mut c = CheckList::new("Copy", items, "init {}");
        press(&mut c, "<Space>j<Space>");
        assert_eq!(c.checked(), ["config", "a"]);
        assert_eq!(draw(&c, 14, 2), "\" [x] Config   \"\n\" [x] Plugin a \"\n");
        press(&mut c, "a");
        assert!(c.checked().is_empty());
        press(&mut c, "a");
        assert_eq!(
            press(&mut c, "<CR>"),
            [Outcome::RunAndClose("init config a".into())]
        );
    }

    #[test]
    fn confirm_and_input() {
        let mut c = Confirm::new("Delete?", "Really?", "delete-it");
        assert_eq!(
            press(&mut c, "xy"),
            [Outcome::Handled, Outcome::RunAndClose("delete-it".into())]
        );
        assert_eq!(press(&mut c, "n"), [Outcome::Close]);
        let mut i = Input::new("Open", "file: ", "e {}");
        press(&mut i, "abx<BS>c");
        assert_eq!(draw(&i, 12, 1), "\"file: abc   \"\n");
        assert_eq!(i.cursor(Rect::new(2, 3, 20, 1)), Some((11, 3)));
        assert_eq!(
            press(&mut i, "<CR>"),
            [Outcome::RunAndClose("e abc".into())]
        );
    }
}
