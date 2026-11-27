//! The `:` and `/` prompts: typing, the completion menu (`Tab`, arrows), and history.

use crate::complete::Completion;
use crate::editor::{Editor, Mode};
use crate::keys::{Key, KeyCode};
use crate::ui::CompletionMode;

/// Most lines kept in each prompt history.
const HISTORY: usize = 100;

/// Prompt state that is not the typed text (which lives in [`Mode`]).
#[derive(Debug, Clone, Default)]
pub(crate) struct PromptState {
    /// Completions being cycled with `Tab` / arrows, and the selected one (already in the
    /// line).
    cycle: Option<(Completion, usize)>,
    /// The text typed before cycling started (`Esc` goes back to it).
    before_cycle: String,
    /// The menu is hidden until the text changes (after walking the history).
    dismissed: bool,
    /// Position in the history while walking it with `Up` / `Down`.
    history_pos: Option<usize>,
    /// What was typed before walking the history.
    saved: String,
    /// `:` lines run, oldest first.
    commands: Vec<String>,
    /// `/` patterns searched, oldest first.
    searches: Vec<String>,
}

/// The completion menu as shown: completions and the selected entry, if any.
pub(crate) struct Menu {
    /// What is offered.
    pub completion: Completion,
    /// Selected entry (its text is in the line).
    pub selected: Option<usize>,
}

impl Editor {
    /// Start typing a `:` command (`search` false) or a `/` pattern.
    pub(crate) fn open_prompt(&mut self, search: bool) {
        self.message = None;
        self.prompt.cycle = None;
        self.prompt.dismissed = false;
        self.prompt.history_pos = None;
        self.mode = if search {
            Mode::Search(String::new())
        } else {
            Mode::Command(String::new())
        };
    }

    /// Lines typed at the `:` prompt, oldest first.
    pub fn command_history(&self) -> &[String] {
        &self.prompt.commands
    }

    /// The completion menu to show now, if any: while cycling, or (with
    /// `completion = "auto"`) whenever the line is not empty and `Esc` did not close it.
    pub(crate) fn prompt_menu(&self) -> Option<Menu> {
        let Mode::Command(line) = &self.mode else {
            return None;
        };
        if let Some((completion, i)) = &self.prompt.cycle {
            return Some(Menu {
                completion: completion.clone(),
                selected: Some(*i),
            });
        }
        if self.ui_config().completion != CompletionMode::Auto
            || self.prompt.dismissed
            || line.trim().is_empty()
        {
            return None;
        }
        let completion = self.complete(line);
        (!completion.items.is_empty() || completion.usage.is_some()).then_some(Menu {
            completion,
            selected: None,
        })
    }

    /// The typed text of the open prompt.
    fn prompt_text(&mut self) -> Option<&mut String> {
        match &mut self.mode {
            Mode::Command(s) | Mode::Search(s) => Some(s),
            _ => None,
        }
    }

    /// Replace the typed text.
    fn set_prompt_text(&mut self, text: String) {
        if let Some(t) = self.prompt_text() {
            *t = text;
        }
    }

    /// Select completion `i` of `completion` (wrapping), putting it into the line.
    fn select(&mut self, completion: Completion, i: isize) {
        let n = completion.items.len() as isize;
        if n == 0 {
            return;
        }
        let i = i.rem_euclid(n) as usize;
        let line = match &self.mode {
            Mode::Command(l) => l.clone(),
            _ => return,
        };
        if self.prompt.cycle.is_none() {
            self.prompt.before_cycle = line.clone();
        }
        // While cycling the line ends with the selected text; the prefix stays.
        let base = &line[..completion.start.min(line.len())];
        let text = format!("{base}{}", completion.items[i].text);
        self.set_prompt_text(text);
        self.prompt.cycle = Some((completion, i));
    }

    /// Move the selection by `step`, starting the menu if needed. Returns `false` if there
    /// is nothing to select.
    fn step_menu(&mut self, step: isize) -> bool {
        if let Some((c, i)) = self.prompt.cycle.clone() {
            self.select(c, i as isize + step);
            return true;
        }
        let Some(menu) = self
            .prompt_menu()
            .filter(|m| !m.completion.items.is_empty())
        else {
            return false;
        };
        let first = if step > 0 { 0 } else { -1 };
        self.select(menu.completion, first);
        true
    }

    /// `Tab`: accept the only completion, extend to the common prefix, or start cycling.
    fn tab(&mut self) {
        if self.ui_config().completion == CompletionMode::Off {
            return;
        }
        if self.prompt.cycle.is_some() {
            self.step_menu(1);
            return;
        }
        let Some(Mode::Command(line)) = Some(&self.mode) else {
            return;
        };
        let line = line.clone();
        let c = self.complete(&line);
        match c.items.len() {
            0 => {}
            1 => {
                let mut text = c.apply(&line, 0);
                if !text.ends_with('/') {
                    text.push(' ');
                }
                self.set_prompt_text(text);
                self.prompt.dismissed = false;
            }
            _ => match c.common_prefix(&line) {
                Some(longer) => self.set_prompt_text(longer),
                None => self.select(c, 0),
            },
        }
    }

    /// Walk the history: `older` goes back.
    fn walk_history(&mut self, older: bool) {
        let search = matches!(self.mode, Mode::Search(_));
        let len = if search {
            self.prompt.searches.len()
        } else {
            self.prompt.commands.len()
        };
        if len == 0 {
            return;
        }
        let pos = match (self.prompt.history_pos, older) {
            (None, true) => {
                self.prompt.saved = self.prompt_text().cloned().unwrap_or_default();
                Some(len - 1)
            }
            (None, false) => return,
            (Some(p), true) => Some(p.saturating_sub(1)),
            (Some(p), false) if p + 1 < len => Some(p + 1),
            (Some(_), false) => None,
        };
        self.prompt.history_pos = pos;
        let text = match pos {
            Some(p) if search => self.prompt.searches[p].clone(),
            Some(p) => self.prompt.commands[p].clone(),
            None => std::mem::take(&mut self.prompt.saved),
        };
        self.set_prompt_text(text);
        self.prompt.dismissed = true;
    }

    /// Remember `line` in the history (not twice in a row).
    fn remember(&mut self, line: &str, search: bool) {
        if line.trim().is_empty() {
            return;
        }
        let list = if search {
            &mut self.prompt.searches
        } else {
            &mut self.prompt.commands
        };
        if list.last().map(String::as_str) != Some(line) {
            list.push(line.to_string());
        }
        if list.len() > HISTORY {
            list.remove(0);
        }
    }

    /// Key handling for the `:` and `/` prompts.
    pub(crate) fn handle_prompt(&mut self, key: Key) {
        let search = matches!(self.mode, Mode::Search(_));
        let ctrl = key.mods.ctrl;
        match key.code {
            KeyCode::Esc => {
                // With an entry selected, go back to what was typed; else leave the prompt.
                if self.prompt.cycle.take().is_some() {
                    let typed = std::mem::take(&mut self.prompt.before_cycle);
                    self.set_prompt_text(typed);
                } else {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Enter => {
                let mode = std::mem::replace(&mut self.mode, Mode::Normal);
                self.prompt.cycle = None;
                match mode {
                    Mode::Search(input) => {
                        self.remember(&input, true);
                        self.search(input);
                    }
                    Mode::Command(input) => {
                        self.remember(&input, false);
                        self.run_command(&input);
                    }
                    _ => {}
                }
            }
            KeyCode::Tab if !search && key.mods.shift => {
                if self.ui_config().completion != CompletionMode::Off {
                    self.step_menu(-1);
                }
            }
            KeyCode::Tab if !search => self.tab(),
            KeyCode::Down => {
                if search || !self.step_menu(1) {
                    self.walk_history(false);
                }
            }
            KeyCode::Up => {
                if search || !self.step_menu(-1) {
                    self.walk_history(true);
                }
            }
            KeyCode::Char('n') if ctrl => {
                if search || !self.step_menu(1) {
                    self.walk_history(false);
                }
            }
            KeyCode::Char('p') if ctrl => {
                if search || !self.step_menu(-1) {
                    self.walk_history(true);
                }
            }
            KeyCode::Backspace => {
                self.prompt.cycle = None;
                self.prompt.dismissed = false;
                self.prompt.history_pos = None;
                if self.prompt_text().is_some_and(|t| t.pop().is_none()) {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Char(c) if !ctrl => {
                self.prompt.cycle = None;
                self.prompt.dismissed = false;
                self.prompt.history_pos = None;
                if let Some(t) = self.prompt_text() {
                    t.push(c);
                }
            }
            _ => {}
        }
    }
}
