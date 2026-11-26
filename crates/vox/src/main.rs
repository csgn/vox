//! vox: a Vim-like terminal editor. Picks the scope from the command line, wires adapter
//! crates into the editor, and runs it.

mod cli;

use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;

use vox_clipboard::SystemClipboard;
use vox_core::Document;
use vox_editor::plugin::{HealthStatus, PluginInfo};
use vox_editor::scope::{self, Scope};
use vox_editor::{Editor, Level, ScopeKind, Services};
use vox_storage::Storage;
use vox_width::UnicodeWidth;

use cli::{Exit, Mode};

/// Parse arguments, open the file, load the scope's config and plugins, run the editor.
fn main() -> ExitCode {
    let args = std::env::args_os().skip(1).collect();
    let cli = match cli::parse(args, Path::is_dir) {
        Ok(cli) => cli,
        Err(Exit::Help) => {
            println!("{}", cli::HELP);
            return ExitCode::SUCCESS;
        }
        Err(Exit::Version) => {
            println!("vox {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(Exit::Error(e)) => {
            eprintln!("vox: {e}");
            return ExitCode::FAILURE;
        }
    };
    let doc = match &cli.file {
        None => Document::new("", Storage::boxed),
        Some(path) => match Document::open(path, Storage::boxed) {
            Ok(doc) => doc,
            Err(e) => {
                eprintln!("vox: {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        },
    };
    let services = Services {
        storage: Storage::boxed,
        width: Box::new(UnicodeWidth),
        clipboard: Box::new(SystemClipboard::default()),
        clock: Box::new(std::time::Instant::now),
    };
    let mut editor = Editor::with_services(doc, services);

    let global = vox_config_file::global_dir().map(Scope::global);
    let active = match cli.mode {
        Mode::Global => global.clone().unwrap_or_else(Scope::vanilla),
        Mode::Vanilla => Scope::vanilla(),
        Mode::Local(dir) => {
            let cwd = std::env::current_dir().unwrap_or_default();
            let root = dir
                .or_else(|| scope::find_project_root(&cwd))
                .unwrap_or(cwd);
            Scope::local(&root)
        }
    };
    editor.set_scope(active.clone(), global);
    editor.set_config_writer(vox_config_file::write_edits);
    load_config(&mut editor, &active);
    load_plugins(&mut editor, &active);
    register_plugin_new(&mut editor);
    if active.kind == ScopeKind::Local && !active.exists() {
        editor.run_command("scope setup");
    }
    editor.start();
    if let Err(e) = vox_term::run(&mut editor) {
        eprintln!("vox: terminal error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Read the scope's config file, if any, and register `:reload config`.
fn load_config(editor: &mut Editor, scope: &Scope) {
    let Some(path) = scope.config_path() else {
        return;
    };
    vox_config_file::load_and_apply(editor, &path);
    vox_config_file::register_commands(editor, path);
}

/// Load every plugin in the scope's plugin folder and register `:reload plugins` and
/// `:checkhealth`. Failed plugins and plugins with warnings are summed up in a notification.
fn load_plugins(editor: &mut Editor, scope: &Scope) {
    let Some(dir) = scope.plugin_dir() else {
        return;
    };
    let host = match vox_plugin_host::PluginHost::new(vox_config_file::parse) {
        Ok(host) => Rc::new(host),
        Err(e) => {
            editor.notify(Level::Error, format!("plugins: {e}"));
            return;
        }
    };
    let infos = host.load_dir(editor, &dir);
    let failed: Vec<&PluginInfo> = infos
        .iter()
        .filter(|i| i.status == HealthStatus::Failed)
        .collect();
    let warned = infos
        .iter()
        .filter(|i| i.status == HealthStatus::Warning)
        .count();
    if !failed.is_empty() {
        let names: Vec<&str> = failed.iter().map(|i| i.name.as_str()).collect();
        editor.notify(
            Level::Error,
            format!(
                "plugin(s) not loaded: {} — see :checkhealth",
                names.join(", ")
            ),
        );
    }
    if warned > 0 {
        editor.notify(
            Level::Warn,
            format!("{warned} plugin(s) with warnings — see :checkhealth"),
        );
    }
    vox_plugin_host::PluginHost::register_commands(host, editor, dir);
}

/// Register `:plugin new NAME [DIR]`: create a plugin project in the standard layout.
fn register_plugin_new(editor: &mut Editor) {
    // Plugins built from this source tree use this SDK.
    let sdk = concat!(env!("CARGO_MANIFEST_DIR"), "/../vox-plugin-sdk");
    editor.with_owner(vox_editor::owner::BUILTIN, |ed| {
        ed.commands_mut().register(
            "plugin new",
            "Create a plugin project: `:plugin new NAME [DIR]` (DIR defaults to the current folder).",
            move |ed, args| {
                let mut words = args.arg.unwrap_or("").split_whitespace();
                let name = words.next().ok_or("E471: Use :plugin new NAME [DIR]")?;
                let dir = words.next().map_or_else(|| std::env::current_dir().unwrap_or_default(), Into::into);
                let sdk_path = std::fs::canonicalize(sdk).unwrap_or_else(|_| sdk.into());
                let dep = format!("path = \"{}\"", sdk_path.display());
                let files = vox_editor::plugin::template::create(&dir, name, &dep, env!("CARGO_PKG_VERSION"))
                    .map_err(|e| format!("E900: {e}"))?;
                for f in &files {
                    ed.log(Level::Info, "editor", format!("created {}", f.display()));
                }
                ed.notify(
                    Level::Info,
                    format!(
                        "created {} — build with scripts/build-plugin.sh {} --install <plugins folder>",
                        dir.join(name).display(),
                        dir.join(name).display()
                    ),
                );
                Ok(())
            },
        );
    });
}
