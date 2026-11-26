//! Load the plugins in `plugins/` into a real editor. Builds them for `wasm32-wasip2` first
//! (once per test run), so `rustup target add wasm32-wasip2` is required.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

use std::rc::Rc;
use vox_core::Document;

use vox_editor::{Editor, Level, owner, parse_keys};
use vox_plugin_host::PluginHost;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Path of the built plugin `name` (built once for all tests).
fn plugin(name: &str) -> PathBuf {
    static BUILT: OnceLock<()> = OnceLock::new();
    BUILT.get_or_init(|| {
        let status = std::process::Command::new(env!("CARGO"))
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
            .expect("run cargo");
        assert!(status.success(), "building plugins failed");
    });
    root().join(format!(
        "plugins/target/wasm32-wasip2/release/{}.wasm",
        name.replace('-', "_")
    ))
}

fn editor(text: &str) -> Editor {
    let mut ed = Editor::new(Document::from_text(text));
    ed.resize(80, 24);
    ed
}

fn feed(ed: &mut Editor, keys: &str) {
    for k in parse_keys(keys) {
        ed.handle_key(k);
    }
}

fn host() -> &'static PluginHost {
    // The host is not `Sync`, so make one per thread.
    thread_local! {
        static HOST: &'static PluginHost = Box::leak(Box::new(PluginHost::new().unwrap()));
    }
    HOST.with(|h| *h)
}

#[test]
fn text_tools_wordcount_command() {
    let mut ed = editor("hello brave\nnew world");
    let name = host().load(&mut ed, &plugin("text-tools")).unwrap();
    assert_eq!(name, "text_tools");
    feed(&mut ed, ":wordcount<CR>");
    assert_eq!(ed.message(), Some("4 words, 2 lines, 20 characters"));
}

#[test]
fn text_tools_mapping_and_edit_with_undo() {
    let mut ed = editor("abc\nmixed Case");
    host().load(&mut ed, &plugin("text-tools")).unwrap();
    // Mapping registered by the plugin, default leader is `\`.
    feed(&mut ed, "\\c");
    assert_eq!(ed.message(), Some("3 words, 2 lines, 13 characters"));
    feed(&mut ed, "jll:upper<CR>");
    assert_eq!(ed.document().to_string(), "abc\nMIXED CASE");
    assert_eq!((ed.cursor().line, ed.cursor().col), (1, 2), "cursor kept");
    // The plugin's delete + insert is one undo step.
    feed(&mut ed, "u");
    assert_eq!(ed.document().to_string(), "abc\nmixed Case");
}

#[test]
fn text_tools_event_after_save() {
    let path = std::env::temp_dir().join(format!("vox-plugin-test-{}.txt", std::process::id()));
    std::fs::write(&path, "one two three\n").unwrap();
    let mut ed =
        Editor::new(Document::open(&path, vox_core::storage::SimpleStorage::boxed).unwrap());
    host().load(&mut ed, &plugin("text-tools")).unwrap();
    feed(&mut ed, ":w<CR>");
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        ed.message(),
        Some(format!("saved {}: 3 words", path.display()).as_str())
    );
}

#[test]
fn infinite_loop_hits_time_limit_and_disables_plugin() {
    let mut ed = editor("x");
    host().load(&mut ed, &plugin("misbehave")).unwrap();
    let start = Instant::now();
    feed(&mut ed, ":spin<CR>");
    assert!(start.elapsed().as_secs() < 5, "stopped in time");
    let msg = ed.message().unwrap().to_string();
    assert!(
        msg.contains("took too long") && msg.contains("disabled"),
        "{msg}"
    );
    feed(&mut ed, ":ok<CR>");
    assert!(
        ed.message()
            .unwrap()
            .contains("disabled after an earlier error")
    );
    // The editor itself still works.
    feed(&mut ed, "x");
    assert_eq!(ed.document().to_string(), "");
}

#[test]
fn memory_limit() {
    let mut ed = editor("x");
    host().load(&mut ed, &plugin("misbehave")).unwrap();
    feed(&mut ed, ":grow<CR>");
    let msg = ed.message().unwrap();
    assert!(
        msg.contains("plugin misbehave stopped") && msg.contains("disabled"),
        "{msg}"
    );
}

#[test]
fn panic_disables_plugin() {
    let mut ed = editor("x");
    host().load(&mut ed, &plugin("misbehave")).unwrap();
    feed(&mut ed, ":crash<CR>");
    assert!(
        ed.message().unwrap().contains("stopped"),
        "{:?}",
        ed.message()
    );
}

#[test]
fn reentrant_call_is_refused_not_deadlocked() {
    let mut ed = editor("x");
    host().load(&mut ed, &plugin("misbehave")).unwrap();
    feed(&mut ed, ":reenter<CR>");
    // The inner call failed with a message, then the outer call finished normally.
    assert_eq!(ed.message(), Some("reenter finished"));
    feed(&mut ed, ":ok<CR>");
    assert_eq!(ed.message(), Some("ok"), "plugin still enabled");
}

#[test]
fn command_error_is_shown_and_plugin_stays_enabled() {
    let mut ed = editor("x");
    host().load(&mut ed, &plugin("misbehave")).unwrap();
    feed(&mut ed, ":fail<CR>");
    assert_eq!(ed.message(), Some("E900: failed on purpose"));
    feed(&mut ed, ":ok<CR>");
    assert_eq!(ed.message(), Some("ok"));
}

#[test]
fn load_errors() {
    let mut ed = editor("x");
    let missing = root().join("plugins/does-not-exist.wasm");
    assert!(host().load(&mut ed, &missing).is_err());
    let not_wasm = std::env::temp_dir().join(format!("vox-not-wasm-{}.wasm", std::process::id()));
    std::fs::write(&not_wasm, "hello").unwrap();
    let err = host().load(&mut ed, &not_wasm).unwrap_err();
    std::fs::remove_file(&not_wasm).unwrap();
    assert!(err.starts_with("plugin vox-not-wasm"), "{err}");
}

#[test]
fn load_dir_loads_all_wasm_files() {
    let dir = std::env::temp_dir().join(format!("vox-plugin-dir-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(plugin("text-tools"), dir.join("a.wasm")).unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
    let mut ed = editor("x");
    let results = host().load_dir(&mut ed, &dir);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(results, vec![Ok("a".to_string())]);
    assert!(ed.commands().get("wordcount").is_some());
    assert!(host().load_dir(&mut ed, &dir).is_empty(), "missing dir");
}

#[test]
fn plugin_log_and_notify() {
    let mut ed = editor("x");
    host().load(&mut ed, &plugin("misbehave")).unwrap();
    feed(&mut ed, ":say<CR>");
    assert!(
        ed.log_entries()
            .entries()
            .any(|e| e.source == "plugin:misbehave"
                && e.level == Level::Warn
                && e.message == "said something")
    );
    assert_eq!(ed.notifications()[0].text, "hello from misbehave");
    assert!(
        ed.log_entries()
            .entries()
            .any(|e| e.source == "plugin:misbehave" && e.message.starts_with("loaded"))
    );
}

#[test]
fn unloading_restores_replaced_builtin() {
    let mut ed = editor("x");
    host().load(&mut ed, &plugin("misbehave")).unwrap();
    assert_eq!(ed.commands().get("x").unwrap().owner, "plugin:misbehave");
    feed(&mut ed, ":x<CR>");
    assert_eq!(ed.message(), Some("plugin xit"));
    ed.remove_owned(&owner::plugin("misbehave"));
    assert_eq!(ed.commands().get("xit").unwrap().owner, owner::BUILTIN);
    assert!(ed.commands().get("spin").is_none());
}

#[test]
fn reload_brings_back_a_disabled_plugin() {
    let host = Rc::new(PluginHost::new().unwrap());
    let mut ed = editor("x");
    host.load(&mut ed, &plugin("misbehave")).unwrap();
    feed(&mut ed, ":crash<CR>:ok<CR>");
    assert!(ed.message().unwrap().contains("disabled"));
    assert!(
        ed.log_entries()
            .entries()
            .any(|e| e.level == Level::Error && e.source == "plugin:misbehave")
    );
    let dir = root().join("plugins/target/wasm32-wasip2/release");
    PluginHost::register_commands(host.clone(), &mut ed, dir);
    feed(&mut ed, ":reload-plugins misbehave<CR>");
    assert_eq!(
        ed.notifications().last().unwrap().text,
        "reloaded 1 plugin(s)"
    );
    feed(&mut ed, ":ok<CR>");
    assert_eq!(ed.message(), Some("ok"), "fresh instance after reload");
    assert_eq!(host.names(), ["misbehave"]);
    feed(&mut ed, ":reload-plugins nope<CR>");
    assert!(
        ed.notifications()
            .last()
            .unwrap()
            .text
            .contains("no plugin named nope")
    );
}

#[test]
fn reload_all_from_dir() {
    let dir = std::env::temp_dir().join(format!("vox-reload-dir-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(plugin("text-tools"), dir.join("text_tools.wasm")).unwrap();
    let host = Rc::new(PluginHost::new().unwrap());
    let mut ed = editor("a b");
    host.load_dir(&mut ed, &dir);
    PluginHost::register_commands(host.clone(), &mut ed, dir.clone());
    feed(&mut ed, ":reload-plugins<CR>");
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(
        ed.notifications().last().unwrap().text,
        "reloaded 1 plugin(s)"
    );
    feed(&mut ed, ":wordcount<CR>");
    assert_eq!(ed.message(), Some("2 words, 1 lines, 3 characters"));
    // Exactly one copy of the plugin's mapping after reloading.
    let maps = ed
        .keymap()
        .iter()
        .filter(|m| m.3 == "plugin:text_tools")
        .count();
    assert_eq!(maps, 1);
}
