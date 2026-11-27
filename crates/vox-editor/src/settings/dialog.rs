//! The settings dialog: `:settings`, `:settings vox`, `:settings keys`, `:settings plugin NAME`.

use vox_view::CharWidth;

use super::{ConfigEdit, get, mode_name, parse_mode, vox_settings};
use crate::config::Value;
use crate::keymap::MapMode;
use crate::plugin::{SettingSpec, SettingValue};

use crate::Editor;
use crate::command::{Args, CommandResult, Commands};
use crate::keys::{Key, KeyCode};
use crate::surface::{Color, Rect, Style, Surface};
use crate::ui::{Confirm, Form, Input, Level, ListItem, ListView, Outcome, View};

/// Title of the key-mapping list (used to refresh it after a change).
const KEYS_TITLE: &str = "Key mappings";

/// Register the `settings` commands.
pub(crate) fn register(c: &mut Commands) {
    use crate::command::category::SETTINGS;
    use crate::command::{Complete, Param};
    c.register(
        "settings",
        "Edit settings: vox, key mappings, and plugins (`:settings NAME` for a plugin).",
        settings,
    )
    .category(SETTINGS)
    .param(
        Param::optional("PLUGIN", "Open this plugin's settings directly.")
            .complete(Complete::Plugin),
    );
    c.register("settings vox", "Edit vox's own settings.", |ed, _| {
        open_vox_form(ed);
        Ok(())
    })
    .category(SETTINGS);
    c.register("settings keys", "Edit key mappings.", |ed, _| {
        open_keys(ed);
        Ok(())
    })
    .category(SETTINGS);
    c.register(
        "settings plugin",
        "Edit a plugin's settings: `:settings plugin NAME`.",
        |ed, a| {
            let name = a.arg.ok_or("E471: Use :settings plugin NAME")?;
            open_plugin_form(ed, name)
        },
    )
    .category(SETTINGS)
    .param(Param::required("PLUGIN", "Plugin whose settings to edit.").complete(Complete::Plugin));
    c.register(
        "settings save",
        "Save a submitted settings form: `:settings save vox|plugin NAME`.",
        save,
    )
    .category(SETTINGS)
    .param(Param::required("TARGET", "`vox`, or `plugin NAME`.").values(&["vox", "plugin"]));
    c.register(
        "settings key",
        "Key mappings: `:settings key add MODE KEYS TARGET`, `delete MODE KEYS`, `list`.",
        key,
    )
    .category(SETTINGS)
    .param(Param::required("ACTION", "What to do.").values(&["add", "delete", "list"]))
    .param(Param::optional("MODE", "n, i, or v.").values(&["n", "i", "v"]))
    .param(Param::optional(
        "KEYS",
        "Keys in Vim notation, e.g. `<leader>w`.",
    ))
    .param(Param::optional("TARGET", "`:command` or keys (for add)."))
    .returns("`list`: a list, one table per mapping: mode, keys, target, owner.")
    .example("settings key add n <leader>w :w", "save with leader w")
    .example("settings key delete n <leader>w", "remove that mapping");
}

/// Where changes go, for titles and messages.
fn destination(ed: &Editor) -> String {
    match ed.scope().config_path() {
        Some(p) => format!("{}: {}", ed.scope().kind.name(), p.display()),
        None => "vanilla: changes are not saved".into(),
    }
}

/// `:settings [PLUGIN]`.
fn settings(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    if let Some(name) = a.arg {
        return open_plugin_form(ed, name);
    }
    let mut items = vec![
        ListItem::new("vox settings")
            .detail("leader, timeouts, panels, developer mode")
            .command("settings vox"),
        ListItem::new("Key mappings")
            .detail(format!("{} in your config", ed.config().keymaps.len()))
            .command("settings keys"),
    ];
    for (name, specs) in ed.plugin_settings() {
        items.push(
            ListItem::new(format!("plugin {name}"))
                .detail(format!("{} setting(s)", specs.len()))
                .command(format!("settings plugin {name}")),
        );
    }
    let title = format!("Settings — {}", destination(ed));
    ed.open_modal(Box::new(ListView::new(title, items)));
    Ok(())
}

/// Current value of `spec` in `root`, or its default if missing or invalid.
pub(super) fn current(root: &Value, key: &str, spec: &SettingSpec) -> SettingValue {
    get(root, key)
        .and_then(|v| spec.check(v).ok())
        .unwrap_or_else(|| spec.default.clone())
}

/// Open the form for vox's own settings, filled with the current values.
fn open_vox_form(ed: &mut Editor) {
    let specs = vox_settings();
    let root = ed.config().to_value();
    let values = specs.iter().map(|s| current(&root, &s.key, s)).collect();
    let form = Form::new("vox settings", specs, values, "settings save vox");
    ed.open_modal(Box::new(form));
}

/// Open the form for plugin `name`'s settings (user values, else defaults).
fn open_plugin_form(ed: &mut Editor, name: &str) -> CommandResult {
    let specs = ed
        .plugin_settings()
        .get(name)
        .cloned()
        .ok_or_else(|| format!("E900: plugin {name} has no settings (or is not installed)"))?;
    let user = Value::Table(ed.plugin_config(name).cloned().unwrap_or_default());
    let values = specs.iter().map(|s| current(&user, &s.key, s)).collect();
    let form = Form::new(
        format!("plugin {name}"),
        specs,
        values,
        format!("settings save plugin {name}"),
    );
    ed.open_modal(Box::new(form));
    Ok(())
}

/// `:settings save vox|plugin NAME`: write the submitted form's changes.
fn save(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let values = ed.take_submitted();
    if values.is_empty() {
        return Ok(());
    }
    let mut words = a.arg.unwrap_or("").split_whitespace();
    let (prefix, plugin): (Vec<String>, Option<String>) = match (words.next(), words.next()) {
        (Some("vox"), None) => (Vec::new(), None),
        (Some("plugin"), Some(name)) => (vec!["plugins".into(), name.into()], Some(name.into())),
        _ => return Err("E471: Use :settings save vox|plugin NAME".into()),
    };
    let edits: Vec<ConfigEdit> = values
        .into_iter()
        .map(|(key, v)| {
            let mut path = prefix.clone();
            path.extend(key.split('.').map(String::from));
            ConfigEdit::Set(path, v)
        })
        .collect();
    let saved = ed.save_config_edits(&edits)?;
    if let Some(name) = &plugin
        && ed.commands().get("reload plugins").is_some()
    {
        ed.run_command(&format!("reload plugins {name}"));
    }
    let what = plugin.map_or("vox settings".into(), |n| format!("plugin {n} settings"));
    if saved {
        ed.notify(
            Level::Info,
            format!("saved {what} ({} change(s))", edits.len()),
        );
    } else {
        ed.notify(
            Level::Warn,
            format!("applied {what} for this session only (nothing is saved)"),
        );
    }
    Ok(())
}

/// `:settings key new|change|remove|add|delete ...`.
fn key(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let arg = a.arg.unwrap_or("");
    let (action, rest) = arg.split_once(' ').unwrap_or((arg, ""));
    let mut words = rest.splitn(3, ' ');
    let mode = words.next().unwrap_or("");
    let lhs = words.next().unwrap_or("");
    let rhs = words.next().unwrap_or("");
    let parse =
        |m: &str| parse_mode(m).ok_or_else(|| format!("E474: mode must be n, i, or v, not `{m}`"));
    match action {
        "new" => {
            let input = Input::new(
                "Add key mapping",
                "MODE KEYS TARGET: ",
                "settings key add {}",
            )
            .with_text("n ")
            .returning();
            ed.open_modal(Box::new(input));
        }
        "change" => {
            parse(mode)?;
            let input = Input::new(
                format!("Change {mode} {lhs}"),
                format!("{mode} {lhs} → "),
                format!("settings key add {mode} {lhs} {{}}"),
            )
            .with_text(current_rhs(ed, mode, lhs).unwrap_or_default())
            .returning();
            ed.open_modal(Box::new(input));
        }
        "remove" => {
            parse(mode)?;
            let ask = Confirm::new(
                "Delete mapping",
                format!("Delete {mode} {lhs} from your config?"),
                format!("settings key delete {mode} {lhs}"),
            )
            .returning();
            ed.open_modal(Box::new(ask));
        }
        "list" => {
            let rows = key_rows(ed)
                .into_iter()
                .map(|r| {
                    Value::table([
                        ("mode", Value::from(mode_name(r.mode))),
                        ("keys", r.lhs.into()),
                        ("target", r.rhs.into()),
                        ("owner", r.owner.into()),
                    ])
                })
                .collect();
            ed.reply(Value::Array(rows));
        }
        "add" | "delete" => {
            let m = parse(mode)?;
            if lhs.is_empty() {
                return Err("E471: Use :settings key add MODE KEYS TARGET".into());
            }
            let path = vec!["keymap".into(), mode_name(m).into(), lhs.into()];
            let edit = if action == "add" {
                ConfigEdit::Set(path, Value::String(rhs.into()))
            } else {
                ConfigEdit::Remove(path)
            };
            let saved = ed.save_config_edits(&[edit])?;
            let verb = if action == "add" { "saved" } else { "deleted" };
            let note = if saved { "" } else { " (this session only)" };
            ed.notify(Level::Info, format!("{verb} mapping {mode} {lhs}{note}"));
            if ed.modal_title().as_deref() == Some(KEYS_TITLE) {
                open_keys(ed); // refresh the list it came from
            }
        }
        _ => {
            return Err(
                "E471: Use :settings key add MODE KEYS TARGET, :settings key delete MODE KEYS, or :settings key list"
                    .into(),
            );
        }
    }
    Ok(())
}

/// The target of config mapping `lhs` in mode `mode`.
fn current_rhs(ed: &Editor, mode: &str, lhs: &str) -> Option<String> {
    let m = parse_mode(mode)?;
    ed.config()
        .keymaps
        .iter()
        .find(|(mm, l, _)| *mm == m && l == lhs)
        .map(|(_, _, r)| r.clone())
}

/// Open (or refresh) the key-mapping list: config mappings first, then other owners'.
fn open_keys(ed: &mut Editor) {
    let rows = key_rows(ed);
    let selected = std::cell::Cell::new(0);
    ed.open_modal(Box::new(KeymapView { rows, selected }));
}

/// All key mappings: config mappings first, then other owners'.
fn key_rows(ed: &Editor) -> Vec<KeyRow> {
    let mut rows: Vec<KeyRow> = ed
        .config()
        .keymaps
        .iter()
        .map(|(m, l, r)| KeyRow {
            mode: m.to_owned(),
            lhs: l.clone(),
            rhs: r.clone(),
            owner: crate::owner::CONFIG.into(),
        })
        .collect();
    for (m, keys, b, owner) in ed.keymap().iter() {
        if owner == crate::owner::CONFIG {
            continue;
        }
        let rhs = match b {
            crate::keymap::Binding::Keys(k) => crate::keys::format_keys(k),
            crate::keymap::Binding::Command(c) => format!(":{c}"),
        };
        rows.push(KeyRow {
            mode: m,
            lhs: crate::keys::format_keys(keys),
            rhs,
            owner: owner.into(),
        });
    }
    rows
}

/// One row of the key-mapping list.
struct KeyRow {
    mode: MapMode,
    /// As written in the config (`<leader>w`), or formatted for other owners.
    lhs: String,
    rhs: String,
    owner: String,
}

/// List of key mappings. Your config's mappings can be changed (`Enter`/`e`), deleted
/// (`d`), and added (`a`); mappings from plugins or developer mode are shown read-only.
struct KeymapView {
    rows: Vec<KeyRow>,
    selected: std::cell::Cell<usize>,
}

impl View for KeymapView {
    fn title(&self) -> String {
        KEYS_TITLE.into()
    }

    fn render(&self, area: Rect, s: &mut Surface, width: &dyn CharWidth) {
        let list_h = area.h.saturating_sub(1) as usize;
        let sel = self.selected.get();
        let top = sel.saturating_sub(list_h.saturating_sub(1));
        if self.rows.is_empty() {
            s.put_in(
                area,
                0,
                "No key mappings yet. Press a to add one.",
                Style::DIM,
                width,
            );
        }
        for (i, r) in self.rows.iter().enumerate().skip(top).take(list_h) {
            let line = Rect::new(area.x, area.y + (i - top) as u16, area.w, 1);
            let mut style = if i == sel {
                Style::REVERSED
            } else {
                Style::default()
            };
            if r.owner != crate::owner::CONFIG {
                style.dim = true;
            }
            s.clear(line, style);
            let m = mode_name(r.mode).chars().next().unwrap_or('n');
            let text = format!(" {m} {:<14} → {:<24} {}", r.lhs, r.rhs, r.owner);
            s.put_in(line, 0, &text, style, width);
        }
        let hint = "a add · Enter/e change · d delete · Esc back";
        let foot = Rect::new(area.x, area.y + area.h.saturating_sub(1), area.w, 1);
        s.put_in(foot, 0, hint, Style::BOLD.fg(Color::Blue), width);
    }

    fn handle_key(&mut self, key: Key) -> Outcome {
        let last = self.rows.len().saturating_sub(1);
        let sel = self.selected.get();
        let row = self.rows.get(sel);
        let editable = |r: &KeyRow, cmd: &str| -> Outcome {
            if r.owner == crate::owner::CONFIG {
                Outcome::RunAndClose(format!(
                    "settings key {cmd} {} {}",
                    mode_name(r.mode),
                    r.lhs
                ))
            } else {
                Outcome::Run(format!(
                    "notify info {} comes from {}; change it there",
                    r.lhs, r.owner
                ))
            }
        };
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.selected.set((sel + 1).min(last)),
            KeyCode::Char('k') | KeyCode::Up => self.selected.set(sel.saturating_sub(1)),
            KeyCode::Char('a') => return Outcome::RunAndClose("settings key new".into()),
            KeyCode::Enter | KeyCode::Char('e') => {
                return row.map_or(Outcome::Handled, |r| editable(r, "change"));
            }
            KeyCode::Char('d') => return row.map_or(Outcome::Handled, |r| editable(r, "remove")),
            KeyCode::Esc | KeyCode::Char('q') => return Outcome::Close,
            _ => return Outcome::Ignored,
        }
        Outcome::Handled
    }
}
