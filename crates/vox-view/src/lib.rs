//! Viewport state for vox: which part of a document is visible.

/// A vertical viewport over a document.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Viewport {
    /// Index of the first visible line.
    pub top: usize,
}

impl Viewport {
    /// Scroll by `delta` lines, clamped so the last document line stays reachable at the top.
    pub fn scroll_by(&mut self, delta: isize, line_count: usize) {
        let max_top = line_count.saturating_sub(1);
        self.top = self.top.saturating_add_signed(delta).min(max_top);
    }

    /// Jump to the first line.
    pub fn scroll_to_top(&mut self) {
        self.top = 0;
    }

    /// Jump so the last line is at the bottom of a viewport `height` lines tall.
    pub fn scroll_to_bottom(&mut self, line_count: usize, height: usize) {
        self.top = line_count.saturating_sub(height.max(1));
    }
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
    fn scroll_top_bottom() {
        let mut v = Viewport { top: 4 };
        v.scroll_to_bottom(100, 20);
        assert_eq!(v.top, 80);
        v.scroll_to_bottom(5, 20);
        assert_eq!(v.top, 0);
        v.scroll_to_top();
        assert_eq!(v.top, 0);
    }
}
