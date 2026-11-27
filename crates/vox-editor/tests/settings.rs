//! The settings dialog: forms, saving through the config writer, key mappings, plugins.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Mutex;

use vox_core::Document;
use vox_editor::config::Value;
use vox_editor::plugin::{SettingKind, SettingSpec, SettingValue};
use vox_editor::settings::ConfigEdit;
use vox_editor::{Editor, Scope, parse_keys};

/// Edits passed to the fake writer, per test thread name.
static WRITTEN: Mutex<Vec<(String, PathBuf, Vec<ConfigEdit>)>> = Mutex::new(Vec::new());

fn thread() -> String {
    std::thread::current().name().unwrap_or("").to_string()
}

fn fake_writer(path: &Path, edits: &[ConfigEdit]) -> Result<(), String> {
    WRITTEN
        .lock()
        .unwrap()
        .push((thread(), path.to_path_buf(), edits.to_vec()));
    Ok(())
}

fn written() -> Vec<Vec<ConfigEdit>> {
    let me = thread();
    WRITTEN
        .lock()
        .unwrap()
        .iter()
        .filter(|(t, _, _)| *t == me)
        .map(|(_, _, e)| e.clone())
        .collect()
}

fn editor() -> Editor {
    let mut ed = Editor::new(Document::from_text("x"));
    ed.resize(100, 30);
    ed.set_scope(Scope::global("/cfg".into()), None);
    ed.set_config_writer(fake_writer);
    ed
}

fn feed(ed: &mut Editor, keys: &str) {
    for k in parse_keys(keys) {
        ed.handle_key(k);
    }
}

fn path(p: &str) -> Vec<String> {
    p.split('.').map(String::from).collect()
}

#[test]
fn vox_form_saves_and_goes_back() {
    let mut ed = editor();
    feed(&mut ed, ":settings<CR>");
    assert!(
        ed.modal_title()
            .unwrap()
            .starts_with("Settings — global: /cfg/config.toml")
    );
    feed(&mut ed, "<CR>"); // vox settings
    assert_eq!(ed.modal_title().as_deref(), Some("vox settings"));
    assert_eq!(ed.modal_depth(), 1);
    // Rows: leader, timeout_ms, developer_mode, ui.side_panel, ui.side_width, ...
    feed(&mut ed, "jjjj<CR><BS><BS>30<CR>");
    feed(&mut ed, "kk<Space>"); // developer_mode on
    feed(&mut ed, "s");
    assert!(
        ed.modal_title().unwrap().starts_with("Settings —"),
        "back at the list"
    );
    assert_eq!(ed.modal_depth(), 0);
    assert_eq!(
        written(),
        [vec![
            ConfigEdit::Set(path("developer_mode"), Value::Bool(true)),
            ConfigEdit::Set(path("ui.side_width"), Value::Integer(30)),
        ]]
    );
    assert_eq!(ed.ui_config().side_width, 30, "applied right away");
    assert!(ed.developer_mode());
    assert_eq!(
        ed.notifications().last().unwrap().text,
        "saved vox settings (2 change(s))"
    );
}

#[test]
fn esc_goes_back_from_form_to_list() {
    let mut ed = editor();
    feed(&mut ed, ":settings<CR><CR><Esc>");
    assert!(ed.modal_title().unwrap().starts_with("Settings —"));
    feed(&mut ed, "<Esc>");
    assert!(!ed.has_modal());
}

#[test]
fn vanilla_applies_without_saving() {
    let mut ed = editor();
    ed.set_scope(Scope::vanilla(), None);
    feed(&mut ed, ":settings vox<CR>jjjj<CR><BS><BS>25<CR>s");
    assert!(written().is_empty());
    assert_eq!(ed.ui_config().side_width, 25);
    assert!(
        ed.notifications()
            .last()
            .unwrap()
            .text
            .contains("this session only")
    );
}

#[test]
fn invalid_edits_never_reach_the_file() {
    let mut ed = editor();
    let err = ed
        .save_config_edits(&[ConfigEdit::Set(path("ui.side_width"), Value::Integer(1))])
        .unwrap_err();
    assert!(err.contains("between 10 and 500"), "{err}");
    assert!(written().is_empty());
}

#[test]
fn key_mappings_add_change_delete() {
    let mut ed = editor();
    feed(&mut ed, ":settings keys<CR>a");
    assert_eq!(ed.modal_title().as_deref(), Some("Add key mapping"));
    feed(&mut ed, "Q :q!<CR>"); // input starts with "n "
    assert_eq!(
        ed.modal_title().as_deref(),
        Some("Key mappings"),
        "back at the list"
    );
    assert!(ed.render().to_string().contains(" n Q "));
    assert_eq!(
        written(),
        [vec![ConfigEdit::Set(
            vec!["keymap".into(), "normal".into(), "Q".into()],
            Value::String(":q!".into())
        )]]
    );
    // Change: Enter, edit the target.
    feed(&mut ed, "<CR>");
    assert_eq!(ed.modal_title().as_deref(), Some("Change normal Q"));
    feed(&mut ed, "<BS>a<CR>"); // ":q!" → ":qa"
    assert_eq!(ed.config().keymaps[0].2, ":qa");
    // Delete with confirmation; Esc leaves it.
    feed(&mut ed, "d<Esc>");
    assert_eq!(ed.config().keymaps.len(), 1);
    feed(&mut ed, "dy");
    assert!(ed.config().keymaps.is_empty());
    assert_eq!(ed.modal_title().as_deref(), Some("Key mappings"));
    assert_eq!(written().len(), 3);
}

#[test]
fn other_owners_mappings_are_read_only() {
    let mut ed = editor();
    ed.with_owner("plugin:x", |ed| {
        ed.map_key(vox_editor::MapMode::Normal, "X", ":q")
    });
    // Rows: config mappings (none here), then the other owners' in keymap order.
    let row = ed
        .keymap()
        .iter()
        .position(|(_, _, _, owner)| owner == "plugin:x")
        .unwrap();
    feed(&mut ed, &format!(":settings keys<CR>{}d", "j".repeat(row)));
    assert_eq!(
        ed.modal_title().as_deref(),
        Some("Key mappings"),
        "no delete dialog"
    );
    assert!(
        ed.notifications()
            .last()
            .unwrap()
            .text
            .contains("comes from plugin:x")
    );
}

#[test]
fn plugin_form_saves_under_plugins_and_reloads() {
    let mut ed = editor();
    let reloaded = Rc::new(Cell::new(false));
    let r = reloaded.clone();
    ed.commands_mut()
        .register("reload plugins", "", move |_, a| {
            assert_eq!(a.arg, Some("demo"));
            r.set(true);
            Ok(())
        });
    ed.set_plugin_settings(
        "demo",
        vec![SettingSpec {
            key: "case".into(),
            kind: SettingKind::Choice(vec!["upper".into(), "lower".into()]),
            default: SettingValue::String("upper".into()),
            description: "case".into(),
        }],
    );
    feed(&mut ed, ":settings<CR>");
    assert!(ed.render().to_string().contains("plugin demo"));
    feed(&mut ed, "jj<CR><Space>s");
    assert_eq!(
        written(),
        [vec![ConfigEdit::Set(
            vec!["plugins".into(), "demo".into(), "case".into()],
            Value::String("lower".into())
        )]]
    );
    assert!(reloaded.get());
    assert_eq!(
        ed.plugin_config("demo").unwrap()["case"],
        Value::String("lower".into())
    );
    feed(&mut ed, "<Esc>:settings nope<CR>");
    assert!(ed.message().unwrap().contains("has no settings"));
}

#[test]
fn added_mapping_takes_effect_immediately() {
    let mut ed = editor();
    feed(&mut ed, ":settings key add n Q :notify mapped<CR>");
    feed(&mut ed, "Q");
    assert_eq!(ed.notifications().last().unwrap().text, "mapped");
}

#[test]
fn settings_get_set_unset_without_dialog() {
    const DEFAULT: i64 = 40;
    let mut ed = editor();
    assert_eq!(
        ed.execute("settings get ui.side_width"),
        Ok(Some(Value::Integer(DEFAULT)))
    );
    assert_eq!(ed.execute("settings set ui.side_width 60"), Ok(None));
    assert_eq!(ed.ui_config().side_width, 60);
    assert_eq!(
        ed.execute("settings get ui.side_width"),
        Ok(Some(Value::Integer(60)))
    );
    assert_eq!(ed.execute("settings unset ui.side_width"), Ok(None));
    assert_eq!(
        written(),
        [
            vec![ConfigEdit::Set(path("ui.side_width"), Value::Integer(60))],
            vec![ConfigEdit::Remove(path("ui.side_width"))],
        ]
    );
    assert_eq!(
        ed.execute("settings get ui.side_width"),
        Ok(Some(Value::Integer(DEFAULT)))
    );
    assert_eq!(ed.execute("settings set developer_mode true"), Ok(None));
    assert!(ed.developer_mode());
    assert_eq!(
        ed.execute("settings set leader <Space>"),
        Ok(None),
        "strings are taken as written"
    );
    assert_eq!(ed.config().leader, "<Space>");
}

#[test]
fn settings_set_rejects_bad_values() {
    let mut ed = editor();
    let err = |ed: &mut Editor, line: &str| ed.execute(line).unwrap_err();
    assert!(err(&mut ed, "settings set ui.side_width 5").contains("between 10 and 500"));
    assert!(err(&mut ed, "settings set ui.side_width wide").contains("must be an integer"));
    assert!(err(&mut ed, "settings set developer_mode yes").contains("true or false"));
    assert!(err(&mut ed, "settings set ui.side_panel top").contains("one of: right, left"));
    assert!(err(&mut ed, "settings set nope 1").contains("no setting `nope`"));
    assert!(err(&mut ed, "settings set ui.side_width").starts_with("E471"));
    assert!(err(&mut ed, "settings get plugins.demo").contains("plugins.NAME.KEY"));
    assert!(written().is_empty());
}

#[test]
fn plugin_settings_by_dotted_key() {
    let mut ed = editor();
    let reloaded = Rc::new(Cell::new(0));
    let r = reloaded.clone();
    ed.commands_mut()
        .register("reload plugins", "", move |_, a| {
            assert_eq!(a.arg, Some("demo"));
            r.set(r.get() + 1);
            Ok(())
        });
    ed.set_plugin_settings(
        "demo",
        vec![SettingSpec {
            key: "case".into(),
            kind: SettingKind::Choice(vec!["upper".into(), "lower".into()]),
            default: SettingValue::String("upper".into()),
            description: "Letter case".into(),
        }],
    );
    let get = |ed: &mut Editor| ed.execute("settings get plugins.demo.case");
    assert_eq!(get(&mut ed), Ok(Some(Value::from("upper"))));
    assert_eq!(ed.execute("settings set plugins.demo.case lower"), Ok(None));
    assert_eq!(get(&mut ed), Ok(Some(Value::from("lower"))));
    assert_eq!(reloaded.get(), 1);
    let e = ed
        .execute("settings set plugins.demo.case mixed")
        .unwrap_err();
    assert!(e.contains("`plugins.demo.case` must be one of"), "{e}");
    assert!(
        ed.execute("settings get plugins.demo.nope")
            .unwrap_err()
            .contains("no setting")
    );

    let Ok(Some(Value::Array(rows))) = ed.execute("settings list") else {
        panic!("settings list returns a list")
    };
    let last = rows.last().unwrap();
    assert_eq!(
        *last,
        Value::table([
            ("key", Value::from("plugins.demo.case")),
            ("value", "lower".into()),
            ("default", "upper".into()),
            ("type", "one of: upper, lower".into()),
            ("description", "Letter case".into()),
        ])
    );
    assert!(
        rows.iter()
            .any(|r| r.to_text().contains("key: ui.side_width"))
    );
}

#[test]
fn key_list_returns_mappings() {
    let mut ed = editor();
    ed.execute("settings key add n Q :notify mapped").unwrap();
    let Ok(Some(Value::Array(rows))) = ed.execute("settings key list") else {
        panic!("key list returns a list")
    };
    assert_eq!(
        rows[0],
        Value::table([
            ("mode", Value::from("normal")),
            ("keys", "Q".into()),
            ("target", ":notify mapped".into()),
            ("owner", "config".into()),
        ])
    );
}

#[test]
fn dialogs_fail_when_not_interactive() {
    let mut ed = editor();
    ed.set_interactive(false);
    let e = ed.execute("settings vox").unwrap_err();
    assert!(e.contains(":settings vox needs the editor UI"), "{e}");
    assert!(ed.execute("settings key new").is_err());
    assert_eq!(ed.modal_title(), None);
    assert_eq!(ed.execute("settings set ui.side_width 40"), Ok(None));
}
