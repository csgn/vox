//! Editor modes: `text` (the Vim-style editor, no AI) and `agent` (the same editor plus AI
//! help on the focused unit). Things only agent mode has are registered with owner
//! [`owner::AGENT`] and removed when switching back to text mode.

use crate::command::{Args, CommandResult, Commands, Param};
use crate::config::Value;
use crate::editor::Editor;
use crate::owner;

/// Which editor vox is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorMode {
    /// The Vim-style editor, no AI.
    #[default]
    Text,
    /// The editor plus AI help on the focused unit.
    Agent,
}

impl EditorMode {
    /// Config name: `text`, `agent`.
    pub fn name(self) -> &'static str {
        match self {
            EditorMode::Text => "text",
            EditorMode::Agent => "agent",
        }
    }

    /// Parse a config name.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "text" => Some(EditorMode::Text),
            "agent" => Some(EditorMode::Agent),
            _ => None,
        }
    }
}

impl Editor {
    /// The editor mode in use.
    pub fn editor_mode(&self) -> EditorMode {
        self.editor_mode
    }

    /// Switch the editor mode. Leaving agent mode removes everything registered for it.
    pub fn set_editor_mode(&mut self, mode: EditorMode) {
        if mode == self.editor_mode {
            return;
        }
        self.editor_mode = mode;
        match mode {
            EditorMode::Text => {
                self.remove_owned(owner::AGENT);
                self.proposal = None;
            }
            EditorMode::Agent => self.register_agent(),
        }
    }
}

/// Register `:mode`.
pub(crate) fn register(c: &mut Commands) {
    use crate::command::category::INTERFACE;
    c.register(
        "mode",
        "Switch the editor mode: `:mode text|agent|toggle`; no argument shows it.",
        mode,
    )
    .category(INTERFACE)
    .param(
        Param::optional("MODE", "text: the editor only; agent: with AI help.")
            .values(&["text", "agent", "toggle"]),
    )
    .returns("Without an argument: the mode (`text` or `agent`).")
    .example("mode agent", "turn on AI help (also <leader>m toggles)");
}

/// `:mode [text|agent|toggle]`.
fn mode(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let new = match a.arg {
        None => {
            ed.reply(Value::from(ed.editor_mode().name()));
            return Ok(());
        }
        Some("toggle") => match ed.editor_mode() {
            EditorMode::Text => EditorMode::Agent,
            EditorMode::Agent => EditorMode::Text,
        },
        Some(m) => EditorMode::parse(m)
            .ok_or_else(|| format!("E474: Use :mode text|agent|toggle, not {m}"))?,
    };
    ed.set_editor_mode(new);
    ed.set_message(format!("{} mode", new.name()));
    Ok(())
}
