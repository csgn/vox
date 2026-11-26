//! Scopes: `:scope`, the project set-up dialog, and copying from the global setup.

use std::cell::Cell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use vox_core::Document;
use vox_editor::{Editor, Scope, parse_keys};

fn temp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("vox-scope-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn feed(ed: &mut Editor, keys: &str) {
    for k in parse_keys(keys) {
        ed.handle_key(k);
    }
}

/// A global setup with a config and two plugins, and an editor in local scope for `project`.
fn setup(name: &str) -> (Editor, PathBuf, Rc<Cell<u32>>) {
    let root = temp(name);
    let global = Scope::global(root.join("global"));
    fs::create_dir_all(global.plugin_dir().unwrap()).unwrap();
    fs::write(global.config_path().unwrap(), "leader = \" \"\n").unwrap();
    fs::write(global.plugin_dir().unwrap().join("alpha.wasm"), "a").unwrap();
    fs::write(global.plugin_dir().unwrap().join("beta.wasm"), "b").unwrap();
    let mut ed = Editor::new(Document::from_text("x"));
    ed.resize(80, 24);
    ed.set_scope(Scope::local(&root.join("project")), Some(global));
    // Stand-ins for the adapters' reload commands, to see they are called.
    let reloads = Rc::new(Cell::new(0));
    for cmd in ["reload config", "reload plugins"] {
        let r = reloads.clone();
        ed.commands_mut().register(cmd, "", move |_, _| {
            r.set(r.get() + 1);
            Ok(())
        });
    }
    (ed, root, reloads)
}

#[test]
fn setup_dialog_copy_everything() {
    let (mut ed, root, reloads) = setup("all");
    feed(&mut ed, ":scope setup<CR>");
    assert!(ed.has_modal());
    let screen = ed.render().to_string();
    assert!(screen.contains("Copy everything") && screen.contains("config + 2 plugin(s)"));
    feed(&mut ed, "G<CR>k<CR>"); // last row is "Not now"; one up is "Copy everything"
    // First Enter on "Not now" closed the dialog; reopen and pick "Copy everything".
    assert!(!root.join("project/.vox").exists());
    feed(&mut ed, ":scope setup<CR>Gk<CR>");
    let vox = root.join("project/.vox");
    assert!(vox.join("config.toml").is_file());
    assert!(vox.join("plugins/alpha.wasm").is_file() && vox.join("plugins/beta.wasm").is_file());
    assert_eq!(reloads.get(), 2, "config and plugins reloaded");
    assert!(
        ed.notifications()
            .last()
            .unwrap()
            .text
            .starts_with("set up ")
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn choose_what_to_copy() {
    let (mut ed, root, _) = setup("pick");
    feed(&mut ed, ":scope pick<CR>");
    // Rows: config.toml, plugin alpha, plugin beta. Pick only beta.
    feed(&mut ed, "jj<Space><CR>");
    let vox = root.join("project/.vox");
    assert!(!vox.join("config.toml").exists());
    assert!(!vox.join("plugins/alpha.wasm").exists());
    assert!(vox.join("plugins/beta.wasm").is_file());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn start_empty_creates_folders_only() {
    let (mut ed, root, _) = setup("empty");
    feed(&mut ed, ":scope init empty<CR>");
    assert!(root.join("project/.vox/plugins").is_dir());
    assert!(!root.join("project/.vox/config.toml").exists());
    // The global setup is untouched.
    assert!(root.join("global/plugins/alpha.wasm").is_file());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn setup_only_in_local_scope() {
    let mut ed = Editor::new(Document::from_text("x"));
    ed.set_scope(Scope::vanilla(), None);
    feed(&mut ed, ":scope setup<CR>");
    assert!(ed.message().unwrap().contains("needs local mode"));
    assert!(!ed.has_modal());
    ed.set_scope(Scope::global("/tmp/nowhere".into()), None);
    feed(&mut ed, ":scope init all<CR>");
    assert!(ed.message().unwrap().contains("needs local mode"));
}

#[test]
fn scope_info_and_status_badge() {
    let (mut ed, root, _) = setup("info");
    let screen = ed.render().to_string();
    assert!(screen.contains(" LOCAL "), "status line badge");
    feed(&mut ed, ":scope<CR>");
    let screen = ed.render().to_string();
    assert!(screen.contains("scope    local"), "{screen}");
    assert!(screen.contains("(missing)"), "project not set up yet");
    feed(&mut ed, "q");
    ed.set_scope(Scope::vanilla(), None);
    assert!(ed.render().to_string().contains(" VANILLA "));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn esc_goes_back_from_checklist_to_setup_dialog() {
    let (mut ed, root, _) = setup("back");
    feed(&mut ed, ":scope setup<CR>");
    assert_eq!(ed.modal_depth(), 0);
    assert!(!ed.render().to_string().contains("Esc: back"));
    // Rows: Start empty, Copy global config, Choose what to copy…, Copy everything, Not now.
    feed(&mut ed, "jj<CR>");
    let screen = ed.render().to_string();
    assert!(screen.contains("[ ] plugin alpha"), "checklist open");
    assert!(
        screen.contains("Esc: back"),
        "back hint on the nested dialog"
    );
    assert_eq!(ed.modal_depth(), 1);
    feed(&mut ed, "<Esc>");
    let screen = ed.render().to_string();
    assert!(
        screen.contains("Set up project"),
        "back at the set-up dialog"
    );
    assert_eq!(ed.modal_depth(), 0);
    feed(&mut ed, "<Esc>");
    assert!(!ed.has_modal(), "first dialog closes as before");
    // Choosing in the nested dialog closes the whole chain.
    feed(&mut ed, ":scope setup<CR>jj<CR><Space><CR>");
    assert!(!ed.has_modal());
    assert_eq!(ed.modal_depth(), 0);
    assert!(root.join("project/.vox/config.toml").is_file());
    // :close closes everything at once.
    feed(&mut ed, ":scope setup<CR>jj<CR>");
    ed.close_focused();
    assert!(!ed.has_modal() && ed.modal_depth() == 0);
    fs::remove_dir_all(&root).unwrap();
}
