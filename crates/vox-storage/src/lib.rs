//! Text storage for real use: fast edits and line lookups on large files.
//!
//! Implements [`TextStorage`] using the `ropey` library, which keeps text as a balanced tree
//! of small chunks (so an edit does not copy the whole file). Built with ropey's
//! `unicode_lines` and `cr_lines` features off, so only `\n` breaks lines (as the trait
//! requires).

use std::ops::Range;

use ropey::Rope;
use vox_core::storage::TextStorage;

/// Tree-based text storage. Use [`Storage::boxed`] as the editor's storage factory.
#[derive(Debug, Clone, Default)]
pub struct Storage(Rope);

impl Storage {
    /// Create storage holding `text`.
    pub fn new(text: &str) -> Self {
        Self(Rope::from_str(text))
    }

    /// [`vox_core::storage::StorageFactory`] for `Storage`.
    pub fn boxed(text: &str) -> Box<dyn TextStorage> {
        Box::new(Self::new(text))
    }
}

impl TextStorage for Storage {
    fn len_chars(&self) -> usize {
        self.0.len_chars()
    }

    fn len_lines(&self) -> usize {
        self.0.len_lines()
    }

    fn char(&self, idx: usize) -> char {
        self.0.char(idx)
    }

    fn line_to_char(&self, line: usize) -> usize {
        self.0.line_to_char(line)
    }

    fn char_to_line(&self, idx: usize) -> usize {
        self.0.char_to_line(idx)
    }

    fn insert(&mut self, idx: usize, text: &str) {
        self.0.insert(idx, text);
    }

    fn remove(&mut self, range: Range<usize>) {
        self.0.remove(range);
    }

    fn slice(&self, range: Range<usize>) -> String {
        self.0.slice(range).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        vox_core::storage::conformance(Storage::boxed);
    }
}
