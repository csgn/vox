//! vox: a Vim-like terminal editor. Wires adapter crates into the editor.

use std::process::ExitCode;

use vox_clipboard::SystemClipboard;
use vox_core::Document;
use vox_editor::{Editor, Services};
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
    };
    let mut editor = Editor::with_services(doc, services);
    if let Err(e) = vox_term::run(&mut editor) {
        eprintln!("vox: terminal error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
