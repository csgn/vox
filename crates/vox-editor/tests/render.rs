//! Render snapshot tests.

use vox_core::Document;
use vox_editor::{Editor, Style, Surface, parse_keys};

/// Render `app` at `width` x `height`; return the screen and cursor (x, y).
fn draw(app: &mut Editor, width: u16, height: u16) -> (Surface, (u16, u16)) {
    app.resize(width, height);
    let surface = app.render();
    let cursor = surface.cursor;
    (surface, cursor)
}

fn feed(app: &mut Editor, keys: &str) {
    for key in parse_keys(keys) {
        app.handle_key(key);
    }
}

#[test]
fn first_render() {
    let mut app = Editor::new(Document::from_text("fn main() {\n\tprintln!(\"hi\");\n}\n"));
    let (screen, cursor) = draw(&mut app, 40, 8);
    insta::assert_snapshot!(screen);
    assert_eq!(cursor, (4, 0));
}

#[test]
fn scrolled_render() {
    let text: String = (1..=20).map(|i| format!("line {i}\n")).collect();
    let mut app = Editor::new(Document::from_text(&text));
    draw(&mut app, 30, 6);
    feed(&mut app, "jjjjjjll");
    let (screen, cursor) = draw(&mut app, 30, 6);
    insta::assert_snapshot!(screen);
    assert_eq!(cursor, (6, 3));
}

#[test]
fn insert_mode_modified() {
    let mut app = Editor::new(Document::from_text("x\n"));
    feed(&mut app, "i<Tab>");
    let (screen, cursor) = draw(&mut app, 30, 4);
    insta::assert_snapshot!(screen);
    assert_eq!(cursor, (8, 0));
}

#[test]
fn horizontal_scroll_render() {
    let line: String = ('a'..='z').cycle().take(60).collect();
    let mut app = Editor::new(Document::from_text(&line));
    draw(&mut app, 20, 3);
    feed(&mut app, "$");
    let (screen, cursor) = draw(&mut app, 20, 3);
    insta::assert_snapshot!(screen);
    assert_eq!(cursor, (19, 0));
}

#[test]
fn command_line_render() {
    let mut app = Editor::new(Document::from_text("x\n"));
    feed(&mut app, ":q");
    let (screen, cursor) = draw(&mut app, 30, 4);
    insta::assert_snapshot!(screen);
    assert_eq!(cursor, (2, 3));
}

#[test]
fn error_message_render() {
    let mut app = Editor::new(Document::from_text("x\n"));
    feed(&mut app, ":nope<CR>");
    let (screen, _) = draw(&mut app, 40, 4);
    insta::assert_snapshot!(screen);
}

#[test]
fn visual_selection_render() {
    let mut app = Editor::new(Document::from_text("foo bar\nbaz qux\n\nend"));
    feed(&mut app, "wvjj");
    let (screen, _) = draw(&mut app, 30, 6);
    insta::assert_snapshot!(screen);
    let reversed = |x: u16, y: u16| screen.cell(x, y).style == Style::REVERSED;
    // Line 1: from "bar" to end of line (incl. line break cell).
    assert!(!reversed(7, 0) && reversed(8, 0) && reversed(11, 0));
    // Line 2: whole line selected.
    assert!(reversed(4, 1) && reversed(11, 1));
    // Empty line 3: one cell.
    assert!(reversed(4, 2) && !reversed(5, 2));
    // Line 4 untouched.
    assert!(!reversed(4, 3));
}

#[test]
fn visual_line_render() {
    let mut app = Editor::new(Document::from_text("ab\ncd"));
    feed(&mut app, "V");
    let (screen, _) = draw(&mut app, 20, 4);
    insta::assert_snapshot!(screen);
}

#[test]
fn search_prompt_render() {
    let mut app = Editor::new(Document::from_text("abc"));
    feed(&mut app, "/b");
    let (screen, cursor) = draw(&mut app, 20, 4);
    insta::assert_snapshot!(screen);
    assert_eq!(cursor, (2, 3));
}

#[test]
fn command_reply_render() {
    let mut app = Editor::new(Document::from_text("x\n"));
    app.set_scope(vox_editor::Scope::global("/cfg".into()), None);
    feed(&mut app, ":scope show<CR>");
    let (screen, _) = draw(&mut app, 60, 14);
    insta::assert_snapshot!(screen);
}
