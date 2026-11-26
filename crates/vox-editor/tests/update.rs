//! Updating vox: checking a git repository's release tags and offering to install.
//! Uses the `git` program with a local repository.

use std::path::{Path, PathBuf};
use std::process::Command;

use vox_core::Document;
use vox_editor::plugin::Version;
use vox_editor::update::UpdateSource;
use vox_editor::{Editor, parse_keys};

/// A git repository in a temp folder with the given tags.
fn repo(name: &str, tags: &[&str]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vox-update-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        let ok = Command::new("git")
            .arg("-C")
            .arg(&dir)
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
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    };
    git(&["init", "--quiet"]);
    git(&["commit", "--quiet", "--allow-empty", "-m", "x"]);
    for t in tags {
        git(&["tag", t]);
    }
    dir
}

fn editor(repo: &Path, current: &str) -> Editor {
    let mut ed = Editor::new(Document::from_text("x"));
    ed.resize(100, 30);
    ed.set_update_source(UpdateSource {
        repo: format!("file://{}", repo.display()),
        current: Version::parse(current).unwrap(),
        install_root: None,
    });
    ed
}

fn wait(ed: &mut Editor) {
    while ed.jobs_running() > 0 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        ed.tick();
    }
}

fn notes(ed: &Editor) -> Vec<String> {
    ed.history().iter().map(|(_, t)| t.clone()).collect()
}

#[test]
fn newer_release_asks_and_yes_installs() {
    let dir = repo("newer", &["v0.1.0", "v0.10.0", "v0.9.0", "nightly"]);
    let mut ed = editor(&dir, "0.9.0");
    ed.run_command("update check");
    wait(&mut ed);
    assert_eq!(ed.modal_title().as_deref(), Some("Update vox"));
    ed.resize(70, 12);
    insta::assert_snapshot!(ed.render());
    // Yes runs `:update install 0.10.0`; this build cannot replace itself.
    for k in parse_keys("y") {
        ed.handle_key(k);
    }
    assert!(ed.modal_title().is_none());
    assert!(
        ed.message()
            .unwrap()
            .contains("not installed with cargo install")
    );
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn no_means_nothing_happens() {
    let dir = repo("no", &["v1.0.0"]);
    let mut ed = editor(&dir, "0.9.0");
    ed.run_command("update check");
    wait(&mut ed);
    for k in parse_keys("n") {
        ed.handle_key(k);
    }
    assert!(ed.modal_title().is_none());
    assert_eq!(ed.jobs_running(), 0);
    assert_eq!(ed.message(), None);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn up_to_date_and_start_up_check_is_quiet() {
    let dir = repo("same", &["v0.9.0"]);
    let mut ed = editor(&dir, "0.9.0");
    vox_editor::update::check_on_start(&mut ed);
    wait(&mut ed);
    assert!(notes(&ed).is_empty(), "{:?}", notes(&ed));
    ed.run_command("update check");
    wait(&mut ed);
    assert_eq!(notes(&ed).last().unwrap(), "vox 0.9.0 is up to date");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn start_up_check_can_be_turned_off() {
    let dir = repo("off", &["v9.0.0"]);
    let mut ed = editor(&dir, "0.9.0");
    ed.execute("settings set check_updates false").unwrap();
    vox_editor::update::check_on_start(&mut ed);
    assert_eq!(ed.jobs_running(), 0);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn without_a_person_or_with_a_dialog_open_it_only_notifies() {
    let dir = repo("headless", &["v1.0.0"]);
    let mut ed = editor(&dir, "0.9.0");
    ed.set_interactive(false);
    ed.execute("update check").unwrap();
    wait(&mut ed);
    assert!(ed.modal_title().is_none());
    assert_eq!(
        notes(&ed).last().unwrap(),
        "vox 1.0.0 is available (you have 0.9.0) — :update install"
    );
    let mut ed = editor(&dir, "0.9.0");
    ed.run_command("commands");
    vox_editor::update::check_on_start(&mut ed);
    wait(&mut ed);
    assert_eq!(ed.modal_title().as_deref(), Some("Commands"), "dialog kept");
    assert!(notes(&ed).last().unwrap().ends_with(":update install"));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn errors() {
    let mut ed = Editor::new(Document::from_text("x"));
    assert!(
        ed.execute("update check")
            .unwrap_err()
            .contains("not available")
    );
    let missing = std::env::temp_dir().join("vox-update-missing-repo");
    let mut ed = editor(&missing, "0.9.0");
    ed.run_command("update check");
    wait(&mut ed);
    assert!(
        notes(&ed)
            .last()
            .unwrap()
            .starts_with("update check failed")
    );
    assert!(
        ed.execute("update install x")
            .unwrap_err()
            .contains("not installed with cargo")
    );
    ed.set_update_source(UpdateSource {
        install_root: Some("/nowhere".into()),
        ..ed.update_source().unwrap().clone()
    });
    assert!(
        ed.execute("update install 1.x")
            .unwrap_err()
            .starts_with("E474")
    );
}
