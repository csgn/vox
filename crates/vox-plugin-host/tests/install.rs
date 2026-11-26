//! Installing plugins into a project scope: from a packaged folder, a plugin project
//! (built with cargo), and a git repository; removing; syncing with vox.lock.
//! Needs `git`, `cargo`, and the `wasm32-wasip2` target.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use vox_core::Document;
use vox_editor::plugin::install::{self, LockState};
use vox_editor::plugin::lock::{self, Source};
use vox_editor::{Editor, Scope, parse_keys};
use vox_plugin_host::PluginHost;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// The built text-tools wasm (built once).
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

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vox-install-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// An editor in the local scope of `project`, with the plugin host and file format.
fn editor(project: &Path) -> Editor {
    let mut ed = Editor::new(Document::from_text("one two three"));
    ed.resize(120, 30);
    let scope = Scope::local(project);
    ed.set_scope(scope.clone(), None);
    ed.set_file_format(vox_config_file::parse, vox_config_file::format);
    let host = Rc::new(PluginHost::new(vox_config_file::parse).unwrap());
    PluginHost::register_commands(host, &mut ed, scope.plugin_dir().unwrap());
    ed
}

fn feed(ed: &mut Editor, keys: &str) {
    for k in parse_keys(keys) {
        ed.handle_key(k);
    }
}

/// Run background jobs to completion.
fn wait(ed: &mut Editor) {
    let start = Instant::now();
    while ed.jobs_running() > 0 {
        assert!(
            start.elapsed() < Duration::from_secs(600),
            "job took too long"
        );
        std::thread::sleep(Duration::from_millis(50));
        ed.tick();
    }
}

fn last(ed: &Editor) -> String {
    ed.notifications()
        .last()
        .map(|n| n.text.clone())
        .unwrap_or_default()
}

fn lockfile(project: &Path) -> lock::Lockfile {
    lock::read(&project.join(".vox/vox.lock"), vox_config_file::parse).unwrap()
}

#[test]
fn add_packaged_folder_then_remove() {
    let base = temp("packaged");
    let pkg = base.join("pkgs/text-tools");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::copy(
        root().join("plugins/text-tools/plugin.toml"),
        pkg.join("plugin.toml"),
    )
    .unwrap();
    std::fs::copy(text_tools_wasm(), pkg.join("plugin.wasm")).unwrap();
    let project = base.join("app");
    std::fs::create_dir_all(&project).unwrap();
    let mut ed = editor(&project);

    feed(&mut ed, &format!(":plugin add {}<CR>", pkg.display()));
    wait(&mut ed);
    assert!(
        ed.notifications()
            .iter()
            .any(|n| n.text == "installed text-tools 0.2.0"),
        "{:?}",
        ed.notifications()
    );
    assert!(
        project
            .join(".vox/plugins/text-tools/plugin.wasm")
            .is_file()
    );
    let l = lockfile(&project);
    let e = l.get("text-tools").unwrap();
    assert_eq!(
        e.source,
        Source::Path("../pkgs/text-tools".into()),
        "relative to the project"
    );
    assert!(e.checksum.starts_with("sha256:") && e.checksum.len() == 7 + 64);
    feed(&mut ed, ":wordcount<CR>");
    assert_eq!(
        ed.message(),
        Some("3 words, 1 lines, 13 characters"),
        "loaded right away"
    );

    feed(&mut ed, ":plugin remove! text-tools<CR>");
    assert!(!project.join(".vox/plugins/text-tools").exists());
    assert!(lockfile(&project).get("text-tools").is_none());
    assert!(ed.commands().get("wordcount").is_none());
    feed(&mut ed, ":plugin remove! text-tools<CR>");
    assert!(ed.message().unwrap().contains("not installed"));
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn add_from_git_tag_and_sync_after_deleting() {
    let base = temp("git");
    // A plugin project made from the template, in a git repository with a tag.
    let sdk = format!(
        "path = \"{}\"",
        root().join("crates/vox-plugin-sdk").display()
    );
    vox_editor::plugin::template::create(&base, "hello-git", &sdk, "0.1.0").unwrap();
    let repo = base.join("hello-git");
    let git = |args: &[&str]| {
        let ok = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            ok.status.success(),
            "{}",
            String::from_utf8_lossy(&ok.stderr)
        );
        String::from_utf8_lossy(&ok.stdout).trim().to_string()
    };
    git(&["init", "--quiet"]);
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "first"]);
    git(&["tag", "v0.1.0"]);
    let rev = git(&["rev-parse", "HEAD"]);

    let project = base.join("app");
    std::fs::create_dir_all(&project).unwrap();
    let mut ed = editor(&project);
    let url = format!("file://{}", repo.display());
    feed(&mut ed, &format!(":plugin add {url}@v0.1.0<CR>"));
    wait(&mut ed);
    assert!(
        ed.notifications()
            .iter()
            .any(|n| n.text == "installed hello-git 0.1.0"),
        "{:?}",
        ed.notifications()
    );
    let e = lockfile(&project).get("hello-git").cloned().unwrap();
    assert_eq!(
        e.source,
        Source::Git {
            url: url.clone(),
            reference: Some("v0.1.0".into()),
            rev: rev.clone()
        }
    );
    feed(&mut ed, ":hello-git<CR>");
    assert_eq!(ed.message(), Some("hello"));

    // Lose the installed copy (like a fresh clone of the project), then sync.
    let plugin_dir = project.join(".vox/plugins");
    std::fs::remove_dir_all(plugin_dir.join("hello-git")).unwrap();
    let states = install::check_lock(&lockfile(&project), &plugin_dir);
    assert_eq!(states, [("hello-git".to_string(), LockState::Missing)]);
    feed(&mut ed, ":plugins sync<CR>");
    wait(&mut ed);
    let notes: Vec<&str> = ed.notifications().iter().map(|n| n.text.as_str()).collect();
    assert!(notes.contains(&"synced 1 of 1 plugin(s)"), "{notes:?}");
    assert!(
        !notes.iter().any(|n| n.contains("differs from vox.lock")),
        "{notes:?}"
    );
    let states = install::check_lock(&lockfile(&project), &plugin_dir);
    assert_eq!(
        states,
        [("hello-git".to_string(), LockState::Ok)],
        "same build, same checksum"
    );
    feed(&mut ed, ":plugins sync<CR>");
    assert_eq!(last(&ed), "all 1 locked plugin(s) are installed");
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn add_errors() {
    let base = temp("errors");
    let mut ed = editor(&base.join("app"));
    feed(
        &mut ed,
        &format!(":plugin add {}<CR>", base.join("nothing").display()),
    );
    wait(&mut ed);
    assert!(last(&ed).starts_with("install failed:"), "{}", last(&ed));
    std::fs::create_dir_all(base.join("not-a-plugin")).unwrap();
    feed(
        &mut ed,
        &format!(":plugin add {}<CR>", base.join("not-a-plugin").display()),
    );
    wait(&mut ed);
    assert!(last(&ed).contains("no plugin.toml"), "{}", last(&ed));
    ed.set_scope(Scope::vanilla(), None);
    feed(&mut ed, ":plugin add x<CR>");
    assert!(ed.message().unwrap().contains("vanilla"));
    std::fs::remove_dir_all(&base).unwrap();
}
