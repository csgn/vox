//! Core text model for vox: documents, positions, edits with undo/redo, and motions.
//! No terminal or UI code.

mod document;
pub mod motion;
mod position;

pub use document::Document;
pub use position::Position;
