//! Built-in `:` commands, registered through the same [`Commands`] API that plugins use.

use crate::Editor;
use crate::command::{Args, CommandResult, Commands};
use crate::config::Value;
use crate::keymap::{Binding, MapMode};
use crate::keys::format_keys;
use crate::log::LogView;
use crate::scope::{self, ScopeKind, SetupChoice};
use crate::surface::Style;
use crate::ui::{CheckList, Focus, Level, ListItem, ListView, PanelSlot, TextView};

const E37: &str = "E37: No write since last change (add ! to override)";

/// Register all built-in commands and their Vim-style short names.
pub(crate) fn register(c: &mut Commands) {
    use crate::command::category::{DEVELOPER, FILE, HELP, INTERFACE, SCOPE};
    use crate::command::{Complete, Param};
    c.register("write", "Save the file. `:w FILE` saves as FILE.", write)
        .category(FILE)
        .param(
            Param::optional("FILE", "Save to this file and make it the current one.")
                .complete(Complete::File),
        )
        .example("w", "save the current file")
        .example("w notes.txt", "save as notes.txt");
    c.alias("w", "write");
    c.register(
        "quit",
        "Quit. Refuses if there are unsaved changes; `:q!` discards them.",
        quit,
    )
    .category(FILE)
    .force("Quit even if there are unsaved changes (they are lost).");
    for a in ["q", "qa", "qall"] {
        c.alias(a, "quit");
    }
    c.register("wq", "Save, then quit.", write_quit)
        .category(FILE)
        .param(Param::optional("FILE", "Save to this file.").complete(Complete::File));
    c.register("xit", "Save if modified, then quit.", xit)
        .category(FILE)
        .param(Param::optional("FILE", "Save to this file.").complete(Complete::File));
    c.alias("x", "xit");
    c.register(
        "edit",
        "Open FILE, or reload the current file. `:e!` discards changes.",
        edit,
    )
    .category(FILE)
    .param(
        Param::optional("FILE", "File to open; none reloads the current file.")
            .complete(Complete::File),
    )
    .force("Open or reload even if there are unsaved changes (they are lost).")
    .example("e notes.txt", "open notes.txt")
    .example("e!", "reload the current file, discarding changes");
    c.alias("e", "edit");
    c.register(
        "notify",
        "Show a notification: `:notify [info|warn|error] TEXT`.",
        notify,
    )
    .category(INTERFACE)
    .param(
        Param::optional("LEVEL", "How important it is (default info).")
            .values(&["info", "warn", "error"]),
    )
    .param(Param::required("TEXT", "The text to show."))
    .example("notify warn disk almost full", "a yellow notification");
    c.register(
        "messages",
        "Show message history in the bottom panel.",
        messages,
    )
    .category(INTERFACE);
    c.alias("mes", "messages");
    c.register("commands", "List all commands; Enter runs one.", commands)
        .category(HELP);
    c.register(
        "commands list",
        "Return every command as data: name, description, owner, aliases.",
        commands_list,
    )
    .category(HELP)
    .returns("A list, one table per command: name, description, owner, aliases, category, usage, params, returns, force, examples.");
    crate::help::register(c);
    c.register(
        "keymaps",
        "List your key mappings in the side panel.",
        keymaps,
    )
    .category(INTERFACE);
    c.register("close", "Close the modal or the focused panel.", close)
        .category(INTERFACE);
    c.register("focus", "Move focus: `:focus editor|side|bottom`.", focus)
        .category(INTERFACE)
        .param(
            Param::required("TARGET", "Where keys go next.").values(&["editor", "side", "bottom"]),
        );
    c.register(
        "log",
        "Show the log in the bottom panel: `:log [debug|info|warn|error] [SOURCE]`.",
        log,
    )
    .category(DEVELOPER)
    .param(
        Param::optional("LEVEL", "Lowest level shown (default debug).")
            .values(&["debug", "info", "warn", "error"]),
    )
    .param(Param::optional(
        "SOURCE",
        "Only entries from this source, e.g. `editor` or `plugin:NAME`.",
    ))
    .example("log warn", "warnings and errors only");
    c.register("log refresh", "Refresh open log views.", |ed, _| {
        ed.refresh_log_views();
        Ok(())
    })
    .category(DEVELOPER);
    c.register("log clear", "Clear the log.", |ed, _| {
        ed.clear_log();
        Ok(())
    })
    .category(DEVELOPER);
    c.register(
        "devmode",
        "Developer mode: `:devmode on|off`, no argument toggles.",
        devmode,
    )
    .category(DEVELOPER)
    .param(Param::optional("STATE", "Turn it on or off; none toggles.").values(&["on", "off"]));
    c.register(
        "scope",
        "Show the active scope (global, local, vanilla) and its paths.",
        scope,
    )
    .category(SCOPE);
    crate::settings::register(c);
    crate::plugin::manage::register(c);
    crate::update::register(c);
    c.register(
        "scope setup",
        "Open the project set-up dialog (local mode).",
        |ed, _| open_setup(ed),
    )
    .category(SCOPE);
    c.register(
        "scope show",
        "Return the active scope and its paths as data.",
        scope_show,
    )
    .category(SCOPE)
    .returns("A table: kind (global, local, vanilla), config, plugins, lockfile (paths; left out in vanilla mode), exists.");
    c.register(
        "scope init",
        "Set up the project's .vox: `:scope init empty|all|[config] [PLUGIN...]`.",
        scope_init,
    )
    .category(SCOPE)
    .param(
        Param::optional(
            "WHAT",
            "`empty`, `all`, `config`, or names of global plugins to copy.",
        )
        .repeated()
        .complete(Complete::Plugin),
    )
    .example(
        "scope init all",
        "copy the global config and all global plugins",
    )
    .example(
        "scope init config text-tools",
        "copy the config and one plugin",
    );
    c.register(
        "scope pick",
        "Choose global config/plugins to copy into the project.",
        scope_pick,
    )
    .category(SCOPE);
}

/// `:write [FILE]` / `:w`.
fn write(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    ed.write(a.arg).map(|_| ())
}

/// `:quit` / `:q`; `:q!` discards changes.
fn quit(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    if ed.document().is_modified() && !a.force {
        return Err(E37.into());
    }
    ed.quit();
    Ok(())
}

/// `:wq [FILE]`.
fn write_quit(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    ed.write(a.arg)?;
    ed.quit();
    Ok(())
}

/// `:xit` / `:x`: save only if modified, then quit.
fn xit(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    if ed.document().is_modified() || a.arg.is_some() {
        ed.write(a.arg)?;
    }
    ed.quit();
    Ok(())
}

/// `:edit [FILE]` / `:e`; `:e!` discards changes.
fn edit(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    if ed.document().is_modified() && !a.force {
        return Err(E37.into());
    }
    ed.edit(a.arg)
}

/// `:notify [level] TEXT`.
fn notify(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let text = a.arg.ok_or("E471: Argument required")?;
    let (level, text) = match text.split_once(' ') {
        Some((l, rest)) if Level::parse(l).is_some() => (Level::parse(l).unwrap(), rest),
        _ => (Level::Info, text),
    };
    ed.notify(level, text);
    Ok(())
}

/// `:messages`: history in a text view in the bottom panel, newest at the bottom.
fn messages(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    let lines = ed
        .history()
        .iter()
        .map(|(level, text)| {
            let style = match level {
                Level::Info => Style::default(),
                l => Style::default().fg(l.color()),
            };
            (style, format!("{:<7} {text}", level.name()))
        })
        .collect();
    ed.open_panel(
        PanelSlot::Bottom,
        Box::new(TextView::new("Messages", lines).at_end()),
    );
    Ok(())
}

/// `:commands`: every command with its aliases and description, in the modal.
fn commands(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    let items = ed
        .commands()
        .iter()
        .map(|c| {
            let aliases: Vec<&str> = ed.commands().aliases_of(&c.name).collect();
            let label = if aliases.is_empty() {
                c.name.clone()
            } else {
                format!("{} ({})", c.name, aliases.join(", "))
            };
            ListItem::new(label).detail(&c.description).command(&c.name)
        })
        .collect();
    ed.open_modal(Box::new(ListView::new("Commands", items)));
    Ok(())
}

/// `:commands list`: every command with its metadata as data.
fn commands_list(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    let list = ed
        .commands()
        .iter()
        .map(|c| crate::help::command_value(ed.commands(), c))
        .collect();
    ed.reply(Value::Array(list));
    Ok(())
}

/// `:keymaps`: user mappings in the side panel.
fn keymaps(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    let items = ed
        .keymap()
        .iter()
        .map(|(mode, lhs, b, owner)| {
            let mode = match mode {
                MapMode::Normal => "n",
                MapMode::Insert => "i",
                MapMode::Visual => "v",
            };
            let rhs = match b {
                Binding::Keys(k) if k.is_empty() => "(disabled)".to_string(),
                Binding::Keys(k) => format_keys(k),
                Binding::Command(c) => format!(":{c}"),
            };
            ListItem::new(format!("{mode} {}", format_keys(lhs)))
                .detail(format!("→ {rhs}  ({owner})"))
        })
        .collect();
    ed.open_panel(
        PanelSlot::Side,
        Box::new(ListView::new("Key mappings", items)),
    );
    Ok(())
}

/// `:close`.
fn close(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    ed.close_focused();
    Ok(())
}

/// `:focus editor|side|bottom`.
fn focus(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let target = match a.arg {
        Some("editor") => Focus::Editor,
        Some("side") => Focus::Side,
        Some("bottom") => Focus::Bottom,
        _ => return Err("E474: Use :focus editor|side|bottom".into()),
    };
    ed.focus(target);
    Ok(())
}

/// `:log [level] [source]`.
fn log(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let mut level = Level::Debug;
    let mut source = None;
    for word in a.arg.unwrap_or("").split_whitespace() {
        match Level::parse(word) {
            Some(l) => level = l,
            None => source = Some(word.to_string()),
        }
    }
    let mut view = LogView::new(level, source);
    view.refresh(ed.log_entries());
    ed.open_panel(PanelSlot::Bottom, Box::new(view));
    Ok(())
}

/// `:devmode [on|off]`.
fn devmode(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let on = match a.arg {
        Some("on") => true,
        Some("off") => false,
        None => !ed.developer_mode(),
        Some(_) => return Err("E474: Use :devmode on|off".into()),
    };
    ed.set_developer_mode(on);
    ed.notify(
        Level::Info,
        format!("developer mode {}", if on { "on" } else { "off" }),
    );
    Ok(())
}

/// `:scope` shows the scope; `:scope setup` opens the project set-up dialog.
fn scope(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    match a.arg {
        None => {
            let sc = ed.scope().clone();
            let show = |p: Option<std::path::PathBuf>| {
                p.map_or("—".to_string(), |p| {
                    let mark = if p.exists() { "" } else { "(missing) " };
                    format!("{mark}{}", p.display())
                })
            };
            let lines = vec![
                (Style::BOLD, format!("scope    {}", sc.kind.name())),
                (
                    Style::default(),
                    format!("config   {}", show(sc.config_path())),
                ),
                (
                    Style::default(),
                    format!("plugins  {}", show(sc.plugin_dir())),
                ),
                (
                    Style::default(),
                    format!("lockfile {}", show(sc.lock_path())),
                ),
                (Style::default(), String::new()),
                (
                    Style::DIM,
                    "vox                     global (~/.config/vox)".into(),
                ),
                (
                    Style::DIM,
                    "vox --local-config [DIR] this project's .vox only".into(),
                ),
                (
                    Style::DIM,
                    "vox --vanilla           no config, no plugins".into(),
                ),
            ];
            ed.open_modal(Box::new(TextView::new("Scope", lines)));
            Ok(())
        }
        Some(_) => {
            Err("E474: Use :scope, :scope show, :scope setup, :scope init, or :scope pick".into())
        }
    }
}

/// `:scope show`: the active scope and its paths (missing in vanilla mode) as data.
fn scope_show(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    let sc = ed.scope();
    let path = |p: Option<std::path::PathBuf>| p.map(|p| Value::from(p.display().to_string()));
    let pairs = [
        ("kind", Some(Value::from(sc.kind.name()))),
        ("config", path(sc.config_path())),
        ("plugins", path(sc.plugin_dir())),
        ("lockfile", path(sc.lock_path())),
        ("exists", Some(sc.exists().into())),
    ];
    let value = Value::table(pairs.into_iter().filter_map(|(k, v)| Some((k, v?))));
    ed.reply(value);
    Ok(())
}

/// Error unless the local scope is active.
fn require_local(ed: &Editor) -> CommandResult {
    match ed.scope().kind {
        ScopeKind::Local => Ok(()),
        _ => Err("E900: project set-up needs local mode: start with vox --local-config".into()),
    }
}

/// The project set-up dialog.
fn open_setup(ed: &mut Editor) -> CommandResult {
    require_local(ed)?;
    let plugins = ed
        .global_scope()
        .and_then(|g| g.plugin_dir())
        .map(|d| scope::list_plugins(&d))
        .unwrap_or_default();
    let has_config = ed
        .global_scope()
        .and_then(|g| g.config_path())
        .is_some_and(|p| p.is_file());
    let root = ed
        .scope()
        .project_root()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let mut items = vec![
        ListItem::new("Start empty")
            .detail("no config, no plugins")
            .command("scope init empty"),
    ];
    if has_config {
        items.push(
            ListItem::new("Copy global config")
                .detail("config.toml")
                .command("scope init config"),
        );
    }
    if has_config || !plugins.is_empty() {
        items.push(
            ListItem::new("Choose what to copy…")
                .detail("pick config and plugins")
                .command("scope pick"),
        );
        items.push(
            ListItem::new("Copy everything")
                .detail(format!("config + {} plugin(s)", plugins.len()))
                .command("scope init all"),
        );
    }
    items.push(
        ListItem::new("Not now")
            .detail("run without project setup")
            .command("notify info Not set up. Run :scope setup any time."),
    );
    let title = format!("Set up project {root}/.vox");
    ed.open_modal(Box::new(ListView::new(title, items)));
    Ok(())
}

/// `:scope pick`: checkboxes for the global config and each global plugin.
fn scope_pick(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    require_local(ed)?;
    let global = ed
        .global_scope()
        .cloned()
        .ok_or("E900: no global setup found")?;
    let mut items = Vec::new();
    if global.config_path().is_some_and(|p| p.is_file()) {
        items.push(("config.toml".to_string(), "config".to_string()));
    }
    for name in global
        .plugin_dir()
        .map(|d| scope::list_plugins(&d))
        .unwrap_or_default()
    {
        items.push((format!("plugin {name}"), name));
    }
    ed.open_modal(Box::new(CheckList::new(
        "Copy into project",
        items,
        "scope init {}",
    )));
    Ok(())
}

/// `:scope init empty|all|[config] [PLUGIN...]`: create `.vox/`, copy, then reload.
fn scope_init(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    require_local(ed)?;
    let global = ed
        .global_scope()
        .cloned()
        .unwrap_or_else(scope::Scope::vanilla);
    let global_plugins = global
        .plugin_dir()
        .map(|d| scope::list_plugins(&d))
        .unwrap_or_default();
    let mut choice = SetupChoice::default();
    for word in a.arg.unwrap_or("empty").split_whitespace() {
        match word {
            "empty" => {}
            "all" => {
                choice.config = true;
                choice.plugins = global_plugins.clone();
            }
            "config" => choice.config = true,
            name => choice.plugins.push(name.to_string()),
        }
    }
    let local = ed.scope().clone();
    let report = scope::setup_local(&global, &local, &choice)
        .map_err(|e| format!("E900: project set-up failed: {e}"))?;
    for line in &report {
        ed.log(Level::Info, "editor", line);
    }
    copy_lock_entries(ed, &global, &local, &choice.plugins);
    for cmd in ["reload config", "reload plugins"] {
        if ed.commands().get(cmd).is_some() {
            ed.run_command(cmd);
        }
    }
    let dir = local
        .dir
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    let summary = if report.is_empty() {
        format!("created {dir}")
    } else {
        format!("set up {dir}: {}", report.join(", "))
    };
    ed.notify(Level::Info, summary);
    Ok(())
}

/// Copy the global lock entries of `plugins` into the project's `vox.lock`, so the project
/// can reinstall them with `:plugins sync`.
fn copy_lock_entries(
    ed: &mut Editor,
    global: &scope::Scope,
    local: &scope::Scope,
    plugins: &[String],
) {
    use crate::plugin::lock;
    let (Some((parse, format)), Some(from), Some(to)) =
        (ed.file_format(), global.lock_path(), local.lock_path())
    else {
        return;
    };
    let result = (|| -> Result<usize, String> {
        let source = lock::read(&from, parse)?;
        let mut target = lock::read(&to, parse)?;
        let mut copied = 0;
        for name in plugins {
            if let Some(e) = source.get(name) {
                target.upsert(e.clone());
                copied += 1;
            }
        }
        if copied > 0 {
            lock::write(&to, &target, format)?;
        }
        Ok(copied)
    })();
    match result {
        Ok(0) => {}
        Ok(n) => ed.log(
            Level::Info,
            "editor",
            format!("copied {n} lock entr(y/ies) to {}", to.display()),
        ),
        Err(e) => ed.notify(Level::Warn, format!("vox.lock not copied: {e}")),
    }
}
