//! Installing plugins from a folder or a git repository, and checking installed plugins
//! against the lockfile. Uses the `git` and `cargo` programs (std `process`), no libraries.
//!
//! A source is either a packaged plugin (`plugin.toml` + `plugin.wasm`), copied as is, or a
//! plugin project (`plugin.toml` + `Cargo.toml`), built for `wasm32-wasip2` first. The result
//! is installed as `<plugin folder>/NAME/{plugin.toml, plugin.wasm, README.md}`.

use std::path::{Component, Path, PathBuf};
use std::process::Command;

use super::health::program_on_path;
use super::lock::{LockEntry, Lockfile, Source};
use super::manifest::PluginManifest;
use super::sha256;
use crate::config::{Value, ValueParser};

/// What the user asked to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceSpec {
    /// A folder (packaged plugin or plugin project).
    Path(PathBuf),
    /// A git repository, optionally at a tag, branch, or commit.
    Git {
        /// Repository URL.
        url: String,
        /// Tag, branch, or commit (`URL@REF`).
        reference: Option<String>,
    },
}

/// Read a source as typed: a git URL (`https://`, `http://`, `ssh://`, `git@`, `file://`,
/// or ending in `.git`), optionally followed by `@REF`, else a path relative to `cwd`.
pub fn parse_source(s: &str, cwd: &Path) -> SourceSpec {
    let s = s.trim();
    let is_git = ["https://", "http://", "ssh://", "git@", "file://"]
        .iter()
        .any(|p| s.starts_with(p))
        || s.trim_end_matches('/').ends_with(".git")
        || s.contains(".git@");
    if !is_git {
        return SourceSpec::Path(cwd.join(s));
    }
    // `@REF` only after the last `/`, so `git@host:...` stays intact.
    let last_slash = s.rfind('/').unwrap_or(0);
    match s[last_slash..].rfind('@') {
        Some(at) if last_slash + at > s.find(':').unwrap_or(0) => {
            let at = last_slash + at;
            SourceSpec::Git {
                url: s[..at].to_string(),
                reference: Some(s[at + 1..].to_string()),
            }
        }
        _ => SourceSpec::Git {
            url: s.to_string(),
            reference: None,
        },
    }
}

/// Where installing happens.
pub struct InstallEnv<'a> {
    /// The scope's plugin folder.
    pub plugin_dir: &'a Path,
    /// The project folder (local scope), to keep path sources relative to it.
    pub project_root: Option<&'a Path>,
    /// Scratch folder for git checkouts and build output.
    pub work_dir: &'a Path,
    /// Reads `plugin.toml` and `Cargo.toml`.
    pub parse: ValueParser,
}

/// Install from `spec`. `progress` gets a short line per step. Returns the lock entry.
pub fn install(
    spec: &SourceSpec,
    env: &InstallEnv<'_>,
    progress: &dyn Fn(&str),
) -> Result<LockEntry, String> {
    match spec {
        SourceSpec::Path(dir) => {
            let dir = dir
                .canonicalize()
                .map_err(|e| format!("{}: {e}", dir.display()))?;
            let source = Source::Path(stored_path(&dir, env.project_root));
            install_dir(&dir, source, env, progress)
        }
        SourceSpec::Git { url, reference } => {
            let checkout = clone(url, reference.as_deref(), env, progress)?;
            let rev = git(&checkout, &["rev-parse", "HEAD"])?;
            let source = Source::Git {
                url: url.clone(),
                reference: reference.clone(),
                rev,
            };
            install_dir(&checkout, source, env, progress)
        }
    }
}

/// Install exactly what `entry` says (git at its commit, or its path).
pub fn install_locked(
    entry: &LockEntry,
    env: &InstallEnv<'_>,
    progress: &dyn Fn(&str),
) -> Result<LockEntry, String> {
    match &entry.source {
        Source::Path(p) => {
            let dir = match env.project_root {
                Some(root) if Path::new(p).is_relative() => root.join(p),
                _ => PathBuf::from(p),
            };
            let dir = dir
                .canonicalize()
                .map_err(|e| format!("{}: {e}", dir.display()))?;
            install_dir(&dir, entry.source.clone(), env, progress)
        }
        Source::Git { url, rev, .. } => {
            let checkout = clone(url, Some(rev), env, progress)?;
            install_dir(&checkout, entry.source.clone(), env, progress)
        }
    }
}

/// Clone `url` into the work folder and check out `reference` (if given).
fn clone(
    url: &str,
    reference: Option<&str>,
    env: &InstallEnv<'_>,
    progress: &dyn Fn(&str),
) -> Result<PathBuf, String> {
    if !program_on_path("git") {
        return Err("installing from git needs the `git` program on PATH".into());
    }
    let name: String = url
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let dir = env.work_dir.join("src").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.parent().expect("has parent")).map_err(|e| e.to_string())?;
    progress(&format!("cloning {url}"));
    run(Command::new("git")
        .args(["clone", "--quiet", url])
        .arg(&dir))?;
    if let Some(r) = reference {
        progress(&format!("checking out {r}"));
        git(&dir, &["checkout", "--quiet", r])?;
    }
    Ok(dir)
}

/// Install from a folder that is a packaged plugin or a plugin project.
fn install_dir(
    dir: &Path,
    source: Source,
    env: &InstallEnv<'_>,
    progress: &dyn Fn(&str),
) -> Result<LockEntry, String> {
    let manifest_text = std::fs::read_to_string(dir.join("plugin.toml"))
        .map_err(|_| format!("{}: no plugin.toml (not a vox plugin)", dir.display()))?;
    let manifest = (env.parse)(&manifest_text)
        .map_err(|e| vec![e])
        .and_then(|v| PluginManifest::from_value(&v))
        .map_err(|errors| format!("plugin.toml: {}", errors.join("; ")))?;
    let wasm = if dir.join("plugin.wasm").is_file() {
        dir.join("plugin.wasm")
    } else if dir.join("Cargo.toml").is_file() {
        progress(&format!("building {} {}", manifest.name, manifest.version));
        build(dir, env)?
    } else {
        return Err(format!(
            "{}: neither plugin.wasm nor Cargo.toml found",
            dir.display()
        ));
    };
    progress(&format!("installing {}", manifest.name));
    let dest = env.plugin_dir.join(&manifest.name);
    let tmp = env
        .plugin_dir
        .join(format!(".{}.installing", manifest.name));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let copy = |from: &Path, to: &str| {
        std::fs::copy(from, tmp.join(to))
            .map(|_| ())
            .map_err(|e| e.to_string())
    };
    copy(&dir.join("plugin.toml"), "plugin.toml")?;
    copy(&wasm, "plugin.wasm")?;
    if dir.join("README.md").is_file() {
        copy(&dir.join("README.md"), "README.md")?;
    }
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    let checksum = installed_checksum(env.plugin_dir, &manifest.name)
        .ok_or("installed plugin.wasm missing")?;
    Ok(LockEntry {
        name: manifest.name,
        version: manifest.version.to_string(),
        source,
        checksum,
    })
}

/// Build a plugin project; returns the built `.wasm`.
fn build(dir: &Path, env: &InstallEnv<'_>) -> Result<PathBuf, String> {
    if !program_on_path("cargo") {
        return Err(
            "building a plugin needs `cargo` (and `rustup target add wasm32-wasip2`)".into(),
        );
    }
    let cargo_toml = std::fs::read_to_string(dir.join("Cargo.toml")).map_err(|e| e.to_string())?;
    let crate_name = match (env.parse)(&cargo_toml)? {
        Value::Table(t) => match t.get("package") {
            Some(Value::Table(p)) => match p.get("name") {
                Some(Value::String(n)) => n.clone(),
                _ => return Err("Cargo.toml: [package] has no name".into()),
            },
            _ => return Err("Cargo.toml: no [package]".into()),
        },
        _ => return Err("Cargo.toml: not a table".into()),
    };
    let target = env.work_dir.join("target");
    run(Command::new("cargo")
        .args([
            "build",
            "--quiet",
            "--release",
            "--target",
            "wasm32-wasip2",
            "--manifest-path",
        ])
        .arg(dir.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target))?;
    let wasm = target
        .join("wasm32-wasip2/release")
        .join(format!("{}.wasm", crate_name.replace('-', "_")));
    if wasm.is_file() {
        Ok(wasm)
    } else {
        Err(format!("build finished but {} is missing", wasm.display()))
    }
}

/// Remove an installed plugin's folder (or single `.wasm` file).
pub fn uninstall(name: &str, plugin_dir: &Path) -> Result<(), String> {
    let dir = plugin_dir.join(name);
    let file = plugin_dir.join(format!("{name}.wasm"));
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())
    } else if file.is_file() {
        std::fs::remove_file(&file).map_err(|e| e.to_string())
    } else {
        Err(format!("plugin {name} is not installed"))
    }
}

/// `sha256:HEX` of an installed package's `plugin.wasm`.
pub fn installed_checksum(plugin_dir: &Path, name: &str) -> Option<String> {
    let bytes = std::fs::read(plugin_dir.join(name).join("plugin.wasm")).ok()?;
    Some(format!("sha256:{}", sha256::hex(&bytes)))
}

/// How an installed plugin compares to the lockfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockState {
    /// Installed and identical to the lock.
    Ok,
    /// In the lock, not installed.
    Missing,
    /// Installed, but `plugin.wasm` differs from the lock.
    Changed,
    /// Installed, not in the lock.
    NotLocked,
}

/// Compare installed packages in `plugin_dir` with `lock`, by name.
pub fn check_lock(lock: &Lockfile, plugin_dir: &Path) -> Vec<(String, LockState)> {
    let mut out: Vec<(String, LockState)> = lock
        .plugins
        .iter()
        .map(|e| {
            let state = match installed_checksum(plugin_dir, &e.name) {
                None => LockState::Missing,
                Some(c) if c == e.checksum => LockState::Ok,
                Some(_) => LockState::Changed,
            };
            (e.name.clone(), state)
        })
        .collect();
    for name in crate::scope::list_plugins(plugin_dir) {
        if lock.get(&name).is_none() && !name.starts_with('.') {
            out.push((name, LockState::NotLocked));
        }
    }
    out
}

/// Path to store in the lock: relative to the project when the plugin is in or near it
/// (at most two `..`, e.g. `../my-plugin` in the same repository), else absolute.
fn stored_path(dir: &Path, root: Option<&Path>) -> String {
    let Some(root) = root.and_then(|r| r.canonicalize().ok()) else {
        return dir.display().to_string();
    };
    let rel = relative(dir, &root);
    let ups = rel
        .components()
        .take_while(|c| matches!(c, Component::ParentDir))
        .count();
    if ups <= 2 {
        rel.display().to_string()
    } else {
        dir.display().to_string()
    }
}

/// `path` relative to `base` (both absolute), using `..` where needed.
fn relative(path: &Path, base: &Path) -> PathBuf {
    let p: Vec<Component> = path.components().collect();
    let b: Vec<Component> = base.components().collect();
    let common = p.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut out = PathBuf::new();
    for _ in common..b.len() {
        out.push("..");
    }
    for c in &p[common..] {
        out.push(c.as_os_str());
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/// Run `git` in `dir`; returns trimmed stdout.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    run(Command::new("git").arg("-C").arg(dir).args(args))
}

/// Run a command; returns trimmed stdout, or an error with the end of stderr.
fn run(cmd: &mut Command) -> Result<String, String> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let out = cmd.output().map_err(|e| format!("{program}: {e}"))?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).trim().to_string());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let tail: Vec<&str> = err
        .lines()
        .filter(|l| !l.trim().is_empty())
        .rev()
        .take(3)
        .collect();
    let tail: Vec<&str> = tail.into_iter().rev().collect();
    Err(format!("{program} failed: {}", tail.join(" / ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources() {
        let cwd = Path::new("/work");
        assert_eq!(
            parse_source("../p", cwd),
            SourceSpec::Path("/work/../p".into())
        );
        assert_eq!(
            parse_source("https://x.org/a/text-tools.git@v0.2.0", cwd),
            SourceSpec::Git {
                url: "https://x.org/a/text-tools.git".into(),
                reference: Some("v0.2.0".into())
            }
        );
        assert_eq!(
            parse_source("git@github.com:me/tool.git", cwd),
            SourceSpec::Git {
                url: "git@github.com:me/tool.git".into(),
                reference: None
            }
        );
        assert_eq!(
            parse_source("git@github.com:me/tool.git@main", cwd),
            SourceSpec::Git {
                url: "git@github.com:me/tool.git".into(),
                reference: Some("main".into())
            }
        );
        assert_eq!(
            parse_source("file:///tmp/repo", cwd),
            SourceSpec::Git {
                url: "file:///tmp/repo".into(),
                reference: None
            }
        );
    }

    #[test]
    fn stored_paths() {
        let base = std::env::temp_dir().join(format!("vox-stored-{}", std::process::id()));
        let app = base.join("repo/app");
        std::fs::create_dir_all(&app).unwrap();
        let near = base.join("repo/my-plugin");
        assert_eq!(stored_path(&near, Some(&app)), "../my-plugin");
        assert_eq!(
            stored_path(Path::new("/opt/far/away/plugin"), Some(&app)),
            "/opt/far/away/plugin"
        );
        assert_eq!(stored_path(&near, None), near.display().to_string());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn relative_paths() {
        assert_eq!(
            relative(Path::new("/a/b/c"), Path::new("/a/b")),
            PathBuf::from("c")
        );
        assert_eq!(
            relative(Path::new("/a/x"), Path::new("/a/b")),
            PathBuf::from("../x")
        );
        assert_eq!(
            relative(Path::new("/a/b"), Path::new("/a/b")),
            PathBuf::from(".")
        );
    }
}
