//! Health checks: is everything a plugin needs present? Each finding has a level; an
//! unmet `error` requirement means the plugin is not loaded.

use std::path::Path;

use super::manifest::{Need, PluginManifest};
use super::version::Version;
use crate::ui::Level;

/// One line of a health report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthItem {
    /// Whether this check passed.
    pub ok: bool,
    /// For a failed check: how bad (`Error` blocks loading). For a passed check: `Info`.
    pub level: Level,
    /// What was checked and the result.
    pub message: String,
}

impl HealthItem {
    /// A passed check.
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            level: Level::Info,
            message: message.into(),
        }
    }

    /// A failed check at `level`.
    pub fn problem(level: Level, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            level,
            message: message.into(),
        }
    }
}

/// Overall result for one plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HealthStatus {
    /// Everything fine.
    Ok,
    /// Only informational findings.
    Info,
    /// Loaded, but something is missing or wrong.
    Warning,
    /// Not loaded.
    Failed,
    /// Turned off by the user (`enabled = false`).
    Disabled,
}

impl HealthStatus {
    /// Short name.
    pub fn name(self) -> &'static str {
        match self {
            HealthStatus::Ok => "ok",
            HealthStatus::Info => "info",
            HealthStatus::Warning => "warning",
            HealthStatus::Failed => "failed",
            HealthStatus::Disabled => "disabled",
        }
    }
}

/// The overall status of `items`: the worst failed check decides.
pub fn status(items: &[HealthItem]) -> HealthStatus {
    items
        .iter()
        .filter(|i| !i.ok)
        .map(|i| match i.level {
            Level::Error => HealthStatus::Failed,
            Level::Warn => HealthStatus::Warning,
            _ => HealthStatus::Info,
        })
        .max()
        .unwrap_or(HealthStatus::Ok)
}

/// What the checks compare against.
pub struct HealthEnv<'a> {
    /// Running vox version.
    pub vox: Version,
    /// Plugin API version vox provides.
    pub api: Version,
    /// Other plugins that are available (name, version), i.e. not failed or disabled.
    pub plugins: &'a [(String, Version)],
    /// Whether a program exists (normally [`program_on_path`]).
    pub program_exists: &'a dyn Fn(&str) -> bool,
}

/// Whether a plugin built for API `plugin` works with a vox providing API `host`: same
/// major version, not newer, and for 0.x the same minor version.
pub fn api_compatible(plugin: Version, host: Version) -> bool {
    plugin.major == host.major && (host.major > 0 || plugin.minor == host.minor) && plugin <= host
}

/// The finding about a plugin built for API `plugin` (an error if incompatible).
pub fn api_item(plugin: Version, host: Version) -> HealthItem {
    if api_compatible(plugin, host) {
        HealthItem::ok(format!(
            "plugin API {} (vox provides {})",
            short(plugin),
            short(host)
        ))
    } else {
        HealthItem::problem(
            Level::Error,
            format!(
                "built for plugin API {}, vox provides {}: rebuild the plugin with the current SDK \
                 (scripts/build-plugin.sh DIR --install PLUGIN_FOLDER)",
                short(plugin),
                short(host)
            ),
        )
    }
}

/// The plugin API version in a component import name like `vox:plugin/editor@0.1.0`.
pub fn api_of_import(name: &str) -> Option<Version> {
    let rest = name.strip_prefix("vox:plugin/")?;
    Version::parse(rest.split_once('@')?.1).ok()
}

/// Check a manifest's API version and requirements.
pub fn check(m: &PluginManifest, env: &HealthEnv<'_>) -> Vec<HealthItem> {
    let mut items = vec![api_item(m.api, env.api)];
    for r in &m.requires {
        let why = if r.reason.is_empty() {
            String::new()
        } else {
            format!(" ({})", r.reason)
        };
        let (met, what) = match &r.need {
            Need::Vox(req) => (req.matches(env.vox), format!("vox {req}, have {}", env.vox)),
            Need::Program(p) => {
                let found = (env.program_exists)(p);
                (
                    found,
                    format!(
                        "program `{p}` {}",
                        if found { "found" } else { "not found on PATH" }
                    ),
                )
            }
            Need::Plugin { name, version } => {
                let have = env.plugins.iter().find(|(n, _)| n == name).map(|(_, v)| *v);
                match (have, version) {
                    (None, _) => (
                        false,
                        format!("plugin `{name}` not installed or not loaded"),
                    ),
                    (Some(v), Some(req)) if !req.matches(v) => {
                        (false, format!("plugin `{name}` {req}, have {v}"))
                    }
                    (Some(v), _) => (true, format!("plugin `{name}` {v}")),
                }
            }
        };
        items.push(if met {
            HealthItem::ok(format!("{what}{why}"))
        } else {
            HealthItem::problem(r.level, format!("needs {what}{why}"))
        });
    }
    items
}

/// `0.2.0` → `0.2`.
fn short(v: Version) -> String {
    if v.patch == 0 {
        format!("{}.{}", v.major, v.minor)
    } else {
        v.to_string()
    }
}

/// Whether an executable named `name` is in a `PATH` folder.
pub fn program_on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| is_executable(&dir.join(name)))
}

/// Whether `p` is a file the user may run.
fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        p.is_file() || p.with_extension("exe").is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::Requirement;
    use crate::plugin::version::VersionReq;

    fn manifest(api: &str, requires: Vec<Requirement>) -> PluginManifest {
        PluginManifest {
            name: "p".into(),
            version: Version::parse("1.0").unwrap(),
            description: String::new(),
            authors: vec![],
            api: Version::parse(api).unwrap(),
            requires,
            settings: vec![],
        }
    }

    fn req(need: Need, level: Level) -> Requirement {
        Requirement {
            need,
            level,
            reason: "because".into(),
        }
    }

    fn env<'a>(
        plugins: &'a [(String, Version)],
        exists: &'a dyn Fn(&str) -> bool,
    ) -> HealthEnv<'a> {
        HealthEnv {
            vox: Version::parse("0.10.0").unwrap(),
            api: Version::parse("0.2.0").unwrap(),
            plugins,
            program_exists: exists,
        }
    }

    #[test]
    fn levels_decide_status() {
        let plugins = [("base".to_string(), Version::parse("1.5").unwrap())];
        let exists = |p: &str| p == "git";
        let m = manifest(
            "0.2",
            vec![
                req(
                    Need::Vox(VersionReq::parse(">=0.10").unwrap()),
                    Level::Error,
                ),
                req(Need::Program("git".into()), Level::Error),
                req(
                    Need::Plugin {
                        name: "base".into(),
                        version: Some(VersionReq::parse(">=2").unwrap()),
                    },
                    Level::Warn,
                ),
                req(Need::Program("rg".into()), Level::Info),
            ],
        );
        let items = check(&m, &env(&plugins, &exists));
        assert_eq!(items.iter().filter(|i| i.ok).count(), 3, "{items:?}");
        assert_eq!(
            items[3].message,
            "needs plugin `base` >=2, have 1.5.0 (because)"
        );
        assert_eq!(
            items[4].message,
            "needs program `rg` not found on PATH (because)"
        );
        assert_eq!(status(&items), HealthStatus::Warning);
        let m = manifest(
            "0.2",
            vec![req(
                Need::Plugin {
                    name: "x".into(),
                    version: None,
                },
                Level::Error,
            )],
        );
        assert_eq!(
            status(&check(&m, &env(&plugins, &exists))),
            HealthStatus::Failed
        );
        assert_eq!(status(&[]), HealthStatus::Ok);
    }

    #[test]
    fn api_version_must_match() {
        let none: [(String, Version); 0] = [];
        let no = |_: &str| false;
        for (api, ok) in [
            ("0.2", true),
            ("0.1", false),
            ("0.3", false),
            ("1.0", false),
        ] {
            let items = check(&manifest(api, vec![]), &env(&none, &no));
            assert_eq!(items[0].ok, ok, "api {api}");
        }
    }

    #[test]
    fn api_from_import_names() {
        assert_eq!(
            api_of_import("vox:plugin/editor@0.1.0"),
            Version::parse("0.1").ok()
        );
        assert_eq!(
            api_of_import("vox:plugin/types@0.2.0"),
            Version::parse("0.2").ok()
        );
        assert_eq!(api_of_import("wasi:cli/environment@0.2.6"), None);
        assert_eq!(api_of_import("vox:plugin/editor"), None);
        let old = api_item(
            Version::parse("0.1").unwrap(),
            Version::parse("0.2").unwrap(),
        );
        assert!(
            !old.ok
                && old
                    .message
                    .starts_with("built for plugin API 0.1, vox provides 0.2")
        );
    }

    #[test]
    fn finds_programs() {
        assert!(program_on_path("sh"));
        assert!(!program_on_path("surely-not-a-program-vox"));
    }
}
