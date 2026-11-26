# Plugins

Plugins add commands, key mappings, and reactions to events. A plugin is a single `.wasm`
file (a WebAssembly component).

## Installing

A plugin is a folder (`NAME/` with `plugin.toml` and `plugin.wasm`) or a single `.wasm` file
in the plugin folder of the active [scope](scopes.md):

```
~/.config/vox/plugins/text-tools/        # global (default)
<project>/.vox/plugins/text-tools/       # local, with --local-config
```

vox loads them at start-up, in name order. Remove the folder to uninstall. With `--vanilla`
no plugins load.

## Settings and turning plugins off

Each plugin lists its settings in its README and `plugin.toml`. Set them in `config.toml`:

```toml
[plugins.text-tools]
case = "title"
count_on_save = false
enabled = true        # false: do not load this plugin
```

Wrong values are ignored (the default is used) with a warning.

## Health

Before loading a plugin, vox checks what it needs: the right plugin API, a vox version, other
plugins, programs. Each requirement is an error (not loaded), a warning, or info.

`:checkhealth` shows every plugin's status — **ok**, **info**, **warning**, **failed**,
**disabled** — with each finding and the settings in use; `:checkhealth NAME` shows one. At
start-up vox notifies you about plugins that failed or have warnings.

## Safety

Plugins run in a sandbox:

- **No access** to your files, the network, environment variables, or other programs. A plugin
  can only use what vox offers: read and edit the open text, show messages, add commands and
  key mappings, and receive events.
- **Time limit:** each call into a plugin may take about 1 second.
- **Memory limit:** 64 MiB per plugin.

A plugin that crashes or breaks a limit is disabled for the rest of the session and a message
says so. The editor keeps working.

## Example: text-tools

The repository includes an example plugin in `plugins/text-tools`. Build and install it:

```sh
rustup target add wasm32-wasip2      # once
./scripts/build-plugin.sh plugins/text-tools --install ~/.config/vox/plugins
```

It adds:

| Command / key | Action |
|---|---|
| `:wordcount`, `<leader>c` | show words, lines, and characters |
| `:upper` | change the case of the cursor line: setting `case` = upper, lower, title (one `u` undoes it) |
| after `:w` | shows the word count (setting `count_on_save`) |

To write your own, see [Writing plugins](../dev/plugins.md).
