# Architecture

Crate map, layers, and the dependency rule: `ARCHITECTURE.md` at the repository root.

## Data flow
```
crossterm event ─> vox-term::convert ─> Key ─> Editor::handle_key ─> Document edits / motions
                                                                       │
terminal <─ vox-term::Terminal::draw <─ Surface <─ Editor::render (pure) <┘
```

## vox-core
- `TextStorage` trait: char-indexed text, lines split on `\n` only. `SimpleStorage`
  (`Vec<char>`) for tests; `vox-storage` for real use. `storage::conformance(factory)` checks
  any implementation.
- `Document`: `Box<dyn TextStorage>` + path + line ending + final-newline flag + undo history.
  The final newline is not stored, so every storage line is a visible line.
  `Document::new(text, factory)`, `Document::open(path, factory)`, `Document::from_text` (tests).
- Edits: `insert(pos, text)`, `delete(from, to)`. `commit()` closes pending edits into one undo
  step. `undo()`/`redo()` return where the change happened.
- `motion`: pure `fn(&Document, Position) -> Position`. `search::find`: literal, wraps.

## vox-view
- `Viewport { top, left }` with `follow(line, display_col, width, height)`.
- `CharWidth` trait (`Monospace` fallback); `display_col` / `visible_slice` take `&dyn CharWidth`.

## vox-editor
- `Editor`: mode, cursor, wanted column, viewport, registers, messages; `handle_key(Key)`.
- `Services { storage, width, clipboard }`: injected platform pieces; `Default` = std stand-ins.
- `Key { code, mods }` + `parse_keys("ihi<Esc>")` (Vim notation).
- `normal::parse(keys, visual) -> Parse<Cmd>`: the grammar
  `["x][count](operator[count](motion|operator) | motion | action)`. Pending keys are
  re-parsed after every key; `Incomplete` waits, `Invalid` discards.
- Input path: `handle_key` → `resolve_mappings` (user `Keymap`, with timeout) → `dispatch` →
  mode handler. Mapped keys are dispatched without re-mapping.
- `Commands`: name/alias → handler `Fn(&mut Editor, Args) -> Result<(), String>`. Built-ins
  live in `builtin.rs` and are registered like any other command. `run_command(line)` runs one.
- `Config::from_value(&Value)`: validates settings from a format-neutral `Value` tree (the TOML
  file is parsed by the `vox-config-file` adapter); `Editor::apply_config` installs them.
- `Editor::exec(Cmd)`: operators turn a motion into a range (exclusive / inclusive /
  linewise), write the register, then delete / change / yank.
- `Surface`: cells (`symbol`, `Style { dim, reversed }`), cursor, cursor shape.
  `Editor::render()` fills it. `Display` prints rows for snapshot tests.

## vox-term
- `Terminal::enter()` (raw mode, alternate screen, panic-safe restore), `draw(&Surface)` with
  per-row diffing, `read_key()`, and `run(&mut Editor)`.
