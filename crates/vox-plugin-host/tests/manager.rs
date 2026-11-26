//! The plugin manager (`:plugins`): list, turn on/off, delete, dialogs going back, sync,
//! vanilla mode, side panel. Needs `cargo` and the `wasm32-wasip2` target.

use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use vox_core::Document;
use vox_editor::{Editor, Scope, parse_keys};
use vox_plugin_host::PluginHost;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn text_tools_wasm() -> PathBuf {
    static BUILT: OnceLock<()> = OnceLock::new();
    BUILT.get_or_init(|| {
        let ok = Command::new(env!("CARGO"))
            .args([
                "build",
                "--release",
                "--target",
                "wasm32-wasip2",
                "--manifest-path",
            ])
            .arg(root().join("plugins/Cargo.toml"))
            .env("CARGO_TARGET_DIR", root().join("plugins/target"))
            .status()
            .unwrap()
            .success();
        assert!(ok);
    });
    root().join("plugins/target/wasm32-wasip2/release/text_tools.wasm")
}

/// A project with text-tools installed through `:plugin add` (so it is in vox.lock).
fn project(name: &str) -> (PathBuf, Editor) {
    let base = std::env::temp_dir().join(format!("vox-manager-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let pkg = base.join("pkgs/text-tools");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::copy(
        root().join("plugins/text-tools/plugin.toml"),
        pkg.join("plugin.toml"),
    )
    .unwrap();
    std::fs::copy(text_tools_wasm(), pkg.join("plugin.wasm")).unwrap();
    let app = base.join("app");
    std::fs::create_dir_all(&app).unwrap();
    let mut ed = Editor::new(Document::from_text("one two three"));
    ed.resize(200, 40);
    let scope = Scope::local(&app);
    ed.set_scope(scope.clone(), None);
    ed.set_file_format(vox_config_file::parse, vox_config_file::format);
    ed.set_config_writer(vox_config_file::write_edits);
    let host = Rc::new(PluginHost::new(vox_config_file::parse).unwrap());
    PluginHost::register_commands(host, &mut ed, scope.plugin_dir().unwrap());
    feed(&mut ed, &format!(":plugin add {}<CR>", pkg.display()));
    wait(&mut ed);
    assert!(
        ed.commands().get("wordcount").is_some(),
        "{:?}",
        ed.notifications()
    );
    (base, ed)
}

fn feed(ed: &mut Editor, keys: &str) {
    for k in parse_keys(keys) {
        ed.handle_key(k);
    }
}

fn wait(ed: &mut Editor) {
    let start = Instant::now();
    while ed.jobs_running() > 0 {
        assert!(start.elapsed() < Duration::from_secs(600));
        std::thread::sleep(Duration::from_millis(50));
        ed.tick();
    }
}

fn screen(ed: &mut Editor) -> String {
    ed.render().to_string()
}

#[test]
fn lists_and_turns_off_and_on() {
    let (base, mut ed) = project("onoff");
    feed(&mut ed, ":plugins<CR>");
    let title = ed.modal_title().unwrap();
    assert!(title.starts_with("Plugins — local: "), "{title}");
    let s = screen(&mut ed);
    assert!(s.contains("text-tools"), "{s}");
    assert!(
        s.contains("0.2.0") && s.contains(" ok ") && s.contains("path ../pkgs/text-tools"),
        "{s}"
    );
    assert!(s.contains(":wordcount"), "what it adds: {s}");

    feed(&mut ed, "<Space>");
    assert!(ed.commands().get("wordcount").is_none(), "unloaded");
    let cfg = std::fs::read_to_string(base.join("app/.vox/config.toml")).unwrap();
    assert!(cfg.contains("enabled = false"), "{cfg}");
    assert!(
        screen(&mut ed).contains("disabled"),
        "the open manager refreshed"
    );
    assert!(ed.has_modal());

    feed(&mut ed, "<Space>");
    assert!(ed.commands().get("wordcount").is_some(), "loaded again");
    let cfg = std::fs::read_to_string(base.join("app/.vox/config.toml")).unwrap();
    assert!(!cfg.contains("enabled"), "{cfg}");
    feed(&mut ed, "q");
    assert!(!ed.has_modal());
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn delete_asks_and_goes_back() {
    let (base, mut ed) = project("delete");
    feed(&mut ed, ":plugins<CR>D");
    assert_eq!(ed.modal_title().as_deref(), Some("Delete plugin"));
    feed(&mut ed, "<Esc>");
    assert!(
        ed.modal_title().unwrap().starts_with("Plugins"),
        "Esc goes back"
    );
    assert!(ed.commands().get("wordcount").is_some());
    feed(&mut ed, "Dy");
    assert!(
        ed.modal_title().unwrap().starts_with("Plugins"),
        "back at the list"
    );
    assert!(ed.commands().get("wordcount").is_none());
    assert!(!base.join("app/.vox/plugins/text-tools").exists());
    assert!(screen(&mut ed).contains("No plugins yet"));
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn dialogs_go_back_to_the_manager() {
    let (base, mut ed) = project("back");
    feed(&mut ed, ":plugins<CR>a");
    assert_eq!(ed.modal_title().as_deref(), Some("Add plugin"));
    feed(&mut ed, "<Esc>");
    assert!(ed.modal_title().unwrap().starts_with("Plugins"));
    feed(&mut ed, "?");
    assert_eq!(ed.modal_title().as_deref(), Some("Plugin manager keys"));
    feed(&mut ed, "<Esc>");
    assert!(ed.modal_title().unwrap().starts_with("Plugins"));
    feed(&mut ed, "E");
    assert!(!ed.has_modal(), "files open in the editor");
    assert!(
        ed.document()
            .path()
            .unwrap()
            .ends_with("text-tools/plugin.toml")
    );
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn missing_plugins_show_and_sync() {
    let (base, mut ed) = project("sync");
    std::fs::remove_dir_all(base.join("app/.vox/plugins/text-tools")).unwrap();
    feed(&mut ed, ":reload plugins<CR>:plugins<CR>");
    let s = screen(&mut ed);
    assert!(s.contains("missing") && s.contains("s to sync"), "{s}");
    feed(&mut ed, "s");
    wait(&mut ed);
    assert!(ed.commands().get("wordcount").is_some());
    let s = screen(&mut ed);
    assert!(s.contains(" ok ") && !s.contains("missing"), "{s}");
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn side_panel_and_vanilla() {
    let (base, mut ed) = project("side");
    let mut cfg = ed.config().clone();
    cfg.ui.plugin_manager = vox_editor::ui::ManagerPlace::Side;
    // Left, so the notifications (top right) do not cover it.
    cfg.ui.side_panel = vox_editor::ui::SidePosition::Left;
    cfg.ui.side_width = 60;
    ed.apply_config(&cfg);
    feed(&mut ed, "\\p");
    assert!(!ed.has_modal(), "{:?}", ed.message());
    let s = screen(&mut ed);
    assert!(s.contains("text-tools  0.2.0"), "in the side panel: {s}");
    feed(&mut ed, "<Space>");
    assert!(ed.commands().get("wordcount").is_none());

    ed.set_scope(Scope::vanilla(), None);
    let mut cfg = ed.config().clone();
    cfg.ui.plugin_manager = vox_editor::ui::ManagerPlace::Modal;
    ed.apply_config(&cfg);
    feed(&mut ed, "<C-w>c:plugins<CR>");
    assert!(screen(&mut ed).contains("Plugins are off in vanilla mode"));
    feed(&mut ed, "a");
    assert!(
        ed.modal_title().unwrap().starts_with("Plugins"),
        "no actions in vanilla"
    );
    std::fs::remove_dir_all(&base).unwrap();
}
