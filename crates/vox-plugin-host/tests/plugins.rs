//! Load the plugins in `plugins/` into a real editor. Builds them for `wasm32-wasip2` first
//! (once per test run), so `rustup target add wasm32-wasip2` is required.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

use std::rc::Rc;
use vox_core::Document;

use std::collections::BTreeMap;

use vox_editor::config::Value;
use vox_editor::plugin::HealthStatus;
use vox_editor::{Config, Editor, Level, owner, parse_keys};
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
        static HOST: &'static PluginHost = Box::leak(Box::new(PluginHost::new(vox_config_file::parse).unwrap()));
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
    let names: Vec<_> = results
        .iter()
        .map(|i| (i.name.as_str(), i.loadable()))
        .collect();
    assert_eq!(names, [("a", true)]);
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
    let host = Rc::new(PluginHost::new(vox_config_file::parse).unwrap());
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
    feed(&mut ed, ":reload plugins misbehave<CR>");
    assert_eq!(
        ed.notifications().last().unwrap().text,
        "reloaded 1 plugin(s)"
    );
    feed(&mut ed, ":ok<CR>");
    assert_eq!(ed.message(), Some("ok"), "fresh instance after reload");
    assert_eq!(host.names(), ["misbehave"]);
    feed(&mut ed, ":reload plugins nope<CR>");
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
    let host = Rc::new(PluginHost::new(vox_config_file::parse).unwrap());
    let mut ed = editor("a b");
    host.load_dir(&mut ed, &dir);
    PluginHost::register_commands(host.clone(), &mut ed, dir.clone());
    feed(&mut ed, ":reload plugins<CR>");
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

/// Install plugin `name` as a package folder `dest/name/{plugin.toml, plugin.wasm}`.
fn install_package(name: &str, dest: &std::path::Path) -> PathBuf {
    let dir = dest.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(
        root().join(format!("plugins/{name}/plugin.toml")),
        dir.join("plugin.toml"),
    )
    .unwrap();
    std::fs::copy(plugin(name), dir.join("plugin.wasm")).unwrap();
    dir
}

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vox-pkg-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Editor whose config sets `[plugins.NAME] KEY = VALUE`.
fn editor_with(text: &str, plugin: &str, values: &[(&str, Value)]) -> Editor {
    let mut ed = editor(text);
    let mut cfg = Config::default();
    cfg.plugins.insert(
        plugin.to_string(),
        values
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
    );
    ed.apply_config(&cfg);
    ed
}

#[test]
fn package_with_settings_from_config() {
    let dir = temp_dir("settings");
    install_package("text-tools", &dir);
    let mut ed = editor_with(
        "mixed Case words",
        "text-tools",
        &[("case", Value::String("title".into()))],
    );
    let host = PluginHost::new(vox_config_file::parse).unwrap();
    let infos = host.load_dir(&mut ed, &dir);
    assert_eq!(infos[0].name, "text-tools");
    assert_eq!(infos[0].status, HealthStatus::Ok, "{:?}", infos[0].items);
    assert_eq!(infos[0].version().unwrap().to_string(), "0.2.0");
    feed(&mut ed, ":upper<CR>");
    assert_eq!(ed.document().to_string(), "Mixed Case Words");
    // Its settings are known to the settings dialog.
    let keys: Vec<&str> = ed.plugin_settings()["text-tools"]
        .iter()
        .map(|s| s.key.as_str())
        .collect();
    assert_eq!(keys, ["case", "count_on_save"]);
    host.reload_all(&mut ed, &dir.join("missing"));
    assert!(ed.plugin_settings().is_empty(), "forgotten after unloading");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn bad_setting_falls_back_with_warning() {
    let dir = temp_dir("badsetting");
    install_package("text-tools", &dir);
    let mut ed = editor_with(
        "abc",
        "text-tools",
        &[("case", Value::String("shout".into()))],
    );
    let host = PluginHost::new(vox_config_file::parse).unwrap();
    let info = host.load_dir(&mut ed, &dir).remove(0);
    assert_eq!(info.status, HealthStatus::Warning);
    assert!(
        info.items
            .iter()
            .any(|i| i.message.contains("must be one of: upper, lower, title"))
    );
    feed(&mut ed, ":upper<CR>");
    assert_eq!(ed.document().to_string(), "ABC", "default case is used");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn disabled_in_config_is_not_loaded() {
    let dir = temp_dir("disabled");
    install_package("text-tools", &dir);
    let mut ed = editor_with("abc", "text-tools", &[("enabled", Value::Bool(false))]);
    let host = PluginHost::new(vox_config_file::parse).unwrap();
    let info = host.load_dir(&mut ed, &dir).remove(0);
    assert_eq!(info.status, HealthStatus::Disabled);
    assert!(ed.commands().get("wordcount").is_none());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn custom_health_levels() {
    for (setting, status, loaded) in [
        ("ok", HealthStatus::Info, true), // the manifest's info-level requirement is unmet
        ("warning", HealthStatus::Warning, true),
        ("error", HealthStatus::Failed, false),
    ] {
        let dir = temp_dir(&format!("health-{setting}"));
        install_package("misbehave", &dir);
        let mut ed = editor_with(
            "x",
            "misbehave",
            &[("health", Value::String(setting.into()))],
        );
        let host = PluginHost::new(vox_config_file::parse).unwrap();
        let info = host.load_dir(&mut ed, &dir).remove(0);
        assert_eq!(info.status, status, "{setting}: {:?}", info.items);
        assert_eq!(
            ed.commands().get("ok").is_some(),
            loaded,
            "{setting}: commands removed on error"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[test]
fn wrong_api_version_is_not_loaded() {
    let dir = temp_dir("api");
    let pkg = install_package("text-tools", &dir);
    let toml = std::fs::read_to_string(pkg.join("plugin.toml")).unwrap();
    std::fs::write(
        pkg.join("plugin.toml"),
        toml.replace("api = \"0.2\"", "api = \"0.1\""),
    )
    .unwrap();
    let mut ed = editor("x");
    let host = PluginHost::new(vox_config_file::parse).unwrap();
    let err = host.load(&mut ed, &pkg).unwrap_err();
    assert!(err.contains("built for plugin API 0.1"), "{err}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn checkhealth_report() {
    let dir = temp_dir("report");
    install_package("text-tools", &dir);
    install_package("misbehave", &dir);
    let mut ed = editor_with(
        "x",
        "misbehave",
        &[("health", Value::String("warning".into()))],
    );
    let host = Rc::new(PluginHost::new(vox_config_file::parse).unwrap());
    host.load_dir(&mut ed, &dir);
    PluginHost::register_commands(host.clone(), &mut ed, dir.clone());
    feed(&mut ed, ":checkhealth<CR>");
    let screen = ed.render().to_string();
    assert!(screen.contains("misbehave 0.1.0 — warning"), "{screen}");
    assert!(
        screen.contains("plugin check: told to be unhealthy"),
        "{screen}"
    );
    feed(&mut ed, "q:checkhealth nope<CR>");
    assert!(ed.message().unwrap().contains("no plugin named nope"));
    std::fs::remove_dir_all(&dir).unwrap();
}
