//! vox: a Vim-like terminal editor. Wires adapter crates into the editor.

use std::process::ExitCode;

use std::rc::Rc;
use vox_clipboard::SystemClipboard;
use vox_core::Document;

use vox_editor::{Editor, Level, Services};
use vox_storage::Storage;
use vox_width::UnicodeWidth;

/// Open the file from the command line, build the adapters, and run the editor.
fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let doc = match args.as_slice() {
        [] => Document::new("", Storage::boxed),
        [path] => match Document::open(path, Storage::boxed) {
            Ok(doc) => doc,
            Err(e) => {
                eprintln!("vox: {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        },
        _ => {
            eprintln!("usage: vox [FILE]");
            return ExitCode::FAILURE;
        }
    };
    let services = Services {
        storage: Storage::boxed,
        width: Box::new(UnicodeWidth),
        clipboard: Box::new(SystemClipboard::default()),
        clock: Box::new(std::time::Instant::now),
    };
    let mut editor = Editor::with_services(doc, services);
    load_config(&mut editor);
    load_plugins(&mut editor);
    editor.start();
    if let Err(e) = vox_term::run(&mut editor) {
        eprintln!("vox: terminal error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Read the user's config file, if any, and register `:reload-config`.
fn load_config(editor: &mut Editor) {
    let Some(path) = vox_config_file::default_path() else {
        return;
    };
    vox_config_file::load_and_apply(editor, &path);
    vox_config_file::register_commands(editor, path);
}

/// Load every plugin in the plugin directory and register `:reload-plugins`. Failures
/// are logged and shown as a notification; the editor runs without the failed plugins.
fn load_plugins(editor: &mut Editor) {
    let Some(dir) = vox_config_file::plugin_dir() else {
        return;
    };
    let host = match vox_plugin_host::PluginHost::new() {
        Ok(host) => Rc::new(host),
        Err(e) => {
            editor.notify(Level::Error, format!("plugins: {e}"));
            return;
        }
    };
    let errors: Vec<String> = host
        .load_dir(editor, &dir)
        .into_iter()
        .filter_map(Result::err)
        .collect();
    if let Some(first) = errors.first() {
        let more = match errors.len() {
            1 => String::new(),
            n => format!(" ({} more, see :log)", n - 1),
        };
        editor.notify(Level::Error, format!("{first}{more}"));
    }
    vox_plugin_host::PluginHost::register_commands(host, editor, dir);
}
