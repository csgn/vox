//! Reads the user's config file. Finds `config.toml`, parses the TOML format with the
//! `toml` library, and converts it to the editor's format-neutral [`Value`].

use std::path::{Path, PathBuf};

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
        ed.commands_mut().register(
            "reload config",
            "Re-read the config file and apply it.",
            move |ed, _| {
                if load_and_apply(ed, &path) {
                    ed.notify(Level::Info, format!("reloaded {}", path.display()));
                }
                Ok(())
            },
        );
    });
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
        assert_eq!(ed.keymap().len(), 2);
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
