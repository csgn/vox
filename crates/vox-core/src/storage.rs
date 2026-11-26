//! Text storage abstraction. Core code never names a concrete text library; adapters
//! (e.g. `vox-storage`) implement [`TextStorage`].

use std::fmt::Debug;
use std::ops::Range;

/// Character-indexed text storage.
///
/// All indices are char indices. Lines are separated by `\n` only (a `\r` before it is
/// part of the line's content as far as storage is concerned). A text with `k` newlines
/// has `k + 1` lines; the empty text has one line.
pub trait TextStorage: Debug {
    /// Number of chars.
    fn len_chars(&self) -> usize;
    /// Number of lines (newline count + 1).
    fn len_lines(&self) -> usize;
    /// Char at `idx`. Panics if `idx >= len_chars()`.
    fn char(&self, idx: usize) -> char;
    /// Char index where `line` starts. `line == len_lines()` returns `len_chars()`.
    fn line_to_char(&self, line: usize) -> usize;
    /// Line containing char `idx`. `idx == len_chars()` returns the last line.
    fn char_to_line(&self, idx: usize) -> usize;
    /// Insert `text` before char `idx`.
    fn insert(&mut self, idx: usize, text: &str);
    /// Remove chars in `range`.
    fn remove(&mut self, range: Range<usize>);
    /// Copy chars in `range` into a string.
    fn slice(&self, range: Range<usize>) -> String;
}

/// Builds a storage from initial text.
pub type StorageFactory = fn(&str) -> Box<dyn TextStorage>;

/// Simple `Vec<char>` storage with a line index. O(n) edits; meant for tests and small
/// texts. Production uses an adapter.
#[derive(Debug, Clone, Default)]
pub struct SimpleStorage {
    chars: Vec<char>,
    /// Char index of each line start.
    line_starts: Vec<usize>,
}

impl SimpleStorage {
    /// Create storage holding `text`.
    pub fn new(text: &str) -> Self {
        let mut s = Self {
            chars: text.chars().collect(),
            line_starts: Vec::new(),
        };
        s.reindex();
        s
    }

    /// [`StorageFactory`] for `SimpleStorage`.
    pub fn boxed(text: &str) -> Box<dyn TextStorage> {
        Box::new(Self::new(text))
    }

    /// Rebuild the line-start index after an edit.
    fn reindex(&mut self) {
        self.line_starts = std::iter::once(0)
            .chain(
                self.chars
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| **c == '\n')
                    .map(|(i, _)| i + 1),
            )
            .collect();
    }
}

impl TextStorage for SimpleStorage {
    fn len_chars(&self) -> usize {
        self.chars.len()
    }

    fn len_lines(&self) -> usize {
        self.line_starts.len()
    }

    fn char(&self, idx: usize) -> char {
        self.chars[idx]
    }

    fn line_to_char(&self, line: usize) -> usize {
        self.line_starts
            .get(line)
            .copied()
            .unwrap_or(self.chars.len())
    }

    fn char_to_line(&self, idx: usize) -> usize {
        self.line_starts.partition_point(|&s| s <= idx) - 1
    }

    fn insert(&mut self, idx: usize, text: &str) {
        self.chars.splice(idx..idx, text.chars());
        self.reindex();
    }

    fn remove(&mut self, range: Range<usize>) {
        self.chars.drain(range);
        self.reindex();
    }

    fn slice(&self, range: Range<usize>) -> String {
        self.chars[range].iter().collect()
    }
}

/// Behaviour checks every [`TextStorage`] implementation must pass. Panics on failure.
/// Adapters call this from their tests with their factory.
pub fn conformance(factory: StorageFactory) {
    let empty = factory("");
    assert_eq!(empty.len_chars(), 0);
    assert_eq!(empty.len_lines(), 1);
    assert_eq!(empty.line_to_char(0), 0);
    assert_eq!(empty.char_to_line(0), 0);

    let mut s = factory("ab\ncd\r\n\nğü");
    assert_eq!(s.len_chars(), 10);
    assert_eq!(s.len_lines(), 4);
    assert_eq!(s.char(8), 'ğ');
    assert_eq!(
        (0..=4).map(|l| s.line_to_char(l)).collect::<Vec<_>>(),
        [0, 3, 7, 8, 10]
    );
    assert_eq!(s.char_to_line(2), 0, "newline belongs to its line");
    assert_eq!(s.char_to_line(3), 1);
    assert_eq!(s.char_to_line(10), 3, "end of text is on last line");
    assert_eq!(s.slice(3..7), "cd\r\n");

    // Lone `\r` is not a line break.
    assert_eq!(factory("a\rb").len_lines(), 1);
    // Unicode line separators are not line breaks.
    assert_eq!(factory("a\u{2028}b\u{85}c").len_lines(), 1);

    s.insert(2, "X\nY");
    assert_eq!(s.slice(0..s.len_chars()), "abX\nY\ncd\r\n\nğü");
    assert_eq!(s.len_lines(), 5);
    s.remove(0..6);
    assert_eq!(s.slice(0..s.len_chars()), "cd\r\n\nğü");
    assert_eq!(s.len_lines(), 3);
    s.insert(s.len_chars(), "\n");
    assert_eq!(s.len_lines(), 4);
    assert_eq!(s.line_to_char(3), s.len_chars());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_storage_conforms() {
        conformance(SimpleStorage::boxed);
    }
}
