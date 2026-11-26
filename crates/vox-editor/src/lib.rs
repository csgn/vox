//! The vox editor: modes, Vim grammar, registers, the command registry, key mappings,
//! configuration, and rendering to a [`Surface`]. `std` only: terminal, clipboard, text
//! storage, char widths, and config file parsing are provided by adapter crates.

mod builtin;
pub mod command;
pub mod config;
mod editor;
pub mod event;
pub mod keymap;
mod keys;
pub mod log;
mod normal;
pub mod owner;
pub mod plugin;
mod register;
mod render;
pub mod scope;
pub mod settings;
mod surface;
pub mod ui;
pub mod update;

pub use command::{Args, CommandResult, Commands};
pub use config::{Config, Value};
pub use editor::{Editor, JobDone, Mode, Services};
pub use event::{Event, EventKind, ModeKind};
pub use keymap::{Binding, Keymap, MapMode};
pub use keys::{Key, KeyCode, Modifiers, format_keys, parse_keys};
pub use log::{Log, LogEntry, LogView};
pub use register::{Clipboard, NoClipboard};
pub use scope::{Scope, ScopeKind};
pub use surface::{Cell, Color, CursorShape, Rect, Style, Surface};
pub use ui::{Focus, Level, Outcome, PanelSlot, UiConfig, View};
