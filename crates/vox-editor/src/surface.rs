//! Screen model: a grid of styled cells plus the cursor. The editor renders into a
//! [`Surface`]; a terminal adapter (e.g. `vox-term`) draws it.

use std::fmt;

use vox_view::CharWidth;

/// Visual attributes of a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    /// Faint text.
    pub dim: bool,
    /// Swapped foreground/background.
    pub reversed: bool,
}

impl Style {
    /// Dim style.
    pub const DIM: Self = Self {
        dim: true,
        reversed: false,
    };
    /// Reversed style.
    pub const REVERSED: Self = Self {
        dim: false,
        reversed: true,
    };
}

/// One screen cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// Text drawn in the cell. Empty for the second half of a wide char.
    pub symbol: String,
    /// Attributes.
    pub style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            symbol: " ".into(),
            style: Style::default(),
        }
    }
}

/// Terminal cursor shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    /// Block cursor (normal mode).
    #[default]
    Block,
    /// Bar cursor (insert mode).
    Bar,
}

/// A `width` x `height` grid of cells with a cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surface {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
    /// Cursor (column, row).
    pub cursor: (u16, u16),
    /// Cursor shape.
    pub cursor_shape: CursorShape,
}

impl Surface {
    /// A blank surface.
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); width as usize * height as usize],
            cursor: (0, 0),
            cursor_shape: CursorShape::Block,
        }
    }

    /// Width in cells.
    pub fn width(&self) -> u16 {
        self.width
    }

    /// Height in cells.
    pub fn height(&self) -> u16 {
        self.height
    }

    /// Cell at (`x`, `y`). Panics if out of bounds.
    pub fn cell(&self, x: u16, y: u16) -> &Cell {
        &self.cells[self.index(x, y)]
    }

    /// Cells of row `y`.
    pub fn row(&self, y: u16) -> &[Cell] {
        let start = self.index(0, y);
        &self.cells[start..start + self.width as usize]
    }

    /// Index into `cells` for (`x`, `y`).
    fn index(&self, x: u16, y: u16) -> usize {
        assert!(x < self.width && y < self.height, "cell out of bounds");
        y as usize * self.width as usize + x as usize
    }

    /// Write `text` at (`x`, `y`) with `style`, clipped at the right edge. Wide chars take
    /// two cells; a wide char that does not fit becomes a space. Returns the next column.
    pub fn put_str(
        &mut self,
        x: u16,
        y: u16,
        text: &str,
        style: Style,
        width: &dyn CharWidth,
    ) -> u16 {
        if y >= self.height {
            return x;
        }
        let mut x = x;
        for c in text.chars() {
            let w = width.width(c).unwrap_or(0) as u16;
            if w == 0 {
                continue;
            }
            if x + w > self.width {
                if x < self.width {
                    let i = self.index(x, y);
                    self.cells[i] = Cell {
                        symbol: " ".into(),
                        style,
                    };
                }
                return self.width;
            }
            let i = self.index(x, y);
            self.cells[i] = Cell {
                symbol: c.to_string(),
                style,
            };
            for k in 1..w {
                let j = self.index(x + k, y);
                self.cells[j] = Cell {
                    symbol: String::new(),
                    style,
                };
            }
            x += w;
        }
        x
    }

    /// Set the style of `len` cells starting at (`x`, `y`), clipped to the surface.
    pub fn set_style(&mut self, x: u16, y: u16, len: u16, style: Style) {
        if y >= self.height {
            return;
        }
        for cx in x..x.saturating_add(len).min(self.width) {
            let i = self.index(cx, y);
            self.cells[i].style = style;
        }
    }

    /// Fill row `y` with blank cells of `style`.
    pub fn fill_row(&mut self, y: u16, style: Style) {
        if y >= self.height {
            return;
        }
        for x in 0..self.width {
            let i = self.index(x, y);
            self.cells[i] = Cell {
                symbol: " ".into(),
                style,
            };
        }
    }
}

/// Rows as quoted text lines (styles omitted). Used for snapshot tests.
impl fmt::Display for Surface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for y in 0..self.height {
            let row: String = self.row(y).iter().map(|c| c.symbol.as_str()).collect();
            writeln!(f, "\"{row}\"")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Wide;
    impl CharWidth for Wide {
        fn width(&self, c: char) -> Option<usize> {
            Some(if c == '日' { 2 } else { 1 })
        }
    }

    #[test]
    fn put_str_clips_and_handles_wide() {
        let mut s = Surface::new(5, 1);
        assert_eq!(s.put_str(0, 0, "ab日", Style::default(), &Wide), 4);
        assert_eq!(s.to_string(), "\"ab日 \"\n");
        assert_eq!(s.cell(3, 0).symbol, "");
        let mut s = Surface::new(3, 1);
        s.put_str(0, 0, "ab日", Style::default(), &Wide);
        assert_eq!(s.to_string(), "\"ab \"\n");
        let mut s = Surface::new(3, 1);
        s.put_str(1, 0, "abcdef", Style::DIM, &Wide);
        assert_eq!(s.to_string(), "\" ab\"\n");
        assert_eq!(s.cell(1, 0).style, Style::DIM);
    }

    #[test]
    fn set_style_clips() {
        let mut s = Surface::new(3, 2);
        s.set_style(1, 1, 10, Style::REVERSED);
        assert_eq!(s.cell(0, 1).style, Style::default());
        assert_eq!(s.cell(2, 1).style, Style::REVERSED);
        s.set_style(0, 5, 1, Style::REVERSED);
    }
}
