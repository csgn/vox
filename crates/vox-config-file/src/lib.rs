//! Reads and writes the user's config file. Parses TOML with the `toml` library into the
//! editor's format-neutral [`Value`], and applies settings changes with the `toml_edit`
//! library, which keeps the user's comments and layout.

use std::path::{Path, PathBuf};

use vox_editor::settings::ConfigEdit;
use vox_editor::{Config, Editor, Level, Value, owner};

/// The global scope's folder: `$VOX_CONFIG_DIR` if set, else
/// `$XDG_CONFIG_HOME/vox`, else `~/.config/vox`.
pub fn global_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("VOX_CONFIG_DIR").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("vox"))
}

/// Load and parse the config file at `path`. `Ok(None)` if the file does not exist.
pub fn load(path: &Path) -> Result<Option<Value>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Load the config at `path` and apply it to `editor` (replacing earlier config
/// mappings). Logs what happened; problems also become a notification. Returns whether a
/// file was found and read.
pub fn load_and_apply(editor: &mut Editor, path: &Path) -> bool {
    let value = match load(path) {
        Ok(Some(v)) => v,
        Ok(None) => {
            editor.log(
                Level::Debug,
                owner::CONFIG,
                format!("no config at {}", path.display()),
            );
            return false;
        }
        Err(e) => {
            editor.log(Level::Error, owner::CONFIG, &e);
            editor.notify(Level::Error, format!("config: {e}"));
            return false;
        }
    };
    let (config, errors) = Config::from_value(&value);
    editor.apply_config(&config);
    editor.log(
        Level::Info,
        owner::CONFIG,
        format!("loaded {}", path.display()),
    );
    for e in &errors {
        editor.log(Level::Warn, owner::CONFIG, e);
    }
    if let Some(first) = errors.first() {
        let more = match errors.len() {
            1 => String::new(),
            n => format!(" ({} more, see :log)", n - 1),
        };
        editor.notify(
            Level::Warn,
            format!("config {}: {first}{more}", path.display()),
        );
    }
    true
}

/// Register `:reload config` (owner `builtin`): re-read `path` and apply it.
pub fn register_commands(editor: &mut Editor, path: PathBuf) {
    editor.with_owner(owner::BUILTIN, |ed| {
        ed.commands_mut()
            .register(
                "reload config",
                "Re-read the config file and apply it.",
                move |ed, _| {
                    if load_and_apply(ed, &path) {
                        ed.notify(Level::Info, format!("reloaded {}", path.display()));
                    }
                    Ok(())
                },
            )
            .category(vox_editor::command::category::DEVELOPER);
    });
}

/// Apply `edits` to the TOML file at `path` (created if missing), changing only the edited
/// keys and keeping comments, order, and formatting. Writes a temporary file next to it and
/// renames it, so a crash never leaves a half-written config.
pub fn write_edits(path: &Path, edits: &[ConfigEdit]) -> Result<(), String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| {
        format!(
            "cannot edit: {}",
            e.to_string().lines().next().unwrap_or_default()
        )
    })?;
    for edit in edits {
        match edit {
            ConfigEdit::Set(keys, v) => set(doc.as_item_mut(), keys, to_toml(v))?,
            ConfigEdit::Remove(keys) => remove(doc.as_item_mut(), keys),
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension(format!("toml.tmp-{}", std::process::id()));
    std::fs::write(&tmp, doc.to_string()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

/// Set `keys` (a path) to `value`, creating tables (`[a.b]` headers) on the way.
fn set(root: &mut toml_edit::Item, keys: &[String], value: toml_edit::Item) -> Result<(), String> {
    let Some((last, parents)) = keys.split_last() else {
        return Ok(());
    };
    let mut item = root;
    for k in parents {
        if item.get(k).is_none() {
            let mut t = toml_edit::Table::new();
            t.set_implicit(true);
            item.as_table_like_mut()
                .ok_or_else(|| format!("`{k}` is inside a value that is not a table"))?
                .insert(k, toml_edit::Item::Table(t));
        }
        item = item.get_mut(k).expect("inserted above");
    }
    let table = item
        .as_table_like_mut()
        .ok_or_else(|| format!("cannot set `{last}`: its parent is not a table"))?;
    match (table.get_mut(last), value) {
        // Change the value in place: the key (with comments above it) and the comment
        // after the value stay.
        (Some(toml_edit::Item::Value(old)), toml_edit::Item::Value(mut new)) => {
            *new.decor_mut() = old.decor().clone();
            *old = new;
        }
        (_, value) => {
            table.insert(last, value);
        }
    }
    Ok(())
}

/// Remove `keys` (a path) if present.
fn remove(item: &mut toml_edit::Item, keys: &[String]) {
    match keys {
        [] => {}
        [last] => {
            if let Some(t) = item.as_table_like_mut() {
                t.remove(last);
            }
        }
        [first, rest @ ..] => {
            let Some(child) = item.get_mut(first) else {
                return;
            };
            remove(child, rest);
            // Drop a table the removal left empty (e.g. `[plugins.x]` after `enabled`).
            let empty = child.as_table_like().is_some_and(|t| t.is_empty());
            if let (true, Some(t)) = (empty, item.as_table_like_mut()) {
                t.remove(first);
            }
        }
    }
}

/// vox value → TOML item.
fn to_toml(v: &Value) -> toml_edit::Item {
    fn val(v: &Value) -> toml_edit::Value {
        match v {
            Value::String(s) => s.as_str().into(),
            Value::Integer(n) => (*n).into(),
            Value::Float(f) => (*f).into(),
            Value::Bool(b) => (*b).into(),
            Value::Array(a) => toml_edit::Value::Array(a.iter().map(val).collect()),
            Value::Table(t) => {
                let mut it = toml_edit::InlineTable::new();
                for (k, v) in t {
                    it.insert(k, val(v));
                }
                toml_edit::Value::InlineTable(it)
            }
        }
    }
    toml_edit::Item::Value(val(v))
}

/// Format a [`Value::Table`] as TOML text (used for `vox.lock`).
pub fn format(v: &Value) -> Result<String, String> {
    fn conv(v: &Value) -> toml::Value {
        match v {
            Value::String(s) => toml::Value::String(s.clone()),
            Value::Integer(n) => toml::Value::Integer(*n),
            Value::Float(f) => toml::Value::Float(*f),
            Value::Bool(b) => toml::Value::Boolean(*b),
            Value::Array(a) => toml::Value::Array(a.iter().map(conv).collect()),
            Value::Table(t) => {
                toml::Value::Table(t.iter().map(|(k, v)| (k.clone(), conv(v))).collect())
            }
        }
    }
    match conv(v) {
        toml::Value::Table(t) => toml::to_string(&t).map_err(|e| e.to_string()),
        _ => Err("only a table can be written as TOML".into()),
    }
}

/// Parse TOML text into a [`Value::Table`].
pub fn parse(text: &str) -> Result<Value, String> {
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| {
        e.to_string().lines().next().unwrap_or_default().to_string()
    })?;
    Ok(convert(toml::Value::Table(table)))
}

/// Convert a `toml` library value into vox's own [`Value`].
fn convert(v: toml::Value) -> Value {
    match v {
        toml::Value::String(s) => Value::String(s),
        toml::Value::Integer(i) => Value::Integer(i),
        toml::Value::Float(f) => Value::Float(f),
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(a) => Value::Array(a.into_iter().map(convert).collect()),
        toml::Value::Table(t) => {
            Value::Table(t.into_iter().map(|(k, v)| (k, convert(v))).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_editor::{Config, MapMode};

    #[test]
    fn parses_full_example() {
        let v = parse(
            r#"
            leader = " "
            timeout_ms = 500

            [keymap.normal]
            "<leader>w" = ":w"
            H = "^"

            [keymap.insert]
            jk = "<Esc>"
            "#,
        )
        .unwrap();
        let (cfg, errors) = Config::from_value(&v);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(cfg.leader, " ");
        assert_eq!(cfg.timeout_ms, 500);
        assert!(
            cfg.keymaps
                .contains(&(MapMode::Insert, "jk".into(), "<Esc>".into()))
        );
        assert!(
            cfg.keymaps
                .contains(&(MapMode::Normal, "<leader>w".into(), ":w".into()))
        );
        assert!(
            cfg.keymaps
                .contains(&(MapMode::Normal, "H".into(), "^".into()))
        );
    }

    #[test]
    fn converts_all_types() {
        let v = parse("a = 1\nb = 1.5\nc = true\nd = [1, \"x\"]\ne = 1979-05-27\n").unwrap();
        let Value::Table(t) = v else { panic!() };
        assert_eq!(t["a"], Value::Integer(1));
        assert_eq!(t["b"], Value::Float(1.5));
        assert_eq!(t["c"], Value::Bool(true));
        assert_eq!(
            t["d"],
            Value::Array(vec![Value::Integer(1), Value::String("x".into())])
        );
        assert_eq!(t["e"], Value::String("1979-05-27".into()));
    }

    #[test]
    fn syntax_error_is_one_line_with_position() {
        let e = parse("leader = \n").unwrap_err();
        assert!(!e.contains('\n'), "{e}");
        assert!(e.contains("line 1"), "{e}");
    }

    #[test]
    fn reload_replaces_config_mappings() {
        use vox_core::Document;
        let p = std::env::temp_dir().join(format!("vox-reload-{}.toml", std::process::id()));
        std::fs::write(&p, "[keymap.normal]\nH = \"^\"\nL = \"$\"\n").unwrap();
        let mut ed = Editor::new(Document::from_text("x"));
        register_commands(&mut ed, p.clone());
        assert!(load_and_apply(&mut ed, &p));
        assert_eq!(
            ed.keymap().iter().filter(|m| m.3 == owner::CONFIG).count(),
            2
        );
        std::fs::write(
            &p,
            "developer_mode = true\nbogus = 1\n[keymap.normal]\nH = \"0\"\n",
        )
        .unwrap();
        ed.run_command("reload config");
        std::fs::remove_file(&p).unwrap();
        let config_maps: Vec<_> = ed
            .keymap()
            .iter()
            .filter(|m| m.3 == owner::CONFIG)
            .collect();
        assert_eq!(config_maps.len(), 1, "old L mapping removed");
        assert!(ed.developer_mode());
        assert!(
            ed.notifications()
                .iter()
                .any(|n| n.text.contains("unknown setting `bogus`"))
        );
        assert!(
            ed.log_entries()
                .entries()
                .any(|e| e.source == "config" && e.message.starts_with("loaded"))
        );
    }

    #[test]
    fn write_edits_keeps_comments() {
        let dir = std::env::temp_dir().join(format!("vox-write-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.join("config.toml");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            &p,
            "# my config\nleader = \" \"  # space!\n\n[ui]\n# wide panel\nside_width = 50\n\n[keymap.normal]\nH = \"^\"\n",
        )
        .unwrap();
        let k = |s: &str| s.split('.').map(String::from).collect::<Vec<_>>();
        write_edits(
            &p,
            &[
                ConfigEdit::Set(k("ui.side_width"), Value::Integer(30)),
                ConfigEdit::Set(k("timeout_ms"), Value::Integer(250)),
                ConfigEdit::Set(
                    vec!["keymap".into(), "normal".into(), "<leader>w".into()],
                    Value::String(":w".into()),
                ),
                ConfigEdit::Remove(k("keymap.normal.H")),
                ConfigEdit::Set(
                    vec!["plugins".into(), "text-tools".into(), "case".into()],
                    Value::String("title".into()),
                ),
                ConfigEdit::Remove(k("not.there")),
            ],
        )
        .unwrap();
        let out = std::fs::read_to_string(&p).unwrap();
        assert!(
            out.contains("# my config") && out.contains("# space!") && out.contains("# wide panel"),
            "{out}"
        );
        assert!(out.contains("side_width = 30"), "{out}");
        assert!(out.contains("\"<leader>w\" = \":w\""), "{out}");
        assert!(!out.contains("H = "), "{out}");
        assert!(
            out.contains("[keymap.normal]"),
            "still has other keys: {out}"
        );
        assert!(
            out.contains("[plugins.text-tools]") && !out.contains("[plugins]\n"),
            "{out}"
        );
        let (cfg, errors) = Config::from_value(&parse(&out).unwrap());
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            (cfg.ui.side_width, cfg.timeout_ms, cfg.leader.as_str()),
            (30, 250, " ")
        );
        let leftovers: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(leftovers.len(), 1, "no temp file left");
        // A missing file is created.
        let new = dir.join("sub/config.toml");
        write_edits(
            &new,
            &[ConfigEdit::Set(k("developer_mode"), Value::Bool(true))],
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&new).unwrap().trim(),
            "developer_mode = true"
        );
        // Broken TOML is not touched.
        std::fs::write(&p, "x = [").unwrap();
        assert!(write_edits(&p, &[ConfigEdit::Set(k("a"), Value::Bool(true))]).is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "x = [");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn removing_the_last_key_drops_the_table() {
        let p = std::env::temp_dir().join(format!("vox-cfg-empty-{}.toml", std::process::id()));
        std::fs::write(&p, "leader = \" \"\n\n[plugins.x]\nenabled = false\n").unwrap();
        let path = vec![
            "plugins".to_string(),
            "x".to_string(),
            "enabled".to_string(),
        ];
        write_edits(&p, &[ConfigEdit::Remove(path)]).unwrap();
        let out = std::fs::read_to_string(&p).unwrap();
        assert_eq!(out.trim(), "leader = \" \"", "{out}");
        std::fs::remove_file(&p).unwrap();
    }

    #[test]
    fn lockfile_text_round_trip() {
        use vox_editor::plugin::lock::{self, LockEntry, Lockfile, Source};
        let mut l = Lockfile::default();
        l.upsert(LockEntry {
            name: "text-tools".into(),
            version: "0.2.0".into(),
            source: Source::Git {
                url: "file:///repo".into(),
                reference: None,
                rev: "abc".into(),
            },
            checksum: "sha256:00".into(),
        });
        let dir = std::env::temp_dir().join(format!("vox-lock-{}", std::process::id()));
        let p = dir.join("vox.lock");
        lock::write(&p, &l, format).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(
            text.starts_with("# vox.lock") && text.contains("[[plugin]]"),
            "{text}"
        );
        assert_eq!(lock::read(&p, parse).unwrap(), l);
        assert_eq!(
            lock::read(&dir.join("missing.lock"), parse).unwrap(),
            Lockfile::default()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_file_is_none() {
        let p = std::env::temp_dir().join("vox-config-file-test-missing.toml");
        assert_eq!(load(&p), Ok(None));
    }

    #[test]
    fn load_reports_path() {
        let p =
            std::env::temp_dir().join(format!("vox-config-file-test-{}.toml", std::process::id()));
        std::fs::write(&p, "x = [").unwrap();
        let e = load(&p).unwrap_err();
        std::fs::remove_file(&p).unwrap();
        assert!(e.starts_with(&p.display().to_string()), "{e}");
    }
}
