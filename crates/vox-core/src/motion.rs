//! Cursor motions. Pure functions: `(document, position) -> position`.
//!
//! Results are not clamped for a mode; callers clamp columns (e.g. normal mode cannot sit
//! on the line terminator).

use crate::{Document, Position};

/// Character class used by word motions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Punct,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Space
    } else if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else {
        Class::Punct
    }
}

/// Whether char index `i` is the start of an empty line (Vim treats those as words).
fn is_empty_line(doc: &Document, i: usize) -> bool {
    let text = doc.text();
    let n = text.len_chars();
    if i >= n {
        return i == n && i > 0 && text.char(i - 1) == '\n';
    }
    let at_line_start = i == 0 || text.char(i - 1) == '\n';
    let c = text.char(i);
    at_line_start && (c == '\n' || (c == '\r' && i + 1 < n && text.char(i + 1) == '\n'))
}

/// `w`: start of the next word. Empty lines count as words. Stops at the last char.
pub fn word_forward(doc: &Document, pos: Position) -> Position {
    let text = doc.text();
    let n = text.len_chars();
    let mut i = doc.char_idx(pos);
    if i >= n {
        return pos;
    }
    let start = class(text.char(i));
    if start != Class::Space {
        while i < n && class(text.char(i)) == start {
            i += 1;
        }
    }
    while i < n && class(text.char(i)) == Class::Space {
        if text.char(i) == '\n' && is_empty_line(doc, i + 1) {
            i += 1;
            break;
        }
        i += 1;
    }
    if i >= n && !is_empty_line(doc, i) {
        return doc.pos_at(n - 1);
    }
    doc.pos_at(i)
}

/// `b`: start of the current or previous word. Empty lines count as words.
pub fn word_backward(doc: &Document, pos: Position) -> Position {
    let text = doc.text();
    let mut i = doc.char_idx(pos);
    if i == 0 {
        return pos;
    }
    i -= 1;
    while i > 0 && class(text.char(i)) == Class::Space && !is_empty_line(doc, i) {
        i -= 1;
    }
    if is_empty_line(doc, i) {
        return doc.pos_at(i);
    }
    let cls = class(text.char(i));
    while i > 0 && class(text.char(i - 1)) == cls {
        i -= 1;
    }
    doc.pos_at(i)
}

/// `0`: first column.
pub fn line_start(pos: Position) -> Position {
    Position::new(pos.line, 0)
}

/// `$`: last char of the line (column 0 on an empty line).
pub fn line_end(doc: &Document, pos: Position) -> Position {
    Position::new(pos.line, doc.line_len(pos.line).saturating_sub(1))
}

/// First non-blank char of `line` (or its last column if all blank).
pub fn first_non_blank(doc: &Document, line: usize) -> Position {
    let line = line.min(doc.line_count() - 1);
    let text = doc.line(line).unwrap_or_default();
    let col = text
        .chars()
        .position(|c| !c.is_whitespace())
        .unwrap_or(text.chars().count().saturating_sub(1));
    Position::new(line, col)
}

/// `gg`: first non-blank of the first line.
pub fn doc_start(doc: &Document) -> Position {
    first_non_blank(doc, 0)
}

/// `G`: first non-blank of the last line.
pub fn doc_end(doc: &Document) -> Position {
    first_non_blank(doc, doc.line_count() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(line: usize, col: usize) -> Position {
        Position::new(line, col)
    }

    /// Apply `motion` repeatedly from (0,0), collecting each stop.
    fn stops(
        text: &str,
        motion: fn(&Document, Position) -> Position,
        from: Position,
    ) -> Vec<Position> {
        let doc = Document::from_text(text);
        let mut out = vec![];
        let mut pos = from;
        loop {
            let next = motion(&doc, pos);
            if next == pos {
                return out;
            }
            out.push(next);
            pos = next;
        }
    }

    #[test]
    fn w_words_and_punctuation() {
        assert_eq!(
            stops("foo bar.baz  qux", word_forward, p(0, 0)),
            vec![p(0, 4), p(0, 7), p(0, 8), p(0, 13), p(0, 15)]
        );
    }

    #[test]
    fn w_crosses_lines_and_stops_on_empty_line() {
        assert_eq!(
            stops("a\n\n  b\nc", word_forward, p(0, 0)),
            vec![p(1, 0), p(2, 2), p(3, 0)]
        );
    }

    #[test]
    fn w_to_trailing_empty_line() {
        assert_eq!(stops("a\n\n", word_forward, p(0, 0)), vec![p(1, 0)]);
        assert_eq!(stops("a  ", word_forward, p(0, 0)), vec![p(0, 2)]);
    }

    #[test]
    fn w_on_crlf() {
        assert_eq!(
            stops("a\r\n\r\nb", word_forward, p(0, 0)),
            vec![p(1, 0), p(2, 0)]
        );
    }

    #[test]
    fn b_words_and_punctuation() {
        assert_eq!(
            stops("foo bar.baz  qux", word_backward, p(0, 15)),
            vec![p(0, 13), p(0, 8), p(0, 7), p(0, 4), p(0, 0)]
        );
    }

    #[test]
    fn b_crosses_lines_and_stops_on_empty_line() {
        assert_eq!(
            stops("a\n\n  b\nc", word_backward, p(3, 0)),
            vec![p(2, 2), p(1, 0), p(0, 0)]
        );
    }

    #[test]
    fn line_motions() {
        let doc = Document::from_text("  hello\n\n   ");
        assert_eq!(line_start(p(0, 4)), p(0, 0));
        assert_eq!(line_end(&doc, p(0, 0)), p(0, 6));
        assert_eq!(line_end(&doc, p(1, 0)), p(1, 0));
        assert_eq!(doc_start(&doc), p(0, 2));
        assert_eq!(doc_end(&doc), p(2, 2));
        assert_eq!(first_non_blank(&doc, 1), p(1, 0));
    }
}
