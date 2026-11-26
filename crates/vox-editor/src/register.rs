//! Registers: the unnamed register and the system clipboard (`"+`, `"*`).

/// Text held in a register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reg {
    /// Content with `\n` line endings. Linewise content ends with `\n`.
    pub text: String,
    /// Whether the content is whole lines.
    pub linewise: bool,
}

/// Access to a clipboard. Implemented by adapters (e.g. `vox-clipboard`) and tests.
pub trait Clipboard {
    /// Read clipboard text.
    fn get(&mut self) -> Result<String, String>;
    /// Replace clipboard text.
    fn set(&mut self, text: String) -> Result<(), String>;
}

/// Placeholder clipboard used when none is injected: every access fails.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoClipboard;

impl Clipboard for NoClipboard {
    fn get(&mut self) -> Result<String, String> {
        Err("no clipboard available".into())
    }

    fn set(&mut self, _text: String) -> Result<(), String> {
        Err("no clipboard available".into())
    }
}

/// All registers.
pub(crate) struct Registers {
    unnamed: Option<Reg>,
    pub(crate) clipboard: Box<dyn Clipboard>,
}

impl std::fmt::Debug for Registers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registers")
            .field("unnamed", &self.unnamed)
            .finish_non_exhaustive()
    }
}

impl Default for Registers {
    fn default() -> Self {
        Self {
            unnamed: None,
            clipboard: Box::new(NoClipboard),
        }
    }
}

/// Whether `name` is a supported register name.
pub(crate) fn is_valid(name: char) -> bool {
    matches!(name, '"' | '+' | '*')
}

/// Whether register `name` is the system clipboard (`"+` or `"*`).
fn is_clipboard(name: Option<char>) -> bool {
    matches!(name, Some('+' | '*'))
}

impl Registers {
    /// Read register `name` (`None` = unnamed).
    pub(crate) fn get(&mut self, name: Option<char>) -> Result<Reg, String> {
        if is_clipboard(name) {
            let text = self.clipboard.get()?.replace("\r\n", "\n");
            let linewise = text.ends_with('\n');
            return Ok(Reg { text, linewise });
        }
        self.unnamed
            .clone()
            .ok_or_else(|| "E353: Nothing in register \"".to_string())
    }

    /// Write register `name` (`None` = unnamed).
    pub(crate) fn set(&mut self, name: Option<char>, reg: Reg) -> Result<(), String> {
        if is_clipboard(name) {
            return self.clipboard.set(reg.text);
        }
        self.unnamed = Some(reg);
        Ok(())
    }
}
