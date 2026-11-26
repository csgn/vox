//! Editor events: things that happened, delivered to subscribers (e.g. plugins).

use std::rc::Rc;

use crate::Editor;

/// The kind of mode the editor is in, without the mode's data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModeKind {
    /// Normal mode.
    Normal,
    /// Insert mode.
    Insert,
    /// Visual mode (charwise or linewise).
    Visual,
    /// Typing a `:` command.
    Command,
    /// Typing a `/` search.
    Search,
}

/// Something that happened in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A file was opened (at start-up or with `:e`). `None` for an unnamed buffer.
    BufOpen {
        /// File path, if any.
        path: Option<String>,
    },
    /// The buffer was saved.
    BufWrite {
        /// File path written.
        path: String,
    },
    /// The text changed (sent once per key press or command, not per char).
    TextChanged,
    /// The mode changed.
    ModeChanged {
        /// The new mode.
        mode: ModeKind,
    },
}

/// Event categories, used to subscribe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// [`Event::BufOpen`].
    BufOpen,
    /// [`Event::BufWrite`].
    BufWrite,
    /// [`Event::TextChanged`].
    TextChanged,
    /// [`Event::ModeChanged`].
    ModeChanged,
}

impl Event {
    /// This event's category.
    pub fn kind(&self) -> EventKind {
        match self {
            Event::BufOpen { .. } => EventKind::BufOpen,
            Event::BufWrite { .. } => EventKind::BufWrite,
            Event::TextChanged => EventKind::TextChanged,
            Event::ModeChanged { .. } => EventKind::ModeChanged,
        }
    }
}

/// An event handler.
pub type EventHandler = Rc<dyn Fn(&mut Editor, &Event)>;
