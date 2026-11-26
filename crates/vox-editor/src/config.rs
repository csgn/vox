//! User configuration. Core only understands a format-neutral [`Value`] tree; reading a
//! file format (TOML) is done by an adapter (`vox-config-file`).

use std::collections::BTreeMap;

use crate::keymap::MapMode;
use crate::ui::{ManagerPlace, NotifyPosition, SidePosition, UiConfig};

/// Parses a file's text (e.g. TOML) into a [`Value`]; provided by an adapter.
pub type ValueParser = fn(&str) -> Result<Value, String>;

/// Formats a [`Value`] as file text (e.g. TOML); provided by an adapter.
pub type ValueFormatter = fn(&Value) -> Result<String, String>;

/// A format-neutral configuration value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Text.
    String(String),
    /// Whole number.
    Integer(i64),
    /// Decimal number.
    Float(f64),
    /// `true` / `false`.
    Bool(bool),
    /// List.
    Array(Vec<Value>),
    /// Named values.
    Table(BTreeMap<String, Value>),
}

impl Value {
    /// Human name of the value's type, for error messages.
    fn type_name(&self) -> &'static str {
        match self {
            Value::String(_) => "a string",
            Value::Integer(_) => "an integer",
            Value::Float(_) => "a number",
            Value::Bool(_) => "true/false",
            Value::Array(_) => "a list",
            Value::Table(_) => "a table",
        }
    }

    /// A table from `(key, value)` pairs, e.g. for a command's reply.
    pub fn table<K: Into<String>>(pairs: impl IntoIterator<Item = (K, Value)>) -> Value {
        Value::Table(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    /// The value as readable text: a scalar as itself, tables as `key: value` lines, lists
    /// as `- item` lines, nested values indented by two spaces.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        match self.scalar_text() {
            Some(s) => out.push_str(&s),
            None => write_text(self, 0, &mut out),
        }
        out.truncate(out.trim_end().len());
        out
    }

    /// Text of a value that fits on one line (scalars, empty tables and lists).
    fn scalar_text(&self) -> Option<String> {
        match self {
            Value::String(s) if s.is_empty() => Some("\"\"".into()),
            Value::String(s) => Some(s.clone()),
            Value::Integer(n) => Some(n.to_string()),
            Value::Float(f) => Some(f.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            Value::Array(a) if a.is_empty() => Some("[]".into()),
            Value::Table(t) if t.is_empty() => Some("{}".into()),
            Value::Array(_) | Value::Table(_) => None,
        }
    }
}

/// Write a table or list as lines indented by `indent` (see [`Value::to_text`]).
fn write_text(v: &Value, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    match v {
        Value::Table(t) => {
            for (k, v) in t {
                match v.scalar_text() {
                    Some(s) => out.push_str(&format!("{pad}{k}: {s}\n")),
                    None => {
                        out.push_str(&format!("{pad}{k}:\n"));
                        write_text(v, indent + 2, out);
                    }
                }
            }
        }
        Value::Array(a) => {
            for item in a {
                match item.scalar_text() {
                    Some(s) => out.push_str(&format!("{pad}- {s}\n")),
                    None => {
                        // The item's first line goes after the `- `.
                        let mut inner = String::new();
                        write_text(item, indent + 2, &mut inner);
                        out.push_str(&format!("{pad}- {}", &inner[indent + 2..]));
                    }
                }
            }
        }
        _ => {}
    }
}

impl From<&str> for Value {
    /// A string value.
    fn from(s: &str) -> Self {
        Value::String(s.into())
    }
}

impl From<String> for Value {
    /// A string value.
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<i64> for Value {
    /// An integer value.
    fn from(n: i64) -> Self {
        Value::Integer(n)
    }
}

impl From<bool> for Value {
    /// A bool value.
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

/// Validated settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// What `<leader>` means in mappings. Default `\`.
    pub leader: String,
    /// How long to wait for the rest of a mapping, in milliseconds. Default 1000.
    pub timeout_ms: u64,
    /// Key mappings: (mode, left side, right side), in Vim notation.
    pub keymaps: Vec<(MapMode, String, String)>,
    /// Panels, modal, and notifications (`[ui]`).
    pub ui: UiConfig,
    /// Developer mode: log shortcuts and reload shortcuts.
    pub developer_mode: bool,
    /// Look for a newer vox release at start-up. Default true.
    pub check_updates: bool,
    /// Per-plugin values (`[plugins.NAME]`): settings and `enabled`. Checked against each
    /// plugin's manifest when it loads.
    pub plugins: BTreeMap<String, BTreeMap<String, Value>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            leader: "\\".into(),
            timeout_ms: 1000,
            keymaps: Vec::new(),
            ui: UiConfig::default(),
            developer_mode: false,
            check_updates: true,
            plugins: BTreeMap::new(),
        }
    }
}

impl Config {
    /// Build a config from a parsed file. Unknown settings and wrong types are errors;
    /// all errors are collected, and valid settings are still applied.
    pub fn from_value(value: &Value) -> (Config, Vec<String>) {
        let mut cfg = Config::default();
        let mut errors = Vec::new();
        let Value::Table(top) = value else {
            return (cfg, vec!["config must be a table".into()]);
        };
        for (key, v) in top {
            match (key.as_str(), v) {
                ("leader", Value::String(s)) if !s.is_empty() => cfg.leader = s.clone(),
                ("leader", Value::String(_)) => errors.push("`leader` must not be empty".into()),
                ("timeout_ms", Value::Integer(n)) if *n >= 0 => cfg.timeout_ms = *n as u64,
                ("timeout_ms", Value::Integer(_)) => {
                    errors.push("`timeout_ms` must not be negative".into())
                }
                ("keymap", Value::Table(modes)) => {
                    for (mode, maps) in modes {
                        let m = match mode.as_str() {
                            "normal" => MapMode::Normal,
                            "insert" => MapMode::Insert,
                            "visual" => MapMode::Visual,
                            other => {
                                errors.push(format!(
                                    "unknown keymap mode `{other}` (use normal, insert, visual)"
                                ));
                                continue;
                            }
                        };
                        let Value::Table(maps) = maps else {
                            errors.push(format!("`keymap.{mode}` must be a table"));
                            continue;
                        };
                        for (lhs, rhs) in maps {
                            match rhs {
                                Value::String(_) if lhs.is_empty() => {
                                    errors.push(format!("`keymap.{mode}`: empty key"))
                                }
                                Value::String(r) => cfg.keymaps.push((m, lhs.clone(), r.clone())),
                                other => errors.push(format!(
                                    "`keymap.{mode}.\"{lhs}\"` must be a string, not {}",
                                    other.type_name()
                                )),
                            }
                        }
                    }
                }
                ("ui", Value::Table(t)) => parse_ui(t, &mut cfg.ui, &mut errors),
                ("developer_mode", Value::Bool(b)) => cfg.developer_mode = *b,
                ("check_updates", Value::Bool(b)) => cfg.check_updates = *b,
                ("plugins", Value::Table(t)) => {
                    for (name, v) in t {
                        match v {
                            Value::Table(values) => {
                                if let Some(e) = values
                                    .get("enabled")
                                    .filter(|e| !matches!(e, Value::Bool(_)))
                                {
                                    errors.push(format!(
                                        "`plugins.{name}.enabled` must be true/false, not {}",
                                        e.type_name()
                                    ));
                                }
                                cfg.plugins.insert(name.clone(), values.clone());
                            }
                            other => errors.push(format!(
                                "`plugins.{name}` must be a table, not {}",
                                other.type_name()
                            )),
                        }
                    }
                }
                (
                    "leader" | "timeout_ms" | "keymap" | "ui" | "developer_mode" | "check_updates"
                    | "plugins",
                    other,
                ) => {
                    let want = match key.as_str() {
                        "leader" => "a string",
                        "timeout_ms" => "an integer",
                        "developer_mode" | "check_updates" => "true/false",
                        "plugins" => "a table",
                        _ => "a table",
                    };
                    errors.push(format!("`{key}` must be {want}, not {}", other.type_name()));
                }
                (other, _) => errors.push(format!("unknown setting `{other}`")),
            }
        }
        (cfg, errors)
    }
}

/// Read `[ui]` settings into `ui`, collecting errors.
fn parse_ui(t: &BTreeMap<String, Value>, ui: &mut UiConfig, errors: &mut Vec<String>) {
    for (key, v) in t {
        let int = |min: i64, max: i64| match v {
            Value::Integer(n) if (min..=max).contains(n) => Ok(*n),
            Value::Integer(_) => Err(format!("`ui.{key}` must be between {min} and {max}")),
            other => Err(format!(
                "`ui.{key}` must be an integer, not {}",
                other.type_name()
            )),
        };
        let result = match key.as_str() {
            "side_panel" => match v {
                Value::String(s) if s == "left" => Some(SidePosition::Left),
                Value::String(s) if s == "right" => Some(SidePosition::Right),
                _ => None,
            }
            .map(|p| ui.side_panel = p)
            .ok_or_else(|| "`ui.side_panel` must be \"left\" or \"right\"".to_string()),
            "notify_position" => match v {
                Value::String(s) if s == "top-right" => Some(NotifyPosition::TopRight),
                Value::String(s) if s == "bottom-right" => Some(NotifyPosition::BottomRight),
                _ => None,
            }
            .map(|p| ui.notify_position = p)
            .ok_or_else(|| {
                "`ui.notify_position` must be \"top-right\" or \"bottom-right\"".to_string()
            }),
            "plugin_manager" => match v {
                Value::String(s) if s == "modal" => Some(ManagerPlace::Modal),
                Value::String(s) if s == "side" => Some(ManagerPlace::Side),
                _ => None,
            }
            .map(|p| ui.plugin_manager = p)
            .ok_or_else(|| "`ui.plugin_manager` must be \"modal\" or \"side\"".to_string()),
            "side_width" => int(10, 500).map(|n| ui.side_width = n as u16),
            "bottom_height" => int(3, 200).map(|n| ui.bottom_height = n as u16),
            "modal_width" => int(10, 100).map(|n| ui.modal_width = n as u16),
            "modal_height" => int(10, 100).map(|n| ui.modal_height = n as u16),
            "notify_timeout_ms" => int(0, 600_000).map(|n| ui.notify_timeout_ms = n as u64),
            other => Err(format!("unknown setting `ui.{other}`")),
        };
        if let Err(e) = result {
            errors.push(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_as_text() {
        assert_eq!(Value::from("hi").to_text(), "hi");
        assert_eq!(Value::from("").to_text(), "\"\"");
        assert_eq!(Value::Array(vec![]).to_text(), "[]");
        let v = Value::table([
            ("n", Value::from(1)),
            (
                "list",
                Value::Array(vec![
                    1.into(),
                    Value::table([("a", true.into()), ("b", Value::Float(0.5))]),
                ]),
            ),
            (
                "t",
                Value::table([("x", Value::table(Vec::<(&str, Value)>::new()))]),
            ),
        ]);
        assert_eq!(
            v.to_text(),
            "list:\n  - 1\n  - a: true\n    b: 0.5\nn: 1\nt:\n  x: {}"
        );
        let nested = Value::Array(vec![Value::Array(vec!["a".into(), "b".into()])]);
        assert_eq!(nested.to_text(), "- - a\n  - b");
    }

    fn table(entries: &[(&str, Value)]) -> Value {
        Value::Table(
            entries
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        )
    }
    fn s(v: &str) -> Value {
        Value::String(v.into())
    }

    #[test]
    fn defaults_from_empty() {
        let (cfg, errors) = Config::from_value(&table(&[]));
        assert_eq!(cfg, Config::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn full_config() {
        let v = table(&[
            ("leader", s(" ")),
            ("timeout_ms", Value::Integer(300)),
            (
                "keymap",
                table(&[
                    ("normal", table(&[("<leader>w", s(":w"))])),
                    ("insert", table(&[("jk", s("<Esc>"))])),
                ]),
            ),
        ]);
        let (cfg, errors) = Config::from_value(&v);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(cfg.leader, " ");
        assert_eq!(cfg.timeout_ms, 300);
        assert_eq!(
            cfg.keymaps,
            vec![
                (MapMode::Insert, "jk".into(), "<Esc>".into()),
                (MapMode::Normal, "<leader>w".into(), ":w".into()),
            ]
        );
    }

    #[test]
    fn plugins_section() {
        let v = table(&[(
            "plugins",
            table(&[
                (
                    "text-tools",
                    table(&[("case", s("lower")), ("enabled", Value::Bool(false))]),
                ),
                ("bad", s("x")),
                ("worse", table(&[("enabled", s("no"))])),
            ]),
        )]);
        let (cfg, errors) = Config::from_value(&v);
        assert_eq!(cfg.plugins["text-tools"]["case"], s("lower"));
        assert_eq!(
            errors,
            [
                "`plugins.bad` must be a table, not a string",
                "`plugins.worse.enabled` must be true/false, not a string"
            ]
        );
    }

    #[test]
    fn ui_section() {
        let v = table(&[(
            "ui",
            table(&[
                ("side_panel", s("left")),
                ("side_width", Value::Integer(30)),
                ("notify_position", s("bottom-right")),
                ("modal_width", Value::Integer(5)),
                ("colour", s("x")),
            ]),
        )]);
        let (cfg, errors) = Config::from_value(&v);
        assert_eq!(cfg.ui.side_panel, SidePosition::Left);
        assert_eq!(cfg.ui.side_width, 30);
        assert_eq!(cfg.ui.notify_position, NotifyPosition::BottomRight);
        assert_eq!(cfg.ui.modal_width, UiConfig::default().modal_width);
        assert_eq!(
            errors,
            [
                "unknown setting `ui.colour`",
                "`ui.modal_width` must be between 10 and 100"
            ]
        );
    }

    #[test]
    fn errors_are_collected_and_valid_parts_kept() {
        let v = table(&[
            ("leader", Value::Integer(1)),
            ("colour", s("red")),
            ("timeout_ms", Value::Integer(-5)),
            (
                "keymap",
                table(&[
                    ("normal", table(&[("H", s("0")), ("L", Value::Bool(true))])),
                    ("command", table(&[])),
                ]),
            ),
        ]);
        let (cfg, errors) = Config::from_value(&v);
        assert_eq!(cfg.keymaps, vec![(MapMode::Normal, "H".into(), "0".into())]);
        assert_eq!(
            errors,
            vec![
                "unknown setting `colour`",
                "unknown keymap mode `command` (use normal, insert, visual)",
                "`keymap.normal.\"L\"` must be a string, not true/false",
                "`leader` must be a string, not an integer",
                "`timeout_ms` must not be negative",
            ]
        );
    }
}
