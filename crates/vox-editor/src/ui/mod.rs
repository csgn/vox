//! UI layer: panels (side, bottom), a modal, and notifications, drawn over the editor.
//!
//! A panel or modal shows a [`View`]. Views are plain data plus key handling; when a view
//! wants the editor to do something it returns an [`Outcome`] that names a command line, so
//! views stay independent of the editor and can later come from plugins.

mod layout;
mod notify;
mod views;

pub(crate) use layout::{Layout, compute};
pub use layout::{NotifyPosition, SidePosition, UiConfig};
pub use notify::{Level, Notification};
pub use views::{Confirm, Input, ListItem, ListView, TextView};

use vox_view::CharWidth;

use crate::keys::Key;
use crate::surface::{Rect, Surface};

/// What a view wants after handling a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The key was used; nothing else to do.
    Handled,
    /// The view does not use this key.
    Ignored,
    /// Close the view.
    Close,
    /// Run a command line (e.g. `"w"`) and keep the view open.
    Run(String),
    /// Close the view, then run a command line.
    RunAndClose(String),
}

/// Something shown in a panel or the modal.
pub trait View {
    /// Title shown in the border.
    fn title(&self) -> String;
    /// Draw the content inside `area` (the border is already drawn).
    fn render(&self, area: Rect, s: &mut Surface, width: &dyn CharWidth);
    /// Handle a key while this view has focus.
    fn handle_key(&mut self, key: Key) -> Outcome;
    /// Where the terminal cursor goes inside `area`; `None` hides it.
    fn cursor(&self, _area: Rect) -> Option<(u16, u16)> {
        None
    }
    /// `Some` if this is the live log view, so the editor can update it.
    fn as_log_view(&mut self) -> Option<&mut crate::log::LogView> {
        None
    }
}

/// Where a panel is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelSlot {
    /// The side panel (left or right, see [`UiConfig::side_panel`]).
    Side,
    /// The bottom panel.
    Bottom,
}

/// Which part of the screen receives keys (when no modal is open).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    /// The text.
    #[default]
    Editor,
    /// The side panel.
    Side,
    /// The bottom panel.
    Bottom,
}
