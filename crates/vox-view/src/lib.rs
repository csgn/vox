//! View state for vox: which part of a document is visible, and how text maps to screen cells.

use unicode_width::UnicodeWidthChar;

/// Display width of a tab stop.
pub const TAB_WIDTH: usize = 4;

/// A viewport over a document: first visible line and first visible display column.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Viewport {
    /// Index of the first visible line.
    pub top: usize,
    /// First visible display column (horizontal scroll).
    pub left: usize,
}

impl Viewport {
    /// Scroll by `delta` lines, clamped so the last document line stays reachable at the top.
    pub fn scroll_by(&mut self, delta: isize, line_count: usize) {
        let max_top = line_count.saturating_sub(1);
        self.top = self.top.saturating_add_signed(delta).min(max_top);
    }

    /// Adjust the viewport minimally so that (`line`, display column `dcol`) is visible in
    /// an area of `width` x `height` cells.
    pub fn follow(&mut self, line: usize, dcol: usize, width: usize, height: usize) {
        let (width, height) = (width.max(1), height.max(1));
        if line < self.top {
            self.top = line;
        } else if line >= self.top + height {
            self.top = line + 1 - height;
        }
        if dcol < self.left {
            self.left = dcol;
        } else if dcol >= self.left + width {
            self.left = dcol + 1 - width;
        }
    }
}

/// Width of `c` when it starts at display column `at`.
fn char_width(c: char, at: usize) -> usize {
    if c == '\t' {
        TAB_WIDTH - at % TAB_WIDTH
    } else {
        c.width().unwrap_or(0)
    }
}

/// Display column where char column `col` of `line` starts. Tabs expand to tab stops.
pub fn display_col(line: &str, col: usize) -> usize {
    line.chars().take(col).fold(0, |w, c| w + char_width(c, w))
}

/// The part of `line` visible in display columns `[left, left + width)`.
/// Tabs become spaces; wide chars cut by an edge become spaces.
pub fn visible_slice(line: &str, left: usize, width: usize) -> String {
    let right = left + width;
    let mut out = String::new();
    let mut x = 0;
    for c in line.chars() {
        let start = x;
        let end = x + char_width(c, x);
        x = end;
        if start < left && end <= left {
            continue;
        }
        if start >= right {
            break;
        }
        if c == '\t' || start < left || end > right {
            out.extend(std::iter::repeat_n(' ', end.min(right) - start.max(left)));
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_clamps() {
        let mut v = Viewport::default();
        v.scroll_by(-5, 10);
        assert_eq!(v.top, 0);
        v.scroll_by(3, 10);
        assert_eq!(v.top, 3);
        v.scroll_by(100, 10);
        assert_eq!(v.top, 9);
        v.scroll_by(1, 0);
        assert_eq!(v.top, 0);
    }

    #[test]
    fn follow_vertical_and_horizontal() {
        let mut v = Viewport::default();
        v.follow(15, 0, 10, 10);
        assert_eq!(v.top, 6);
        v.follow(8, 0, 10, 10);
        assert_eq!(v.top, 6, "already visible");
        v.follow(2, 0, 10, 10);
        assert_eq!(v.top, 2);
        v.follow(2, 25, 10, 10);
        assert_eq!(v.left, 16);
        v.follow(2, 3, 10, 10);
        assert_eq!(v.left, 3);
    }

    #[test]
    fn display_col_tabs_and_wide() {
        assert_eq!(display_col("abc", 2), 2);
        assert_eq!(display_col("\tx", 1), 4);
        assert_eq!(display_col("ab\tx", 3), 4);
        assert_eq!(display_col("日本x", 2), 4);
    }

    #[test]
    fn visible_slice_cases() {
        assert_eq!(visible_slice("hello world", 0, 5), "hello");
        assert_eq!(visible_slice("hello world", 6, 20), "world");
        assert_eq!(visible_slice("a\tb", 0, 10), "a   b");
        assert_eq!(visible_slice("a\tb", 2, 10), "  b");
        assert_eq!(visible_slice("日本", 1, 10), " 本");
        assert_eq!(visible_slice("日本", 0, 3), "日 ");
    }
}
