//! View state for vox: which part of a document is visible, and how text maps to screen cells.
//! `std` only: char widths come from an injected [`CharWidth`].

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

/// Terminal cell width of a char. Implemented by adapters (e.g. `vox-width`).
pub trait CharWidth {
    /// Cells `c` occupies: 0 (combining), 1, or 2 (wide). `None` for control chars.
    fn width(&self, c: char) -> Option<usize>;
}

/// Every non-control char is one cell. Fallback for tests and ASCII text.
#[derive(Debug, Clone, Copy, Default)]
pub struct Monospace;

impl CharWidth for Monospace {
    fn width(&self, c: char) -> Option<usize> {
        (!c.is_control()).then_some(1)
    }
}

/// Width of `c` when it starts at display column `at`.
fn char_width(w: &dyn CharWidth, c: char, at: usize) -> usize {
    if c == '\t' {
        TAB_WIDTH - at % TAB_WIDTH
    } else {
        w.width(c).unwrap_or(0)
    }
}

/// Display column where char column `col` of `line` starts. Tabs expand to tab stops.
pub fn display_col(w: &dyn CharWidth, line: &str, col: usize) -> usize {
    line.chars()
        .take(col)
        .fold(0, |x, c| x + char_width(w, c, x))
}

/// The part of `line` visible in display columns `[left, left + width)`.
/// Tabs become spaces; wide chars cut by an edge become spaces.
pub fn visible_slice(w: &dyn CharWidth, line: &str, left: usize, width: usize) -> String {
    let right = left + width;
    let mut out = String::new();
    let mut x = 0;
    for c in line.chars() {
        let start = x;
        let end = x + char_width(w, c, x);
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

    /// Test width: CJK ideographs are 2 cells, everything else 1.
    struct Wide;
    impl CharWidth for Wide {
        fn width(&self, c: char) -> Option<usize> {
            Some(if ('\u{4e00}'..='\u{9fff}').contains(&c) {
                2
            } else {
                1
            })
        }
    }

    #[test]
    fn display_col_tabs_and_wide() {
        assert_eq!(display_col(&Monospace, "abc", 2), 2);
        assert_eq!(display_col(&Monospace, "\tx", 1), 4);
        assert_eq!(display_col(&Monospace, "ab\tx", 3), 4);
        assert_eq!(display_col(&Wide, "日本x", 2), 4);
    }

    #[test]
    fn visible_slice_cases() {
        let m = &Monospace;
        assert_eq!(visible_slice(m, "hello world", 0, 5), "hello");
        assert_eq!(visible_slice(m, "hello world", 6, 20), "world");
        assert_eq!(visible_slice(m, "a\tb", 0, 10), "a   b");
        assert_eq!(visible_slice(m, "a\tb", 2, 10), "  b");
        assert_eq!(visible_slice(&Wide, "日本", 1, 10), " 本");
        assert_eq!(visible_slice(&Wide, "日本", 0, 3), "日 ");
    }

    #[test]
    fn monospace_control_chars() {
        assert_eq!(Monospace.width('a'), Some(1));
        assert_eq!(Monospace.width('\u{7}'), None);
    }
}
