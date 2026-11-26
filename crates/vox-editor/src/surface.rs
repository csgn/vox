//! Screen model: a grid of styled cells plus the cursor. The editor renders into a
//! [`Surface`]; a terminal adapter (e.g. `vox-term`) draws it.

use std::fmt;

use vox_view::CharWidth;

/// Foreground color of a cell. A small fixed set; the terminal adapter picks the real colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    /// Terminal default.
    #[default]
    Default,
    /// Errors.
    Red,
    /// Warnings.
    Yellow,
    /// Success.
    Green,
    /// Information, focus.
    Blue,
    /// Secondary text.
    Gray,
}

/// Visual attributes of a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    /// Faint text.
    pub dim: bool,
    /// Swapped foreground/background.
    pub reversed: bool,
    /// Bold text.
    pub bold: bool,
    /// Foreground color.
    pub fg: Color,
}

impl Style {
    /// Dim style.
    pub const DIM: Self = Self {
        dim: true,
        reversed: false,
        bold: false,
        fg: Color::Default,
    };
    /// Reversed style.
    pub const REVERSED: Self = Self {
        dim: false,
        reversed: true,
        bold: false,
        fg: Color::Default,
    };
    /// Bold style.
    pub const BOLD: Self = Self {
        dim: false,
        reversed: false,
        bold: true,
        fg: Color::Default,
    };

    /// This style with foreground `fg`.
    pub const fn fg(mut self, fg: Color) -> Self {
        self.fg = fg;
        self
    }
}

/// A rectangle of cells: top-left corner (`x`, `y`) and size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    /// Left column.
    pub x: u16,
    /// Top row.
    pub y: u16,
    /// Width in cells.
    pub w: u16,
    /// Height in cells.
    pub h: u16,
}

impl Rect {
    /// Create a rectangle.
    pub const fn new(x: u16, y: u16, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }

    /// The area inside a one-cell border.
    pub fn inner(self) -> Rect {
        Rect::new(
            self.x + 1,
            self.y + 1,
            self.w.saturating_sub(2),
            self.h.saturating_sub(2),
        )
    }

    /// Whether the rectangle has no cells.
    pub fn is_empty(self) -> bool {
        self.w == 0 || self.h == 0
    }
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
    /// Whether the terminal cursor is shown.
    pub cursor_visible: bool,
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
            cursor_visible: true,
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

impl Surface {
    /// Fill `area` with blank cells of `style`.
    pub fn clear(&mut self, area: Rect, style: Style) {
        for y in area.y..(area.y + area.h).min(self.height) {
            for x in area.x..(area.x + area.w).min(self.width) {
                let i = self.index(x, y);
                self.cells[i] = Cell {
                    symbol: " ".into(),
                    style,
                };
            }
        }
    }

    /// Clear `area` and draw a border around it with `title` in the top edge.
    pub fn draw_box(&mut self, area: Rect, title: &str, style: Style, width: &dyn CharWidth) {
        if area.w < 2 || area.h < 2 {
            return;
        }
        self.clear(area, Style::default());
        let (x0, y0) = (area.x, area.y);
        let (x1, y1) = (area.x + area.w - 1, area.y + area.h - 1);
        if x1 >= self.width || y1 >= self.height {
            return;
        }
        let mut put = |x: u16, y: u16, c: &str| {
            let i = self.index(x, y);
            self.cells[i] = Cell {
                symbol: c.into(),
                style,
            };
        };
        for x in x0 + 1..x1 {
            put(x, y0, "─");
            put(x, y1, "─");
        }
        for y in y0 + 1..y1 {
            put(x0, y, "│");
            put(x1, y, "│");
        }
        put(x0, y0, "┌");
        put(x1, y0, "┐");
        put(x0, y1, "└");
        put(x1, y1, "┘");
        if !title.is_empty() && area.w > 4 {
            let title = format!(" {title} ");
            let max = (area.w - 4) as usize;
            let title: String = title.chars().take(max).collect();
            self.put_str(x0 + 2, y0, &title, style, width);
        }
    }

    /// Write `text` inside `area` at row `row` (relative), clipped to the area.
    pub fn put_in(
        &mut self,
        area: Rect,
        row: u16,
        text: &str,
        style: Style,
        width: &dyn CharWidth,
    ) {
        if row >= area.h || area.w == 0 {
            return;
        }
        let mut cols = 0;
        let clipped: String = text
            .chars()
            .take_while(|c| {
                cols += width.width(*c).unwrap_or(0);
                cols <= area.w as usize
            })
            .collect();
        self.put_str(area.x, area.y + row, &clipped, style, width);
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
    fn draw_box_and_put_in() {
        let mut s = Surface::new(10, 4);
        s.draw_box(Rect::new(0, 0, 10, 4), "Hi", Style::default(), &Wide);
        s.put_in(
            Rect::new(0, 0, 10, 4).inner(),
            0,
            "abcdefghijk",
            Style::default(),
            &Wide,
        );
        assert_eq!(
            s.to_string(),
            "\"┌─ Hi ───┐\"\n\"│abcdefgh│\"\n\"│        │\"\n\"└────────┘\"\n"
        );
        assert_eq!(Rect::new(0, 0, 1, 1).inner(), Rect::new(1, 1, 0, 0));
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
