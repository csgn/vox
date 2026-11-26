//! The vox editor: modes, Vim grammar, registers, the command registry, key mappings,
//! configuration, and rendering to a [`Surface`]. `std` only: terminal, clipboard, text
//! storage, char widths, and config file parsing are provided by adapter crates.

mod builtin;
pub mod command;
pub mod config;
mod editor;
pub mod keymap;
mod keys;
mod normal;
mod register;
mod render;
mod surface;

pub use command::{Args, CommandResult, Commands};
pub use config::{Config, Value};
pub use editor::{Editor, Mode, Services};
pub use keymap::{Binding, Keymap, MapMode};
pub use keys::{Key, KeyCode, Modifiers, parse_keys};
pub use register::{Clipboard, NoClipboard};
pub use surface::{Cell, CursorShape, Style, Surface};
