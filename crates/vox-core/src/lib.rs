//! Core text model for vox: documents backed by a rope. No terminal or UI code.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ropey::Rope;

/// A text document, optionally associated with a file on disk.
#[derive(Debug, Clone, Default)]
pub struct Document {
    text: Rope,
    path: Option<PathBuf>,
}

impl Document {
    /// Create an unnamed document from a string.
    pub fn from_text(text: &str) -> Self {
        Self {
            text: Rope::from_str(text),
            path: None,
        }
    }

    /// Load a document from `path`. A missing file yields an empty document bound to `path`.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let text = match fs::File::open(path) {
            Ok(file) => Rope::from_reader(io::BufReader::new(file))?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => Rope::new(),
            Err(e) => return Err(e),
        };
        Ok(Self {
            text,
            path: Some(path.to_path_buf()),
        })
    }

    /// The underlying rope.
    pub fn text(&self) -> &Rope {
        &self.text
    }

    /// File path, if any.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Number of displayable lines. A trailing newline does not add an extra empty line.
    pub fn line_count(&self) -> usize {
        let n = self.text.len_lines();
        if n > 1 && self.text.line(n - 1).len_chars() == 0 {
            n - 1
        } else {
            n
        }
    }

    /// Line `idx` without its line terminator. Returns `None` if out of range.
    pub fn line(&self, idx: usize) -> Option<String> {
        if idx >= self.line_count() {
            return None;
        }
        let line = self.text.line(idx).to_string();
        Some(line.trim_end_matches(['\n', '\r']).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_count_ignores_trailing_newline() {
        assert_eq!(Document::from_text("").line_count(), 1);
        assert_eq!(Document::from_text("a").line_count(), 1);
        assert_eq!(Document::from_text("a\n").line_count(), 1);
        assert_eq!(Document::from_text("a\nb").line_count(), 2);
        assert_eq!(Document::from_text("a\n\n").line_count(), 2);
    }

    #[test]
    fn line_strips_terminators() {
        let doc = Document::from_text("one\r\ntwo\n");
        assert_eq!(doc.line(0).as_deref(), Some("one"));
        assert_eq!(doc.line(1).as_deref(), Some("two"));
        assert_eq!(doc.line(2), None);
    }

    #[test]
    fn open_missing_file_is_empty() {
        let dir = std::env::temp_dir().join("vox-core-test-missing");
        let path = dir.join("does-not-exist.txt");
        let doc = Document::open(&path).unwrap();
        assert_eq!(doc.path(), Some(path.as_path()));
        assert_eq!(doc.line_count(), 1);
    }

    #[test]
    fn open_reads_file() {
        let path = std::env::temp_dir().join(format!("vox-core-test-{}.txt", std::process::id()));
        fs::write(&path, "hello\nworld\n").unwrap();
        let doc = Document::open(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(doc.line_count(), 2);
        assert_eq!(doc.line(1).as_deref(), Some("world"));
    }
}
