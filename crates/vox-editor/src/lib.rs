//! The vox editor: modes, Vim grammar, registers, `:` commands, and rendering to a
//! [`Surface`]. `std` only: terminal, clipboard, text storage, and char widths are
//! injected through [`Services`] by adapter crates.

mod editor;
mod keys;
mod normal;
mod register;
mod render;
mod surface;

pub use editor::{Editor, Mode, Services};
pub use keys::{Key, KeyCode, Modifiers, parse_keys};
pub use register::{Clipboard, NoClipboard};
pub use surface::{Cell, CursorShape, Style, Surface};
