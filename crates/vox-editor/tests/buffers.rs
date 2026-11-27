//! Buffers: several open files, switching, closing, quitting with unsaved changes.

use std::path::{Path, PathBuf};

use vox_core::Document;
use vox_editor::{Editor, Value, parse_keys};

fn feed(ed: &mut Editor, keys: &str) {
    for k in parse_keys(keys) {
        ed.handle_key(k);
    }
}

/// A fresh folder with files `a.txt` ("alpha\nbeta") and `b.txt` ("bravo").
fn files(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vox-buffers-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "alpha\nbeta\n").unwrap();
    std::fs::write(dir.join("b.txt"), "bravo\n").unwrap();
    dir
}

fn open(ed: &mut Editor, p: &Path) {
    feed(ed, &format!(":e {}<CR>", p.display()));
}

fn names(ed: &Editor) -> Vec<String> {
    ed.buffers()
        .iter()
        .map(|b| {
            Path::new(&b.name)
                .file_name()
                .map_or(b.name.clone(), |n| n.to_string_lossy().into_owned())
        })
        .collect()
}

fn editor() -> Editor {
    let mut ed = Editor::new(Document::from_text(""));
    ed.resize(60, 10);
    ed
}

#[test]
fn open_switch_and_keep_cursor() {
    let dir = files("switch");
    let mut ed = editor();
    open(&mut ed, &dir.join("a.txt"));
    assert_eq!(names(&ed), ["a.txt"], "the empty start buffer is replaced");
    feed(&mut ed, "j");
    open(&mut ed, &dir.join("b.txt"));
    assert_eq!(names(&ed), ["a.txt", "b.txt"]);
    assert_eq!(ed.document().to_string(), "bravo");
    // Opening an open file switches to it, with its cursor where it was.
    open(&mut ed, &dir.join("a.txt"));
    assert_eq!(ed.buffers().len(), 2);
    assert_eq!(ed.cursor().line, 1);
    feed(&mut ed, ":bn<CR>");
    assert_eq!(ed.document().to_string(), "bravo");
    feed(&mut ed, ":bn<CR>");
    assert_eq!(ed.document().to_string(), "alpha\nbeta", "wraps around");
    feed(&mut ed, ":bp<CR>:b 1<CR>");
    assert_eq!(ed.buffer_id(), 1);
    feed(&mut ed, ":b b.t<CR>");
    assert_eq!(ed.document().to_string(), "bravo", "by part of the name");
    feed(&mut ed, ":b txt<CR>");
    assert_eq!(ed.message(), Some("E93: More than one match for txt"));
    feed(&mut ed, ":b zz<CR>");
    assert_eq!(ed.message(), Some("E94: No matching buffer for zz"));
    feed(&mut ed, ":b 9<CR>");
    assert_eq!(ed.message(), Some("E86: Buffer 9 does not exist"));
}

#[test]
fn undo_stays_with_its_buffer() {
    let dir = files("undo");
    let mut ed = editor();
    open(&mut ed, &dir.join("a.txt"));
    feed(&mut ed, "xx");
    open(&mut ed, &dir.join("b.txt"));
    feed(&mut ed, "u");
    assert_eq!(ed.document().to_string(), "bravo", "nothing to undo here");
    feed(&mut ed, ":bp<CR>u");
    assert_eq!(ed.document().to_string(), "lpha\nbeta", "one x undone");
}

#[test]
fn quit_checks_every_buffer() {
    let dir = files("quit");
    let mut ed = editor();
    open(&mut ed, &dir.join("a.txt"));
    feed(&mut ed, "x");
    open(&mut ed, &dir.join("b.txt"));
    feed(&mut ed, ":q<CR>");
    assert!(!ed.should_quit());
    let msg = ed.message().unwrap().to_string();
    assert!(msg.starts_with("E162") && msg.contains("a.txt"), "{msg}");
    feed(&mut ed, ":wq<CR>");
    assert!(
        !ed.should_quit(),
        "wq saves this one but a.txt is still unsaved"
    );
    feed(&mut ed, "x:wa<CR>");
    assert_eq!(ed.message(), Some("2 buffer(s) written"));
    assert_eq!(
        std::fs::read_to_string(dir.join("a.txt")).unwrap(),
        "lpha\nbeta\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("b.txt")).unwrap(),
        "ravo\n"
    );
    assert_eq!(ed.document().to_string(), "ravo", "still on b.txt");
    feed(&mut ed, ":q<CR>");
    assert!(ed.should_quit());
}

#[test]
fn force_quit_discards() {
    let dir = files("force");
    let mut ed = editor();
    open(&mut ed, &dir.join("a.txt"));
    feed(&mut ed, "x");
    open(&mut ed, &dir.join("b.txt"));
    feed(&mut ed, ":q!<CR>");
    assert!(ed.should_quit());
}

#[test]
fn delete_buffers() {
    let dir = files("delete");
    let mut ed = editor();
    open(&mut ed, &dir.join("a.txt"));
    open(&mut ed, &dir.join("b.txt"));
    feed(&mut ed, "x:bd<CR>");
    assert_eq!(
        ed.message(),
        Some("E89: No write since last change for buffer 2 (add ! to override)")
    );
    feed(&mut ed, ":bd!<CR>");
    assert_eq!(names(&ed), ["a.txt"]);
    assert_eq!(ed.document().to_string(), "alpha\nbeta");
    // Closing the last buffer leaves an empty one.
    feed(&mut ed, ":buffer delete<CR>");
    assert_eq!(names(&ed), ["[No Name]"]);
    assert_eq!(ed.document().to_string(), "");
}

#[test]
fn add_buffer_keeps_the_shown_one() {
    let dir = files("add");
    let mut ed = Editor::new(
        Document::open(dir.join("a.txt"), vox_core::storage::SimpleStorage::boxed).unwrap(),
    );
    ed.add_buffer(&dir.join("b.txt")).unwrap();
    assert_eq!(names(&ed), ["a.txt", "b.txt"]);
    assert_eq!(ed.document().to_string(), "alpha\nbeta");
}

#[test]
fn list_as_data_and_view_and_completion() {
    let dir = files("list");
    let mut ed = editor();
    open(&mut ed, &dir.join("a.txt"));
    open(&mut ed, &dir.join("b.txt"));
    let Some(Value::Array(rows)) = ed.execute("buffer list").unwrap() else {
        panic!("list")
    };
    assert_eq!(rows.len(), 2);
    let Value::Table(b) = &rows[1] else { panic!() };
    assert_eq!(b["active"], Value::Bool(true));
    assert_eq!(b["id"], Value::Integer(2));
    // :ls opens the list; Enter on the first row switches to it.
    feed(&mut ed, ":ls<CR>");
    assert_eq!(ed.modal_title().as_deref(), Some("Buffers"));
    feed(&mut ed, "<CR>");
    assert_eq!(ed.buffer_id(), 1);
    // Completion offers the subcommands and the open buffers.
    let c = ed.complete("b ");
    let texts: Vec<&str> = c.items.iter().map(|i| i.text.as_str()).collect();
    assert!(texts.contains(&"next"));
    assert_eq!(texts.iter().filter(|t| t.ends_with(".txt")).count(), 2);
}

#[test]
fn status_line_shows_which_buffer() {
    let dir = files("status");
    let mut ed = editor();
    open(&mut ed, &dir.join("a.txt"));
    assert!(!ed.render().to_string().contains("[1/"));
    open(&mut ed, &dir.join("b.txt"));
    let screen = ed.render().to_string();
    assert!(screen.contains("b.txt [2/2]"), "{screen}");
}
