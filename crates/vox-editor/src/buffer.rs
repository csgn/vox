//! Buffers: several open files. The active one lives in the editor's own fields
//! (`doc`, `cursor`, `view`); the others are parked here with their cursor and scroll
//! position and swapped in when switched to.

use std::path::{Path, PathBuf};

use vox_core::{Document, Position};
use vox_view::Viewport;

use crate::command::{Args, CommandResult, Commands, Complete, Param};
use crate::config::Value;
use crate::editor::{Editor, Mode};
use crate::event::Event;
use crate::ui::{ListItem, ListView};

/// A file that is open but not shown.
#[derive(Debug)]
pub(crate) struct Buffer {
    /// Stable number, like Vim's buffer number.
    pub id: usize,
    pub doc: Document,
    pub cursor: Position,
    pub view: Viewport,
}

/// One open buffer, for lists and data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferInfo {
    /// Buffer number.
    pub id: usize,
    /// File path as given, or `[No Name]`.
    pub name: String,
    /// Has unsaved changes.
    pub modified: bool,
    /// Is the one shown.
    pub active: bool,
    /// Number of lines.
    pub lines: usize,
}

/// Name shown for a document.
fn name_of(doc: &Document) -> String {
    doc.path()
        .map_or("[No Name]".into(), |p| p.display().to_string())
}

/// `p` made absolute (without touching the file system), for comparing paths.
fn absolute(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| {
        std::env::current_dir()
            .map(|d| d.join(p))
            .unwrap_or_else(|_| p.to_path_buf())
    })
}

impl Editor {
    /// All open buffers in number order; the active one has `active` set.
    pub fn buffers(&self) -> Vec<BufferInfo> {
        let mut out: Vec<BufferInfo> = self
            .parked
            .iter()
            .map(|b| BufferInfo {
                id: b.id,
                name: name_of(&b.doc),
                modified: b.doc.is_modified(),
                active: false,
                lines: b.doc.line_count(),
            })
            .collect();
        out.push(BufferInfo {
            id: self.buffer_id,
            name: name_of(&self.doc),
            modified: self.doc.is_modified(),
            active: true,
            lines: self.doc.line_count(),
        });
        out.sort_by_key(|b| b.id);
        out
    }

    /// Number of the active buffer.
    pub fn buffer_id(&self) -> usize {
        self.buffer_id
    }

    /// The first buffer other than the active one with unsaved changes.
    pub fn other_unsaved_buffer(&self) -> Option<BufferInfo> {
        self.buffers().into_iter().find(|b| b.modified && !b.active)
    }

    /// Show buffer `id`. The current one keeps its cursor and scroll position.
    pub fn switch_buffer(&mut self, id: usize) -> Result<(), String> {
        if id == self.buffer_id {
            return Ok(());
        }
        let i = self
            .parked
            .iter()
            .position(|b| b.id == id)
            .ok_or_else(|| format!("E86: Buffer {id} does not exist"))?;
        let next = self.parked.remove(i);
        let current = Buffer {
            id: self.buffer_id,
            doc: std::mem::replace(&mut self.doc, next.doc),
            cursor: self.cursor,
            view: self.view,
        };
        self.parked.push(current);
        self.buffer_id = next.id;
        self.unit_focus = None;
        self.view = next.view;
        if matches!(self.mode, Mode::Visual { .. } | Mode::Insert) {
            self.mode = Mode::Normal;
        }
        self.set_cursor(next.cursor);
        self.message = Some(format!(
            "\"{}\" {}L",
            name_of(&self.doc),
            self.doc.line_count()
        ));
        Ok(())
    }

    /// Open `path` in a new buffer and show it, or show it if it is open already. An
    /// empty, unnamed, unchanged buffer is replaced instead of kept.
    pub fn open_buffer(&mut self, path: &Path) -> Result<usize, String> {
        let want = absolute(path);
        let open = self
            .buffers()
            .into_iter()
            .find(|b| b.name != "[No Name]" && absolute(Path::new(&b.name)) == want);
        if let Some(b) = open {
            self.switch_buffer(b.id)?;
            return Ok(b.id);
        }
        let doc = Document::open(path, self.storage)
            .map_err(|e| format!("E484: Can't open file {}: {e}", path.display()))?;
        let scratch = self.doc.path().is_none()
            && !self.doc.is_modified()
            && self.doc.line_count() == 1
            && self.doc.line(0).unwrap_or_default().is_empty();
        // The replaced empty buffer gives its number to the file.
        let id = if scratch {
            self.buffer_id
        } else {
            self.next_buffer_id += 1;
            self.next_buffer_id - 1
        };
        let old = Buffer {
            id: self.buffer_id,
            doc: std::mem::replace(&mut self.doc, doc),
            cursor: self.cursor,
            view: self.view,
        };
        if !scratch {
            self.parked.push(old);
        }
        self.buffer_id = id;
        self.unit_focus = None;
        self.view = Viewport::default();
        self.mode = Mode::Normal;
        self.set_cursor(Position::default());
        self.message = Some(format!("\"{}\" {}L", path.display(), self.doc.line_count()));
        self.emit(&Event::BufOpen {
            path: Some(path.display().to_string()),
        });
        Ok(id)
    }

    /// Open `path` as a buffer without showing it (e.g. more files on the command line).
    pub fn add_buffer(&mut self, path: &Path) -> Result<usize, String> {
        let current = self.buffer_id;
        let id = self.open_buffer(path)?;
        if self.parked.iter().any(|b| b.id == current) {
            self.switch_buffer(current)?;
        }
        Ok(id)
    }

    /// The buffer after (`step` 1) or before (`step` -1) the active one, wrapping.
    fn buffer_step(&self, step: isize) -> usize {
        let ids: Vec<usize> = self.buffers().iter().map(|b| b.id).collect();
        let i = ids.iter().position(|&id| id == self.buffer_id).unwrap_or(0) as isize;
        ids[(i + step).rem_euclid(ids.len() as isize) as usize]
    }

    /// Close buffer `id`. Refuses unsaved changes unless `force`. Closing the last buffer
    /// leaves an empty one.
    pub fn close_buffer(&mut self, id: usize, force: bool) -> Result<(), String> {
        let info = self
            .buffers()
            .into_iter()
            .find(|b| b.id == id)
            .ok_or_else(|| format!("E516: No buffers were deleted: {id} does not exist"))?;
        if info.modified && !force {
            return Err(format!(
                "E89: No write since last change for buffer {id} (add ! to override)"
            ));
        }
        if info.active {
            if self.parked.is_empty() {
                // The last buffer: start over with an empty one.
                self.doc = Document::new("", self.storage);
                self.buffer_id = self.next_buffer_id;
                self.next_buffer_id += 1;
                self.view = Viewport::default();
                self.set_cursor(Position::default());
                return Ok(());
            }
            let next = self.buffer_step(1);
            self.switch_buffer(next)?;
        }
        self.parked.retain(|b| b.id != id);
        Ok(())
    }

    /// Find a buffer by number or by (part of its) name.
    fn find_buffer(&self, what: &str) -> Result<usize, String> {
        let list = self.buffers();
        if let Ok(n) = what.parse::<usize>() {
            return list
                .iter()
                .find(|b| b.id == n)
                .map(|b| b.id)
                .ok_or_else(|| format!("E86: Buffer {n} does not exist"));
        }
        if let Some(b) = list.iter().find(|b| b.name == what) {
            return Ok(b.id);
        }
        let matches: Vec<&BufferInfo> = list.iter().filter(|b| b.name.contains(what)).collect();
        match matches.as_slice() {
            [one] => Ok(one.id),
            [] => Err(format!("E94: No matching buffer for {what}")),
            _ => Err(format!("E93: More than one match for {what}")),
        }
    }
}

/// Register the buffer commands.
pub(crate) fn register(c: &mut Commands) {
    use crate::command::category::FILE;
    let which = || {
        Param::optional("BUFFER", "Buffer number, or (part of) its file name.")
            .complete(Complete::Buffer)
    };
    c.register(
        "buffer",
        "Switch to a buffer: `:buffer N|NAME`; no argument lists the open buffers.",
        buffer,
    )
    .category(FILE)
    .param(which())
    .example("b 2", "show buffer 2")
    .example("b main", "the buffer whose name contains `main`");
    c.alias("b", "buffer");
    c.alias("ls", "buffer");
    c.alias("buffers", "buffer");
    c.register("buffer next", "Show the next buffer.", |ed, _| {
        let id = ed.buffer_step(1);
        ed.switch_buffer(id)
    })
    .category(FILE);
    c.alias("bn", "buffer next");
    c.alias("bnext", "buffer next");
    c.register("buffer prev", "Show the previous buffer.", |ed, _| {
        let id = ed.buffer_step(-1);
        ed.switch_buffer(id)
    })
    .category(FILE);
    c.alias("bp", "buffer prev");
    c.alias("bprevious", "buffer prev");
    c.register(
        "buffer delete",
        "Close a buffer (default: the current one): `:buffer delete [N|NAME]`.",
        delete,
    )
    .category(FILE)
    .param(which())
    .force("Close it even if it has unsaved changes (they are lost).");
    c.alias("bd", "buffer delete");
    c.alias("bdelete", "buffer delete");
    c.register(
        "buffer list",
        "Return the open buffers as data: id, name, modified, active, lines.",
        list,
    )
    .category(FILE)
    .returns("A list, one table per buffer: id, name, modified, active, lines.");
    c.register("wall", "Save every buffer with unsaved changes.", wall)
        .category(FILE);
    c.alias("wa", "wall");
}

/// `:buffer [N|NAME]`.
fn buffer(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let Some(what) = a.arg else {
        let items = ed
            .buffers()
            .into_iter()
            .map(|b| {
                let mark = if b.active { "%" } else { " " };
                let modified = if b.modified { " [+]" } else { "" };
                ListItem::new(format!("{:>3} {mark} {}{modified}", b.id, b.name))
                    .detail(format!("{} lines", b.lines))
                    .command(format!("buffer {}", b.id))
            })
            .collect();
        ed.open_modal(Box::new(ListView::new("Buffers", items)));
        return Ok(());
    };
    let id = ed.find_buffer(what)?;
    ed.switch_buffer(id)
}

/// `:buffer delete [N|NAME]`.
fn delete(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let id = match a.arg {
        Some(what) => ed.find_buffer(what)?,
        None => ed.buffer_id(),
    };
    ed.close_buffer(id, a.force)
}

/// `:buffer list`.
fn list(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    let rows = ed
        .buffers()
        .into_iter()
        .map(|b| {
            Value::table([
                ("id", Value::Integer(b.id as i64)),
                ("name", b.name.into()),
                ("modified", b.modified.into()),
                ("active", b.active.into()),
                ("lines", Value::Integer(b.lines as i64)),
            ])
        })
        .collect();
    ed.reply(Value::Array(rows));
    Ok(())
}

/// `:wall`: save every modified buffer; the active one last, so it keeps the message.
fn wall(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    let current = ed.buffer_id();
    let modified: Vec<BufferInfo> = ed.buffers().into_iter().filter(|b| b.modified).collect();
    if let Some(b) = modified.iter().find(|b| b.name == "[No Name]") {
        return Err(format!("E141: No file name for buffer {}", b.id));
    }
    for b in modified.iter().filter(|b| !b.active) {
        ed.switch_buffer(b.id)?;
        let saved = ed.write(None);
        ed.switch_buffer(current)?;
        saved?;
    }
    if modified.iter().any(|b| b.active) {
        ed.write(None)?;
    }
    ed.set_message(format!("{} buffer(s) written", modified.len()));
    Ok(())
}
