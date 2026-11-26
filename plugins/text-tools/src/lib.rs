//! Example vox plugin: word count and uppercase.
//!
//! - `:wordcount` shows words, lines, and characters of the buffer (also `<leader>c`).
//! - `:upper` uppercases the cursor line.
//! - After every save, shows the word count.

use vox_plugin_sdk::{Event, EventKind, MapMode, Plugin, Position, editor, export_plugin};

struct TextTools;

/// (words, lines, chars) of the whole buffer.
fn count() -> (usize, u32, usize) {
    let lines = editor::line_count();
    let (mut words, mut chars) = (0, 0);
    for i in 0..lines {
        let line = editor::line(i).unwrap_or_default();
        words += line.split_whitespace().count();
        chars += line.chars().count();
    }
    (words, lines, chars)
}

impl Plugin for TextTools {
    fn init() -> Result<(), String> {
        editor::register_command("wordcount", "Count words, lines, and characters");
        editor::register_command("upper", "Uppercase the cursor line");
        editor::map_key(MapMode::Normal, "<leader>c", ":wordcount");
        editor::subscribe(EventKind::BufWrite);
        Ok(())
    }

    fn run_command(name: String, _arg: Option<String>, _force: bool) -> Result<(), String> {
        match name.as_str() {
            "wordcount" => {
                let (words, lines, chars) = count();
                editor::message(&format!("{words} words, {lines} lines, {chars} characters"));
                Ok(())
            }
            "upper" => {
                let cursor = editor::cursor();
                let line = editor::line(cursor.line).unwrap_or_default();
                let start = Position { line: cursor.line, col: 0 };
                let end = Position { line: cursor.line, col: line.chars().count() as u32 };
                editor::delete_text(start, end);
                editor::insert_text(start, &line.to_uppercase());
                editor::set_cursor(cursor);
                Ok(())
            }
            other => Err(format!("text-tools: unknown command {other}")),
        }
    }

    fn on_event(event: Event) {
        if let Event::BufWrite(path) = event {
            let (words, _, _) = count();
            editor::message(&format!("saved {path}: {words} words"));
        }
    }
}

export_plugin!(TextTools);
