//! Commands that change the installed plugins of the active scope, keeping `vox.lock` up to
//! date: `:plugin add SOURCE`, `:plugin remove NAME`, `:plugins sync`. Installing runs as a
//! background job.

use std::path::{Path, PathBuf};

use super::install::{self, InstallEnv, LockState, SourceSpec};
use super::lock::{self, LockEntry};
use crate::Editor;
use crate::command::{Args, CommandResult, Commands};
use crate::config::{ValueFormatter, ValueParser};
use crate::ui::Level;

/// Register the commands.
pub(crate) fn register(c: &mut Commands) {
    c.register(
        "plugin add",
        "Install a plugin into the active scope: `:plugin add PATH` or `:plugin add GIT_URL[@REF]`.",
        add,
    );
    c.register(
        "plugin remove",
        "Uninstall a plugin from the active scope: `:plugin remove NAME`.",
        remove,
    );
    c.register(
        "plugins sync",
        "Install exactly what vox.lock lists (missing or changed plugins).",
        sync,
    );
}

/// Everything a job needs, owned (jobs run on another thread).
struct Ctx {
    plugin_dir: PathBuf,
    lock_path: PathBuf,
    work_dir: PathBuf,
    project_root: Option<PathBuf>,
    parse: ValueParser,
    format: ValueFormatter,
}

impl Ctx {
    /// The installer's view of these paths.
    fn env(&self) -> InstallEnv<'_> {
        InstallEnv {
            plugin_dir: &self.plugin_dir,
            project_root: self.project_root.as_deref(),
            work_dir: &self.work_dir,
            parse: self.parse,
        }
    }
}

/// The active scope's paths, or why plugins cannot be managed.
fn ctx(ed: &Editor) -> Result<Ctx, String> {
    let scope = ed.scope();
    let (Some(dir), Some(plugin_dir), Some(lock_path)) =
        (scope.dir.clone(), scope.plugin_dir(), scope.lock_path())
    else {
        return Err("E900: no plugins in vanilla mode".into());
    };
    let (parse, format) = ed
        .file_format()
        .ok_or("E900: plugin files cannot be read here")?;
    Ok(Ctx {
        plugin_dir,
        lock_path,
        work_dir: dir.join(".build"),
        project_root: scope.project_root().map(Path::to_path_buf),
        parse,
        format,
    })
}

/// Load (or reload) plugin `name` if the plugin host is present.
fn load(ed: &mut Editor, name: Option<&str>) {
    if ed.commands().get("reload plugins").is_some() {
        match name {
            Some(n) => ed.run_command(&format!("reload plugins {n}")),
            None => ed.run_command("reload plugins"),
        }
    }
}

/// `:plugin add SOURCE`.
fn add(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let source = a
        .arg
        .ok_or("E471: Use :plugin add PATH or :plugin add GIT_URL[@REF]")?;
    let ctx = ctx(ed)?;
    let cwd = std::env::current_dir().unwrap_or_default();
    let spec = install::parse_source(source, &cwd);
    let label = match &spec {
        SourceSpec::Path(p) => p.display().to_string(),
        SourceSpec::Git { url, .. } => url.clone(),
    };
    ed.log(Level::Info, "editor", format!("installing from {label}"));
    ed.spawn_job(&format!("installing {source}"), move |progress| {
        std::fs::create_dir_all(&ctx.plugin_dir).ok();
        let result = install::install(&spec, &ctx.env(), progress);
        Box::new(move |ed: &mut Editor| match result {
            Ok(entry) => match record(&ctx, std::slice::from_ref(&entry)) {
                Ok(()) => {
                    ed.notify(
                        Level::Info,
                        format!("installed {} {}", entry.name, entry.version),
                    );
                    ed.log(
                        Level::Info,
                        "editor",
                        format!(
                            "installed {} {} ({})",
                            entry.name, entry.version, entry.checksum
                        ),
                    );
                    load(ed, Some(&entry.name));
                }
                Err(e) => ed.notify(
                    Level::Error,
                    format!("installed {}, but vox.lock: {e}", entry.name),
                ),
            },
            Err(e) => {
                ed.log(Level::Error, "editor", format!("install failed: {e}"));
                ed.notify(Level::Error, format!("install failed: {e}"));
            }
        })
    });
    Ok(())
}

/// Put `entries` into the lockfile.
fn record(ctx: &Ctx, entries: &[LockEntry]) -> Result<(), String> {
    let mut l = lock::read(&ctx.lock_path, ctx.parse)?;
    for e in entries {
        l.upsert(e.clone());
    }
    lock::write(&ctx.lock_path, &l, ctx.format)
}

/// `:plugin remove NAME`.
fn remove(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let name = a.arg.ok_or("E471: Use :plugin remove NAME")?;
    let ctx = ctx(ed)?;
    install::uninstall(name, &ctx.plugin_dir).map_err(|e| format!("E900: {e}"))?;
    let mut l = lock::read(&ctx.lock_path, ctx.parse)?;
    if l.remove(name) {
        lock::write(&ctx.lock_path, &l, ctx.format)?;
    }
    ed.remove_owned(&crate::owner::plugin(name));
    ed.set_plugin_settings(name, Vec::new());
    ed.log(Level::Info, "editor", format!("removed plugin {name}"));
    load(ed, None);
    ed.notify(Level::Info, format!("removed {name}"));
    Ok(())
}

/// `:plugins sync`.
fn sync(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    let ctx = ctx(ed)?;
    let l = lock::read(&ctx.lock_path, ctx.parse)?;
    let states = install::check_lock(&l, &ctx.plugin_dir);
    for (name, state) in &states {
        if *state == LockState::NotLocked {
            ed.notify(
                Level::Info,
                format!("{name} is installed but not in vox.lock"),
            );
        }
    }
    let todo: Vec<LockEntry> = states
        .iter()
        .filter(|(_, s)| matches!(s, LockState::Missing | LockState::Changed))
        .filter_map(|(n, _)| l.get(n).cloned())
        .collect();
    if todo.is_empty() {
        ed.notify(
            Level::Info,
            format!("all {} locked plugin(s) are installed", l.plugins.len()),
        );
        return Ok(());
    }
    let count = todo.len();
    ed.spawn_job(&format!("syncing {count} plugin(s) with vox.lock"), move |progress| {
        std::fs::create_dir_all(&ctx.plugin_dir).ok();
        let results: Vec<(LockEntry, Result<LockEntry, String>)> = todo
            .into_iter()
            .map(|e| {
                let r = install::install_locked(&e, &ctx.env(), progress);
                (e, r)
            })
            .collect();
        Box::new(move |ed: &mut Editor| {
            let mut updated = Vec::new();
            let mut failed = 0;
            for (locked, result) in results {
                match result {
                    Ok(got) => {
                        if got.checksum != locked.checksum {
                            ed.notify(
                                Level::Warn,
                                format!("{}: rebuilt, but it differs from vox.lock (another Rust version?); lock updated", got.name),
                            );
                            updated.push(got);
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        ed.notify(Level::Error, format!("{}: {e}", locked.name));
                    }
                }
            }
            if !updated.is_empty()
                && let Err(e) = record(&ctx, &updated)
            {
                ed.notify(Level::Error, format!("vox.lock: {e}"));
            }
            let ok = count - failed;
            ed.notify(
                if failed == 0 { Level::Info } else { Level::Warn },
                format!("synced {ok} of {count} plugin(s)"),
            );
            load(ed, None);
        })
    });
    Ok(())
}

/// Start-up check: a notification if the scope's plugins differ from its lockfile.
pub fn lock_status_notice(ed: &mut Editor) {
    let Ok(ctx) = ctx(ed) else { return };
    let Ok(l) = lock::read(&ctx.lock_path, ctx.parse) else {
        ed.notify(
            Level::Error,
            format!("{}: cannot read", ctx.lock_path.display()),
        );
        return;
    };
    let off = install::check_lock(&l, &ctx.plugin_dir)
        .into_iter()
        .filter(|(_, s)| matches!(s, LockState::Missing | LockState::Changed))
        .count();
    if off > 0 {
        ed.notify(
            Level::Warn,
            format!("{off} plugin(s) missing or changed — run :plugins sync"),
        );
    }
}
