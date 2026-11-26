# Plugins

Plugins add commands, key mappings, and reactions to events. A plugin is a single `.wasm`
file (a WebAssembly component).

## Installing

Install into the active [scope](scopes.md) (your global setup, or the project with
`--local-config`):

```
:plugin add ~/src/my-plugin                                  # a folder
:plugin add https://github.com/me/my-plugin.git              # a git repository
:plugin add https://github.com/me/my-plugin.git@v1.2.0       # at a tag, branch, or commit
:plugin remove my-plugin
```

The source may be a packaged plugin (`plugin.toml` + `plugin.wasm`), copied as is, or a
plugin project (`plugin.toml` + `Cargo.toml`), which vox builds first. Building needs `cargo`
and `rustup target add wasm32-wasip2`; git sources need `git`. Installing runs in the
background with progress notifications, and the plugin is loaded as soon as it is ready.

Installed plugins live in the scope's plugin folder:

```
~/.config/vox/plugins/text-tools/        # global (default)
<project>/.vox/plugins/text-tools/       # local, with --local-config
```

A plugin folder copied there by hand also loads (and so does a single `NAME.wasm` file). With
`--vanilla` no plugins load.

## The lockfile

Every `:plugin add` and `:plugin remove` updates the scope's `vox.lock`
(`~/.config/vox/vox.lock`, or `<project>/.vox/vox.lock`). It records each plugin's version,
where it came from (a path, or a git URL with the exact commit), and a checksum of the
installed `plugin.wasm`:

```toml
[[plugin]]
name = "text-tools"
version = "0.2.0"
source = "git"
url = "https://github.com/me/text-tools.git"
ref = "v0.2.0"
rev = "a07d61ecc8f88ee2bee73217c385c2f83c2fd847"
checksum = "sha256:9b3bf287…"
```

`:plugins sync` installs exactly what the lockfile lists — plugins that are missing, or whose
installed file differs — from the recorded commit or path. At start-up vox tells you when the
installed plugins do not match the lockfile.

For projects, commit `.vox/config.toml` and `.vox/vox.lock`; `.vox/.gitignore` (written by
the project set-up) keeps the installed files out of git. After cloning, start
`vox --local-config` and run `:plugins sync`.

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
