//! Registers: the unnamed register and the system clipboard (`"+`, `"*`).

/// Text held in a register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reg {
    /// Content with `\n` line endings. Linewise content ends with `\n`.
    pub text: String,
    /// Whether the content is whole lines.
    pub linewise: bool,
}

/// Access to a clipboard. Lets tests replace the system clipboard.
pub trait Clipboard {
    /// Read clipboard text.
    fn get(&mut self) -> Result<String, String>;
    /// Replace clipboard text.
    fn set(&mut self, text: String) -> Result<(), String>;
}

/// The system clipboard, opened on first use.
#[derive(Default)]
pub struct SystemClipboard(Option<arboard::Clipboard>);

impl SystemClipboard {
    fn inner(&mut self) -> Result<&mut arboard::Clipboard, String> {
        if self.0.is_none() {
            self.0 = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
        }
        Ok(self.0.as_mut().expect("initialized above"))
    }
}

impl Clipboard for SystemClipboard {
    fn get(&mut self) -> Result<String, String> {
        self.inner()?.get_text().map_err(|e| e.to_string())
    }

    fn set(&mut self, text: String) -> Result<(), String> {
        self.inner()?.set_text(text).map_err(|e| e.to_string())
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
            clipboard: Box::new(SystemClipboard::default()),
        }
    }
}

/// Whether `name` is a supported register name.
pub(crate) fn is_valid(name: char) -> bool {
    matches!(name, '"' | '+' | '*')
}

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
