# Writing plugins

A plugin is a Rust library compiled to a WebAssembly component (`wasm32-wasip2`). The
**contract** between vox and plugins is one file: [`wit/vox.wit`](https://component-model.bytecodealliance.org/design/wit.html)
(WIT is a small language for describing such interfaces). The SDK crate `vox-plugin-sdk`
turns it into normal Rust functions.

## Setup

```sh
rustup target add wasm32-wasip2
```

`Cargo.toml`:

```toml
[package]
name = "hello"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
vox-plugin-sdk = { path = "../../crates/vox-plugin-sdk" }
```

## A complete plugin

```rust
use vox_plugin_sdk::{Event, EventKind, MapMode, Plugin, editor, export_plugin};

struct Hello;

impl Plugin for Hello {
    /// Called once at load time: register everything here.
    fn init() -> Result<(), String> {
        editor::register_command("hello", "Say hello");
        editor::map_key(MapMode::Normal, "<leader>h", ":hello");
        editor::subscribe(EventKind::BufWrite);
        Ok(())
    }

    /// Called when the user runs one of this plugin's commands.
    fn run_command(name: String, arg: Option<String>, force: bool) -> Result<(), String> {
        match name.as_str() {
            "hello" => {
                let who = arg.unwrap_or_else(|| "world".into());
                editor::message(&format!("hello, {who}{}", if force { "!" } else { "" }));
                Ok(())
            }
            other => Err(format!("unknown command {other}")),
        }
    }

    /// Called for events this plugin subscribed to.
    fn on_event(event: Event) {
        if let Event::BufWrite(path) = event {
            editor::message(&format!("saved {path}"));
        }
    }
}

export_plugin!(Hello);
```

Build: `cargo build --release --target wasm32-wasip2`, then copy
`target/wasm32-wasip2/release/hello.wasm` into `~/.config/vox/plugins/`.

## What a plugin can call (`editor::…`)

| Function | Purpose |
|---|---|
| `register_command(name, description)` | add `:name`; vox calls `run_command(name, …)` |
| `map_key(mode, keys, binding)` | key mapping, same notation as the config file |
| `subscribe(kind)` | receive `BufOpen`, `BufWrite`, `TextChanged`, `ModeChanged` in `on_event` |
| `message(text)` | show text on the bottom line |
| `run_command(line)` | run any command, e.g. `"w"` |
| `file_path()`, `line_count()`, `line(i)` | read the buffer |
| `insert_text(at, text)`, `delete_text(start, end)` | edit; one command's edits undo together |
| `cursor()`, `set_cursor(pos)`, `current_mode()` | cursor and mode |

Positions are `Position { line, col }`, zero-based, `col` in characters.

## Rules and limits
- Each call (`init`, `run_command`, `on_event`) has about 1 s and the plugin 64 MiB of memory.
  A panic, a crash, or a broken limit disables the plugin.
- A plugin cannot call its own commands while it is running (the inner call returns an
  error message instead of deadlocking).
- No files, network, environment, clock-based sleeping, or printing: everything goes through
  the `editor` functions.

## How it works inside vox
- `vox-plugin-sdk` (for plugin authors): generates the Rust glue from `wit/vox.wit` with
  `wit-bindgen`.
- `vox-plugin-host` (adapter, inside vox): runs plugins with `wasmtime`. It turns each
  registered command into a normal `Commands` entry and each subscription into an
  `Editor::subscribe` handler. See the code-tour walkthroughs **"Loading a plugin"** and
  **"Running a plugin command"**.
- Core (`vox-editor`) knows nothing about WebAssembly: it only offers the developer API
  (commands, mappings, events, edit functions).

## Testing plugins
`crates/vox-plugin-host/tests/plugins.rs` builds `plugins/` and loads the plugins into a real
editor. `plugins/misbehave` breaks every rule on purpose to test the limits.
