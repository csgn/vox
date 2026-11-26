//! [`Clipboard`] adapter for the system clipboard, backed by [`arboard`]
//! (X11 and Wayland on Linux, plus macOS and Windows).

use vox_editor::Clipboard;

/// The system clipboard, opened on first use.
#[derive(Default)]
pub struct SystemClipboard(Option<arboard::Clipboard>);

impl SystemClipboard {
    /// The arboard clipboard handle, created the first time it is needed.
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
