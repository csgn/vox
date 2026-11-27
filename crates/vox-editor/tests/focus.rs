//! Focus mode (one unit, the rest dimmed) and the text / agent editor mode.

use vox_core::Document;
use vox_editor::agent::EditorMode;
use vox_editor::structure::UnitKind;
use vox_editor::{Config, Editor, Value, owner, parse_keys};

const CODE: &str = "use std::io;

impl Shape {
    fn area(&self) -> u32 {
        if self.w > 0 {
            self.w * self.h
        } else {
            0
        }
    }

    fn name(&self) -> &str {
        \"shape\"
    }
}
";

fn editor() -> Editor {
    let mut ed = Editor::new(Document::from_text(CODE));
    ed.resize(60, 20);
    ed
}

fn feed(ed: &mut Editor, keys: &str) {
    for k in parse_keys(keys) {
        ed.handle_key(k);
    }
}

/// (kind, first line, last line) of the focus, lines 1-based.
fn focus(ed: &Editor) -> Option<(UnitKind, usize, usize)> {
    ed.focused_unit().map(|u| (u.kind, u.start + 1, u.end + 1))
}

#[test]
fn widen_and_narrow_with_keys() {
    let mut ed = editor();
    feed(&mut ed, "5G"); // inside the `if`
    feed(&mut ed, "\\f");
    assert_eq!(focus(&ed), Some((UnitKind::Block, 5, 7)));
    feed(&mut ed, "\\f");
    assert_eq!(focus(&ed), Some((UnitKind::Function, 4, 10)));
    feed(&mut ed, "\\f");
    assert_eq!(focus(&ed), Some((UnitKind::Class, 3, 15)));
    feed(&mut ed, "\\f");
    assert_eq!(focus(&ed), Some((UnitKind::File, 1, 15)));
    feed(&mut ed, "\\f");
    assert_eq!(focus(&ed), Some((UnitKind::File, 1, 15)), "nothing larger");
    feed(&mut ed, "\\F\\F");
    assert_eq!(focus(&ed), Some((UnitKind::Function, 4, 10)));
    // Esc ends focus (after any pending keys).
    feed(&mut ed, "<Esc>");
    assert_eq!(focus(&ed), None);
}

#[test]
fn narrow_commands() {
    let mut ed = editor();
    feed(&mut ed, "6G:narrow function<CR>");
    assert_eq!(focus(&ed), Some((UnitKind::Function, 4, 10)));
    assert_eq!(ed.message(), Some("focus: function area (L4–10)"));
    feed(&mut ed, ":narrow class<CR>");
    assert_eq!(focus(&ed), Some((UnitKind::Class, 3, 15)));
    feed(&mut ed, ":widen<CR>");
    assert_eq!(focus(&ed), None);
    feed(&mut ed, "1G:narrow function<CR>");
    assert_eq!(ed.message(), Some("E900: no function around the cursor"));
    // A selection: the lines of the visual selection.
    feed(&mut ed, "12GVj\\f");
    assert_eq!(focus(&ed), Some((UnitKind::Selection, 12, 13)));
    assert_eq!(ed.mode(), &vox_editor::Mode::Normal);
    feed(&mut ed, ":narrow selection<CR>");
    assert!(ed.message().unwrap().contains("no selection"));
}

#[test]
fn focus_follows_edits() {
    let mut ed = editor();
    feed(&mut ed, "12G:narrow function<CR>");
    assert_eq!(focus(&ed), Some((UnitKind::Function, 12, 14)));
    // Lines added above shift it; lines added inside grow it.
    feed(&mut ed, "1GOa<CR>b<Esc>");
    assert_eq!(focus(&ed), Some((UnitKind::Function, 14, 16)));
    feed(&mut ed, "15Gox<Esc>");
    assert_eq!(focus(&ed), Some((UnitKind::Function, 14, 17)));
    feed(&mut ed, "15Gdd");
    assert_eq!(focus(&ed), Some((UnitKind::Function, 14, 16)));
}

#[test]
fn outside_the_focus_is_dimmed() {
    let mut ed = editor();
    feed(&mut ed, "12G:narrow function<CR>");
    let s = ed.render();
    // Text column starts after the gutter ("12 ").
    let x = 3;
    assert!(!s.cell(x + 4, 11).style.dim, "line 12 is in focus");
    assert!(s.cell(x, 0).style.dim, "line 1 is dimmed");
    let status = s.to_string();
    assert!(status.contains("FOCUS function name (L12–14)"), "{status}");
}

#[test]
fn switching_buffers_ends_focus() {
    let dir = std::env::temp_dir().join(format!("vox-focus-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("other.txt");
    std::fs::write(&file, "x\n").unwrap();
    let mut ed = editor();
    feed(&mut ed, "6G\\f");
    assert!(focus(&ed).is_some());
    feed(&mut ed, &format!(":e {}<CR>", file.display()));
    assert_eq!(focus(&ed), None);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn editor_mode_switches() {
    let mut ed = editor();
    assert_eq!(ed.editor_mode(), EditorMode::Text);
    assert_eq!(ed.execute("mode"), Ok(Some(Value::from("text"))));
    feed(&mut ed, "\\m");
    assert_eq!(ed.editor_mode(), EditorMode::Agent);
    assert!(ed.render().to_string().contains(" NORMAL AGENT "));
    // Agent-only registrations go away in text mode.
    ed.with_owner(owner::AGENT, |ed| {
        ed.commands_mut().register("ai", "test", |_, _| Ok(()));
    });
    feed(&mut ed, ":mode text<CR>");
    assert_eq!(ed.editor_mode(), EditorMode::Text);
    assert!(ed.commands().get("ai").is_none());
    assert!(ed.execute("mode nope").unwrap_err().starts_with("E474"));
}

#[test]
fn editor_mode_from_config_only_when_it_changes() {
    let mut ed = editor();
    let cfg = Config {
        editor_mode: EditorMode::Agent,
        ..Config::default()
    };
    ed.apply_config(&cfg);
    assert_eq!(ed.editor_mode(), EditorMode::Agent);
    // Switched by hand, then the same config is applied again (reload): it stays.
    feed(&mut ed, ":mode text<CR>");
    ed.apply_config(&cfg);
    assert_eq!(ed.editor_mode(), EditorMode::Text);
    let (parsed, errors) = Config::from_value(&Value::table([("editor_mode", "agent".into())]));
    assert!(errors.is_empty());
    assert_eq!(parsed.editor_mode, EditorMode::Agent);
    let (_, errors) = Config::from_value(&Value::table([("editor_mode", "x".into())]));
    assert_eq!(errors.len(), 1);
}

mod snapshots {
    use super::*;

    #[test]
    fn focused_function() {
        let mut ed = editor();
        feed(&mut ed, "\\m12G\\f");
        insta::assert_snapshot!(ed.render().to_string());
    }
}
