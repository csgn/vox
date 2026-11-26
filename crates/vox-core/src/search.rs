//! Plain-text (literal, case-sensitive) search with wraparound.

use crate::{Document, Position};

/// A search hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    /// Start of the match.
    pub pos: Position,
    /// Whether the search wrapped past the end (forward) or start (backward).
    pub wrapped: bool,
}

/// Find `pattern` after (`forward`) or before `from`, wrapping around the document.
/// Returns `None` for an empty pattern or no match.
pub fn find(doc: &Document, from: Position, pattern: &str, forward: bool) -> Option<Match> {
    if pattern.is_empty() {
        return None;
    }
    let text = doc.to_string();
    let cur = doc.char_idx(from);
    let hit = |byte: usize, wrapped| Match {
        pos: doc.pos_at(text[..byte].chars().count()),
        wrapped,
    };
    if forward {
        let start = char_to_byte(&text, cur + 1);
        if let Some(b) = text[start..].find(pattern) {
            return Some(hit(start + b, false));
        }
        text.find(pattern).map(|b| hit(b, true))
    } else {
        let end = char_to_byte(&text, cur);
        if let Some(b) = text[..end].rfind(pattern) {
            return Some(hit(b, false));
        }
        text.rfind(pattern).map(|b| hit(b, true))
    }
}

/// Byte offset of char `idx` in `s`, or `s.len()` past the end.
fn char_to_byte(s: &str, idx: usize) -> usize {
    s.char_indices().nth(idx).map_or(s.len(), |(b, _)| b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(line: usize, col: usize) -> Position {
        Position::new(line, col)
    }

    #[test]
    fn forward_and_wrap() {
        let doc = Document::from_text("foo bar\nfoo baz");
        let m = find(&doc, p(0, 0), "foo", true).unwrap();
        assert_eq!((m.pos, m.wrapped), (p(1, 0), false));
        let m = find(&doc, p(1, 0), "foo", true).unwrap();
        assert_eq!((m.pos, m.wrapped), (p(0, 0), true));
    }

    #[test]
    fn backward_and_wrap() {
        let doc = Document::from_text("foo bar\nfoo baz");
        let m = find(&doc, p(1, 0), "foo", false).unwrap();
        assert_eq!((m.pos, m.wrapped), (p(0, 0), false));
        let m = find(&doc, p(0, 0), "foo", false).unwrap();
        assert_eq!((m.pos, m.wrapped), (p(1, 0), true));
    }

    #[test]
    fn no_match_and_empty() {
        let doc = Document::from_text("abc");
        assert_eq!(find(&doc, p(0, 0), "x", true), None);
        assert_eq!(find(&doc, p(0, 0), "", true), None);
    }

    #[test]
    fn multibyte() {
        let doc = Document::from_text("ğüş ğüş");
        let m = find(&doc, p(0, 0), "ğü", true).unwrap();
        assert_eq!(m.pos, p(0, 4));
    }
}
