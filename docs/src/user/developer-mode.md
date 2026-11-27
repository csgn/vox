# Developer mode

For people writing plugins or changing their config: see what vox and plugins are doing,
and reload without restarting.

Turn it on in `config.toml`:

```toml
developer_mode = true
```

or for this session with `:devmode` (toggle), `:devmode on`, `:devmode off`. The status line
shows `DEV` while it is on.

## Commands and shortcuts (developer mode only)

Developer mode is separate from the [editor mode](agent-mode.md) (`text` / `agent`); both
can be on. Like `:ai` in agent mode, the `:log` commands exist only while developer mode is
on: `:log`, `:log clear`, `:log refresh`. Turning it off removes them and closes an open log.
`:devmode`, `:reload config`, and `:reload plugins` work in every mode.

| Keys | Command | Action |
|---|---|---|
| `<leader>dl` | `:log` | open the log |
| `<leader>dr` | `:reload plugins` | reload all plugins |
| `<leader>dc` | `:reload config` | re-read `config.toml` |

They are ordinary mappings, so you can change them in `[keymap.normal]`.

## The log

`:log [debug|info|warn|error] [SOURCE]` opens the log in the bottom panel, newest at the
bottom. It updates while open.

| Key | Action |
|---|---|
| `f` | minimum level: debug → info → warn → error → debug |
| `s` | source: all → each source in turn → all |
| `c` | clear the log |
| `j`/`k`, `g`/`G` | scroll; `G` follows new lines |
| `q`, `Esc` | close |

Sources:

| Source | What |
|---|---|
| `editor` | commands you run (debug), errors |
| `config` | config loaded, warnings about bad settings |
| `builtin` | errors from built-in commands |
| `plugin:NAME` | plugin loaded / failed / disabled, and the plugin's own log lines |

The log keeps the last 2000 entries.

## Reloading

- `:reload config` re-reads the config file. Mappings from the old config are removed first,
  so deleted mappings really go away.
- `:reload plugins` unloads every plugin and loads all files in the plugin folder again.
  `:reload plugins NAME` reloads one. Reloading also revives a plugin that was disabled after
  an error.

Unloading is exact: vox remembers who added every command, mapping, and event handler
(`builtin`, `config`, `devmode`, `plugin:NAME`) and removes only that owner's. If a plugin
replaced a built-in command, the built-in comes back. `:keymaps` shows each mapping's owner.
