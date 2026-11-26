# Configuration

vox reads one file at start-up:

1. `$VOX_CONFIG` if set (any path), else
2. `$XDG_CONFIG_HOME/vox/config.toml`, else
3. `~/.config/vox/config.toml`.

No file means default settings. The file uses [TOML](https://toml.io), a simple
`key = value` format. Problems (syntax errors, unknown settings, wrong types) are shown on the
bottom line when vox starts; the valid parts of the file are still used.

## Example

```toml
leader = " "          # <leader> is the space bar
timeout_ms = 500      # wait 0.5 s for the rest of a mapping

[keymap.normal]
"<leader>w" = ":w"    # space w saves
"<leader>q" = ":q"
"<C-s>"     = ":w"
H = "^"               # H jumps to the first non-blank char
L = "$"
x = ""                # disable x

[keymap.insert]
jk = "<Esc>"          # type jk quickly to leave insert mode

[keymap.visual]
"<leader>y" = "\"+y"  # copy the selection to the system clipboard
```

## Settings

| Setting | Type | Default | Meaning |
|---|---|---|---|
| `leader` | string | `"\"` (backslash) | What `<leader>` means in mappings. |
| `timeout_ms` | integer | `1000` | How long to wait for the rest of a mapping before using the keys as typed. |
| `[keymap.normal]` | table | — | Mappings in normal mode (also after an operator like `d`). |
| `[keymap.insert]` | table | — | Mappings in insert mode. |
| `[keymap.visual]` | table | — | Mappings in visual mode (`v` and `V`). |

## Mappings

Each line is `"keys" = "what to do"`. Both sides use Vim key notation: `<Esc>`, `<CR>`,
`<C-x>` (Ctrl+x), `<Space>`, `<Tab>`, `<BS>`, `<Up>`, `<lt>` (a literal `<`), `<leader>`.

| Right side | Meaning | Example |
|---|---|---|
| starts with `:` | run that command | `"<leader>w" = ":w"` |
| anything else | press these keys instead | `H = "^"` |
| empty string | do nothing (disable the key) | `x = ""` |

- Mappings are checked before vox's built-in keys, so they can replace them.
- Keys produced by a mapping are **not** mapped again (like Vim's `noremap`), so `H = "^"`
  always runs the built-in `^`.
- When you type the start of a mapping (e.g. `j` of `jk`), vox waits up to `timeout_ms` for the
  rest. If the next key does not continue the mapping, or time runs out, the keys work as typed.
- Mappings do not apply while typing in the `:` and `/` prompts.

Commands available to mappings: see [Commands](commands.md).
