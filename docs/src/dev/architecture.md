# Architecture

Crate map and layering rules: `ARCHITECTURE.md` at the repository root.

## Data flow
```
key event ─> App::handle_key ─> Document edits / motions ─> App::follow_cursor
                                                             │
terminal.draw ─> impl Widget for &App (pure) ─> screen  <────┘
```

## vox-core
- `Document`: `ropey::Rope` + path + line ending + final-newline flag + undo history.
  The final newline is not stored in the rope, so every rope line is a visible line.
- Edits: `insert(pos, text)`, `delete(from, to)`. Each is recorded; `commit()` closes
  the pending edits into one undo step. `undo()`/`redo()` return where the change happened.
- `is_modified()` compares the current undo step id with the one saved on disk.
- `motion`: pure `fn(&Document, Position) -> Position` (`word_forward`, `word_backward`,
  `word_end`, `line_start`, `line_end`, `doc_start`, `doc_end`, `first_non_blank`, ...).
- `search::find`: literal search with wraparound.
- `Position { line, col }`: `col` counts chars, not bytes or cells.

## vox-view
- `Viewport { top, left }` with `follow(line, display_col, width, height)`.
- `display_col` / `visible_slice`: char columns to screen cells (tabs, wide chars).

## vox-tui
- `App`: mode, cursor, wanted column, viewport, registers, messages; `handle_key`.
- `normal::parse(keys, visual) -> Parse<Cmd>`: the Vim grammar
  `["x][count](operator[count](motion|operator) | motion | action)`. Pending keys are
  re-parsed after every key; `Incomplete` waits, `Invalid` discards.
- `App::exec(Cmd)`: runs a command. Operators turn a motion into a `Range`
  (exclusive / inclusive / linewise), write the register, then delete/change/yank.
- `register`: unnamed register + `Clipboard` trait (`SystemClipboard` via `arboard`;
  tests inject an in-memory one with `App::set_clipboard`).
- `parse_keys("ihi<Esc>")`: Vim key notation → key events (tests now, keymap config later).
- Rendering is a pure `impl Widget for &App`; `cursor_screen_position` gives the terminal cursor.
- `run`: draw → set cursor shape (bar in insert) → read key.
