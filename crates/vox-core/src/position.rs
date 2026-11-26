/// A line/column position in a document.
///
/// `col` counts chars (not bytes or display cells) and excludes the line terminator.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    /// Zero-based line index.
    pub line: usize,
    /// Zero-based char column.
    pub col: usize,
}

impl Position {
    /// Create a position.
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}
