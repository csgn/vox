//! Key-sequence tests: `(initial text, keys) -> (text, cursor, mode)`.

use std::fs;
use std::path::PathBuf;

use vox_core::storage::SimpleStorage;
use vox_core::{Document, Position};
use vox_editor::{Editor, Mode, parse_keys};

fn app(text: &str) -> Editor {
    let mut app = Editor::new(Document::from_text(text));
    app.resize(40, 12);
    app
}

fn feed(app: &mut Editor, keys: &str) {
    for key in parse_keys(keys) {
        app.handle_key(key);
    }
}

/// Run `keys` on `text`; return resulting text and cursor (line, col).
fn run(text: &str, keys: &str) -> (String, (usize, usize)) {
    let mut a = app(text);
    feed(&mut a, keys);
    let c = a.cursor();
    (a.document().to_string(), (c.line, c.col))
}

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("vox-tui-{}-{name}", std::process::id()))
}

#[test]
fn motions() {
    let text = "foo bar\n  baz qux\n\nend";
    let cases: &[(&str, (usize, usize))] = &[
        ("", (0, 0)),
        ("l", (0, 1)),
        ("lllllllll", (0, 6)),
        ("h", (0, 0)),
        ("j", (1, 0)),
        ("$", (0, 6)),
        ("$j", (1, 8)),
        ("$jj", (2, 0)),
        ("$jjj", (3, 2)),
        ("w", (0, 4)),
        ("ww", (1, 2)),
        ("www", (1, 6)),
        ("wwww", (2, 0)),
        ("G", (3, 0)),
        ("Gb", (2, 0)),
        ("Ggg", (0, 0)),
        ("jgg", (0, 0)),
        ("jjjg", (3, 0)),
        ("ll0", (0, 0)),
        ("<Down><Right><Right>", (1, 2)),
    ];
    for (keys, want) in cases {
        assert_eq!(run(text, keys).1, *want, "keys: {keys:?}");
    }
}

#[test]
fn vertical_keeps_wanted_column() {
    let text = "abcdef\nab\nabcdef";
    assert_eq!(run(text, "lllll").1, (0, 5));
    assert_eq!(run(text, "lllllj").1, (1, 1));
    assert_eq!(run(text, "llllljj").1, (2, 5));
}

#[test]
fn insert_entry_points() {
    let cases: &[(&str, &str, &str, (usize, usize))] = &[
        ("abc", "iX<Esc>", "Xabc", (0, 0)),
        ("abc", "lliX<Esc>", "abXc", (0, 2)),
        ("abc", "aX<Esc>", "aXbc", (0, 1)),
        ("abc", "$aX<Esc>", "abcX", (0, 3)),
        ("", "aX<Esc>", "X", (0, 0)),
        ("a\nb", "oX<Esc>", "a\nX\nb", (1, 0)),
        ("a\nb", "jOX<Esc>", "a\nX\nb", (1, 0)),
        ("a", "o<Esc>", "a\n", (1, 0)),
    ];
    for (text, keys, want_text, want_cursor) in cases {
        let (t, c) = run(text, keys);
        assert_eq!(
            (t.as_str(), c),
            (*want_text, *want_cursor),
            "{text:?} + {keys:?}"
        );
    }
}

#[test]
fn insert_mode_editing() {
    assert_eq!(
        run("", "ihello<CR>world<Esc>"),
        ("hello\nworld".into(), (1, 4))
    );
    assert_eq!(run("ab", "a<Tab><Esc>"), ("a\tb".into(), (0, 1)));
    assert_eq!(run("ab", "i<Right>X<Left>Y<Esc>"), ("aYXb".into(), (0, 1)));
}

#[test]
fn backspace_joins_lines() {
    assert_eq!(run("ab\ncd", "ji<BS><Esc>"), ("abcd".into(), (0, 1)));
    assert_eq!(run("ab", "i<BS><Esc>"), ("ab".into(), (0, 0)));
}

#[test]
fn x_deletes_char() {
    assert_eq!(run("abc", "x"), ("bc".into(), (0, 0)));
    assert_eq!(run("abc", "$x"), ("ab".into(), (0, 1)));
    assert_eq!(run("", "x"), ("".into(), (0, 0)));
}

#[test]
fn undo_redo() {
    assert_eq!(run("abc", "xxu"), ("bc".into(), (0, 0)));
    assert_eq!(run("abc", "xxuu"), ("abc".into(), (0, 0)));
    assert_eq!(run("abc", "xxuu<C-r>"), ("bc".into(), (0, 0)));
    // One insert session is one undo step.
    assert_eq!(run("", "ione<CR>two<Esc>u").0, "");
    assert_eq!(run("a", "oline<Esc>u"), ("a".into(), (0, 0)));
}

#[test]
fn undo_messages() {
    let mut a = app("a");
    feed(&mut a, "u");
    assert_eq!(a.message(), Some("Already at oldest change"));
    feed(&mut a, "<C-r>");
    assert_eq!(a.message(), Some("Already at newest change"));
}

#[test]
fn quit_refuses_when_modified() {
    let mut a = app("a");
    feed(&mut a, "x:q<CR>");
    assert!(!a.should_quit());
    assert_eq!(
        a.message(),
        Some("E37: No write since last change (add ! to override)")
    );
    feed(&mut a, ":q!<CR>");
    assert!(a.should_quit());
}

#[test]
fn write_without_name() {
    let mut a = app("a");
    feed(&mut a, ":w<CR>");
    assert_eq!(a.message(), Some("E32: No file name"));
}

#[test]
fn write_and_wq() {
    let path = temp_path("write.txt");
    fs::write(&path, "hello\n").unwrap();
    let mut a = Editor::new(Document::open(&path, SimpleStorage::boxed).unwrap());
    feed(&mut a, "x:w<CR>");
    assert_eq!(fs::read_to_string(&path).unwrap(), "ello\n");
    assert!(!a.document().is_modified());
    assert_eq!(
        a.message(),
        Some(format!("\"{}\" 1L, 5B written", path.display()).as_str())
    );
    feed(&mut a, "x:wq<CR>");
    assert!(a.should_quit());
    assert_eq!(fs::read_to_string(&path).unwrap(), "llo\n");
    fs::remove_file(&path).unwrap();
}

#[test]
fn write_as_names_buffer() {
    let path = temp_path("write-as.txt");
    let mut a = app("");
    feed(&mut a, &format!("inew<Esc>:w {}<CR>", path.display()));
    assert_eq!(fs::read_to_string(&path).unwrap(), "new\n");
    assert_eq!(a.document().path(), Some(path.as_path()));
    fs::remove_file(&path).unwrap();
}

#[test]
fn edit_other_file() {
    let path = temp_path("edit.txt");
    fs::write(&path, "one\ntwo\n").unwrap();
    let mut a = app("a");
    feed(&mut a, "x");
    feed(&mut a, &format!(":e {}<CR>", path.display()));
    assert_eq!(
        a.message(),
        Some("E37: No write since last change (add ! to override)")
    );
    feed(&mut a, &format!(":e! {}<CR>", path.display()));
    assert_eq!(a.document().line(1).as_deref(), Some("two"));
    assert_eq!(a.cursor(), Position::new(0, 0));
    fs::remove_file(&path).unwrap();
}

#[test]
fn unknown_command() {
    let mut a = app("a");
    feed(&mut a, ":foo<CR>");
    assert_eq!(a.mode(), &Mode::Normal);
    assert_eq!(a.message(), Some("E492: Not an editor command: foo"));
}

#[test]
fn command_line_cancel() {
    let mut a = app("a");
    feed(&mut a, ":x<Esc>");
    assert_eq!(a.mode(), &Mode::Normal);
    feed(&mut a, ":<BS>");
    assert_eq!(a.mode(), &Mode::Normal);
}

#[test]
fn viewport_follows_cursor() {
    let text: String = (1..=50).map(|i| format!("line {i}\n")).collect();
    let mut a = app(&text); // text height 10
    feed(&mut a, "G");
    assert_eq!(a.viewport().top, 40);
    feed(&mut a, "gg");
    assert_eq!(a.viewport().top, 0);
    feed(&mut a, "<C-d>");
    assert_eq!((a.viewport().top, a.cursor().line), (5, 5));
    feed(&mut a, "<C-u>");
    assert_eq!((a.viewport().top, a.cursor().line), (0, 0));
}

#[test]
fn horizontal_scroll() {
    let mut a = app(&"x".repeat(100)); // text width 40 - 4 = 36
    feed(&mut a, "$");
    assert_eq!(a.viewport().left, 64);
    feed(&mut a, "0");
    assert_eq!(a.viewport().left, 0);
}
