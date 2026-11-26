//! The standard plugin project. Every plugin has the same layout:
//!
//! ```text
//! NAME/
//!   plugin.toml     manifest: name, version, requirements, settings
//!   Cargo.toml      a `cdylib` crate depending on vox-plugin-sdk
//!   src/lib.rs      the plugin
//!   README.md       what it does, commands, settings
//!   CHANGELOG.md    changes per version
//!   .gitignore      build output
//! ```
//!
//! Built and packaged (by `scripts/build-plugin.sh`), a plugin is a folder
//! `NAME/{plugin.toml, plugin.wasm, README.md}` that goes into a scope's plugin folder.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::API_VERSION;

/// Files of the standard project, in creation order.
pub const PROJECT_FILES: [&str; 6] = [
    "plugin.toml",
    "Cargo.toml",
    "src/lib.rs",
    "README.md",
    "CHANGELOG.md",
    ".gitignore",
];

/// Files of an installed (packaged) plugin; `plugin.wasm` and `plugin.toml` are required.
pub const PACKAGE_FILES: [&str; 3] = ["plugin.toml", "plugin.wasm", "README.md"];

/// Whether `name` is a valid plugin name: lowercase letters, digits, `-`, starting with a
/// letter.
pub fn valid_name(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Create a new plugin project `dir/name` from the template. `sdk` is how its Cargo.toml
/// finds the SDK (e.g. `path = "/src/vox/crates/vox-plugin-sdk"`). Fails if the folder
/// exists. Returns the created files.
pub fn create(dir: &Path, name: &str, sdk: &str, vox_version: &str) -> io::Result<Vec<PathBuf>> {
    if !valid_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("`{name}`: use lowercase letters, digits, and -, starting with a letter"),
        ));
    }
    let root = dir.join(name);
    if root.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", root.display()),
        ));
    }
    fs::create_dir_all(root.join("src"))?;
    let ty = name
        .split('-')
        .map(|p| {
            let mut c = p.chars();
            c.next().map_or(String::new(), |f| {
                f.to_ascii_uppercase().to_string() + c.as_str()
            })
        })
        .collect::<String>();
    let api = format!("{}.{}", API_VERSION.major, API_VERSION.minor);
    let files = [
        (
            "plugin.toml",
            format!(
                r#"[plugin]
name = "{name}"
version = "0.1.0"
description = "TODO: one line about what {name} does"
authors = []
api = "{api}"
vox = ">={vox_version}"

# Things this plugin needs. level: error (not loaded) | warning | info
# [[requires]]
# program = "git"
# level = "warning"
# reason = "used for ..."

[settings.greeting]
type = "string"
default = "hello"
description = "What :{name} says"
"#
            ),
        ),
        (
            "Cargo.toml",
            format!(
                r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
vox-plugin-sdk = {{ {sdk} }}

[profile.release]
opt-level = "s"
lto = true
strip = true

# Not part of any outer workspace.
[workspace]
"#
            ),
        ),
        (
            "src/lib.rs",
            format!(
                r#"//! {name}: a vox plugin.

use vox_plugin_sdk::{{Level, Plugin, editor, export_plugin}};

struct {ty};

impl Plugin for {ty} {{
    fn init() -> Result<(), String> {{
        editor::register_command("{name}", "Say the greeting");
        editor::log(Level::Info, "{name} loaded");
        Ok(())
    }}

    fn run_command(name: String, _arg: Option<String>, _force: bool) -> Result<(), String> {{
        match name.as_str() {{
            "{name}" => {{
                let greeting = editor::setting("greeting")
                    .and_then(|v| v.as_text())
                    .unwrap_or_else(|| "hello".into());
                editor::message(&greeting);
                Ok(())
            }}
            other => Err(format!("{name}: unknown command {{other}}")),
        }}
    }}
}}

export_plugin!({ty});
"#
            ),
        ),
        (
            "README.md",
            format!(
                "# {name}\n\nTODO: what this plugin does.\n\n## Commands\n\n| Command | Action |\n|---|---|\n| `:{name}` | show the greeting |\n\n## Settings\n\nIn `config.toml`:\n\n```toml\n[plugins.{name}]\ngreeting = \"hello\"\n```\n\n## Build\n\n```sh\nscripts/build-plugin.sh path/to/{name}\n```\n"
            ),
        ),
        (
            "CHANGELOG.md",
            "# Changelog\n\n## [0.1.0]\n\n### Added\n- First version.\n".to_string(),
        ),
        (".gitignore", "/target\n/dist\n".to_string()),
    ];
    let mut created = Vec::new();
    for (path, content) in files {
        let p = root.join(path);
        fs::write(&p, content)?;
        created.push(p);
    }
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(valid_name("text-tools") && valid_name("a1"));
        assert!(!valid_name("Text") && !valid_name("1a") && !valid_name("a_b") && !valid_name(""));
    }

    #[test]
    fn creates_standard_layout() {
        let dir = std::env::temp_dir().join(format!("vox-template-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let files = create(&dir, "my-tool", "path = \"/sdk\"", "0.10.0").unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|p| {
                p.strip_prefix(dir.join("my-tool"))
                    .unwrap()
                    .display()
                    .to_string()
            })
            .collect();
        assert_eq!(names, PROJECT_FILES);
        let lib = fs::read_to_string(dir.join("my-tool/src/lib.rs")).unwrap();
        assert!(lib.contains("struct MyTool;") && lib.contains("export_plugin!(MyTool);"));
        let cargo = fs::read_to_string(dir.join("my-tool/Cargo.toml")).unwrap();
        assert!(cargo.contains("vox-plugin-sdk = { path = \"/sdk\" }"));
        assert!(create(&dir, "my-tool", "", "0.10.0").is_err(), "exists");
        assert!(create(&dir, "Bad_Name", "", "0.10.0").is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
