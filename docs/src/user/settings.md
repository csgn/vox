# Settings dialog

`:settings` edits settings inside vox. Changes are written to the active
[scope](scopes.md)'s `config.toml` — only the changed entries; your comments and layout stay —
and take effect immediately.

| Row | Opens | Command |
|---|---|---|
| vox settings | form: leader, timeouts, panels, notifications, developer mode | `:settings vox` |
| Key mappings | list of mappings: add, change, delete | `:settings keys` |
| plugin NAME | form of the plugin's own settings (from its `plugin.toml`) | `:settings NAME` or `:settings plugin NAME` |

The dialog's title says where changes go, e.g. `Settings — global: ~/.config/vox/config.toml`.
In vanilla mode (`--vanilla`) changes apply for the session only and nothing is saved.

## Forms

| Key | Action |
|---|---|
| `j` / `k` | move between settings |
| `Enter` | edit text or a number (type, `Enter` accept, `Esc` cancel); toggle or cycle otherwise |
| `Space`, `l` / `h` | on/off, or next / previous choice |
| `r` | reset to the default |
| `s` | save and go back |
| `Esc` / `q` | go back; with unsaved changes it asks first (press `Esc` again to discard) |

Changed settings are marked with `*`. Below the list: what the selected setting does, its
allowed values, its default, and any problem with what you typed. Invalid values are never
written.

Saving a plugin's settings reloads that plugin, so the new values apply right away.

## Key mappings

The list shows every mapping with its owner. Mappings from your config can be changed;
mappings from plugins or developer mode are shown dimmed and are read-only.

| Key | Action |
|---|---|
| `a` | add: type `MODE KEYS TARGET`, e.g. `n <leader>w :w` (mode `n`, `i`, or `v`) |
| `Enter` / `e` | change the target of the selected mapping |
| `d` | delete (asks first) |
| `Esc` | back |

The same as commands: `:settings key add n <leader>w :w`, `:settings key delete n <leader>w`.

## Example

Before:

```toml
# my vox config
leader = " "   # space as leader

[ui]
# I like a wide side panel
side_width = 50
```

After changing `ui.side_width` to 32, setting text-tools' `case` to `lower`, and adding
`Q → :wordcount`:

```toml
# my vox config
leader = " "   # space as leader

[ui]
# I like a wide side panel
side_width = 32

[plugins.text-tools]
case = "lower"

[keymap.normal]
Q = ":wordcount"
```
