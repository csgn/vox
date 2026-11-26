//! Render snapshot tests.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use vox_core::Document;
use vox_tui::App;

fn draw(app: &mut App, width: u16, height: u16) -> TestBackend {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            app.resize(frame.area().height);
            frame.render_widget(&*app, frame.area());
        })
        .unwrap();
    terminal.backend().clone()
}

fn press(app: &mut App, c: char) {
    app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
}

#[test]
fn first_render() {
    let mut app = App::new(Document::from_text("fn main() {\n\tprintln!(\"hi\");\n}\n"));
    insta::assert_snapshot!(draw(&mut app, 40, 8));
}

#[test]
fn scrolled_render() {
    let text: String = (1..=20).map(|i| format!("line {i}\n")).collect();
    let mut app = App::new(Document::from_text(&text));
    for _ in 0..5 {
        press(&mut app, 'j');
    }
    insta::assert_snapshot!(draw(&mut app, 30, 6));
}

#[test]
fn command_line_render() {
    let mut app = App::new(Document::from_text("x\n"));
    press(&mut app, ':');
    press(&mut app, 'q');
    insta::assert_snapshot!(draw(&mut app, 30, 4));
}
