//! Settings as data: a schema of vox's own settings, the config as a [`Value`] tree, and
//! edits to it. The settings dialog (`:settings`) builds forms from schemas and turns
//! saved forms into [`ConfigEdit`]s; an adapter writes those into `config.toml`, keeping the
//! user's comments.

use std::collections::BTreeMap;
use std::path::Path;

use crate::config::{Config, Value};
use crate::keymap::MapMode;
use crate::plugin::{SettingKind, SettingSpec, SettingValue};
use crate::ui::{ManagerPlace, NotifyPosition, SidePosition};

mod commands;
mod dialog;

/// Register the settings commands (dialog and data).
pub(crate) fn register(c: &mut crate::command::Commands) {
    dialog::register(c);
    commands::register(c);
}

/// One change to the config file. Paths are keys from the top, e.g.
/// `["ui", "side_width"]` or `["keymap", "normal", "<leader>w"]`.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigEdit {
    /// Set a value, creating tables on the way.
    Set(Vec<String>, Value),
    /// Remove a key (nothing happens if it is missing).
    Remove(Vec<String>),
}

/// Applies edits to a config file, keeping its comments and layout; provided by an adapter.
pub type ConfigWriter = fn(&Path, &[ConfigEdit]) -> Result<(), String>;

/// Apply `edits` to a value tree (the in-memory equivalent of [`ConfigWriter`]).
pub fn apply_edits(root: &mut Value, edits: &[ConfigEdit]) {
    for edit in edits {
        match edit {
            ConfigEdit::Set(path, v) => apply_one(root, path, Some(v)),
            ConfigEdit::Remove(path) => apply_one(root, path, None),
        }
    }
}

/// Set (`Some`) or remove (`None`) the value at `path` below `v`.
fn apply_one(v: &mut Value, path: &[String], value: Option<&Value>) {
    let Value::Table(t) = v else {
        return;
    };
    match path {
        [] => {}
        [last] => match value {
            Some(x) => {
                t.insert(last.clone(), x.clone());
            }
            None => {
                t.remove(last);
            }
        },
        [first, rest @ ..] => {
            if value.is_some() {
                let child = t
                    .entry(first.clone())
                    .or_insert_with(|| Value::Table(BTreeMap::new()));
                apply_one(child, rest, value);
            } else if let Some(child) = t.get_mut(first) {
                apply_one(child, rest, value);
            }
        }
    }
}

/// Shorthand for building a [`SettingSpec`].
fn spec(key: &str, kind: SettingKind, default: SettingValue, description: &str) -> SettingSpec {
    SettingSpec {
        key: key.into(),
        kind,
        default,
        description: description.into(),
    }
}

/// An integer setting limited to `min..=max`.
fn int(min: i64, max: i64) -> SettingKind {
    SettingKind::Integer {
        min: Some(min),
        max: Some(max),
    }
}

/// Schema of vox's own settings (everything in `config.toml` except key mappings and
/// plugin tables). Keys are dotted paths, e.g. `ui.side_width`. Limits match the ones
/// [`Config::from_value`] enforces.
pub fn vox_settings() -> Vec<SettingSpec> {
    use SettingValue::{Bool, Integer, String as Text};
    let d = Config::default();
    let choice = |c: &[&str]| SettingKind::Choice(c.iter().map(|s| s.to_string()).collect());
    vec![
        spec(
            "leader",
            SettingKind::String,
            Text(d.leader.clone()),
            "What <leader> means in key mappings",
        ),
        spec(
            "timeout_ms",
            int(0, 600_000),
            Integer(d.timeout_ms as i64),
            "How long to wait for the rest of a mapping (ms)",
        ),
        spec(
            "developer_mode",
            SettingKind::Bool,
            Bool(d.developer_mode),
            "Log and reload shortcuts, DEV badge",
        ),
        spec(
            "ui.side_panel",
            choice(&["right", "left"]),
            Text("right".into()),
            "Side panel position",
        ),
        spec(
            "ui.side_width",
            int(10, 500),
            Integer(d.ui.side_width.into()),
            "Side panel width (columns)",
        ),
        spec(
            "ui.bottom_height",
            int(3, 200),
            Integer(d.ui.bottom_height.into()),
            "Bottom panel height (rows)",
        ),
        spec(
            "ui.modal_width",
            int(10, 100),
            Integer(d.ui.modal_width.into()),
            "Modal width (% of the screen)",
        ),
        spec(
            "ui.modal_height",
            int(10, 100),
            Integer(d.ui.modal_height.into()),
            "Modal height (% of the screen)",
        ),
        spec(
            "ui.notify_timeout_ms",
            int(0, 600_000),
            Integer(d.ui.notify_timeout_ms as i64),
            "How long notifications stay (ms)",
        ),
        spec(
            "ui.notify_position",
            choice(&["top-right", "bottom-right"]),
            Text("top-right".into()),
            "Notification corner",
        ),
        spec(
            "ui.plugin_manager",
            choice(&["modal", "side"]),
            Text("modal".into()),
            "Where :plugins opens",
        ),
    ]
}

/// Mode name used in `[keymap.MODE]`.
pub fn mode_name(m: MapMode) -> &'static str {
    match m {
        MapMode::Normal => "normal",
        MapMode::Insert => "insert",
        MapMode::Visual => "visual",
    }
}

/// Mode from its name or first letter (`n`, `i`, `v`).
pub fn parse_mode(s: &str) -> Option<MapMode> {
    match s {
        "n" | "normal" => Some(MapMode::Normal),
        "i" | "insert" => Some(MapMode::Insert),
        "v" | "visual" => Some(MapMode::Visual),
        _ => None,
    }
}

impl Config {
    /// The config as a value tree, in the same shape as `config.toml`.
    pub fn to_value(&self) -> Value {
        let s = |v: &str| Value::String(v.into());
        let mut ui = BTreeMap::new();
        let side = match self.ui.side_panel {
            SidePosition::Left => "left",
            SidePosition::Right => "right",
        };
        let notify = match self.ui.notify_position {
            NotifyPosition::TopRight => "top-right",
            NotifyPosition::BottomRight => "bottom-right",
        };
        ui.insert("side_panel".into(), s(side));
        ui.insert(
            "side_width".into(),
            Value::Integer(self.ui.side_width.into()),
        );
        ui.insert(
            "bottom_height".into(),
            Value::Integer(self.ui.bottom_height.into()),
        );
        ui.insert(
            "modal_width".into(),
            Value::Integer(self.ui.modal_width.into()),
        );
        ui.insert(
            "modal_height".into(),
            Value::Integer(self.ui.modal_height.into()),
        );
        ui.insert(
            "notify_timeout_ms".into(),
            Value::Integer(self.ui.notify_timeout_ms as i64),
        );
        ui.insert("notify_position".into(), s(notify));
        let manager = match self.ui.plugin_manager {
            ManagerPlace::Modal => "modal",
            ManagerPlace::Side => "side",
        };
        ui.insert("plugin_manager".into(), s(manager));
        let mut keymap: BTreeMap<String, Value> = BTreeMap::new();
        for (mode, lhs, rhs) in &self.keymaps {
            let table = keymap
                .entry(mode_name(*mode).into())
                .or_insert_with(|| Value::Table(BTreeMap::new()));
            if let Value::Table(t) = table {
                t.insert(lhs.clone(), s(rhs));
            }
        }
        let plugins = self
            .plugins
            .iter()
            .map(|(k, v)| (k.clone(), Value::Table(v.clone())))
            .collect();
        let mut top = BTreeMap::new();
        top.insert("leader".into(), s(&self.leader));
        top.insert("timeout_ms".into(), Value::Integer(self.timeout_ms as i64));
        top.insert("developer_mode".into(), Value::Bool(self.developer_mode));
        top.insert("ui".into(), Value::Table(ui));
        if !keymap.is_empty() {
            top.insert("keymap".into(), Value::Table(keymap));
        }
        if !self.plugins.is_empty() {
            top.insert("plugins".into(), Value::Table(plugins));
        }
        Value::Table(top)
    }
}

/// Look up a dotted key (`ui.side_width`) in a value tree.
pub fn get<'a>(root: &'a Value, dotted: &str) -> Option<&'a Value> {
    dotted.split('.').try_fold(root, |v, k| match v {
        Value::Table(t) => t.get(k),
        _ => None,
    })
}

/// A setting value as a config value.
pub fn to_config_value(v: &SettingValue) -> Value {
    match v {
        SettingValue::String(s) => Value::String(s.clone()),
        SettingValue::Integer(n) => Value::Integer(*n),
        SettingValue::Bool(b) => Value::Bool(*b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(p: &str) -> Vec<String> {
        p.split('.').map(String::from).collect()
    }

    #[test]
    fn schema_matches_config_both_ways() {
        // Every schema default is accepted by Config::from_value and gives the default config.
        let mut v = Value::Table(BTreeMap::new());
        let edits: Vec<_> = vox_settings()
            .iter()
            .map(|s| ConfigEdit::Set(path(&s.key), to_config_value(&s.default)))
            .collect();
        apply_edits(&mut v, &edits);
        let (cfg, errors) = Config::from_value(&v);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(cfg, Config::default());
        // Every value in the config tree has a schema entry (keymap and plugins aside).
        let schema: Vec<String> = vox_settings().into_iter().map(|s| s.key).collect();
        let Value::Table(top) = Config::default().to_value() else {
            panic!()
        };
        for (k, v) in &top {
            match v {
                Value::Table(t) => {
                    for sub in t.keys() {
                        assert!(
                            schema.contains(&format!("{k}.{sub}")),
                            "{k}.{sub} missing from schema"
                        );
                    }
                }
                _ => assert!(schema.contains(k), "{k} missing from schema"),
            }
        }
        // Each schema entry's check agrees with from_value's limits.
        for s in vox_settings() {
            if let SettingKind::Integer { min: Some(min), .. } = s.kind {
                let mut bad = Value::Table(BTreeMap::new());
                apply_edits(
                    &mut bad,
                    &[ConfigEdit::Set(path(&s.key), Value::Integer(min - 1))],
                );
                assert!(
                    !Config::from_value(&bad).1.is_empty(),
                    "{} below min accepted",
                    s.key
                );
            }
        }
    }

    #[test]
    fn to_value_round_trips() {
        let mut cfg = Config {
            leader: " ".into(),
            ..Config::default()
        };
        cfg.ui.side_panel = SidePosition::Left;
        cfg.keymaps
            .push((MapMode::Insert, "jk".into(), "<Esc>".into()));
        cfg.plugins.insert(
            "p".into(),
            BTreeMap::from([("x".to_string(), Value::Integer(1))]),
        );
        let (back, errors) = Config::from_value(&cfg.to_value());
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(back, cfg);
    }

    #[test]
    fn edits_set_and_remove() {
        let mut v = Value::Table(BTreeMap::new());
        apply_edits(
            &mut v,
            &[
                ConfigEdit::Set(path("ui.side_width"), Value::Integer(30)),
                ConfigEdit::Set(
                    vec!["keymap".into(), "normal".into(), "<leader>w".into()],
                    Value::String(":w".into()),
                ),
                ConfigEdit::Remove(path("nothing.here")),
            ],
        );
        assert_eq!(get(&v, "ui.side_width"), Some(&Value::Integer(30)));
        assert_eq!(
            get(&v, "keymap.normal.<leader>w"),
            Some(&Value::String(":w".into()))
        );
        apply_edits(&mut v, &[ConfigEdit::Remove(path("ui.side_width"))]);
        assert_eq!(get(&v, "ui.side_width"), None);
        assert_eq!(parse_mode("v"), Some(MapMode::Visual));
        assert_eq!(mode_name(MapMode::Insert), "insert");
    }
}
