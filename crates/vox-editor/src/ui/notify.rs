//! Notifications: short messages shown in a corner for a few seconds.

use std::time::Instant;

use crate::surface::Color;

/// Importance of a notification or message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Something happened.
    Info,
    /// Something may be wrong.
    Warn,
    /// Something failed.
    Error,
}

impl Level {
    /// Lowercase name: `info`, `warning`, `error`.
    pub fn name(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warning",
            Level::Error => "error",
        }
    }

    /// Parse `info`, `warn`/`warning`, `error`.
    pub fn parse(s: &str) -> Option<Level> {
        match s {
            "info" => Some(Level::Info),
            "warn" | "warning" => Some(Level::Warn),
            "error" => Some(Level::Error),
            _ => None,
        }
    }

    /// Color used to show this level.
    pub fn color(self) -> Color {
        match self {
            Level::Info => Color::Blue,
            Level::Warn => Color::Yellow,
            Level::Error => Color::Red,
        }
    }
}

/// A notification on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Importance.
    pub level: Level,
    /// Text.
    pub text: String,
    /// When it disappears.
    pub expires: Instant,
}
