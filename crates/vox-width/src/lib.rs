//! Character widths for real use: how many terminal cells each char takes
//! (e.g. 2 for CJK, 0 for combining accents). Implements [`CharWidth`] using the
//! `unicode-width` library.

use unicode_width::UnicodeWidthChar;
use vox_view::CharWidth;

/// Char widths per Unicode East Asian Width rules.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnicodeWidth;

impl CharWidth for UnicodeWidth {
    fn width(&self, c: char) -> Option<usize> {
        c.width()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_view::{display_col, visible_slice};

    #[test]
    fn widths() {
        assert_eq!(UnicodeWidth.width('a'), Some(1));
        assert_eq!(UnicodeWidth.width('日'), Some(2));
        assert_eq!(UnicodeWidth.width('\u{301}'), Some(0), "combining accent");
        assert_eq!(UnicodeWidth.width('\u{7}'), None, "control char");
    }

    #[test]
    fn with_view_helpers() {
        assert_eq!(display_col(&UnicodeWidth, "日本x", 2), 4);
        assert_eq!(visible_slice(&UnicodeWidth, "日本", 1, 10), " 本");
        assert_eq!(visible_slice(&UnicodeWidth, "日本", 0, 3), "日 ");
    }
}
