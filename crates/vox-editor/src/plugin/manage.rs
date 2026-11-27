//! Commands that change the installed plugins of the active scope, keeping `vox.lock` up to
//! date: `:plugin add SOURCE`, `:plugin remove NAME`, `:plugins sync`. Installing runs as a
//! background job.

use std::path::{Path, PathBuf};

use super::install::{self, InstallEnv, LockState, SourceSpec};
use super::lock::{self, LockEntry};
use crate::Editor;
use crate::command::{Args, CommandResult, Commands};
use crate::config::{Value, ValueFormatter, ValueParser};
use crate::ui::Level;

/// Register the commands.
pub(crate) fn register(c: &mut Commands) {
    use crate::command::category::PLUGINS;
    use crate::command::{Complete, Param};
    let plugin =
        || Param::required("NAME", "A plugin of the active scope.").complete(Complete::Plugin);
    c.register(
        "plugins",
        "Open the plugin manager (modal or side panel, see [ui] plugin_manager).",
        |ed, _| {
            open_manager(ed);
            Ok(())
        },
    )
    .category(PLUGINS);
    c.register("plugins help", "Keys of the plugin manager.", |ed, _| {
        let lines = super::manager::HELP
            .iter()
            .map(|(k, v)| (crate::surface::Style::default(), format!("{k:<10} {v}")))
            .collect();
        ed.open_modal(Box::new(crate::ui::TextView::new(
            "Plugin manager keys",
            lines,
        )));
        Ok(())
    })
    .category(PLUGINS);
    c.register(
        "plugin enable",
        "Turn a plugin on (removes `enabled = false` from config.toml) and load it.",
        |ed, a| set_enabled(ed, a, true),
    )
    .category(PLUGINS)
    .param(plugin());
    c.register(
        "plugin disable",
        "Turn a plugin off (`enabled = false` in config.toml) and unload it.",
        |ed, a| set_enabled(ed, a, false),
    )
    .category(PLUGINS)
    .param(plugin());
    c.register(
        "plugin files",
        "Open a plugin's plugin.toml: `:plugin files NAME`.",
        |ed, a| {
            let name = a.arg.ok_or("E471: Use :plugin files NAME")?;
            let info = ed
                .plugin_infos()
                .iter()
                .find(|i| i.name == name)
                .ok_or_else(|| format!("E900: no plugin named {name}"))?;
            let path = if info.path.is_dir() {
                info.path.join("plugin.toml")
            } else {
                info.path.clone()
            };
            ed.close_modal();
            ed.run_command(&format!("e {}", path.display()));
            Ok(())
        },
    )
    .category(PLUGINS)
    .param(plugin());
    c.register(
        "plugin list",
        "Return the active scope's plugins as data: name, version, status, path.",
        list,
    )
    .category(PLUGINS)
    .returns("A list, one table per plugin: name, status, path, version.");
    c.register(
        "plugin info",
        "Return one plugin as data: `:plugin info NAME` (manifest, settings, health).",
        info,
    )
    .category(PLUGINS)
    .param(plugin())
    .returns("A table: name, version, status, path, description, authors, settings, health.");
    c.register(
        "plugin add",
        "Install a plugin into the active scope: `:plugin add PATH` or `:plugin add GIT_URL[@REF]`.",
        add,
    )
    .category(PLUGINS)
    .param(
        Param::required("SOURCE", "A plugin folder, or a git URL with an optional `@REF` (tag, branch, commit).")
            .complete(Complete::Dir),
    )
    .example("plugin add ~/code/my-plugin", "install from a folder")
    .example("plugin add https://github.com/USER/REPO.git@v1.0.0", "install a tag from git");
    c.register(
        "plugin remove",
        "Uninstall a plugin from the active scope: asks first; `:plugin remove! NAME` does not.",
        remove,
    )
    .category(PLUGINS)
    .param(plugin())
    .force("Remove without asking.")
    .example("plugin remove! text-tools", "uninstall without a question");
    c.register(
        "plugins sync",
        "Install exactly what vox.lock lists (missing or changed plugins).",
        sync,
    )
    .category(PLUGINS);
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

/// `:plugin list`.
fn list(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    let list = ed.plugin_infos().iter().map(summary).collect();
    ed.reply(Value::Array(list));
    Ok(())
}

/// `:plugin info NAME`.
fn info(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let name = a.arg.ok_or("E471: Use :plugin info NAME")?;
    let info = ed
        .plugin_infos()
        .iter()
        .find(|i| i.name == name)
        .ok_or_else(|| format!("E900: no plugin named {name}"))?;
    let Value::Table(mut t) = summary(info) else {
        unreachable!("summary is a table")
    };
    if let Some(m) = &info.manifest {
        t.insert("description".into(), m.description.as_str().into());
        let authors = m.authors.iter().map(|a| a.as_str().into()).collect();
        t.insert("authors".into(), Value::Array(authors));
    }
    let settings = info
        .settings
        .iter()
        .map(|(k, v)| (k.as_str(), crate::settings::to_config_value(v)));
    t.insert("settings".into(), Value::table(settings));
    let health = info
        .items
        .iter()
        .map(|i| {
            Value::table([
                ("ok", Value::from(i.ok)),
                ("level", i.level.name().into()),
                ("message", i.message.as_str().into()),
            ])
        })
        .collect();
    t.insert("health".into(), Value::Array(health));
    ed.reply(Value::Table(t));
    Ok(())
}

/// Name, status, path, and version (if it has a manifest) of a plugin.
fn summary(i: &super::PluginInfo) -> Value {
    let mut pairs = vec![
        ("name", Value::from(i.name.as_str())),
        ("status", i.status.name().into()),
        ("path", i.path.display().to_string().into()),
    ];
    if let Some(v) = i.version() {
        pairs.push(("version", v.to_string().into()));
    }
    Value::table(pairs)
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
    let ctx = ctx(ed)?;
    let Some(source) = a.arg else {
        if !ed.interactive() {
            return Err("E471: Use :plugin add PATH|GIT_URL[@REF]".into());
        }
        let input =
            crate::ui::Input::new("Add plugin", "folder or git URL[@REF]: ", "plugin add {}")
                .returning();
        ed.open_modal(Box::new(input));
        return Ok(());
    };
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
    let installed =
        ctx.plugin_dir.join(name).exists() || ctx.plugin_dir.join(format!("{name}.wasm")).exists();
    if !a.force && installed {
        if !ed.interactive() {
            return Err(format!(
                "E900: :plugin remove asks first; use :plugin remove! {name}"
            ));
        }
        let ask = crate::ui::Confirm::new(
            "Delete plugin",
            format!("Delete {name} from {}?", ctx.plugin_dir.display()),
            format!("plugin remove! {name}"),
        )
        .returning();
        ed.open_modal(Box::new(ask));
        return Ok(());
    }
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

/// Open the plugin manager where `[ui] plugin_manager` says.
fn open_manager(ed: &mut Editor) {
    use crate::ui::{ManagerPlace, PanelSlot};
    match ed.ui_config().plugin_manager {
        ManagerPlace::Modal => {
            let view = super::manager::ManagerView::new(ed, true);
            ed.open_modal(Box::new(view));
        }
        ManagerPlace::Side => {
            let view = super::manager::ManagerView::new(ed, false);
            ed.open_panel(PanelSlot::Side, Box::new(view));
        }
    }
}

/// `:plugin enable|disable NAME`: change `[plugins.NAME] enabled` and reload the plugin.
fn set_enabled(ed: &mut Editor, a: Args<'_>, on: bool) -> CommandResult {
    use crate::config::Value;
    use crate::settings::ConfigEdit;
    let name = a
        .arg
        .ok_or("E471: Use :plugin enable NAME / :plugin disable NAME")?;
    if !ed.plugin_infos().iter().any(|i| i.name == name) {
        return Err(format!("E900: no plugin named {name}"));
    }
    let path = vec![
        "plugins".to_string(),
        name.to_string(),
        "enabled".to_string(),
    ];
    let edit = if on {
        ConfigEdit::Remove(path)
    } else {
        ConfigEdit::Set(path, Value::Bool(false))
    };
    let saved = ed.save_config_edits(&[edit])?;
    load(ed, Some(name));
    let state = if on { "on" } else { "off" };
    let note = if saved { "" } else { " (this session only)" };
    ed.notify(Level::Info, format!("{name} is {state}{note}"));
    Ok(())
}
