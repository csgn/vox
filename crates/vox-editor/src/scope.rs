//! Scopes: where config and plugins come from.
//!
//! | Scope | Config | Plugins |
//! |---|---|---|
//! | global (default) | `~/.config/vox/config.toml` | `~/.config/vox/plugins/` |
//! | local (`--local-config`) | `<project>/.vox/config.toml` | `<project>/.vox/plugins/` |
//! | vanilla (`--vanilla`) | none | none |
//!
//! Exactly one scope is active; nothing is mixed.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Name of a project's vox folder.
pub const VOX_DIR: &str = ".vox";

/// Which kind of scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// The user's own setup (`~/.config/vox`).
    Global,
    /// A project's own setup (`<project>/.vox`).
    Local,
    /// No config, no plugins.
    Vanilla,
}

impl ScopeKind {
    /// Lowercase name: `global`, `local`, `vanilla`.
    pub fn name(self) -> &'static str {
        match self {
            ScopeKind::Global => "global",
            ScopeKind::Local => "local",
            ScopeKind::Vanilla => "vanilla",
        }
    }
}

/// The active setup and its paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    /// Kind.
    pub kind: ScopeKind,
    /// Folder holding config, plugins, and lockfile (`~/.config/vox` or `<project>/.vox`).
    pub dir: Option<PathBuf>,
}

impl Scope {
    /// The global scope rooted at `config_dir` (usually `~/.config/vox`).
    pub fn global(config_dir: PathBuf) -> Self {
        Self {
            kind: ScopeKind::Global,
            dir: Some(config_dir),
        }
    }

    /// The local scope of the project at `project_root` (uses `<root>/.vox`).
    pub fn local(project_root: &Path) -> Self {
        Self {
            kind: ScopeKind::Local,
            dir: Some(project_root.join(VOX_DIR)),
        }
    }

    /// No config, no plugins.
    pub fn vanilla() -> Self {
        Self {
            kind: ScopeKind::Vanilla,
            dir: None,
        }
    }

    /// `config.toml` path.
    pub fn config_path(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join("config.toml"))
    }

    /// Plugin folder path.
    pub fn plugin_dir(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join("plugins"))
    }

    /// Lockfile path (used from M9 on).
    pub fn lock_path(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join("vox.lock"))
    }

    /// The project folder of a local scope (the parent of `.vox`).
    pub fn project_root(&self) -> Option<&Path> {
        match self.kind {
            ScopeKind::Local => self.dir.as_deref().and_then(Path::parent),
            _ => None,
        }
    }

    /// Whether the scope's folder exists (a local scope that was never set up does not).
    pub fn exists(&self) -> bool {
        self.dir.as_ref().is_some_and(|d| d.is_dir())
    }
}

/// The nearest folder at or above `start` that contains `.vox/`.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(VOX_DIR).is_dir())
        .map(Path::to_path_buf)
}

/// Plugins installed in `dir`: `NAME.wasm` files and `NAME/` folders, sorted by name.
pub fn list_plugins(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let p = e.path();
            if p.is_dir() {
                p.file_name().map(|n| n.to_string_lossy().into_owned())
            } else if p.extension().is_some_and(|x| x == "wasm") {
                p.file_stem().map(|n| n.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

/// What to copy when setting up a local scope from the global one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SetupChoice {
    /// Copy the global `config.toml`.
    pub config: bool,
    /// Copy these global plugins (names from [`list_plugins`]).
    pub plugins: Vec<String>,
}

/// Create `local`'s folder (with `plugins/`) and copy what `choice` asks for from `global`.
/// Existing files in the project are not overwritten. Returns one line per action.
pub fn setup_local(global: &Scope, local: &Scope, choice: &SetupChoice) -> io::Result<Vec<String>> {
    let (Some(vox_dir), Some(plugin_dir)) = (&local.dir, local.plugin_dir()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a local scope",
        ));
    };
    fs::create_dir_all(vox_dir)?;
    // Installed plugins and build files are rebuilt from vox.lock (`:plugins sync`); commit
    // config.toml and vox.lock.
    let gitignore = vox_dir.join(".gitignore");
    if !gitignore.exists() {
        fs::write(
            &gitignore,
            "# Rebuilt from vox.lock with :plugins sync\nplugins/\n.build/\n",
        )?;
    }
    fs::create_dir_all(&plugin_dir)?;
    let mut report = Vec::new();
    if choice.config {
        let (from, to) = (global.config_path(), local.config_path());
        match (from, to) {
            (Some(from), Some(to)) if from.is_file() => {
                if copy_new(&from, &to)? {
                    report.push("copied config.toml".to_string());
                } else {
                    report.push("kept existing config.toml".to_string());
                }
            }
            _ => report.push("no global config.toml to copy".to_string()),
        }
    }
    let global_plugins = global.plugin_dir().unwrap_or_default();
    for name in &choice.plugins {
        let file = global_plugins.join(format!("{name}.wasm"));
        let folder = global_plugins.join(name);
        let copied = if file.is_file() {
            copy_new(&file, &plugin_dir.join(format!("{name}.wasm")))?
        } else if folder.is_dir() {
            copy_dir_new(&folder, &plugin_dir.join(name))?
        } else {
            report.push(format!("global plugin {name} not found"));
            continue;
        };
        report.push(if copied {
            format!("copied plugin {name}")
        } else {
            format!("kept existing plugin {name}")
        });
    }
    Ok(report)
}

/// Copy `from` to `to` unless `to` exists. Returns whether it copied.
fn copy_new(from: &Path, to: &Path) -> io::Result<bool> {
    if to.exists() {
        return Ok(false);
    }
    fs::copy(from, to)?;
    Ok(true)
}

/// Copy folder `from` to `to` recursively unless `to` exists. Returns whether it copied.
fn copy_dir_new(from: &Path, to: &Path) -> io::Result<bool> {
    if to.exists() {
        return Ok(false);
    }
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir_new(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("vox-scope-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn paths() {
        let g = Scope::global("/home/u/.config/vox".into());
        assert_eq!(
            g.plugin_dir().unwrap(),
            PathBuf::from("/home/u/.config/vox/plugins")
        );
        let l = Scope::local(Path::new("/work/app"));
        assert_eq!(
            l.config_path().unwrap(),
            PathBuf::from("/work/app/.vox/config.toml")
        );
        assert_eq!(
            l.lock_path().unwrap(),
            PathBuf::from("/work/app/.vox/vox.lock")
        );
        assert_eq!(l.project_root(), Some(Path::new("/work/app")));
        let v = Scope::vanilla();
        assert_eq!(
            (v.config_path(), v.plugin_dir(), v.exists()),
            (None, None, false)
        );
    }

    #[test]
    fn finds_project_root_upward() {
        let root = temp("find");
        fs::create_dir_all(root.join("app/.vox")).unwrap();
        fs::create_dir_all(root.join("app/src/deep")).unwrap();
        assert_eq!(
            find_project_root(&root.join("app/src/deep")),
            Some(root.join("app"))
        );
        assert_eq!(find_project_root(&root.join("app")), Some(root.join("app")));
        assert_eq!(find_project_root(&root), None);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn setup_copies_chosen_things_without_overwriting() {
        let root = temp("setup");
        let global = Scope::global(root.join("global"));
        fs::create_dir_all(global.plugin_dir().unwrap().join("folder-plugin/sub")).unwrap();
        fs::write(global.config_path().unwrap(), "leader = \" \"\n").unwrap();
        fs::write(global.plugin_dir().unwrap().join("a.wasm"), "A").unwrap();
        fs::write(global.plugin_dir().unwrap().join("b.wasm"), "B").unwrap();
        fs::write(
            global.plugin_dir().unwrap().join("folder-plugin/sub/x"),
            "X",
        )
        .unwrap();
        fs::write(global.plugin_dir().unwrap().join("notes.txt"), "").unwrap();
        assert_eq!(
            list_plugins(&global.plugin_dir().unwrap()),
            ["a", "b", "folder-plugin"]
        );

        let local = Scope::local(&root.join("project"));
        assert!(!local.exists());
        let choice = SetupChoice {
            config: true,
            plugins: vec!["a".into(), "folder-plugin".into(), "missing".into()],
        };
        let report = setup_local(&global, &local, &choice).unwrap();
        assert!(local.exists());
        assert_eq!(
            report,
            [
                "copied config.toml",
                "copied plugin a",
                "copied plugin folder-plugin",
                "global plugin missing not found"
            ]
        );
        let lp = local.plugin_dir().unwrap();
        assert_eq!(list_plugins(&lp), ["a", "folder-plugin"]);
        assert_eq!(
            fs::read_to_string(lp.join("folder-plugin/sub/x")).unwrap(),
            "X"
        );

        fs::write(local.config_path().unwrap(), "# project").unwrap();
        let again = setup_local(
            &global,
            &local,
            &SetupChoice {
                config: true,
                plugins: vec!["a".into()],
            },
        )
        .unwrap();
        assert_eq!(
            again,
            ["kept existing config.toml", "kept existing plugin a"]
        );
        assert_eq!(
            fs::read_to_string(local.config_path().unwrap()).unwrap(),
            "# project"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn setup_empty_just_creates_folders() {
        let root = temp("empty");
        let report = setup_local(
            &Scope::global(root.join("g")),
            &Scope::local(&root.join("p")),
            &SetupChoice::default(),
        )
        .unwrap();
        assert!(report.is_empty());
        assert!(root.join("p/.vox/plugins").is_dir());
        let ignore = fs::read_to_string(root.join("p/.vox/.gitignore")).unwrap();
        assert!(ignore.contains("plugins/") && ignore.contains(".build/"));
        assert!(
            setup_local(
                &Scope::vanilla(),
                &Scope::vanilla(),
                &SetupChoice::default()
            )
            .is_err()
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
