//! A form of typed settings. Keys: `j`/`k` move, `Enter` edit (text, number) or change
//! (on/off, choice), `Space`/`h`/`l` change, `r` reset to default, `s` save, `Esc`/`q` back
//! (asks once if there are unsaved changes). While editing a field: type, `Backspace`,
//! `Enter` accept, `Esc` cancel.

use vox_view::CharWidth;

use super::{Outcome, View};
use crate::config::Value;
use crate::keys::{Key, KeyCode};
use crate::plugin::{SettingKind, SettingSpec, SettingValue};
use crate::settings::to_config_value;
use crate::surface::{Color, Rect, Style, Surface};

/// Rows below the fields: description, message, key hints.
const FOOTER: u16 = 3;

/// A settings form. Saving returns [`Outcome::Submit`] with the changed fields.
pub struct Form {
    title: String,
    specs: Vec<SettingSpec>,
    original: Vec<SettingValue>,
    values: Vec<SettingValue>,
    selected: usize,
    scroll: std::cell::Cell<usize>,
    /// Text being typed into the selected field.
    editing: Option<String>,
    /// Last validation error or warning.
    message: Option<String>,
    /// `Esc` was pressed once with unsaved changes.
    warned: bool,
    /// Command run on save; gets the changes through [`Outcome::Submit`].
    command: String,
}

impl Form {
    /// A form for `specs` with their current `values` (same order). `command` is run on save.
    pub fn new(
        title: impl Into<String>,
        specs: Vec<SettingSpec>,
        values: Vec<SettingValue>,
        command: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            original: values.clone(),
            values,
            specs,
            selected: 0,
            scroll: std::cell::Cell::new(0),
            editing: None,
            message: None,
            warned: false,
            command: command.into(),
        }
    }

    /// Changed fields as (key, value).
    pub fn changes(&self) -> Vec<(String, Value)> {
        self.specs
            .iter()
            .zip(self.values.iter().zip(&self.original))
            .filter(|(_, (v, o))| v != o)
            .map(|(s, (v, _))| (s.key.clone(), to_config_value(v)))
            .collect()
    }

    /// Current value of field `key`.
    pub fn value(&self, key: &str) -> Option<&SettingValue> {
        self.specs
            .iter()
            .position(|s| s.key == key)
            .map(|i| &self.values[i])
    }

    /// Set the selected field and clear messages.
    fn set(&mut self, v: SettingValue) {
        self.values[self.selected] = v;
        self.message = None;
        self.warned = false;
    }

    /// Change an on/off or choice field by `step` (+1 next, -1 previous).
    fn change(&mut self, step: isize) {
        let i = self.selected;
        let next = match (&self.specs[i].kind, &self.values[i]) {
            (SettingKind::Bool, SettingValue::Bool(b)) => Some(SettingValue::Bool(!b)),
            (SettingKind::Choice(c), SettingValue::String(cur)) => {
                let pos = c.iter().position(|x| x == cur).unwrap_or(0) as isize;
                let n = c.len() as isize;
                Some(SettingValue::String(
                    c[(pos + step).rem_euclid(n) as usize].clone(),
                ))
            }
            _ => None,
        };
        if let Some(v) = next {
            self.set(v);
        }
    }

    /// Accept the typed text for the selected field, if valid.
    fn accept(&mut self, text: String) {
        let spec = &self.specs[self.selected];
        let value = match spec.kind {
            SettingKind::Integer { .. } => match text.trim().parse::<i64>() {
                Ok(n) => Value::Integer(n),
                Err(_) => {
                    self.message = Some(format!("`{}` must be a whole number", spec.key));
                    self.editing = Some(text);
                    return;
                }
            },
            _ => Value::String(text.clone()),
        };
        match spec.check(&value) {
            Ok(v) => {
                self.editing = None;
                self.set(v);
            }
            Err(e) => {
                self.message = Some(e);
                self.editing = Some(text);
            }
        }
    }

    /// How a value is shown in the form.
    fn show(v: &SettingValue) -> String {
        match v {
            SettingValue::String(s) => format!("{s:?}"),
            SettingValue::Integer(n) => n.to_string(),
            SettingValue::Bool(b) => (if *b { "on" } else { "off" }).into(),
        }
    }
}

impl View for Form {
    fn title(&self) -> String {
        self.title.clone()
    }

    fn render(&self, area: Rect, s: &mut Surface, width: &dyn CharWidth) {
        if self.specs.is_empty() {
            s.put_in(area, 0, "No settings.", Style::DIM, width);
            return;
        }
        let rows = area.h.saturating_sub(FOOTER) as usize;
        let key_w = self
            .specs
            .iter()
            .map(|s| s.key.chars().count())
            .max()
            .unwrap_or(0)
            + 2;
        let mut scroll = self.scroll.get();
        if self.selected < scroll {
            scroll = self.selected;
        } else if rows > 0 && self.selected >= scroll + rows {
            scroll = self.selected + 1 - rows;
        }
        self.scroll.set(scroll);
        for (i, spec) in self.specs.iter().enumerate().skip(scroll).take(rows) {
            let y = (i - scroll) as u16;
            let line = Rect::new(area.x, area.y + y, area.w, 1);
            let selected = i == self.selected;
            let changed = self.values[i] != self.original[i];
            let base = if selected {
                Style::REVERSED
            } else {
                Style::default()
            };
            s.clear(line, base);
            let value = match (&self.editing, selected) {
                (Some(text), true) => format!("{text}▏"),
                _ => Self::show(&self.values[i]),
            };
            let mark = if changed { "*" } else { " " };
            let text = format!("{mark}{:<key_w$}{value}", spec.key);
            let style = if changed {
                Style { bold: true, ..base }
            } else {
                base
            };
            s.put_in(line, 0, &text, style, width);
        }
        let foot = area.y + area.h.saturating_sub(FOOTER);
        let spec = &self.specs[self.selected];
        let mut desc = spec.description.clone();
        match &spec.kind {
            SettingKind::Choice(c) => desc.push_str(&format!("  ({})", c.join(" | "))),
            SettingKind::Integer { min, max } if min.is_some() || max.is_some() => {
                desc.push_str(&format!(
                    "  ({}..{})",
                    min.map_or(String::new(), |m| m.to_string()),
                    max.map_or(String::new(), |m| m.to_string())
                ))
            }
            _ => {}
        }
        desc.push_str(&format!("  default {}", Self::show(&spec.default)));
        let row = |y: u16| Rect::new(area.x, y, area.w, 1);
        s.put_in(row(foot), 0, &desc, Style::DIM, width);
        if let Some(m) = &self.message {
            let color = if self.warned {
                Color::Yellow
            } else {
                Color::Red
            };
            s.put_in(row(foot + 1), 0, m, Style::default().fg(color), width);
        }
        let hint = if self.editing.is_some() {
            "type · Enter accept · Esc cancel"
        } else {
            "Enter edit · Space change · r default · s save · Esc back"
        };
        s.put_in(row(foot + 2), 0, hint, Style::BOLD.fg(Color::Blue), width);
    }

    fn handle_key(&mut self, key: Key) -> Outcome {
        if self.specs.is_empty() {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('q') => Outcome::Close,
                _ => Outcome::Handled,
            };
        }
        if let Some(text) = self.editing.as_mut() {
            match key.code {
                KeyCode::Enter => {
                    let t = std::mem::take(text);
                    self.accept(t);
                }
                KeyCode::Esc => {
                    self.editing = None;
                    self.message = None;
                }
                KeyCode::Backspace => {
                    text.pop();
                }
                KeyCode::Char(c) if !key.mods.ctrl => text.push(c),
                _ => {}
            }
            return Outcome::Handled;
        }
        let last = self.specs.len() - 1;
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.selected = (self.selected + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => self.selected = last,
            KeyCode::Enter => match self.specs[self.selected].kind {
                SettingKind::String => {
                    if let SettingValue::String(s) = &self.values[self.selected] {
                        self.editing = Some(s.clone());
                    }
                }
                SettingKind::Integer { .. } => {
                    if let SettingValue::Integer(n) = &self.values[self.selected] {
                        self.editing = Some(n.to_string());
                    }
                }
                _ => self.change(1),
            },
            KeyCode::Char(' ') | KeyCode::Char('l') | KeyCode::Right => self.change(1),
            KeyCode::Char('h') | KeyCode::Left => self.change(-1),
            KeyCode::Char('r') => {
                let d = self.specs[self.selected].default.clone();
                self.set(d);
            }
            KeyCode::Char('s') => {
                let values = self.changes();
                if values.is_empty() {
                    return Outcome::Close;
                }
                return Outcome::Submit {
                    command: self.command.clone(),
                    values,
                };
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                if self.changes().is_empty() || self.warned {
                    return Outcome::Close;
                }
                self.warned = true;
                self.message = Some("Unsaved changes: s save, Esc again to discard".into());
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Handled
    }

    /// No terminal cursor: the edited field shows its own caret.
    fn cursor(&self, _area: Rect) -> Option<(u16, u16)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::parse_keys;
    use vox_view::Monospace;

    fn form() -> Form {
        let specs = crate::settings::vox_settings()
            .into_iter()
            .filter(|s| {
                ["leader", "timeout_ms", "developer_mode", "ui.side_panel"]
                    .contains(&s.key.as_str())
            })
            .collect::<Vec<_>>();
        let values = specs.iter().map(|s| s.default.clone()).collect();
        Form::new("vox settings", specs, values, "settings save vox")
    }

    fn press(f: &mut Form, keys: &str) -> Vec<Outcome> {
        parse_keys(keys)
            .into_iter()
            .map(|k| f.handle_key(k))
            .collect()
    }

    fn draw(f: &Form) -> String {
        let mut s = Surface::new(70, 8);
        f.render(Rect::new(0, 0, 70, 8), &mut s, &Monospace);
        s.to_string()
    }

    #[test]
    fn edit_toggle_choice_and_save() {
        let mut f = form();
        // leader: edit text
        press(&mut f, "<CR><BS><Space><CR>");
        assert_eq!(f.value("leader"), Some(&SettingValue::String(" ".into())));
        // timeout_ms: invalid, then valid
        press(&mut f, "j<CR>x<CR>");
        assert!(draw(&f).contains("must be a whole number"));
        press(&mut f, "<BS><BS><BS><BS><BS>250<CR>");
        assert_eq!(f.value("timeout_ms"), Some(&SettingValue::Integer(250)));
        // bool and choice
        press(&mut f, "j<Space>jh");
        assert_eq!(f.value("developer_mode"), Some(&SettingValue::Bool(true)));
        assert_eq!(
            f.value("ui.side_panel"),
            Some(&SettingValue::String("left".into()))
        );
        assert!(draw(&f).contains("*leader"), "changed fields are marked");
        let out = press(&mut f, "s");
        let Outcome::Submit { command, values } = &out[0] else {
            panic!("{out:?}")
        };
        assert_eq!(command, "settings save vox");
        assert_eq!(values.len(), 4);
        assert!(values.contains(&("timeout_ms".into(), Value::Integer(250))));
    }

    #[test]
    fn limits_reset_and_unsaved_warning() {
        let mut f = form();
        press(&mut f, "j<CR><BS><BS><BS><BS>-5<CR>");
        assert!(draw(&f).contains("between 0 and 600000"));
        press(&mut f, "<Esc>"); // cancel the edit
        assert_eq!(f.value("timeout_ms"), Some(&SettingValue::Integer(1000)));
        press(&mut f, "jj<Space>");
        assert_eq!(press(&mut f, "<Esc>"), [Outcome::Handled], "warns first");
        assert!(draw(&f).contains("Unsaved changes"));
        assert_eq!(press(&mut f, "<Esc>"), [Outcome::Close]);
        let mut f = form();
        press(&mut f, "jj<Space>r");
        assert!(f.changes().is_empty(), "reset to default");
        assert_eq!(press(&mut f, "s"), [Outcome::Close], "nothing to save");
    }
}
