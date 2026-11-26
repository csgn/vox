//! Built-in `:` commands, registered through the same [`Commands`] API that plugins use.

use crate::Editor;
use crate::command::{Args, CommandResult, Commands};
use crate::keymap::{Binding, MapMode};
use crate::keys::format_keys;
use crate::surface::Style;
use crate::ui::{Focus, Level, ListItem, ListView, PanelSlot, TextView};

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
    c.register(
        "notify",
        "Show a notification: `:notify [info|warn|error] TEXT`.",
        notify,
    );
    c.register(
        "messages",
        "Show message history in the bottom panel.",
        messages,
    );
    c.alias("mes", "messages");
    c.register("commands", "List all commands; Enter runs one.", commands);
    c.register(
        "keymaps",
        "List your key mappings in the side panel.",
        keymaps,
    );
    c.register("close", "Close the modal or the focused panel.", close);
    c.register("focus", "Move focus: `:focus editor|side|bottom`.", focus);
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

/// `:notify [level] TEXT`.
fn notify(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let text = a.arg.ok_or("E471: Argument required")?;
    let (level, text) = match text.split_once(' ') {
        Some((l, rest)) if Level::parse(l).is_some() => (Level::parse(l).unwrap(), rest),
        _ => (Level::Info, text),
    };
    ed.notify(level, text);
    Ok(())
}

/// `:messages`: history in a text view in the bottom panel, newest at the bottom.
fn messages(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    let lines = ed
        .history()
        .iter()
        .map(|(level, text)| {
            let style = match level {
                Level::Info => Style::default(),
                l => Style::default().fg(l.color()),
            };
            (style, format!("{:<7} {text}", level.name()))
        })
        .collect();
    ed.open_panel(
        PanelSlot::Bottom,
        Box::new(TextView::new("Messages", lines).at_end()),
    );
    Ok(())
}

/// `:commands`: every command with its aliases and description, in the modal.
fn commands(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    let items = ed
        .commands()
        .iter()
        .map(|c| {
            let aliases: Vec<&str> = ed.commands().aliases_of(&c.name).collect();
            let label = if aliases.is_empty() {
                c.name.clone()
            } else {
                format!("{} ({})", c.name, aliases.join(", "))
            };
            ListItem::new(label).detail(&c.description).command(&c.name)
        })
        .collect();
    ed.open_modal(Box::new(ListView::new("Commands", items)));
    Ok(())
}

/// `:keymaps`: user mappings in the side panel.
fn keymaps(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    let items = ed
        .keymap()
        .iter()
        .map(|(mode, lhs, b)| {
            let mode = match mode {
                MapMode::Normal => "n",
                MapMode::Insert => "i",
                MapMode::Visual => "v",
            };
            let rhs = match b {
                Binding::Keys(k) if k.is_empty() => "(disabled)".to_string(),
                Binding::Keys(k) => format_keys(k),
                Binding::Command(c) => format!(":{c}"),
            };
            ListItem::new(format!("{mode} {}", format_keys(lhs))).detail(format!("→ {rhs}"))
        })
        .collect();
    ed.open_panel(
        PanelSlot::Side,
        Box::new(ListView::new("Key mappings", items)),
    );
    Ok(())
}

/// `:close`.
fn close(ed: &mut Editor, _: Args<'_>) -> CommandResult {
    ed.close_focused();
    Ok(())
}

/// `:focus editor|side|bottom`.
fn focus(ed: &mut Editor, a: Args<'_>) -> CommandResult {
    let target = match a.arg {
        Some("editor") => Focus::Editor,
        Some("side") => Focus::Side,
        Some("bottom") => Focus::Bottom,
        _ => return Err("E474: Use :focus editor|side|bottom".into()),
    };
    ed.focus(target);
    Ok(())
}
