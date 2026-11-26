# Plugins

Plugins add commands, key mappings, and reactions to events. A plugin is a single `.wasm`
file (a WebAssembly component).

## Installing

Put the `.wasm` file in the `plugins` folder of the active [scope](scopes.md):

```
~/.config/vox/plugins/text_tools.wasm        # global (default)
<project>/.vox/plugins/text_tools.wasm       # local, with --local-config
```

vox loads every `*.wasm` file there at start-up, in name order. Remove the file to
uninstall. With `--vanilla` no plugins load.

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
./scripts/build-plugins.sh
mkdir -p ~/.config/vox/plugins
cp plugins/target/wasm32-wasip2/release/text_tools.wasm ~/.config/vox/plugins/
```

It adds:

| Command / key | Action |
|---|---|
| `:wordcount`, `<leader>c` | show words, lines, and characters |
| `:upper` | uppercase the cursor line (one `u` undoes it) |
| after `:w` | shows the word count |

To write your own, see [Writing plugins](../dev/plugins.md).
