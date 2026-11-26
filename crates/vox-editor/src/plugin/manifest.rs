//! `plugin.toml`: a plugin's name, version, requirements, and settings.
//!
//! ```toml
//! [plugin]
//! name = "text-tools"
//! version = "0.2.0"
//! description = "Word count and case changes"
//! authors = ["Jane"]
//! api = "0.2"              # plugin API version it was built for
//! vox = ">=0.10"           # optional: needed vox version (an error if not met)
//!
//! [[requires]]
//! plugin = "other"         # or: program = "git"   or: vox = ">=0.11"
//! version = ">=0.2"        # for plugin requirements
//! level = "warning"        # error (not loaded) | warning | info
//! reason = "shows X in Y"
//!
//! [settings.case]
//! type = "choice"          # string | integer | bool | choice
//! choices = ["upper", "lower", "title"]
//! default = "upper"
//! description = "What :upper does"
//! ```

use std::collections::BTreeMap;

use super::version::{Version, VersionReq};
use crate::config::Value;
use crate::ui::Level;

/// What a requirement asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Need {
    /// Another plugin, optionally in a version range.
    Plugin {
        /// Its name.
        name: String,
        /// Allowed versions.
        version: Option<VersionReq>,
    },
    /// A vox version.
    Vox(VersionReq),
    /// A program on `PATH`.
    Program(String),
}

/// One `[[requires]]` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    /// What is needed.
    pub need: Need,
    /// How bad it is when missing: `Error` = the plugin is not loaded.
    pub level: Level,
    /// Why it is needed (shown in the health report).
    pub reason: String,
}

/// The type of a setting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingKind {
    /// Text.
    String,
    /// Whole number, optionally limited.
    Integer {
        /// Smallest allowed.
        min: Option<i64>,
        /// Largest allowed.
        max: Option<i64>,
    },
    /// On / off.
    Bool,
    /// One of a fixed list.
    Choice(Vec<String>),
}

/// A setting's value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingValue {
    /// Text (also used for choices).
    String(String),
    /// Whole number.
    Integer(i64),
    /// On / off.
    Bool(bool),
}

impl std::fmt::Display for SettingValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingValue::String(s) => write!(f, "{s:?}"),
            SettingValue::Integer(n) => write!(f, "{n}"),
            SettingValue::Bool(b) => write!(f, "{b}"),
        }
    }
}

/// One `[settings.KEY]` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingSpec {
    /// Key, e.g. `case`.
    pub key: String,
    /// Type and limits.
    pub kind: SettingKind,
    /// Value when the user sets nothing.
    pub default: SettingValue,
    /// What it does.
    pub description: String,
}

impl SettingSpec {
    /// Check a user value against this spec.
    pub fn check(&self, v: &Value) -> Result<SettingValue, String> {
        let key = &self.key;
        match (&self.kind, v) {
            (SettingKind::String, Value::String(s)) => Ok(SettingValue::String(s.clone())),
            (SettingKind::Bool, Value::Bool(b)) => Ok(SettingValue::Bool(*b)),
            (SettingKind::Integer { min, max }, Value::Integer(n)) => {
                if min.is_some_and(|m| *n < m) || max.is_some_and(|m| *n > m) {
                    let lo = min.map_or("…".into(), |m| m.to_string());
                    let hi = max.map_or("…".into(), |m| m.to_string());
                    Err(format!("`{key}` must be between {lo} and {hi}"))
                } else {
                    Ok(SettingValue::Integer(*n))
                }
            }
            (SettingKind::Choice(choices), Value::String(s)) if choices.contains(s) => {
                Ok(SettingValue::String(s.clone()))
            }
            (SettingKind::Choice(choices), _) => {
                Err(format!("`{key}` must be one of: {}", choices.join(", ")))
            }
            (kind, _) => Err(format!("`{key}` must be {}", kind_name(kind))),
        }
    }
}

/// Human name of a setting type, for error messages.
fn kind_name(k: &SettingKind) -> &'static str {
    match k {
        SettingKind::String => "a string",
        SettingKind::Integer { .. } => "an integer",
        SettingKind::Bool => "true or false",
        SettingKind::Choice(_) => "a choice",
    }
}

/// A parsed `plugin.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifest {
    /// Name; must match the plugin's folder name.
    pub name: String,
    /// Version.
    pub version: Version,
    /// One-line description.
    pub description: String,
    /// Authors.
    pub authors: Vec<String>,
    /// Plugin API version it was built for (`wit/vox.wit` package version).
    pub api: Version,
    /// Requirements, including `[plugin] vox` as an error-level requirement.
    pub requires: Vec<Requirement>,
    /// Declared settings, in key order.
    pub settings: Vec<SettingSpec>,
}

/// Get a string field.
fn string(t: &BTreeMap<String, Value>, key: &str, errors: &mut Vec<String>) -> Option<String> {
    match t.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => {
            errors.push(format!("`{key}` must be a string"));
            None
        }
        None => None,
    }
}

impl PluginManifest {
    /// Read a manifest from a parsed `plugin.toml`. Collects all problems.
    pub fn from_value(v: &Value) -> Result<PluginManifest, Vec<String>> {
        let mut errors = Vec::new();
        let Value::Table(top) = v else {
            return Err(vec!["plugin.toml must be a table".into()]);
        };
        let empty = BTreeMap::new();
        let plugin = match top.get("plugin") {
            Some(Value::Table(t)) => t,
            _ => {
                errors.push("missing [plugin] section".into());
                &empty
            }
        };
        let name = string(plugin, "name", &mut errors).unwrap_or_default();
        if name.is_empty() {
            errors.push("[plugin] needs `name`".into());
        }
        let parse_version = |key: &str, errors: &mut Vec<String>| {
            string(plugin, key, errors).and_then(|s| match Version::parse(&s) {
                Ok(v) => Some(v),
                Err(e) => {
                    errors.push(format!("[plugin] `{key}`: {e}"));
                    None
                }
            })
        };
        let version = parse_version("version", &mut errors);
        let api = parse_version("api", &mut errors);
        if version.is_none() && !plugin.contains_key("version") {
            errors.push("[plugin] needs `version`".into());
        }
        if api.is_none() && !plugin.contains_key("api") {
            errors.push("[plugin] needs `api` (the plugin API version, e.g. \"0.2\")".into());
        }
        let description = string(plugin, "description", &mut errors).unwrap_or_default();
        let authors = match plugin.get("authors") {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            Some(_) => {
                errors.push("`authors` must be a list of strings".into());
                Vec::new()
            }
            None => Vec::new(),
        };
        let mut requires = Vec::new();
        if let Some(req) = string(plugin, "vox", &mut errors) {
            match VersionReq::parse(&req) {
                Ok(r) => requires.push(Requirement {
                    need: Need::Vox(r),
                    level: Level::Error,
                    reason: String::new(),
                }),
                Err(e) => errors.push(format!("[plugin] `vox`: {e}")),
            }
        }
        match top.get("requires") {
            Some(Value::Array(items)) => {
                for (i, item) in items.iter().enumerate() {
                    match parse_requirement(item) {
                        Ok(r) => requires.push(r),
                        Err(e) => errors.push(format!("[[requires]] #{}: {e}", i + 1)),
                    }
                }
            }
            Some(_) => errors.push("`requires` must be a list of [[requires]] tables".into()),
            None => {}
        }
        let mut settings = Vec::new();
        match top.get("settings") {
            Some(Value::Table(t)) => {
                for (key, spec) in t {
                    match parse_setting(key, spec) {
                        Ok(s) => settings.push(s),
                        Err(e) => errors.push(format!("[settings.{key}]: {e}")),
                    }
                }
            }
            Some(_) => errors.push("`settings` must be a table".into()),
            None => {}
        }
        for key in top.keys() {
            if !["plugin", "requires", "settings"].contains(&key.as_str()) {
                errors.push(format!("unknown section `{key}`"));
            }
        }
        match (errors.is_empty(), version, api) {
            (true, Some(version), Some(api)) => Ok(PluginManifest {
                name,
                version,
                description,
                authors,
                api,
                requires,
                settings,
            }),
            _ => Err(errors),
        }
    }

    /// The spec of setting `key`.
    pub fn setting(&self, key: &str) -> Option<&SettingSpec> {
        self.settings.iter().find(|s| s.key == key)
    }
}

/// Read one `[[requires]]` table.
fn parse_requirement(v: &Value) -> Result<Requirement, String> {
    let Value::Table(t) = v else {
        return Err("must be a table".into());
    };
    let mut errs = Vec::new();
    let level = match string(t, "level", &mut errs).as_deref() {
        None | Some("error") => Level::Error,
        Some("warning" | "warn") => Level::Warn,
        Some("info") => Level::Info,
        Some(other) => return Err(format!("level `{other}` must be error, warning, or info")),
    };
    let reason = string(t, "reason", &mut errs).unwrap_or_default();
    let req = |s: Option<String>| -> Result<Option<VersionReq>, String> {
        s.map(|s| VersionReq::parse(&s)).transpose()
    };
    let need = match (
        string(t, "plugin", &mut errs),
        string(t, "program", &mut errs),
        string(t, "vox", &mut errs),
    ) {
        (Some(name), None, None) => Need::Plugin {
            name,
            version: req(string(t, "version", &mut errs))?,
        },
        (None, Some(p), None) => Need::Program(p),
        (None, None, Some(v)) => Need::Vox(VersionReq::parse(&v)?),
        _ => return Err("needs exactly one of `plugin`, `program`, `vox`".into()),
    };
    match errs.first() {
        Some(e) => Err(e.clone()),
        None => Ok(Requirement {
            need,
            level,
            reason,
        }),
    }
}

/// Read one `[settings.KEY]` table, checking its default against its type.
fn parse_setting(key: &str, v: &Value) -> Result<SettingSpec, String> {
    let Value::Table(t) = v else {
        return Err("must be a table".into());
    };
    let mut errs = Vec::new();
    let int = |k: &str| match t.get(k) {
        Some(Value::Integer(n)) => Ok(Some(*n)),
        Some(_) => Err(format!("`{k}` must be an integer")),
        None => Ok(None),
    };
    let kind = match string(t, "type", &mut errs).as_deref() {
        Some("string") => SettingKind::String,
        Some("integer") => SettingKind::Integer {
            min: int("min")?,
            max: int("max")?,
        },
        Some("bool") => SettingKind::Bool,
        Some("choice") => match t.get("choices") {
            Some(Value::Array(a)) if !a.is_empty() => SettingKind::Choice(
                a.iter()
                    .map(|v| match v {
                        Value::String(s) => Ok(s.clone()),
                        _ => Err("`choices` must be strings".to_string()),
                    })
                    .collect::<Result<_, _>>()?,
            ),
            _ => return Err("type choice needs a non-empty `choices` list".into()),
        },
        Some(other) => {
            return Err(format!(
                "unknown type `{other}` (string, integer, bool, choice)"
            ));
        }
        None => return Err("needs `type`".into()),
    };
    let description = string(t, "description", &mut errs).unwrap_or_default();
    if let Some(e) = errs.first() {
        return Err(e.clone());
    }
    let mut spec = SettingSpec {
        key: key.to_string(),
        kind,
        default: SettingValue::Bool(false),
        description,
    };
    spec.default = match t.get("default") {
        Some(v) => spec.check(v).map_err(|e| format!("default: {e}"))?,
        None => return Err("needs `default`".into()),
    };
    Ok(spec)
}

/// Effective settings: manifest defaults overridden by valid user values from
/// `[plugins.NAME]`. Returns the values and one warning per ignored user value.
/// The `enabled` key is not a setting and is skipped.
pub fn resolve_settings(
    specs: &[SettingSpec],
    user: Option<&BTreeMap<String, Value>>,
) -> (Vec<(String, SettingValue)>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut values: Vec<(String, SettingValue)> = specs
        .iter()
        .map(|s| (s.key.clone(), s.default.clone()))
        .collect();
    for (key, v) in user.into_iter().flatten() {
        if key == "enabled" {
            continue;
        }
        let Some(spec) = specs.iter().find(|s| s.key == *key) else {
            warnings.push(format!("unknown setting `{key}` ignored"));
            continue;
        };
        match spec.check(v) {
            Ok(val) => {
                if let Some(slot) = values.iter_mut().find(|(k, _)| k == key) {
                    slot.1 = val;
                }
            }
            Err(e) => warnings.push(format!("{e}; using default {}", spec.default)),
        }
    }
    (values, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Value {
        Value::String(v.into())
    }

    fn t(entries: Vec<(&str, Value)>) -> Value {
        Value::Table(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

    fn full() -> Value {
        t(vec![
            (
                "plugin",
                t(vec![
                    ("name", s("text-tools")),
                    ("version", s("0.2.0")),
                    ("api", s("0.2")),
                    ("vox", s(">=0.10")),
                    ("description", s("Word count")),
                    ("authors", Value::Array(vec![s("Jane")])),
                ]),
            ),
            (
                "requires",
                Value::Array(vec![
                    t(vec![
                        ("plugin", s("base")),
                        ("version", s(">=1")),
                        ("level", s("warning")),
                    ]),
                    t(vec![
                        ("program", s("git")),
                        ("level", s("info")),
                        ("reason", s("diffs")),
                    ]),
                ]),
            ),
            (
                "settings",
                t(vec![
                    (
                        "case",
                        t(vec![
                            ("type", s("choice")),
                            ("choices", Value::Array(vec![s("upper"), s("lower")])),
                            ("default", s("upper")),
                        ]),
                    ),
                    (
                        "limit",
                        t(vec![
                            ("type", s("integer")),
                            ("min", Value::Integer(1)),
                            ("max", Value::Integer(10)),
                            ("default", Value::Integer(5)),
                        ]),
                    ),
                ]),
            ),
        ])
    }

    #[test]
    fn parses_full_manifest() {
        let m = PluginManifest::from_value(&full()).unwrap();
        assert_eq!(m.name, "text-tools");
        assert_eq!(m.version.to_string(), "0.2.0");
        assert_eq!(m.api.to_string(), "0.2.0");
        assert_eq!(m.authors, ["Jane"]);
        assert_eq!(m.requires.len(), 3, "vox + 2 [[requires]]");
        assert_eq!(m.requires[0].level, Level::Error);
        assert!(
            matches!(&m.requires[1].need, Need::Plugin { name, version: Some(_) } if name == "base")
        );
        assert_eq!(m.requires[1].level, Level::Warn);
        assert_eq!(m.requires[2].need, Need::Program("git".into()));
        assert_eq!(m.settings.len(), 2);
        assert_eq!(
            m.setting("limit").unwrap().default,
            SettingValue::Integer(5)
        );
    }

    #[test]
    fn collects_errors() {
        let bad = t(vec![
            ("plugin", t(vec![("name", s("x")), ("version", s("one"))])),
            (
                "requires",
                Value::Array(vec![t(vec![("plugin", s("a")), ("program", s("b"))])]),
            ),
            (
                "settings",
                t(vec![(
                    "n",
                    t(vec![("type", s("integer")), ("default", s("x"))]),
                )]),
            ),
            ("extra", t(vec![])),
        ]);
        let errors = PluginManifest::from_value(&bad).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("`version`")), "{errors:?}");
        assert!(
            errors.iter().any(|e| e.contains("needs `api`")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.contains("exactly one")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.contains("[settings.n]: default")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.contains("unknown section `extra`")),
            "{errors:?}"
        );
        assert!(PluginManifest::from_value(&t(vec![])).is_err());
    }

    #[test]
    fn settings_resolution() {
        let m = PluginManifest::from_value(&full()).unwrap();
        let user: BTreeMap<String, Value> = [
            ("case".to_string(), s("lower")),
            ("limit".to_string(), Value::Integer(99)),
            ("enabled".to_string(), Value::Bool(true)),
            ("nope".to_string(), Value::Bool(true)),
        ]
        .into();
        let (values, warnings) = resolve_settings(&m.settings, Some(&user));
        assert_eq!(
            values,
            [
                ("case".to_string(), SettingValue::String("lower".into())),
                ("limit".to_string(), SettingValue::Integer(5)),
            ]
        );
        assert_eq!(
            warnings,
            [
                "`limit` must be between 1 and 10; using default 5",
                "unknown setting `nope` ignored"
            ]
        );
        let (values, warnings) = resolve_settings(&m.settings, None);
        assert_eq!(values[0].1, SettingValue::String("upper".into()));
        assert!(warnings.is_empty());
    }

    #[test]
    fn setting_checks() {
        let m = PluginManifest::from_value(&full()).unwrap();
        let case = m.setting("case").unwrap();
        assert_eq!(
            case.check(&s("title")).unwrap_err(),
            "`case` must be one of: upper, lower"
        );
        assert_eq!(
            m.setting("limit")
                .unwrap()
                .check(&Value::Bool(true))
                .unwrap_err(),
            "`limit` must be an integer"
        );
    }
}
