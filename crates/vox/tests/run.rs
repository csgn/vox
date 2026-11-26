//! `vox run`: commands without a terminal, using the real binary.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

/// An empty project folder with `.vox/`.
fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vox-run-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".vox")).unwrap();
    dir
}

fn vox(args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let input = stdin.unwrap_or("");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn out(o: &Output) -> (i32, String, String) {
    (
        o.status.code().unwrap(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn replies_as_text_and_json() {
    let (code, stdout, _) = out(&vox(&["run", "--vanilla", "scope show"], None));
    assert_eq!(
        (code, stdout.as_str()),
        (0, "exists: false\nkind: vanilla\n")
    );
    let (code, stdout, _) = out(&vox(
        &[
            "run",
            "--vanilla",
            "--json",
            "scope show",
            "notify hi",
            "settings get leader",
        ],
        None,
    ));
    assert_eq!(code, 0);
    assert_eq!(
        stdout,
        "{\"exists\":false,\"kind\":\"vanilla\"}\n\"\\\\\"\n"
    );
}

#[test]
fn settings_are_saved_to_the_scope() {
    let dir = project("settings");
    let d = dir.to_str().unwrap();
    let (code, stdout, stderr) = out(&vox(
        &[
            "run",
            "--local-config",
            d,
            "settings set ui.side_width 60",
            "settings get ui.side_width",
        ],
        None,
    ));
    assert_eq!((code, stdout.as_str()), (0, "60\n"), "{stderr}");
    assert!(stderr.contains("vox: info: ui.side_width = 60"), "{stderr}");
    let config = std::fs::read_to_string(dir.join(".vox/config.toml")).unwrap();
    assert!(config.contains("side_width = 60"), "{config}");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn first_failure_stops_with_status_1() {
    let (code, stdout, stderr) = out(&vox(&["run", "--vanilla", "nope", "scope show"], None));
    assert_eq!((code, stdout.as_str()), (1, ""));
    assert!(
        stderr.contains("vox: nope: E492: Not an editor command: nope"),
        "{stderr}"
    );
    let (code, _, stderr) = out(&vox(&["run", "--vanilla", "commands"], None));
    assert_eq!(code, 1);
    assert!(stderr.contains(":commands needs the editor UI"), "{stderr}");
    let dir = project("remove");
    std::fs::create_dir_all(dir.join(".vox/plugins/demo")).unwrap();
    let d = dir.to_str().unwrap();
    let (code, _, stderr) = out(&vox(
        &["run", "--local-config", d, "plugin remove demo"],
        None,
    ));
    assert_eq!(code, 1, "asks, so it fails");
    assert!(stderr.contains("use :plugin remove! demo"), "{stderr}");
    assert!(dir.join(".vox/plugins/demo").exists());
    // Installing runs as a job; vox run waits for it and fails if it fails.
    let missing = dir.join("no-such-plugin");
    let add = format!("plugin add {}", missing.display());
    let (code, _, stderr) = out(&vox(
        &["run", "--local-config", d, &add, "scope show"],
        None,
    ));
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("vox: error: install failed"), "{stderr}");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn commands_from_stdin() {
    let (code, stdout, _) = out(&vox(
        &["run", "--vanilla", "--json", "scope show", "-"],
        Some("settings get ui.side_width\n\n  \nplugin list\n"),
    ));
    assert_eq!(code, 0);
    assert_eq!(stdout, "{\"exists\":false,\"kind\":\"vanilla\"}\n40\n[]\n");
}

#[test]
fn file_is_opened_first() {
    let dir = project("file");
    std::fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let a = dir.join("a.txt");
    let b = dir.join("b.txt");
    let write = format!("write {}", b.display());
    let (code, _, stderr) = out(&vox(
        &["run", "--vanilla", "--file", a.to_str().unwrap(), &write],
        None,
    ));
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(std::fs::read_to_string(b).unwrap(), "hello\n");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn bad_arguments_exit_2() {
    let (code, _, stderr) = out(&vox(&["run"], None));
    assert_eq!(code, 2);
    assert!(stderr.contains("needs a COMMAND"));
    assert_eq!(out(&vox(&["run", "--nope", "x"], None)).0, 2);
    let (code, stdout, _) = out(&vox(&["run", "--help"], None));
    assert_eq!(code, 0);
    assert!(stdout.starts_with("vox run"));
}
