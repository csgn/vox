//! User configuration. Core only understands a format-neutral [`Value`] tree; reading a
//! file format (TOML) is done by an adapter (`vox-config-file`).

use std::collections::BTreeMap;

use crate::keymap::MapMode;
use crate::ui::{NotifyPosition, SidePosition, UiConfig};

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
}

/// Validated settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// What `<leader>` means in mappings. Default `\`.
    pub leader: String,
    /// How long to wait for the rest of a mapping, in milliseconds. Default 1000.
    pub timeout_ms: u64,
    /// Key mappings: (mode, left side, right side), in Vim notation.
    pub keymaps: Vec<(MapMode, String, String)>,
    /// Panels, modal, and notifications (`[ui]`).
    pub ui: UiConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            leader: "\\".into(),
            timeout_ms: 1000,
            keymaps: Vec::new(),
            ui: UiConfig::default(),
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
                ("leader" | "timeout_ms" | "keymap" | "ui", other) => {
                    let want = match key.as_str() {
                        "leader" => "a string",
                        "timeout_ms" => "an integer",
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
