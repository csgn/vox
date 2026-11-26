//! Reads the user's config file. Finds `config.toml`, parses the TOML format with the
//! `toml` library, and converts it to the editor's format-neutral [`Value`].

use std::path::{Path, PathBuf};

use vox_editor::Value;

/// Where the config file is looked for: `$VOX_CONFIG` if set, else
/// `$XDG_CONFIG_HOME/vox/config.toml`, else `~/.config/vox/config.toml`.
pub fn default_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("VOX_CONFIG") {
        return Some(PathBuf::from(p));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("vox").join("config.toml"))
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
