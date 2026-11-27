# Editor modes: text and agent

vox has two editor modes:

- **text** (default) — the Vim-style editor. No AI anywhere.
- **agent** — the same editor plus AI help on the [focused unit](focus.md): press a key,
  say what to change, and the AI's change appears in the buffer as a diff that you accept or
  decline. There is no chat panel; the editor stays the UI.

The AI part arrives in the next versions (see the [roadmap](../dev/roadmap.md)); for now
agent mode shows `AGENT` in the status line and is where those features will live.

## Switching

| How | |
|---|---|
| `<leader>m`, `:mode toggle` | switch between text and agent |
| `:mode text`, `:mode agent` | switch to one |
| `:mode` | show the current mode |
| `vox --agent FILE` | start in agent mode |
| `editor_mode = "agent"` in `config.toml` | the default for this scope (also in `:settings`) |

`vox --agent` wins over the config file, also when the config is reloaded; the config's
`editor_mode` only takes effect when its value changes. Everything that only agent mode has
is removed when you switch back to text mode.
