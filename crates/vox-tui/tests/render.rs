//! Render snapshot tests.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use vox_core::Document;
use vox_tui::{App, parse_keys};

/// Draw `app` into a `width` x `height` test terminal; return the screen and cursor (x, y).
fn draw(app: &mut App, width: u16, height: u16) -> (TestBackend, (u16, u16)) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut cursor = (0, 0);
    terminal
        .draw(|frame| {
            let area = frame.area();
            app.resize(area.width, area.height);
            frame.render_widget(&*app, area);
            cursor = app.cursor_screen_position(area);
        })
        .unwrap();
    (terminal.backend().clone(), cursor)
}

fn feed(app: &mut App, keys: &str) {
    for key in parse_keys(keys) {
        app.handle_key(key);
    }
}

#[test]
fn first_render() {
    let mut app = App::new(Document::from_text("fn main() {\n\tprintln!(\"hi\");\n}\n"));
    let (screen, cursor) = draw(&mut app, 40, 8);
    insta::assert_snapshot!(screen);
    assert_eq!(cursor, (4, 0));
}

#[test]
fn scrolled_render() {
    let text: String = (1..=20).map(|i| format!("line {i}\n")).collect();
    let mut app = App::new(Document::from_text(&text));
    draw(&mut app, 30, 6);
    feed(&mut app, "jjjjjjll");
    let (screen, cursor) = draw(&mut app, 30, 6);
    insta::assert_snapshot!(screen);
    assert_eq!(cursor, (6, 3));
}

#[test]
fn insert_mode_modified() {
    let mut app = App::new(Document::from_text("x\n"));
    feed(&mut app, "i<Tab>");
    let (screen, cursor) = draw(&mut app, 30, 4);
    insta::assert_snapshot!(screen);
    assert_eq!(cursor, (8, 0));
}

#[test]
fn horizontal_scroll_render() {
    let line: String = ('a'..='z').cycle().take(60).collect();
    let mut app = App::new(Document::from_text(&line));
    draw(&mut app, 20, 3);
    feed(&mut app, "$");
    let (screen, cursor) = draw(&mut app, 20, 3);
    insta::assert_snapshot!(screen);
    assert_eq!(cursor, (19, 0));
}

#[test]
fn command_line_render() {
    let mut app = App::new(Document::from_text("x\n"));
    feed(&mut app, ":q");
    let (screen, cursor) = draw(&mut app, 30, 4);
    insta::assert_snapshot!(screen);
    assert_eq!(cursor, (2, 3));
}

#[test]
fn error_message_render() {
    let mut app = App::new(Document::from_text("x\n"));
    feed(&mut app, ":nope<CR>");
    let (screen, _) = draw(&mut app, 40, 4);
    insta::assert_snapshot!(screen);
}
