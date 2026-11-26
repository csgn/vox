//! `vox.lock`: which plugins a scope uses, the exact version and source of each, and a
//! checksum of the installed `plugin.wasm`, so the same plugins can be installed again
//! (`:plugins sync`). Written by vox; meant to be committed in projects.
//!
//! ```toml
//! version = 1
//!
//! [[plugin]]
//! name = "text-tools"
//! version = "0.2.0"
//! source = "git"                                  # git | path
//! url = "https://example.com/text-tools.git"
//! ref = "v0.2.0"                                  # optional: tag or branch asked for
//! rev = "3f2a9c1..."                              # exact commit
//! checksum = "sha256:..."
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use crate::config::{Value, ValueFormatter, ValueParser};

/// Where a plugin came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A folder on disk (relative paths are relative to the project).
    Path(String),
    /// A git repository at an exact commit.
    Git {
        /// Repository URL.
        url: String,
        /// Tag or branch that was asked for, if any.
        reference: Option<String>,
        /// Exact commit.
        rev: String,
    },
}

/// One locked plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockEntry {
    /// Plugin name (its folder in the plugin folder).
    pub name: String,
    /// Version from its `plugin.toml`.
    pub version: String,
    /// Where to get it again.
    pub source: Source,
    /// `sha256:HEX` of the installed `plugin.wasm`.
    pub checksum: String,
}

/// The whole lockfile.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lockfile {
    /// Locked plugins, sorted by name.
    pub plugins: Vec<LockEntry>,
}

/// Format version written into the file.
pub const LOCK_VERSION: i64 = 1;

impl Lockfile {
    /// Read from a parsed lockfile.
    pub fn from_value(v: &Value) -> Result<Lockfile, String> {
        let Value::Table(top) = v else {
            return Err("vox.lock must be a table".into());
        };
        match top.get("version") {
            Some(Value::Integer(LOCK_VERSION)) => {}
            Some(Value::Integer(n)) => {
                return Err(format!(
                    "vox.lock version {n} is not supported (expected {LOCK_VERSION})"
                ));
            }
            _ => return Err("vox.lock needs `version = 1`".into()),
        }
        let mut plugins = Vec::new();
        match top.get("plugin") {
            None => {}
            Some(Value::Array(items)) => {
                for (i, item) in items.iter().enumerate() {
                    plugins.push(entry(item).map_err(|e| format!("[[plugin]] #{}: {e}", i + 1))?);
                }
            }
            Some(_) => return Err("`plugin` must be a list of [[plugin]] tables".into()),
        }
        plugins.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Lockfile { plugins })
    }

    /// As a value tree in the file's shape.
    pub fn to_value(&self) -> Value {
        let s = |v: &str| Value::String(v.into());
        let items = self
            .plugins
            .iter()
            .map(|e| {
                let mut t = BTreeMap::new();
                t.insert("name".into(), s(&e.name));
                t.insert("version".into(), s(&e.version));
                match &e.source {
                    Source::Path(p) => {
                        t.insert("source".into(), s("path"));
                        t.insert("path".into(), s(p));
                    }
                    Source::Git {
                        url,
                        reference,
                        rev,
                    } => {
                        t.insert("source".into(), s("git"));
                        t.insert("url".into(), s(url));
                        if let Some(r) = reference {
                            t.insert("ref".into(), s(r));
                        }
                        t.insert("rev".into(), s(rev));
                    }
                }
                t.insert("checksum".into(), s(&e.checksum));
                Value::Table(t)
            })
            .collect();
        let mut top = BTreeMap::new();
        top.insert("version".into(), Value::Integer(LOCK_VERSION));
        top.insert("plugin".into(), Value::Array(items));
        Value::Table(top)
    }

    /// The entry for `name`.
    pub fn get(&self, name: &str) -> Option<&LockEntry> {
        self.plugins.iter().find(|e| e.name == name)
    }

    /// Add or replace the entry for `entry.name`.
    pub fn upsert(&mut self, entry: LockEntry) {
        self.plugins.retain(|e| e.name != entry.name);
        self.plugins.push(entry);
        self.plugins.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Remove the entry for `name`. Returns whether there was one.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.plugins.len();
        self.plugins.retain(|e| e.name != name);
        self.plugins.len() != before
    }
}

/// Read one `[[plugin]]` table.
fn entry(v: &Value) -> Result<LockEntry, String> {
    let Value::Table(t) = v else {
        return Err("must be a table".into());
    };
    let get = |k: &str| match t.get(k) {
        Some(Value::String(s)) => Ok(s.clone()),
        _ => Err(format!("needs `{k}` (a string)")),
    };
    let opt = |k: &str| match t.get(k) {
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    };
    let source = match get("source")?.as_str() {
        "path" => Source::Path(get("path")?),
        "git" => Source::Git {
            url: get("url")?,
            reference: opt("ref"),
            rev: get("rev")?,
        },
        other => return Err(format!("source `{other}` must be git or path")),
    };
    let checksum = get("checksum")?;
    if !checksum.starts_with("sha256:") {
        return Err("checksum must start with sha256:".into());
    }
    Ok(LockEntry {
        name: get("name")?,
        version: get("version")?,
        source,
        checksum,
    })
}

/// First line of a written lockfile.
const HEADER: &str = "# vox.lock: written by vox (:plugin add, :plugin remove, :plugins sync).\n# Commit it; do not edit it by hand.\n\n";

/// Read the lockfile at `path`. A missing file is an empty lockfile.
pub fn read(path: &Path, parse: ValueParser) -> Result<Lockfile, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text)
            .and_then(|v| Lockfile::from_value(&v))
            .map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Lockfile::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Write `lock` to `path` (temp file + rename).
pub fn write(path: &Path, lock: &Lockfile, format: ValueFormatter) -> Result<(), String> {
    let text = format!("{HEADER}{}", format(&lock.to_value())?);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension(format!("lock.tmp-{}", std::process::id()));
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Lockfile {
        let mut l = Lockfile::default();
        l.upsert(LockEntry {
            name: "zeta".into(),
            version: "1.0.0".into(),
            source: Source::Path("../zeta".into()),
            checksum: "sha256:aa".into(),
        });
        l.upsert(LockEntry {
            name: "alpha".into(),
            version: "0.2.0".into(),
            source: Source::Git {
                url: "https://example.com/alpha.git".into(),
                reference: Some("v0.2.0".into()),
                rev: "abc123".into(),
            },
            checksum: "sha256:bb".into(),
        });
        l
    }

    #[test]
    fn round_trip_and_order() {
        let l = entries();
        assert_eq!(l.plugins[0].name, "alpha", "sorted");
        assert_eq!(Lockfile::from_value(&l.to_value()).unwrap(), l);
    }

    #[test]
    fn upsert_and_remove() {
        let mut l = entries();
        let mut z = l.get("zeta").unwrap().clone();
        z.version = "1.1.0".into();
        l.upsert(z);
        assert_eq!(l.plugins.len(), 2);
        assert_eq!(l.get("zeta").unwrap().version, "1.1.0");
        assert!(l.remove("zeta") && !l.remove("zeta"));
    }

    #[test]
    fn rejects_bad_files() {
        let t = |e: Vec<(&str, Value)>| {
            Value::Table(e.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
        };
        assert!(
            Lockfile::from_value(&t(vec![]))
                .unwrap_err()
                .contains("version = 1")
        );
        assert!(
            Lockfile::from_value(&t(vec![("version", Value::Integer(9))]))
                .unwrap_err()
                .contains("not supported")
        );
        let bad = t(vec![
            ("version", Value::Integer(1)),
            (
                "plugin",
                Value::Array(vec![t(vec![("name", Value::String("x".into()))])]),
            ),
        ]);
        assert!(
            Lockfile::from_value(&bad)
                .unwrap_err()
                .starts_with("[[plugin]] #1")
        );
    }
}
