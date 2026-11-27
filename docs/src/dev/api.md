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

**Help and completion:** `register` returns the command, so metadata can follow. Every
built-in command has a category; a test fails if one is missing.

```rust
use vox_editor::{Complete, Param, command::category};

editor
    .commands_mut()
    .register("upper", "Uppercase the line", upper)
    .category("Text")                                 // default: Other / User / Plugin: NAME
    .param(Param::optional("WHAT", "line or all").values(&["line", "all"]))
    .force("Also uppercase the next line.")           // what `:upper!` does
    .returns("Nothing.")                              // for data commands: what `reply` holds
    .example("upper all", "the whole buffer");
```

`Param::required` / `Param::optional`, `.repeated()` (`NAME...`), and `.complete(..)` say where
the `:` prompt finds completions: `Complete::File`, `Dir`, `Command`, `Plugin`, `SettingKey`,
`Version`, or `Values`. `Command::usage()` builds the usage line (`upper[!] [WHAT]`). `:help
NAME` shows all of it; `:commands info NAME` returns it as data.

**Buffers:** `editor.open_buffer(path)` opens a file in a new buffer (or shows it if open),
`add_buffer` opens one without showing it, `switch_buffer(id)`, `close_buffer(id, force)`,
`buffers()` lists `BufferInfo { id, name, modified, active, lines }`. `document()` and the
cursor always refer to the buffer shown; plugins see that one.

**Completion:** `editor.complete(line)` returns what can follow a command line: the start
of the word to replace, the candidates (text and description), and the usage and current
parameter of the command. The `:` prompt uses it; so can anything that needs completion
without the UI.

**Subcommands:** a name with one space, e.g. `register("reload plugins", ...)`, makes `:reload
plugins NAME` work (argument `NAME`). Different owners can add subcommands to the same root.
If there is no command named just the root, `:reload` lists the subcommands. Do not use
hyphenated names like `reload-plugins` for related commands.

Run it with `:upper`, `:up! all`, a mapping (`"<leader>u" = ":upper"`), or from code with
`editor.run_command("upper all")`. Registering an existing name replaces it, so built-in
commands (`crates/vox-editor/src/builtin.rs`) can be overridden.

**Returning data:** a command calls `ed.reply(value)` with a `Value` (build tables with
`Value::table([("name", "x".into()), ...])`). `editor.execute(line)` runs a command line and
returns `Ok(Some(value))`, `Ok(None)`, or `Err(message)` without showing anything; this is
what `vox run` and tests use. `run_command` shows the reply instead: one line in the message
line, more in a modal (`Value::to_text`). `vox run --json` prints it with `Value::to_json`.

**Without a person:** `editor.set_interactive(false)` (for `vox run`). Then `open_modal` and
`open_panel` open nothing and the command fails with "needs the editor UI". A command that
would ask for a missing argument or a confirmation checks `ed.interactive()` and returns an
error naming the form that does not ask (`:plugin remove! NAME`). Give every action such a
form, so it can be scripted.

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
| `run_command(line)` | run a command line; errors and replies are shown |
| `execute(line)` | run a command line; returns the reply or the error |
| `reply(value)` | return data from the running command |
| `interactive()`, `set_interactive(on)` | whether a person is at the keyboard |
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
| `config()`, `save_config_edits(edits)`, `set_config_writer(w)` | read and change the config |
| `set_plugin_settings(name, specs)`, `plugin_settings()` | plugin settings schemas (from the plugin host) |
| `plugin_infos()`, `set_plugin_infos(infos)` | status of every plugin (set by the plugin host) |
| `refresh_views()` | let open panels and the modal update (`View::refresh`); runs after commands and jobs |
| `spawn_quiet_job(work)` | like `spawn_job`, but nothing is shown; progress only goes to the log |
| `set_update_source(src)`, `update_source()` | where vox updates come from (set by the binary) |
| `spawn_job(title, work)`, `jobs_running()`, `job_status()` | background work; a box shows `title`, the last progress `work` reported, and the time so far until it is done; `work` returns a `JobDone` run on the editor's thread |
| `set_file_format(parse, format)` | TOML reader/writer for plugin and lock files (from an adapter) |

## UI: panels, modal, notifications

A **view** is anything that implements `vox_editor::View`: a title, `render` into an area,
`handle_key` returning an `Outcome`, and optionally a cursor. Built-in views: `ListView`,
`TextView`, `Confirm`, `Input`, `TreeView` (`vox_editor::ui`). `TreeView` browses
`TreeNode`s one level at a time (columns, breadcrumb, filter); each node can name a command
line for `Enter` and one for `?`. The command browser is built with it
(`vox_editor::help::command_tree`).

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

## Settings

- `vox_editor::settings::vox_settings()`: the schema of vox's own settings (same
  `SettingSpec` type plugins use). A test checks it covers every `Config` field.
- `Config::to_value()` and `ConfigEdit::{Set, Remove}` with dotted paths;
  `settings::apply_edits` changes a value tree.
- `Editor::save_config_edits(&edits)`: validates, writes through the injected
  `ConfigWriter` (`vox-config-file::write_edits`, comment-preserving), applies. Returns
  whether a file was written (not in vanilla mode).
- `ui::Form`: a form over `SettingSpec`s; saving returns `Outcome::Submit { command,
  values }`, the editor goes back to the previous dialog and runs `command`, which reads the
  values with `Editor::take_submitted()`.
- `Input::returning()` / `Confirm::returning()`: on confirm, go back to the dialog they came
  from before running the command (used by the key-mapping list to refresh itself).

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
