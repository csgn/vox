//! Settings commands that work without the dialog: `:settings list`, `:settings get KEY`,
//! `:settings set KEY VALUE`, `:settings unset KEY`. Keys are dotted config paths: vox's own
//! (`ui.side_width`) or a plugin's (`plugins.NAME.KEY`).

use super::{ConfigEdit, to_config_value, vox_settings};
use crate::Editor;
use crate::command::{Args, CommandResult, Commands};
use crate::config::Value;
use crate::plugin::{SettingKind, SettingSpec};
use crate::ui::Level;

/// Register the commands.
pub(super) fn register(c: &mut Commands) {
    use crate::command::category::SETTINGS;
    use crate::command::{Complete, Param};
    c.register(
        "settings list",
        "Return every setting as data: key, value, default, type, description.",
        list,
    )
    .category(SETTINGS)
    .returns("A list, one table per setting: key, value, default, type, description.");
    c.register(
        "settings get",
        "Return a setting's value: `:settings get KEY` (e.g. `ui.side_width`, `plugins.NAME.KEY`).",
        get,
    )
    .category(SETTINGS)
    .param(Param::required("KEY", "Dotted setting key.").complete(Complete::SettingKey))
    .returns("The setting's current value.")
    .example("settings get ui.side_width", "width of the side panel");
    c.register(
        "settings set",
        "Change a setting and save it to the scope's config: `:settings set KEY VALUE`.",
        set,
    )
    .category(SETTINGS)
    .param(Param::required("KEY", "Dotted setting key.").complete(Complete::SettingKey))
    .param(Param::required(
        "VALUE",
        "New value; checked against the setting's type and limits.",
    ))
    .example("settings set ui.side_width 50", "a wider side panel")
    .example(
        "settings set plugins.text-tools.case lower",
        "a plugin's setting",
    );
    c.register(
        "settings unset",
        "Remove a setting from the scope's config (back to its default): `:settings unset KEY`.",
        unset,
    )
    .category(SETTINGS)
    .param(Param::required("KEY", "Dotted setting key.").complete(Complete::SettingKey));
}

/// A setting found by its dotted key.
struct Found {
    spec: SettingSpec,
    /// The plugin it belongs to; `None` for vox's own settings.
    plugin: Option<String>,
    /// Path in the config file.
    path: Vec<String>,
}

impl Found {
    /// The full dotted key.
    fn key(&self) -> String {
        self.path.join(".")
    }
}

/// Look up `key`: vox's own settings, or `plugins.NAME.KEY` from the plugin's manifest.
fn find(ed: &Editor, key: &str) -> Result<Found, String> {
    let path = key.split('.').map(String::from).collect();
    if let Some(rest) = key.strip_prefix("plugins.") {
        let (name, sub) = rest
            .split_once('.')
            .ok_or_else(|| format!("E474: Use plugins.NAME.KEY, not `{key}`"))?;
        let spec = ed
            .plugin_settings()
            .get(name)
            .and_then(|specs| specs.iter().find(|s| s.key == sub))
            .cloned()
            .ok_or_else(|| format!("E900: plugin {name} has no setting `{sub}`"))?;
        return Ok(Found {
            spec,
            plugin: Some(name.into()),
            path,
        });
    }
    let spec = vox_settings()
        .into_iter()
        .find(|s| s.key == key)
        .ok_or_else(|| format!("E900: no setting `{key}` (see :settings list)"))?;
    Ok(Found {
        spec,
        plugin: None,
        path,
    })
}

/// The value in use for `f`: the config's (vox) or the user's `[plugins.NAME]` value, else
/// the default.
fn value_of(ed: &Editor, f: &Found) -> Value {
    let v = match &f.plugin {
        None => super::dialog::current(&ed.config().to_value(), &f.spec.key, &f.spec),
        Some(name) => {
            let user = Value::Table(ed.plugin_config(name).cloned().unwrap_or_default());
            super::dialog::current(&user, &f.spec.key, &f.spec)
        }
    };
    to_config_value(&v)
}

/// A setting type as text: `string`, `integer 10..500`, `bool`, `one of: a, b`.
fn kind_text(k: &SettingKind) -> String {
    match k {
        SettingKind::String => "string".into(),
        SettingKind::Integer { min, max } => {
            let lo = min.map_or(String::new(), |m| m.to_string());
            let hi = max.map_or(String::new(), |m| m.to_string());
            if min.is_none() && max.is_none() {
                "integer".into()
            } else {
                format!("integer {lo}..{hi}")
            }
        }
        SettingKind::Bool => "bool".into(),
        SettingKind::Choice(c) => format!("one of: {}", c.join(", ")),
    }
}

/// Parse `text` as a value of `spec`'s type and check its limits.
fn parse_value(spec: &SettingSpec, key: &str, text: &str) -> Result<Value, String> {
    let v = match spec.kind {
        SettingKind::Bool => match text {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => return Err(format!("E474: `{key}` must be true or false")),
        },
        SettingKind::Integer { .. } => text
            .parse::<i64>()
            .map(Value::Integer)
            .map_err(|_| format!("E474: `{key}` must be an integer"))?,
        SettingKind::String | SettingKind::Choice(_) => Value::String(text.into()),
    };
    spec.check(&v).map(|_| v).map_err(|e| {
        format!(
            "E474: {}",
            e.replace(&format!("`{}`", spec.key), &format!("`{key}`"))
        )
    })
}

/// Every setting key with its description: vox's settings, then each plugin's.
pub(crate) fn setting_keys(ed: &Editor) -> Vec<(String, String)> {
    let mut keys: Vec<(String, String)> = vox_settings()
        .into_iter()
        .map(|s| (s.key, s.description))
        .collect();
    for (name, specs) in ed.plugin_settings() {
        keys.extend(
            specs
                .iter()
                .map(|s| (format!("plugins.{name}.{}", s.key), s.description.clone())),
        );
    }
    keys
}

/// `:settings list`: vox's settings, then each plugin's.
fn list(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    let mut keys: Vec<String> = vox_settings().into_iter().map(|s| s.key).collect();
    for (name, specs) in ed.plugin_settings() {
        keys.extend(specs.iter().map(|s| format!("plugins.{name}.{}", s.key)));
    }
    let mut rows = Vec::new();
    for key in keys {
        let f = find(ed, &key)?;
        rows.push(Value::table([
            ("key", Value::from(key)),
            ("value", value_of(ed, &f)),
            ("default", to_config_value(&f.spec.default)),
            ("type", kind_text(&f.spec.kind).into()),
            ("description", f.spec.description.clone().into()),
        ]));
    }
    ed.reply(Value::Array(rows));
    Ok(())
}

/// `:settings get KEY`.
fn get(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let key = a.arg.ok_or("E471: Use :settings get KEY")?;
    let f = find(ed, key)?;
    let v = value_of(ed, &f);
    ed.reply(v);
    Ok(())
}

/// `:settings set KEY VALUE`.
fn set(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let (key, text) = a
        .arg
        .and_then(|s| s.split_once(char::is_whitespace))
        .ok_or("E471: Use :settings set KEY VALUE")?;
    let f = find(ed, key)?;
    let v = parse_value(&f.spec, key, text.trim())?;
    let shown = v.to_text();
    save(
        ed,
        &f,
        ConfigEdit::Set(f.path.clone(), v),
        &format!("{key} = {shown}"),
    )
}

/// `:settings unset KEY`.
fn unset(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let key = a.arg.ok_or("E471: Use :settings unset KEY")?;
    let f = find(ed, key)?;
    save(
        ed,
        &f,
        ConfigEdit::Remove(f.path.clone()),
        &format!("unset {}", f.key()),
    )
}

/// Save one edit, reload the plugin it belongs to, and say what happened.
fn save(ed: &mut Editor, f: &Found, edit: ConfigEdit, what: &str) -> CommandResult {
    let saved = ed.save_config_edits(&[edit])?;
    if let Some(name) = &f.plugin
        && ed.commands().get("reload plugins").is_some()
    {
        ed.run_command(&format!("reload plugins {name}"));
    }
    let note = if saved { "" } else { " (this session only)" };
    ed.notify(Level::Info, format!("{what}{note}"));
    Ok(())
}
