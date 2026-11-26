use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use ropey::Rope;

use crate::Position;

/// A text document, optionally associated with a file on disk.
///
/// The final line terminator of a file is not stored in the rope; it is remembered and
/// written back on save. So every rope line is a visible line.
#[derive(Debug, Clone)]
pub struct Document {
    text: Rope,
    path: Option<PathBuf>,
    /// Line ending used for new lines and the final terminator: `"\n"` or `"\r\n"`.
    line_ending: &'static str,
    /// Whether the file ends with a line terminator.
    eol: bool,
    history: History,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            text: Rope::new(),
            path: None,
            line_ending: "\n",
            eol: true,
            history: History::default(),
        }
    }
}

impl Document {
    /// Create an unnamed document from a string.
    pub fn from_text(text: &str) -> Self {
        let mut doc = Self {
            text: Rope::from_str(text),
            ..Self::default()
        };
        doc.normalize_loaded();
        doc
    }

    /// Load a document from `path`. A missing file yields an empty document bound to `path`.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let mut doc = Self {
            path: Some(path.to_path_buf()),
            ..Self::default()
        };
        match fs::File::open(path) {
            Ok(file) => {
                doc.text = Rope::from_reader(io::BufReader::new(file))?;
                doc.normalize_loaded();
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        Ok(doc)
    }

    /// Detect line ending and strip the final terminator into `eol`.
    fn normalize_loaded(&mut self) {
        if self.text.len_lines() > 1 && self.text.line(0).to_string().ends_with("\r\n") {
            self.line_ending = "\r\n";
        }
        let n = self.text.len_chars();
        // Empty input: new text gets a final newline on save.
        self.eol = n == 0 || self.text.char(n - 1) == '\n';
        if n > 0 && self.eol {
            let cut = if n >= 2 && self.text.char(n - 2) == '\r' {
                n - 2
            } else {
                n - 1
            };
            self.text.remove(cut..n);
        }
    }

    /// Write the document to its path. Returns the number of bytes written.
    pub fn save(&mut self) -> io::Result<usize> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
        let mut out = BufWriter::new(fs::File::create(&path)?);
        self.text.write_to(&mut out)?;
        let mut bytes = self.text.len_bytes();
        if self.eol && bytes > 0 {
            out.write_all(self.line_ending.as_bytes())?;
            bytes += self.line_ending.len();
        }
        out.flush()?;
        self.history.commit();
        self.history.saved_id = self.history.current_id();
        Ok(bytes)
    }

    /// Bind the document to `path` and save it there.
    pub fn save_as(&mut self, path: impl Into<PathBuf>) -> io::Result<usize> {
        self.path = Some(path.into());
        self.save()
    }

    /// The underlying rope.
    pub fn text(&self) -> &Rope {
        &self.text
    }

    /// File path, if any.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Line ending used for new lines.
    pub fn line_ending(&self) -> &'static str {
        self.line_ending
    }

    /// Whether the document differs from the last saved (or loaded) state.
    pub fn is_modified(&self) -> bool {
        !self.history.pending.is_empty() || self.history.current_id() != self.history.saved_id
    }

    /// Number of lines. Always at least 1.
    pub fn line_count(&self) -> usize {
        self.text.len_lines()
    }

    /// Line `idx` without its line terminator. Returns `None` if out of range.
    pub fn line(&self, idx: usize) -> Option<String> {
        if idx >= self.line_count() {
            return None;
        }
        let line = self.text.line(idx).to_string();
        Some(line.trim_end_matches(['\n', '\r']).to_string())
    }

    /// Length of line `idx` in chars, excluding the terminator. 0 if out of range.
    pub fn line_len(&self, idx: usize) -> usize {
        if idx >= self.line_count() {
            return 0;
        }
        let line = self.text.line(idx);
        let mut len = line.len_chars();
        if len > 0 && line.char(len - 1) == '\n' {
            len -= 1;
            if len > 0 && line.char(len - 1) == '\r' {
                len -= 1;
            }
        }
        len
    }

    /// Char index of `pos`, clamped into the document.
    pub fn char_idx(&self, pos: Position) -> usize {
        let line = pos.line.min(self.line_count() - 1);
        self.text.line_to_char(line) + pos.col.min(self.line_len(line))
    }

    /// Position of char index `idx`, clamped into the document.
    pub fn pos_at(&self, idx: usize) -> Position {
        let idx = idx.min(self.text.len_chars());
        let line = self.text.char_to_line(idx);
        Position::new(line, idx - self.text.line_to_char(line))
    }

    /// Text in `[from, to)` (clamped).
    pub fn slice(&self, from: Position, to: Position) -> String {
        let (a, b) = (self.char_idx(from), self.char_idx(to));
        if a >= b {
            return String::new();
        }
        self.text.slice(a..b).to_string()
    }

    /// Insert `text` at `at`. Returns the position just after the inserted text.
    pub fn insert(&mut self, at: Position, text: &str) -> Position {
        let idx = self.char_idx(at);
        self.text.insert(idx, text);
        self.history.record(Edit {
            at: idx,
            removed: String::new(),
            inserted: text.to_string(),
        });
        self.pos_at(idx + text.chars().count())
    }

    /// Delete the text in `[from, to)`. Does nothing if the range is empty.
    pub fn delete(&mut self, from: Position, to: Position) {
        let (a, b) = (self.char_idx(from), self.char_idx(to));
        if a >= b {
            return;
        }
        let removed = self.text.slice(a..b).to_string();
        self.text.remove(a..b);
        self.history.record(Edit {
            at: a,
            removed,
            inserted: String::new(),
        });
    }

    /// Close the current group of edits into one undo step.
    pub fn commit(&mut self) {
        self.history.commit();
    }

    /// Undo the last undo step. Returns where the change happened.
    pub fn undo(&mut self) -> Option<Position> {
        self.history.commit();
        let group = self.history.undo.pop()?;
        for e in group.edits.iter().rev() {
            self.text.remove(e.at..e.at + e.inserted.chars().count());
            self.text.insert(e.at, &e.removed);
        }
        let at = group.edits[0].at;
        self.history.redo.push(group);
        Some(self.pos_at(at))
    }

    /// Redo the last undone step. Returns where the change happened.
    pub fn redo(&mut self) -> Option<Position> {
        self.history.commit();
        let group = self.history.redo.pop()?;
        for e in &group.edits {
            self.text.remove(e.at..e.at + e.removed.chars().count());
            self.text.insert(e.at, &e.inserted);
        }
        let at = group.edits[0].at;
        self.history.undo.push(group);
        Some(self.pos_at(at))
    }
}

/// One primitive change at char index `at`.
#[derive(Debug, Clone)]
struct Edit {
    at: usize,
    removed: String,
    inserted: String,
}

/// Edits that are undone/redone together.
#[derive(Debug, Clone)]
struct Group {
    id: u64,
    edits: Vec<Edit>,
}

/// Linear undo/redo history.
#[derive(Debug, Clone, Default)]
struct History {
    undo: Vec<Group>,
    redo: Vec<Group>,
    pending: Vec<Edit>,
    /// Last assigned group id. Id 0 means "no changes".
    last_id: u64,
    /// Id of the state on disk.
    saved_id: u64,
}

impl History {
    fn record(&mut self, edit: Edit) {
        self.pending.push(edit);
        self.redo.clear();
    }

    fn commit(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        self.last_id += 1;
        self.undo.push(Group {
            id: self.last_id,
            edits: std::mem::take(&mut self.pending),
        });
    }

    fn current_id(&self) -> u64 {
        self.undo.last().map_or(0, |g| g.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("vox-core-{}-{name}", std::process::id()))
    }

    #[test]
    fn trailing_newline_is_not_a_line() {
        assert_eq!(Document::from_text("").line_count(), 1);
        assert_eq!(Document::from_text("a").line_count(), 1);
        assert_eq!(Document::from_text("a\n").line_count(), 1);
        assert_eq!(Document::from_text("a\nb").line_count(), 2);
        assert_eq!(Document::from_text("a\n\n").line_count(), 2);
    }

    #[test]
    fn line_strips_terminators() {
        let doc = Document::from_text("one\r\ntwo\r\n");
        assert_eq!(doc.line_ending(), "\r\n");
        assert_eq!(doc.line(0).as_deref(), Some("one"));
        assert_eq!(doc.line(1).as_deref(), Some("two"));
        assert_eq!(doc.line(2), None);
        assert_eq!(doc.line_len(0), 3);
        assert_eq!(doc.line_len(1), 3);
    }

    #[test]
    fn open_missing_file_is_empty() {
        let path = temp_path("missing.txt");
        let doc = Document::open(&path).unwrap();
        assert_eq!(doc.path(), Some(path.as_path()));
        assert_eq!(doc.line_count(), 1);
        assert!(!doc.is_modified());
    }

    #[test]
    fn save_round_trip_keeps_eol_and_crlf() {
        for content in ["hello\nworld\n", "no eol", "crlf\r\nfile\r\n", ""] {
            let path = temp_path("roundtrip.txt");
            fs::write(&path, content).unwrap();
            let mut doc = Document::open(&path).unwrap();
            let n = doc.save().unwrap();
            let back = fs::read_to_string(&path).unwrap();
            fs::remove_file(&path).unwrap();
            assert_eq!(back, content);
            assert_eq!(n, content.len());
        }
    }

    #[test]
    fn save_without_path_fails() {
        let mut doc = Document::from_text("x");
        assert_eq!(doc.save().unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn insert_and_delete() {
        let mut doc = Document::from_text("hello\nworld");
        let end = doc.insert(Position::new(0, 5), ",");
        assert_eq!(end, Position::new(0, 6));
        let end = doc.insert(Position::new(1, 0), "big\n");
        assert_eq!(end, Position::new(2, 0));
        assert_eq!(doc.text().to_string(), "hello,\nbig\nworld");
        // Join lines: delete the newline at end of line 0.
        doc.delete(Position::new(0, 6), Position::new(1, 0));
        assert_eq!(doc.text().to_string(), "hello,big\nworld");
    }

    #[test]
    fn undo_redo_groups() {
        let mut doc = Document::from_text("abc");
        doc.insert(Position::new(0, 3), "d");
        doc.insert(Position::new(0, 4), "e");
        doc.commit();
        doc.delete(Position::new(0, 0), Position::new(0, 1));
        doc.commit();
        assert_eq!(doc.text().to_string(), "bcde");

        assert_eq!(doc.undo(), Some(Position::new(0, 0)));
        assert_eq!(doc.text().to_string(), "abcde");
        assert_eq!(doc.undo(), Some(Position::new(0, 3)));
        assert_eq!(doc.text().to_string(), "abc");
        assert_eq!(doc.undo(), None);

        assert_eq!(doc.redo(), Some(Position::new(0, 3)));
        assert_eq!(doc.text().to_string(), "abcde");
        assert_eq!(doc.redo(), Some(Position::new(0, 0)));
        assert_eq!(doc.redo(), None);
        assert_eq!(doc.text().to_string(), "bcde");
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut doc = Document::from_text("a");
        doc.insert(Position::new(0, 1), "b");
        doc.commit();
        doc.undo();
        doc.insert(Position::new(0, 1), "c");
        doc.commit();
        assert_eq!(doc.redo(), None);
        assert_eq!(doc.text().to_string(), "ac");
    }

    #[test]
    fn modified_tracks_saved_state() {
        let path = temp_path("modified.txt");
        fs::write(&path, "x\n").unwrap();
        let mut doc = Document::open(&path).unwrap();
        assert!(!doc.is_modified());
        doc.insert(Position::new(0, 0), "y");
        assert!(doc.is_modified(), "pending edit counts as modified");
        doc.commit();
        doc.undo();
        assert!(!doc.is_modified(), "undo back to loaded state");
        doc.redo();
        assert!(doc.is_modified());
        doc.save().unwrap();
        assert!(!doc.is_modified());
        doc.undo();
        assert!(doc.is_modified(), "undo past saved state");
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn positions_clamp() {
        let doc = Document::from_text("ab\ncd");
        assert_eq!(doc.char_idx(Position::new(0, 99)), 2);
        assert_eq!(doc.char_idx(Position::new(99, 0)), 3);
        assert_eq!(doc.pos_at(4), Position::new(1, 1));
        assert_eq!(doc.pos_at(99), Position::new(1, 2));
    }
}
