//! Core text model for vox: documents, positions, edits with undo/redo, motions, search.
//! `std` only: concrete text storage is injected through [`storage::TextStorage`].

mod document;
pub mod motion;
mod position;
pub mod search;
pub mod storage;

pub use document::Document;
pub use position::Position;
