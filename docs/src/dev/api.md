# Developer API

Everything a feature needs goes through a small public API on `vox_editor::Editor`. Built-in
commands use the same API, and plugins (M4) will too.

## Commands

A command is a name, a description, and a handler:

```rust
use vox_editor::{Args, CommandResult, Editor};

fn upper(ed: &mut Editor, args: Args<'_>) -> CommandResult {
    // args.arg: text after the name (e.g. `:upper all` → Some("all"))
    // args.force: whether the name ended with `!`
    ed.set_message(format!("upper {:?}", args.arg));
    Ok(())               // or Err("E123: message".into()) to show an error
}

editor.commands_mut().register("upper", "Uppercase the line", upper);
editor.commands_mut().alias("up", "upper");
```

Run it with `:upper`, `:up! all`, a mapping (`"<leader>u" = ":upper"`), or from code with
`editor.run_command("upper all")`. Registering an existing name replaces it, so built-in
commands (`crates/vox-editor/src/builtin.rs`) can be overridden.

## Key mappings

```rust
use vox_editor::{Binding, MapMode, parse_keys};

editor.keymap_mut().set(MapMode::Normal, parse_keys("Q"), Binding::Command("q!".into()));
editor.keymap_mut().set(MapMode::Insert, parse_keys("jk"), Binding::Keys(parse_keys("<Esc>")));
```

User mappings from the config file are added the same way by `Editor::apply_config`.

## Configuration

`Config::from_value(&Value)` validates settings from a format-neutral `Value` tree and returns
`(Config, errors)`. Core never parses a file format; the `vox-config-file` adapter turns TOML
into a `Value`. A new setting means: a field in `Config`, a case in `from_value`, and using it in
`apply_config`.

## API summary

| Function | Purpose |
|---|---|
| `commands()` / `commands_mut()` | look up / register commands |
| `run_command(line)` | run a command line |
| `keymap_mut()` | add or remove key mappings |
| `apply_config(&Config)` | install leader, timeout, mappings |
| `set_message(text)` | show text on the bottom line |
| `quit()` | ask the run loop to exit |
| `document()`, `cursor()`, `mode()`, `selection()` | read editor state |
| `handle_key(key)`, `timeout()`, `has_pending_keys()` | input (used by the run loop and tests) |
| `subscribe(kind, handler)` | react to `BufOpen`, `BufWrite`, `TextChanged`, `ModeChanged` |
| `insert_text`, `delete_text`, `set_cursor` | edit; edits made inside one command undo together |
| `map_key(mode, lhs, rhs)` | add a mapping with the user's leader key |
| `start()` | send the first `BufOpen` (called once by the binary) |
| `notify(level, text)`, `history()` | notifications; message history |
| `open_modal(view)`, `open_panel(slot, view)`, `close_focused()`, `focus(f)` | UI |
| `next_deadline()`, `tick()` | time-based work (run loop) |
| `with_owner(owner, f)`, `remove_owned(owner)` | ownership, unloading |
| `log(level, source, text)`, `log_entries()` | developer log |
| `set_developer_mode(on)`, `developer_mode()` | developer mode |

## UI: panels, modal, notifications

A **view** is anything that implements `vox_editor::View`: a title, `render` into an area,
`handle_key` returning an `Outcome`, and optionally a cursor. Built-in views: `ListView`,
`TextView`, `Confirm`, `Input` (`vox_editor::ui`).

```rust
use vox_editor::ui::{Confirm, ListItem, ListView};
use vox_editor::{Level, PanelSlot};

editor.notify(Level::Warn, "low disk space");
editor.open_modal(Box::new(Confirm::new("Delete", "Delete foo?", "delete-foo")));
editor.open_panel(PanelSlot::Side, Box::new(ListView::new("Items", vec![
    ListItem::new("first").detail("more info").command("notify picked first"),
])));
```

If a view's `RunAndClose` command opens another modal, the editor keeps the first one in a
history: the new one's `Close` goes back to it (`modal_depth()` tells how deep). A command that
opens no modal ends the chain.

Views do not get `&mut Editor`. When they want something done they return
`Outcome::Run("command line")` or `Outcome::RunAndClose(...)`, and the editor runs that
command. So every action a view can trigger is also a normal, mappable command.

Time: the editor never sleeps. `next_deadline()` says when something time-based is due
(notification expiry, mapping timeout); the run loop waits at most until then and calls
`tick()`. Tests inject a fake clock through `Services::clock`.

## Owners and reloading

Everything registered (commands, mappings, subscriptions) records its **owner**:
`builtin`, `config`, `devmode`, `plugin:NAME`, or `user` (the default). Constants are in
`vox_editor::owner`.

```rust
editor.with_owner("plugin:demo", |ed| {
    ed.commands_mut().register("demo", "", |_, _| Ok(()));
    ed.map_key(MapMode::Normal, "<leader>x", ":demo");
});
editor.remove_owned("plugin:demo"); // removes exactly those two
```

A newer owner's command or mapping with the same name hides the older one; removing it
brings the older one back.

## Log

`editor.log(Level::Warn, "plugin:demo", "something odd")` adds an entry (`Level::Debug` is
log-only). `log_entries()` reads it; open `LogView`s update automatically.

## Events

```rust
use vox_editor::{Event, EventKind};

editor.subscribe(EventKind::BufWrite, |ed, event| {
    if let Event::BufWrite { path } = event {
        ed.set_message(format!("saved {path}"));
    }
});
```

`TextChanged` and `ModeChanged` are sent once per key press or command, after it finishes.
Edits made by an event handler do not trigger another `TextChanged`, so handlers cannot loop.

See the code tour walkthrough **"Adding your own command"** for the call path.
