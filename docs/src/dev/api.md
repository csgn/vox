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
