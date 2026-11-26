# Writing plugins

A plugin is a Rust library compiled to a WebAssembly component (`wasm32-wasip2`) and shipped
with a manifest, `plugin.toml`. The **contract** between vox and plugins is one file,
[`wit/vox.wit`](https://component-model.bytecodealliance.org/design/wit.html) (WIT is a small
language for describing such interfaces), currently **plugin API 0.2**. The SDK crate
`vox-plugin-sdk` turns it into normal Rust functions.

## Every plugin looks the same

```text
my-plugin/
  plugin.toml     manifest: name, version, requirements, settings
  Cargo.toml      a cdylib crate depending on vox-plugin-sdk
  src/lib.rs      the plugin
  README.md       what it does, commands, settings
  CHANGELOG.md    changes per version
  .gitignore      build output
```

Installed, a plugin is a folder in a scope's plugin folder (see
[Project setups](../user/scopes.md)):

```text
~/.config/vox/plugins/my-plugin/      (or <project>/.vox/plugins/my-plugin/)
  plugin.toml
  plugin.wasm
  README.md
```

## Start, build, install

```sh
rustup target add wasm32-wasip2                       # once
```

In vox: `:plugin new my-plugin ~/src` creates `~/src/my-plugin` from the template (a working
plugin with one command and one setting). Then:

```sh
scripts/build-plugin.sh ~/src/my-plugin --install ~/.config/vox/plugins
```

builds it, packages `dist/my-plugin/{plugin.toml, plugin.wasm, README.md}`, and copies the
package into the plugin folder. In vox, `:reload plugins my-plugin` (or `<leader>dr` in
[developer mode](../user/developer-mode.md)) loads the new build.

## plugin.toml

```toml
[plugin]
name = "text-tools"          # must match the folder name
version = "0.2.0"
description = "Word count and case changes"
authors = ["Jane"]
api = "0.2"                  # plugin API it was built for; must match vox's
vox = ">=0.10"               # optional; an error-level requirement

[[requires]]                 # zero or more
plugin = "other-plugin"      # or:  program = "git"   or:  vox = ">=0.11"
version = ">=0.2"            # only for plugin = ...
level = "warning"            # error (not loaded) | warning | info
reason = "shows X in Y"

[settings.case]              # zero or more
type = "choice"              # string | integer | bool | choice
choices = ["upper", "lower", "title"]
default = "upper"
description = "What :upper does"

[settings.limit]
type = "integer"
min = 1                      # optional
max = 100                    # optional
default = 10
description = "..."
```

Versions are `MAJOR.MINOR.PATCH`; requirements use `>=`, `>`, `<=`, `<`, `=`, `^` (compatible,
the default for a bare version) and commas for "and": `">=1, <2"`.

## A complete plugin

```rust
use vox_plugin_sdk::{Event, EventKind, HealthItem, Level, MapMode, Plugin, editor, export_plugin};

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
    fn run_command(name: String, arg: Option<String>, _force: bool) -> Result<(), String> {
        match name.as_str() {
            "hello" => {
                let greeting = editor::setting("greeting").and_then(|v| v.as_text());
                let who = arg.unwrap_or_else(|| "world".into());
                editor::message(&format!("{}, {who}", greeting.unwrap_or("hello".into())));
                Ok(())
            }
            other => Err(format!("unknown command {other}")),
        }
    }

    /// Optional: events this plugin subscribed to.
    fn on_event(event: Event) {
        if let Event::BufWrite(path) = event {
            editor::log(Level::Info, &format!("saved {path}"));
        }
    }

    /// Optional: custom health checks (after init, and in :checkhealth).
    fn health() -> Vec<HealthItem> {
        Vec::new()
    }
}

export_plugin!(Hello);
```

## What a plugin can call (`editor::…`)

| Function | Purpose |
|---|---|
| `register_command(name, description)` | add `:name`; vox calls `run_command(name, …)` |
| `map_key(mode, keys, binding)` | key mapping, same notation as the config file |
| `subscribe(kind)` | receive `BufOpen`, `BufWrite`, `TextChanged`, `ModeChanged` in `on_event` |
| `setting(key)` | this plugin's setting (`as_text()`, `as_integer()`, `as_bool()`) |
| `message(text)` | show text on the bottom line |
| `notify(level, text)` | notification in the corner |
| `log(level, text)` | developer log line, source `plugin:NAME` |
| `run_command(line)` | run any command, e.g. `"w"` |
| `file_path()`, `line_count()`, `line(i)` | read the buffer |
| `insert_text(at, text)`, `delete_text(start, end)` | edit; one command's edits undo together |
| `cursor()`, `set_cursor(pos)`, `current_mode()` | cursor and mode |

## Settings

Users set values in the active scope's `config.toml`:

```toml
[plugins.text-tools]
case = "title"
enabled = true          # false: the plugin is not loaded
```

vox checks each value against `[settings.KEY]`. A wrong value is ignored with a warning (the
default is used) and shows up in `:checkhealth`. Unknown keys are warnings too.

## Health

Before loading, vox checks the package (files present, valid manifest, name matches folder),
the plugin API version, and every requirement. After `init` it calls the plugin's `health()`.

| Finding | Result |
|---|---|
| any unmet `error` (or an `Error` from `health()`) | **failed**: not loaded (or unloaded again) |
| unmet `warning` | **warning**: loaded |
| unmet `info` | **info**: loaded |
| all met | **ok** |
| `enabled = false` | **disabled** |

`:checkhealth [NAME]` shows the report and runs `health()` again. A plugin that requires a
plugin which failed fails too.

## Rules and limits
- Each call (`init`, `run_command`, `on_event`, `health`) has about 1 s and the plugin 64 MiB of
  memory. A panic, a crash, or a broken limit disables the plugin until it is reloaded.
- A plugin cannot call its own commands while it is running.
- No files, network, environment, or printing: everything goes through `editor`.
- A single `NAME.wasm` file without a manifest still loads (no version, settings, or
  requirements).

## How it works inside vox
- Core (`vox-editor::plugin`): manifest, versions, health checks, package discovery and
  assessment, the project template. No WebAssembly, std only.
- `vox-plugin-host` (adapter): runs plugins with `wasmtime`, feeds them their settings, and
  turns their registrations into normal commands, mappings, and event handlers.
- `vox-plugin-sdk` (for plugin authors): Rust glue from `wit/vox.wit` (`wit-bindgen`).
- See the code-tour walkthroughs **"Loading a plugin"** and **"Running a plugin command"**.

## Testing plugins
`crates/vox-plugin-host/tests/plugins.rs` builds `plugins/` and loads them into a real editor,
both as single files and as packages. `plugins/misbehave` breaks every rule on purpose; its
`health` setting makes its health check report ok, warning, or error.
