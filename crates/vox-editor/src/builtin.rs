//! Built-in `:` commands, registered through the same [`Commands`] API that plugins use.

use crate::Editor;
use crate::command::{Args, CommandResult, Commands};

const E37: &str = "E37: No write since last change (add ! to override)";

/// Register all built-in commands and their Vim-style short names.
pub(crate) fn register(c: &mut Commands) {
    c.register("write", "Save the file. `:w FILE` saves as FILE.", write);
    c.alias("w", "write");
    c.register(
        "quit",
        "Quit. Refuses if there are unsaved changes; `:q!` discards them.",
        quit,
    );
    for a in ["q", "qa", "qall"] {
        c.alias(a, "quit");
    }
    c.register("wq", "Save, then quit.", write_quit);
    c.register("xit", "Save if modified, then quit.", xit);
    c.alias("x", "xit");
    c.register(
        "edit",
        "Open FILE, or reload the current file. `:e!` discards changes.",
        edit,
    );
    c.alias("e", "edit");
}

/// `:write [FILE]` / `:w`.
fn write(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    ed.write(a.arg).map(|_| ())
}

/// `:quit` / `:q`; `:q!` discards changes.
fn quit(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    if ed.document().is_modified() && !a.force {
        return Err(E37.into());
    }
    ed.quit();
    Ok(())
}

/// `:wq [FILE]`.
fn write_quit(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    ed.write(a.arg)?;
    ed.quit();
    Ok(())
}

/// `:xit` / `:x`: save only if modified, then quit.
fn xit(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    if ed.document().is_modified() || a.arg.is_some() {
        ed.write(a.arg)?;
    }
    ed.quit();
    Ok(())
}

/// `:edit [FILE]` / `:e`; `:e!` discards changes.
fn edit(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    if ed.document().is_modified() && !a.force {
        return Err(E37.into());
    }
    ed.edit(a.arg)
}
