//! Example vox plugin: word count and uppercase.
//!
//! - `:wordcount` shows words, lines, and characters of the buffer (also `<leader>c`).
//! - `:upper` changes the case of the cursor line (setting `case`: upper, lower, title).
//! - After every save, shows the word count (setting `count_on_save`).

use vox_plugin_sdk::{
    CommandInfo, Event, EventKind, Level, MapMode, Plugin, Position, editor, export_plugin,
};

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

/// Capitalize the first letter of each word, lowercase the rest.
fn title_case(s: &str) -> String {
    let mut out = String::new();
    let mut start = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            start = false;
        } else {
            out.push(c);
            start = true;
        }
    }
    out
}

impl Plugin for TextTools {
    fn init() -> Result<(), String> {
        editor::register_command("wordcount", "Count words, lines, and characters");
        editor::register_command("upper", "Change the case of the cursor line (setting `case`)");
        editor::describe_command(
            "wordcount",
            &CommandInfo::new().category("Text").example("wordcount", "e.g. 12 words, 3 lines, 70 characters"),
        );
        editor::describe_command(
            "upper",
            &CommandInfo::new()
                .category("Text")
                .example("upper", "HELLO WORLD (or lower / Title, see the `case` setting)"),
        );
        editor::map_key(MapMode::Normal, "<leader>c", ":wordcount");
        editor::subscribe(EventKind::BufWrite);
        editor::log(Level::Info, "text-tools ready: :wordcount, :upper, <leader>c");
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
                editor::log(Level::Debug, &format!("upper: line {}", cursor.line + 1));
                let line = editor::line(cursor.line).unwrap_or_default();
                let start = Position { line: cursor.line, col: 0 };
                let end = Position { line: cursor.line, col: line.chars().count() as u32 };
                let case = editor::setting("case").and_then(|v| v.as_text());
                let changed = match case.as_deref() {
                    Some("lower") => line.to_lowercase(),
                    Some("title") => title_case(&line),
                    _ => line.to_uppercase(),
                };
                editor::delete_text(start, end);
                editor::insert_text(start, &changed);
                editor::set_cursor(cursor);
                Ok(())
            }
            other => Err(format!("text-tools: unknown command {other}")),
        }
    }

    fn on_event(event: Event) {
        let on = editor::setting("count_on_save").and_then(|v| v.as_bool());
        if let (Event::BufWrite(path), Some(true) | None) = (event, on) {
            let (words, _, _) = count();
            editor::message(&format!("saved {path}: {words} words"));
        }
    }
}

export_plugin!(TextTools);
