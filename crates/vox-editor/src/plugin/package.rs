//! Finding installed plugins and deciding which can load.
//!
//! A plugin in a scope's plugin folder is either
//! - a package folder `NAME/` with `plugin.toml` and `plugin.wasm` (the standard layout), or
//! - a single file `NAME.wasm` (no manifest: no version, settings, or requirements).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::health::{self, HealthEnv, HealthItem, HealthStatus};
use super::manifest::{PluginManifest, SettingValue, resolve_settings};
use super::version::Version;
use crate::config::{Value, ValueParser};
use crate::ui::Level;

/// An installed plugin as found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// Name: the folder name, or the file name without `.wasm`.
    pub name: String,
    /// The folder or `.wasm` file.
    pub path: PathBuf,
    /// The WebAssembly file to run.
    pub wasm: PathBuf,
    /// The manifest, if the package has a valid one.
    pub manifest: Option<PluginManifest>,
    /// Problems with the package itself (missing files, bad manifest, name mismatch).
    pub items: Vec<HealthItem>,
}

/// Read the package at `path` (a folder or a `.wasm` file).
pub fn read_package(path: &Path, parse: ValueParser) -> Package {
    let is_dir = path.is_dir();
    let name = if is_dir {
        path.file_name()
    } else {
        path.file_stem()
    }
    .map(|n| n.to_string_lossy().into_owned())
    .unwrap_or_default();
    let mut items = Vec::new();
    if !is_dir {
        items.push(HealthItem::ok(
            "single-file plugin (no plugin.toml: no version, settings, or requirements)",
        ));
        return Package {
            name,
            path: path.to_path_buf(),
            wasm: path.to_path_buf(),
            manifest: None,
            items,
        };
    }
    let wasm = path.join("plugin.wasm");
    if wasm.is_file() {
        items.push(HealthItem::ok("plugin.wasm present"));
    } else {
        items.push(HealthItem::problem(
            Level::Error,
            "plugin.wasm missing: build and package the plugin",
        ));
    }
    let toml = path.join("plugin.toml");
    let manifest = match std::fs::read_to_string(&toml) {
        Err(_) => {
            items.push(HealthItem::problem(Level::Error, "plugin.toml missing"));
            None
        }
        Ok(text) => match parse(&text)
            .map_err(|e| vec![e])
            .and_then(|v| PluginManifest::from_value(&v))
        {
            Ok(m) => {
                if m.name == name {
                    items.push(HealthItem::ok("plugin.toml valid"));
                } else {
                    items.push(HealthItem::problem(
                        Level::Error,
                        format!(
                            "plugin.toml name `{}` does not match folder `{name}`",
                            m.name
                        ),
                    ));
                }
                Some(m)
            }
            Err(errors) => {
                for e in errors {
                    items.push(HealthItem::problem(
                        Level::Error,
                        format!("plugin.toml: {e}"),
                    ));
                }
                None
            }
        },
    };
    Package {
        name,
        path: path.to_path_buf(),
        wasm,
        manifest,
        items,
    }
}

/// All packages in `dir` (folders and `.wasm` files), sorted by name. A missing folder
/// yields none. Other files are ignored.
pub fn discover(dir: &Path, parse: ValueParser) -> Vec<Package> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir() || p.extension().is_some_and(|x| x == "wasm"))
        .collect();
    paths.sort();
    paths.iter().map(|p| read_package(p, parse)).collect()
}

/// A plugin's state as known to vox: what it is, whether it can load, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInfo {
    /// Name.
    pub name: String,
    /// Folder or file it came from.
    pub path: PathBuf,
    /// WebAssembly file.
    pub wasm: PathBuf,
    /// Manifest, if any.
    pub manifest: Option<PluginManifest>,
    /// Overall result.
    pub status: HealthStatus,
    /// All findings: package, API, requirements, settings, and (after loading) custom checks.
    pub items: Vec<HealthItem>,
    /// Effective settings (defaults plus valid user values).
    pub settings: Vec<(String, SettingValue)>,
}

impl PluginInfo {
    /// Version from the manifest.
    pub fn version(&self) -> Option<Version> {
        self.manifest.as_ref().map(|m| m.version)
    }

    /// Whether the plugin may be loaded (not failed, not disabled).
    pub fn loadable(&self) -> bool {
        !matches!(self.status, HealthStatus::Failed | HealthStatus::Disabled)
    }

    /// Add findings and recompute the status (keeps `Disabled`).
    pub fn add_items(&mut self, items: impl IntoIterator<Item = HealthItem>) {
        self.items.extend(items);
        if self.status != HealthStatus::Disabled {
            self.status = health::status(&self.items);
        }
    }
}

/// What [`assess`] compares packages against.
pub struct AssessEnv<'a> {
    /// Running vox version.
    pub vox: Version,
    /// Plugin API version vox provides.
    pub api: Version,
    /// Plugins already loaded (name, version) that requirements may refer to.
    pub loaded: &'a [(String, Version)],
    /// `[plugins.NAME]` values from the config.
    pub config: &'a dyn Fn(&str) -> Option<&'a BTreeMap<String, Value>>,
    /// Whether a program exists on `PATH`.
    pub program_exists: &'a dyn Fn(&str) -> bool,
}

/// Decide for each package whether it can load: disabled by the user, broken package,
/// API version, requirements (including other plugins, re-checked until stable so a plugin
/// that needs a failed plugin fails too), and settings.
pub fn assess(packages: Vec<Package>, env: &AssessEnv<'_>) -> Vec<PluginInfo> {
    let mut infos: Vec<PluginInfo> = packages
        .into_iter()
        .map(|p| {
            let user = (env.config)(&p.name);
            let disabled = user
                .and_then(|u| u.get("enabled"))
                .is_some_and(|e| *e == Value::Bool(false));
            let (settings, warnings) = match &p.manifest {
                Some(m) => resolve_settings(&m.settings, user),
                None => (Vec::new(), Vec::new()),
            };
            let mut items = p.items;
            items.extend(
                warnings
                    .into_iter()
                    .map(|w| HealthItem::problem(Level::Warn, format!("setting {w}"))),
            );
            let status = if disabled {
                items.push(HealthItem::ok("disabled in config (enabled = false)"));
                HealthStatus::Disabled
            } else {
                health::status(&items)
            };
            PluginInfo {
                name: p.name,
                path: p.path,
                wasm: p.wasm,
                manifest: p.manifest,
                status,
                items,
                settings,
            }
        })
        .collect();
    let base: Vec<Vec<HealthItem>> = infos.iter().map(|i| i.items.clone()).collect();
    // Requirement checks depend on which other plugins can load; repeat until stable.
    for _ in 0..=infos.len() {
        let available: Vec<(String, Version)> = env
            .loaded
            .iter()
            .cloned()
            .chain(infos.iter().filter(|i| i.loadable()).map(|i| {
                (
                    i.name.clone(),
                    i.version().unwrap_or(Version {
                        major: 0,
                        minor: 0,
                        patch: 0,
                    }),
                )
            }))
            .collect();
        let mut changed = false;
        for (info, base) in infos.iter_mut().zip(&base) {
            if info.status == HealthStatus::Disabled || health::status(base) == HealthStatus::Failed
            {
                continue;
            }
            let Some(m) = &info.manifest else { continue };
            let others: Vec<(String, Version)> = available
                .iter()
                .filter(|(n, _)| *n != info.name)
                .cloned()
                .collect();
            let checks = health::check(
                m,
                &HealthEnv {
                    vox: env.vox,
                    api: env.api,
                    plugins: &others,
                    program_exists: env.program_exists,
                },
            );
            let mut items = base.clone();
            items.extend(checks);
            let status = health::status(&items);
            if status != info.status || items != info.items {
                changed |= status != info.status;
                info.items = items;
                info.status = status;
            }
        }
        if !changed {
            break;
        }
    }
    infos
}

/// Report lines for `:checkhealth`: (style key, text). Style key: `ok`, `info`, `warn`,
/// `error`, `title`.
pub fn report(infos: &[PluginInfo]) -> Vec<(&'static str, String)> {
    let mut lines = Vec::new();
    if infos.is_empty() {
        lines.push(("info", "No plugins installed in this scope.".to_string()));
    }
    for info in infos {
        let version = info
            .version()
            .map_or("(no version)".into(), |v| v.to_string());
        lines.push((
            "title",
            format!("{} {version} — {}", info.name, info.status.name()),
        ));
        if let Some(d) = info
            .manifest
            .as_ref()
            .map(|m| &m.description)
            .filter(|d| !d.is_empty())
        {
            lines.push(("info", format!("  {d}")));
        }
        for item in &info.items {
            let (key, mark) = match (item.ok, item.level) {
                (true, _) => ("ok", "✓"),
                (false, Level::Error) => ("error", "✗"),
                (false, Level::Warn) => ("warn", "!"),
                (false, _) => ("info", "i"),
            };
            lines.push((key, format!("  {mark} {}", item.message)));
        }
        for (k, v) in &info.settings {
            lines.push(("info", format!("  setting {k} = {v}")));
        }
        lines.push(("info", String::new()));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A tiny `key = "value"` / `[section]` parser, enough for test manifests.
    fn parse(text: &str) -> Result<Value, String> {
        let mut top: BTreeMap<String, Value> = BTreeMap::new();
        let mut section: Option<Vec<String>> = None;
        let mut requires: Vec<Value> = Vec::new();
        let mut current_req: Option<BTreeMap<String, Value>> = None;
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            if line == "[[requires]]" {
                if let Some(r) = current_req.take() {
                    requires.push(Value::Table(r));
                }
                current_req = Some(BTreeMap::new());
                section = None;
                continue;
            }
            if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                if let Some(r) = current_req.take() {
                    requires.push(Value::Table(r));
                }
                section = Some(s.split('.').map(String::from).collect());
                continue;
            }
            let (k, v) = line.split_once('=').ok_or("bad line")?;
            let v = v.trim();
            let value = if let Some(s) = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
                Value::String(s.into())
            } else if v == "true" || v == "false" {
                Value::Bool(v == "true")
            } else {
                Value::Integer(v.parse().map_err(|_| "bad value")?)
            };
            if let Some(r) = current_req.as_mut() {
                r.insert(k.trim().into(), value);
                continue;
            }
            let mut table = &mut top;
            for part in section.iter().flatten() {
                let entry = table
                    .entry(part.clone())
                    .or_insert_with(|| Value::Table(BTreeMap::new()));
                let Value::Table(t) = entry else {
                    return Err("not a table".into());
                };
                table = t;
            }
            table.insert(k.trim().into(), value);
        }
        if let Some(r) = current_req.take() {
            requires.push(Value::Table(r));
        }
        if !requires.is_empty() {
            top.insert("requires".into(), Value::Array(requires));
        }
        Ok(Value::Table(top))
    }

    fn temp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("vox-pkg-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn package(dir: &Path, name: &str, extra: &str) {
        let p = dir.join(name);
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("plugin.wasm"), "").unwrap();
        fs::write(
            p.join("plugin.toml"),
            format!("[plugin]\nname = \"{name}\"\nversion = \"1.0.0\"\napi = \"0.2\"\n{extra}"),
        )
        .unwrap();
    }

    fn assess_dir(
        dir: &Path,
        config: &BTreeMap<String, BTreeMap<String, Value>>,
    ) -> Vec<PluginInfo> {
        let lookup = |n: &str| config.get(n);
        assess(
            discover(dir, parse),
            &AssessEnv {
                vox: Version::parse("0.10.0").unwrap(),
                api: Version::parse("0.2.0").unwrap(),
                loaded: &[],
                config: &lookup,
                program_exists: &|p| p == "sh",
            },
        )
    }

    fn status_of<'a>(infos: &'a [PluginInfo], name: &str) -> &'a PluginInfo {
        infos.iter().find(|i| i.name == name).unwrap()
    }

    #[test]
    fn discovers_folders_and_single_files() {
        let dir = temp("discover");
        package(&dir, "good", "");
        fs::write(dir.join("single.wasm"), "").unwrap();
        fs::write(dir.join("notes.txt"), "").unwrap();
        fs::create_dir_all(dir.join("empty")).unwrap();
        let infos = assess_dir(&dir, &BTreeMap::new());
        let names: Vec<_> = infos.iter().map(|i| (i.name.as_str(), i.status)).collect();
        assert_eq!(
            names,
            [
                ("empty", HealthStatus::Failed),
                ("good", HealthStatus::Ok),
                ("single", HealthStatus::Ok)
            ]
        );
        assert!(
            status_of(&infos, "empty")
                .items
                .iter()
                .any(|i| i.message == "plugin.toml missing")
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn requirement_chain_and_levels() {
        let dir = temp("chain");
        package(&dir, "base", "vox = \">=0.11\"");
        package(
            &dir,
            "needs-base",
            "[[requires]]\nplugin = \"base\"\nlevel = \"error\"",
        );
        package(
            &dir,
            "likes-base",
            "[[requires]]\nplugin = \"base\"\nlevel = \"warning\"",
        );
        package(
            &dir,
            "wants-rg",
            "[[requires]]\nprogram = \"rg-not-here\"\nlevel = \"info\"",
        );
        let infos = assess_dir(&dir, &BTreeMap::new());
        assert_eq!(
            status_of(&infos, "base").status,
            HealthStatus::Failed,
            "vox too old"
        );
        assert_eq!(
            status_of(&infos, "needs-base").status,
            HealthStatus::Failed,
            "base failed"
        );
        assert_eq!(
            status_of(&infos, "likes-base").status,
            HealthStatus::Warning
        );
        assert_eq!(status_of(&infos, "wants-rg").status, HealthStatus::Info);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn disabled_name_mismatch_and_settings() {
        let dir = temp("misc");
        package(&dir, "off", "");
        package(
            &dir,
            "set",
            "[settings.n]\ntype = \"integer\"\ndefault = 3\nmax = 5",
        );
        fs::create_dir_all(dir.join("renamed")).unwrap();
        fs::write(dir.join("renamed/plugin.wasm"), "").unwrap();
        fs::write(
            dir.join("renamed/plugin.toml"),
            "[plugin]\nname = \"other\"\nversion = \"1\"\napi = \"0.2\"\n",
        )
        .unwrap();
        let mut config = BTreeMap::new();
        config.insert(
            "off".to_string(),
            BTreeMap::from([("enabled".to_string(), Value::Bool(false))]),
        );
        config.insert(
            "set".to_string(),
            BTreeMap::from([("n".to_string(), Value::Integer(9))]),
        );
        let infos = assess_dir(&dir, &config);
        assert_eq!(status_of(&infos, "off").status, HealthStatus::Disabled);
        assert_eq!(status_of(&infos, "renamed").status, HealthStatus::Failed);
        let set = status_of(&infos, "set");
        assert_eq!(set.status, HealthStatus::Warning);
        assert_eq!(set.settings, [("n".to_string(), SettingValue::Integer(3))]);
        let report = report(&infos);
        assert!(
            report
                .iter()
                .any(|(k, l)| *k == "title" && l == "set 1.0.0 — warning")
        );
        assert!(
            report
                .iter()
                .any(|(k, l)| *k == "warn" && l.contains("between"))
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
