# Editor modes: text and agent

vox has two editor modes:

- **text** (default) — the Vim-style editor. No AI anywhere.
- **agent** — the same editor plus AI help on the [focused unit](focus.md): press a key,
  say what to change, and the AI's change appears in the buffer as a diff that you accept or
  decline. There is no chat panel; the editor stays the UI.

Agent mode shows `AGENT` in the status line. The AI is Claude, asked through Claude Code
(the `claude` command), so it runs on your Claude subscription (Pro or Max) or whatever
`claude` is logged in with. No API key is needed.

## Setting up Claude

1. Install Claude Code ([claude.com/claude-code](https://claude.com/claude-code)) so that
   `claude` is on your `PATH`.
2. Run `claude` once and log in with your Claude account.

That's all. Each `:ai` runs `claude -p` headless in the background: no tools, no saved
session, the answer as JSON. It counts against your plan's usage limits like any Claude Code
use. If `claude` is missing, not logged in, or out of usage, `:ai` shows its message.

| Setting | Default | |
|---|---|---|
| `ai_model` | `""` → `claude`'s default | which model answers: an alias (`opus`, `sonnet`, `haiku`, `fable`) or a full name like `claude-sonnet-5-5` |
| `ai_effort` | `"medium"` | `low`, `medium`, `high`, `xhigh`, `max`: how hard the model works; more is slower and uses more of your plan |

Both are in `config.toml` and `:settings`. Each question sends the whole file and the target
to Claude.

## Asking

| Key / command | Action |
|---|---|
| `<leader>a` | ask about the target in a small dialog (`Ask AI · function area (L4–10)`) |
| `:ai INSTRUCTION` | ask without the dialog, e.g. `:ai add error handling` |

The **target** is what the AI may change: the visual selection if you are selecting, else the
[focused unit](focus.md), else the cursor line. The AI gets the whole file for context, but
its answer only replaces the target's lines. While it works, a box shows the request and the
time so far; you can keep moving around.

## Reviewing

The answer appears in the buffer as a diff: lines it removes are marked `-` in red, lines it
adds follow them marked `+` in green. Nothing is changed until you decide.

| Key / command | Action |
|---|---|
| `ga` / `:ai accept` | accept the change at the cursor |
| `gx` / `:ai decline` | decline it |
| `gA` / `gX` (`:ai accept all`, `:ai decline all`) | all changes |
| `]c` / `[c` (`:ai next`, `:ai prev`) | next / previous change |

Accepted changes are one undo step (`u`). Editing the text, switching buffers, or leaving
agent mode drops a pending change. If the text changed while the AI was working, its
answer is not used; ask again.

With `ai_apply = "direct"` (in `config.toml` or `:settings`), the answer is applied at once
as one undo step instead.

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
