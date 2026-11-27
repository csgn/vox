//! Command help: metadata of built-in commands, `:help` / `:?`, `:commands info`,
//! `:commands run`.

use vox_core::Document;
use vox_editor::{Editor, PanelSlot, Value, parse_keys};

fn editor() -> Editor {
    let mut ed = Editor::new(Document::from_text("hello"));
    ed.resize(70, 20);
    ed
}

fn feed(ed: &mut Editor, keys: &str) {
    for k in parse_keys(keys) {
        ed.handle_key(k);
    }
}

/// `table[key]` of a reply.
fn field<'a>(v: &'a Value, key: &str) -> &'a Value {
    match v {
        Value::Table(t) => t.get(key).unwrap_or_else(|| panic!("no {key} in {v:?}")),
        _ => panic!("not a table: {v:?}"),
    }
}

#[test]
fn every_builtin_has_metadata() {
    let ed = editor();
    for c in ed.commands().iter().filter(|c| c.owner == "builtin") {
        assert!(!c.description.is_empty(), "{} has no description", c.name);
        assert_ne!(c.category, "Other", "{} has no category", c.name);
        // A description that shows arguments (`:name ARG`) needs declared parameters.
        if c.description.contains(&format!("`:{} ", c.name)) {
            assert!(!c.params.is_empty(), "{} has no params", c.name);
        }
        // Commands that reply with data say what they return.
        if c.description.starts_with("Return ") {
            assert!(c.returns.is_some(), "{} has no returns", c.name);
        }
    }
}

#[test]
fn help_and_short_form_open_the_dialog() {
    let mut ed = editor();
    feed(&mut ed, ":help w<CR>");
    assert_eq!(ed.modal_title().as_deref(), Some(":help write"));
    feed(&mut ed, "<Esc>:?plugin add<CR>");
    assert_eq!(ed.modal_title().as_deref(), Some(":help plugin add"));
    feed(&mut ed, "<Esc>:h plugin<CR>");
    assert_eq!(ed.modal_title().as_deref(), Some(":help plugin"));
    feed(&mut ed, "<Esc>:help nope<CR>");
    assert!(!ed.has_modal());
    assert_eq!(ed.message(), Some("E149: Sorry, no help for nope"));
    // No topic: the list of all commands.
    feed(&mut ed, ":help<CR>");
    assert_eq!(ed.modal_title().as_deref(), Some("Commands"));
}

#[test]
fn subcommand_help_goes_back_to_the_parent() {
    let mut ed = editor();
    feed(&mut ed, ":help plugin<CR>j<CR>");
    assert_eq!(ed.modal_title().as_deref(), Some(":help plugin add"));
    assert_eq!(ed.modal_depth(), 1);
    feed(&mut ed, "<Esc>");
    assert_eq!(ed.modal_title().as_deref(), Some(":help plugin"));
    feed(&mut ed, "<Esc>");
    assert!(!ed.has_modal());
}

#[test]
fn run_from_help_asks_for_arguments() {
    let mut ed = editor();
    feed(&mut ed, ":help plugin add<CR>r");
    assert_eq!(ed.modal_title().as_deref(), Some("Run :plugin add SOURCE"));
    feed(&mut ed, "<Esc>");
    assert_eq!(ed.modal_title().as_deref(), Some(":help plugin add"));
    // No required arguments: runs at once.
    feed(&mut ed, "<Esc>:help messages<CR>r");
    assert!(!ed.has_modal());
    assert!(ed.has_panel(PanelSlot::Bottom));
}

#[test]
fn help_as_data() {
    let mut ed = editor();
    let v = ed.execute("commands info plugin remove").unwrap().unwrap();
    assert_eq!(field(&v, "usage"), &Value::from("plugin remove[!] NAME"));
    assert_eq!(field(&v, "category"), &Value::from("Plugins"));
    assert!(matches!(field(&v, "force"), Value::String(_)));
    let Value::Array(params) = field(&v, "params") else {
        panic!("params")
    };
    assert_eq!(field(&params[0], "complete"), &Value::from("plugin"));
    let v = ed.execute("commands info notify").unwrap().unwrap();
    let Value::Array(params) = field(&v, "params") else {
        panic!("params")
    };
    assert_eq!(
        field(&params[0], "values"),
        &Value::Array(vec!["info".into(), "warn".into(), "error".into()])
    );
    let v = ed.execute("commands info plugin").unwrap().unwrap();
    assert!(matches!(field(&v, "subcommands"), Value::Array(s) if s.len() > 3));
    assert!(
        ed.execute("commands info nope")
            .unwrap_err()
            .starts_with("E149")
    );
    // Every entry of `:commands list` has the metadata.
    let Some(Value::Array(all)) = ed.execute("commands list").unwrap() else {
        panic!("list")
    };
    assert!(
        all.iter()
            .all(|c| matches!(field(c, "category"), Value::String(_)))
    );
}

#[test]
fn without_ui_help_replies_and_run_needs_arguments() {
    let mut ed = editor();
    ed.set_interactive(false);
    let v = ed.execute("?w").unwrap().unwrap();
    assert_eq!(field(&v, "name"), &Value::from("write"));
    let e = ed.execute("commands run plugin add").unwrap_err();
    assert_eq!(e, "E471: :plugin add needs arguments: :plugin add SOURCE");
    assert!(
        ed.execute("commands run nope")
            .unwrap_err()
            .starts_with("E492")
    );
}

mod snapshots {
    use super::*;

    #[test]
    fn help_dialog() {
        let mut ed = editor();
        ed.resize(110, 34);
        feed(&mut ed, ":help plugin remove<CR>");
        insta::assert_snapshot!(ed.render().to_string());
    }

    #[test]
    fn help_group_with_selection() {
        let mut ed = editor();
        ed.resize(110, 34);
        feed(&mut ed, ":help scope<CR>jj");
        insta::assert_snapshot!(ed.render().to_string());
    }
}
