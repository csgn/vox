# Panels, modal, and notifications

Besides the text, vox can show:

| Part | Where | Used by |
|---|---|---|
| **Side panel** | left or right of the text | `:keymaps` |
| **Bottom panel** | under the text | `:messages` |
| **Modal** | centered, on top of everything | `:commands`, questions (yes/no), prompts |
| **Notifications** | a screen corner, for a few seconds | `:notify`, plugins |

## Focus

One part has **focus** and gets your keys. The status line shows `PANEL` while a panel has it,
and the focused panel's border is highlighted.

| Key | Action |
|---|---|
| `<C-w>h` / `<C-w>l` | focus the part to the left / right (text ↔ side panel) |
| `<C-w>j` / `<C-w>k` | focus the bottom panel / back up to the text |
| `<C-w>w` | focus the next part |
| `<C-w>c`, `<C-w>q`, `:close` | close the focused panel (or the modal) |
| `q`, `Esc` | close the panel or modal you are in |
| `:` | the command line works while a panel has focus |

A **modal** always takes all keys until it closes.

## Inside a panel or modal

| View | Keys |
|---|---|
| List | `j`/`k` or arrows move, `g`/`G` first/last, `Enter` choose, `q`/`Esc` close |
| Text | `j`/`k` scroll, `g`/`G` top/bottom, `q`/`Esc` close |
| Question | `y`/`Enter` yes, `n`/`q`/`Esc` no |
| Prompt | type, `Backspace`, `Enter` confirm, `Esc` cancel |

## Notifications

Short messages in a box colored by level: **info** (blue), **warning** (yellow), **error**
(red). They disappear after `notify_timeout_ms`. All messages and notifications are kept in
`:messages`.

## Settings

In `config.toml`:

```toml
[ui]
side_panel = "right"          # or "left"
side_width = 40               # columns
bottom_height = 12            # rows
modal_width = 70              # percent of the screen
modal_height = 60             # percent of the screen
notify_timeout_ms = 4000
notify_position = "top-right" # or "bottom-right"
```

Panels shrink on small screens so the text stays usable.
