//! vox: a Vim-like terminal editor. Picks the scope from the command line, wires adapter
//! crates into the editor, and runs it, or runs commands without a terminal (`vox run`).

mod cli;

use std::io::BufRead;
use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;

use vox_clipboard::SystemClipboard;
use vox_core::Document;
use vox_editor::plugin::{HealthStatus, PluginInfo, Version};
use vox_editor::scope::{self, Scope};
use vox_editor::update::UpdateSource;
use vox_editor::{Editor, Level, NoClipboard, ScopeKind, Services};
use vox_storage::Storage;
use vox_width::UnicodeWidth;

use cli::{Exit, Mode, Run};

/// Where vox releases come from: `vX.Y.Z` tags of this repository.
const REPO: &str = "https://github.com/csgn/vox.git";

/// Parse arguments, open the file, load the scope's config and plugins, run the editor (or
/// the commands of `vox run`).
fn main() -> ExitCode {
    let args = std::env::args_os().skip(1).collect();
    let cli = match cli::parse(args, Path::is_dir) {
        Ok(cli) => cli,
        Err(Exit::Help) => {
            println!("{}", cli::HELP);
            return ExitCode::SUCCESS;
        }
        Err(Exit::RunHelp) => {
            println!("{}", cli::RUN_HELP);
            return ExitCode::SUCCESS;
        }
        Err(Exit::Version) => {
            println!("vox {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(Exit::Error(e)) => {
            eprintln!("vox: {e}");
            return ExitCode::from(2);
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
        clipboard: if cli.run.is_some() {
            Box::new(NoClipboard)
        } else {
            Box::new(SystemClipboard::default())
        },
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
    // Without a terminal nobody can answer a dialog.
    editor.set_interactive(cli.run.is_none());
    editor.set_config_writer(vox_config_file::write_edits);
    editor.set_file_format(vox_config_file::parse, vox_config_file::format);
    load_config(&mut editor, &active);
    load_plugins(&mut editor, &active);
    vox_editor::plugin::manage::lock_status_notice(&mut editor);
    editor.set_update_source(update_source());
    register_plugin_new(&mut editor);
    if let Some(run) = &cli.run {
        editor.start();
        return run_commands(&mut editor, run);
    }
    if active.kind != ScopeKind::Vanilla {
        vox_editor::update::check_on_start(&mut editor);
    }
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

/// `vox run`: run each command line, wait for its background jobs (printing their progress),
/// print its reply to stdout and its messages to stderr. Stops at the first command that fails (its error, or an
/// error message such as a failed install); exit status 1 then.
fn run_commands(editor: &mut Editor, run: &Run) -> ExitCode {
    let mut lines = run.commands.clone();
    if run.stdin {
        for line in std::io::stdin().lock().lines() {
            match line {
                Ok(l) if l.trim().is_empty() => {}
                Ok(l) => lines.push(l),
                Err(e) => {
                    eprintln!("vox: stdin: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    let mut shown = print_messages(editor, 0);
    for line in &lines {
        let result = editor.execute(line);
        let mut last_progress: Vec<String> = Vec::new();
        while editor.jobs_running() > 0 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            editor.tick();
            for detail in editor.job_status().iter().filter_map(|j| j.detail.as_ref()) {
                if !last_progress.contains(detail) {
                    eprintln!("vox: progress: {detail}");
                }
            }
            last_progress = editor
                .job_status()
                .iter()
                .filter_map(|j| j.detail.clone())
                .collect();
        }
        let failed = editor.history()[shown..]
            .iter()
            .any(|(level, _)| *level == Level::Error);
        shown = print_messages(editor, shown);
        match result {
            Ok(Some(v)) if run.json => println!("{}", v.to_json()),
            Ok(Some(v)) => println!("{}", v.to_text()),
            Ok(None) => {}
            Err(e) => {
                eprintln!("vox: {}: {e}", line.trim());
                return ExitCode::FAILURE;
            }
        }
        if failed {
            return ExitCode::FAILURE;
        }
        if editor.should_quit() {
            break;
        }
    }
    ExitCode::SUCCESS
}

/// Print messages and notifications after the first `from` to stderr; returns how many
/// there are now.
fn print_messages(editor: &Editor, from: usize) -> usize {
    let history = editor.history();
    for (level, text) in &history[from..] {
        eprintln!("vox: {}: {text}", level.name());
    }
    history.len()
}

/// vox's repository (the one install.sh uses), this version, and where it is installed.
fn update_source() -> UpdateSource {
    let exe = std::env::current_exe().unwrap_or_default();
    UpdateSource {
        repo: REPO.into(),
        current: Version::parse(env!("CARGO_PKG_VERSION")).expect("package version is valid"),
        install_root: vox_editor::update::install_root_of(&exe),
    }
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
