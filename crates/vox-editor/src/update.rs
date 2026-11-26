//! Updating vox itself: find the newest `vX.Y.Z` tag of vox's git repository and install it
//! with `cargo install`. Uses the `git` and `cargo` programs (std `process`), no libraries.
//!
//! `:update check` looks for a newer version and asks whether to install it;
//! `:update install [VERSION]` installs one. The binary checks once at start-up
//! ([`check_on_start`]) unless `check_updates = false`.

use std::cell::Cell;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::Editor;
use crate::command::{Args, CommandResult, Commands};
use crate::plugin::Version;
use crate::plugin::install::run;
use crate::ui::Level;

/// Where updates come from and where they go; set by the binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateSource {
    /// Git URL of vox's repository.
    pub repo: String,
    /// The running version.
    pub current: Version,
    /// `cargo install --root` folder of the running binary (see [`install_root_of`]); `None`
    /// if vox was not installed with `cargo install`, so it cannot replace itself.
    pub install_root: Option<PathBuf>,
}

/// Register `:update check` and `:update install`.
pub(crate) fn register(c: &mut Commands) {
    c.register(
        "update check",
        "Look for a newer vox release and ask whether to install it.",
        |ed, _| {
            let src = source(ed)?;
            spawn_check(ed, src, false);
            Ok(())
        },
    );
    c.register(
        "update install",
        "Install a vox release with cargo: `:update install [VERSION]` (default: the newest).",
        install_cmd,
    );
}

/// The update source, or why updates are not available.
fn source(ed: &Editor) -> Result<UpdateSource, String> {
    ed.update_source()
        .cloned()
        .ok_or_else(|| "E900: updates are not available in this build".into())
}

/// The newest `vX.Y.Z` tag in `git ls-remote --tags --refs` output.
pub fn newest_tag(ls_remote: &str) -> Option<Version> {
    ls_remote
        .lines()
        .filter_map(|l| l.rsplit("refs/tags/").next())
        .filter_map(|tag| tag.trim().strip_prefix('v'))
        .filter_map(|v| Version::parse(v).ok())
        .max()
}

/// Ask `repo` for its newest release tag.
pub fn latest(repo: &str) -> Result<Option<Version>, String> {
    let out = run(Command::new("git").args(["ls-remote", "--tags", "--refs", repo, "v*"]))?;
    Ok(newest_tag(&out))
}

/// The `cargo install --root` folder of an installed binary: `ROOT/bin/vox` → `ROOT`.
/// `None` for anything else (e.g. `target/release/vox`), which must not be replaced.
pub fn install_root_of(exe: &Path) -> Option<PathBuf> {
    let bin = exe.parent()?;
    (bin.file_name()? == "bin").then(|| bin.parent().map(Path::to_path_buf))?
}

/// Arguments for `cargo` that install release `version` from `repo` into `root`.
pub fn install_args(repo: &str, version: Version, root: &Path) -> Vec<String> {
    let mut args: Vec<String> = ["install", "--locked", "--force", "--git", repo, "--tag"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    args.push(format!("v{version}"));
    args.push("--root".into());
    args.push(root.display().to_string());
    args.push("vox".into());
    args
}

/// Run `cmd` and pass each line it writes to stderr (where cargo reports progress) to
/// `on_line`. On failure the error ends with its last few lines.
pub fn run_lines(cmd: &mut Command, on_line: &dyn Fn(&str)) -> Result<(), String> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let mut child = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    let mut tail: Vec<String> = Vec::new();
    if let Some(err) = child.stderr.take() {
        for line in std::io::BufReader::new(err).lines().map_while(Result::ok) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            on_line(line);
            tail.push(line.to_string());
            if tail.len() > 3 {
                tail.remove(0);
            }
        }
    }
    let status = child.wait().map_err(|e| format!("{program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed: {}", tail.join(" / ")))
    }
}

/// Progress text for a line of cargo output: `Compiling NAME vX` lines are counted
/// (`compiling (12): NAME vX`); other lines are shown as they are.
pub fn cargo_progress(line: &str, compiled: &Cell<usize>) -> String {
    match line.strip_prefix("Compiling ") {
        Some(what) => {
            compiled.set(compiled.get() + 1);
            format!("compiling ({}): {what}", compiled.get())
        }
        None => line.to_string(),
    }
}

/// What a check found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// A version newer than the running and the installed one: offer it.
    Offer(Version),
    /// The newest version was installed since start; it runs after a restart.
    Restart(Version),
    /// Nothing newer.
    UpToDate,
}

/// Compare the `newest` release with the `running` version and the one `installed` since
/// start (by `:update install`).
pub fn decide(running: Version, installed: Option<Version>, newest: Option<Version>) -> Decision {
    let have = installed.map_or(running, |i| i.max(running));
    match newest {
        Some(n) if n > have => Decision::Offer(n),
        _ => match installed {
            Some(i) if i > running => Decision::Restart(i),
            _ => Decision::UpToDate,
        },
    }
}

/// Check in the background. `quiet` (start-up): no "checking" notification, and errors
/// and "up to date" only go to the log.
fn spawn_check(ed: &mut Editor, src: UpdateSource, quiet: bool) {
    let work = move |_: &dyn Fn(&str)| -> crate::JobDone {
        let result = latest(&src.repo);
        Box::new(move |ed: &mut Editor| match result {
            Ok(newest) => match decide(src.current, ed.update_installed(), newest) {
                Decision::Offer(v) => offer(ed, src.current, v),
                Decision::Restart(v) => {
                    let text = format!("vox {v} is installed — restart vox to use it");
                    ed.log(Level::Info, "update", &text);
                    ed.notify(Level::Info, text);
                }
                Decision::UpToDate => {
                    let text = format!("vox {} is up to date", src.current);
                    ed.log(Level::Info, "update", &text);
                    if !quiet {
                        ed.notify(Level::Info, text);
                    }
                }
            },
            Err(e) => {
                ed.log(Level::Warn, "update", format!("update check failed: {e}"));
                if !quiet {
                    ed.notify(Level::Error, format!("update check failed: {e}"));
                }
            }
        })
    };
    if quiet {
        ed.spawn_quiet_job(work);
    } else {
        ed.spawn_job("checking for updates", work);
    }
}

/// A newer version exists: ask to install it, or just say so if a dialog is already open
/// or nobody is at the keyboard.
fn offer(ed: &mut Editor, current: Version, newer: Version) {
    let text = format!("vox {newer} is available (you have {current})");
    ed.log(Level::Info, "update", &text);
    if ed.interactive() && ed.modal_title().is_none() {
        let ask = crate::ui::Confirm::new(
            "Update vox",
            format!("Install vox {newer}? You have {current}."),
            format!("update install {newer}"),
        );
        ed.open_modal(Box::new(ask));
    } else {
        ed.notify(Level::Info, format!("{text} — :update install"));
    }
}

/// `:update install [VERSION]`.
fn install_cmd(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let src = source(ed)?;
    let root = src.install_root.clone().ok_or(
        "E900: this vox was not installed with cargo install, so it cannot update itself; \
         use install.sh",
    )?;
    let wanted = a
        .arg
        .map(|v| Version::parse(v.trim_start_matches('v')))
        .transpose()
        .map_err(|e| format!("E474: {e}"))?;
    let title = match wanted {
        Some(v) => format!("installing vox {v}"),
        None => "installing the newest vox".into(),
    };
    ed.spawn_job(&title, move |progress| {
        let result = (|| {
            let version = match wanted {
                Some(v) => v,
                None => latest(&src.repo)?.ok_or("no release tags found")?,
            };
            progress(&format!(
                "building vox {version} with cargo (takes a few minutes)"
            ));
            let compiled = std::cell::Cell::new(0);
            run_lines(
                Command::new("cargo").args(install_args(&src.repo, version, &root)),
                &|line| progress(&cargo_progress(line, &compiled)),
            )?;
            Ok::<Version, String>(version)
        })();
        Box::new(move |ed: &mut Editor| match result {
            Ok(v) => {
                ed.set_update_installed(v);
                ed.log(Level::Info, "update", format!("installed vox {v}"));
                ed.notify(
                    Level::Info,
                    format!("installed vox {v} — restart vox to use it"),
                );
            }
            Err(e) => {
                ed.log(Level::Error, "update", format!("update failed: {e}"));
                ed.notify(Level::Error, format!("update failed: {e}"));
            }
        })
    });
    Ok(())
}

/// Start-up check (called by the binary): in the background, without notifications unless
/// a newer version exists. Does nothing if `check_updates = false` or no source is set.
pub fn check_on_start(ed: &mut Editor) {
    if !ed.config().check_updates {
        return;
    }
    if let Some(src) = ed.update_source().cloned() {
        spawn_check(ed, src, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn newest_tag_picks_highest_version() {
        let out = "abc\trefs/tags/v0.9.0\nabd\trefs/tags/v0.14.0\nabe\trefs/tags/v0.13.2\n\
                   abf\trefs/tags/vnext\nabg\trefs/tags/other";
        assert_eq!(newest_tag(out), Some(v("0.14.0")));
        assert_eq!(newest_tag(""), None);
    }

    #[test]
    fn install_root_only_for_cargo_installs() {
        assert_eq!(
            install_root_of(Path::new("/home/u/.cargo/bin/vox")),
            Some(PathBuf::from("/home/u/.cargo"))
        );
        assert_eq!(
            install_root_of(Path::new("/src/vox/target/release/vox")),
            None
        );
        assert_eq!(install_root_of(Path::new("vox")), None);
    }

    #[test]
    fn decide_after_install() {
        let (old, new, newer) = (v("0.16.1"), v("0.16.2"), v("0.17.0"));
        assert_eq!(decide(old, None, Some(new)), Decision::Offer(new));
        assert_eq!(decide(old, None, Some(old)), Decision::UpToDate);
        assert_eq!(decide(old, None, None), Decision::UpToDate);
        assert_eq!(decide(old, Some(new), Some(new)), Decision::Restart(new));
        assert_eq!(decide(old, Some(new), Some(newer)), Decision::Offer(newer));
        assert_eq!(
            decide(old, Some(old), Some(old)),
            Decision::UpToDate,
            "reinstalled same"
        );
    }

    #[test]
    fn cargo_lines_become_progress() {
        let n = Cell::new(0);
        assert_eq!(
            cargo_progress("Updating git repository `x`", &n),
            "Updating git repository `x`"
        );
        assert_eq!(
            cargo_progress("Compiling serde v1.0.0", &n),
            "compiling (1): serde v1.0.0"
        );
        assert_eq!(
            cargo_progress("Compiling vox v0.16.0 (/x)", &n),
            "compiling (2): vox v0.16.0 (/x)"
        );
    }

    #[test]
    fn run_lines_streams_stderr() {
        let seen = std::cell::RefCell::new(Vec::new());
        let ok = run_lines(
            Command::new("sh").args(["-c", "echo out; echo '  one ' >&2; echo >&2; echo two >&2"]),
            &|l| seen.borrow_mut().push(l.to_string()),
        );
        assert_eq!(ok, Ok(()));
        assert_eq!(*seen.borrow(), ["one", "two"]);
        let err = run_lines(
            Command::new("sh").args(["-c", "for i in 1 2 3 4 5; do echo l$i >&2; done; exit 3"]),
            &|_| {},
        );
        assert_eq!(err, Err("sh failed: l3 / l4 / l5".into()));
    }

    #[test]
    fn cargo_install_arguments() {
        assert_eq!(
            install_args("https://x/vox.git", v("1.2.3"), Path::new("/r")).join(" "),
            "install --locked --force --git https://x/vox.git --tag v1.2.3 --root /r vox"
        );
    }
}
