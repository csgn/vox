# Changelog

All notable changes to this project are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [SemVer](https://semver.org/).

## [Unreleased]

## [0.7.0] - 2026-11-26 — M5 panels, modal, notifications

### Added
- UI layer in core: side panel (left/right), bottom panel, centered modal, notification stack.
- Views: `ListView`, `TextView`, `Confirm` (yes/no), `Input` (one-line prompt); `View` trait
  for custom views. Views act through command lines (`Outcome::Run`), so actions are mappable.
- Focus and keys: `<C-w>h/j/k/l/w`, `<C-w>c`/`q`, `q`/`Esc` in views; modal takes all keys;
  `:` works from panels; status line shows `PANEL`.
- Notifications (info / warning / error, colored, auto-dismiss) and message history.
- Commands: `:notify`, `:messages` (`:mes`), `:commands`, `:keymaps`, `:close`, `:focus`.
- `[ui]` settings: `side_panel`, `side_width`, `bottom_height`, `modal_width`,
  `modal_height`, `notify_timeout_ms`, `notify_position`.
- `Style` gets `bold` and `fg: Color`; `Surface` gets `Rect`, `draw_box`, `put_in`,
  `cursor_visible`.
- Time handling: `Editor::next_deadline()` / `tick()`, clock injected via `Services::clock`.
- `format_keys` (inverse of `parse_keys`), `Keymap::iter`, `Editor::keymap`.
- Docs: Panels and notifications page; UI section in Developer API; code-tour walkthroughs.

### Changed
- `vox-term` run loop waits until the next deadline instead of only for mapping timeouts;
  hides the cursor when a view has none; draws colors and bold.

## [0.6.0] - 2026-11-26 — M4 plugins

### Added
- WebAssembly plugins written in Rust. Contract: `wit/vox.wit` (commands, key mappings,
  events, read/edit text, cursor, mode, messages).
- `vox-plugin-host` adapter (wasmtime): loads `~/.config/vox/plugins/*.wasm`; per call ~1 s
  time limit, 64 MiB memory limit, no file/network/environment access. A crashing or
  misbehaving plugin is disabled; re-entrant calls return an error instead of deadlocking.
- `vox-plugin-sdk` for plugin authors: `Plugin` trait, `editor::*` functions,
  `export_plugin!` (wit-bindgen).
- Example plugin `plugins/text-tools` (`:wordcount`, `:upper`, `<leader>c`, message after save);
  test plugin `plugins/misbehave`; `scripts/build-plugins.sh`.
- Editor events: `Event` (`BufOpen`, `BufWrite`, `TextChanged`, `ModeChanged`),
  `Editor::subscribe`, `Editor::start`.
- Edit API: `Editor::insert_text`, `delete_text`, public `set_cursor`, `map_key`, `mode_kind`.
- `Document::revision`.
- Docs: Plugins (users), Writing plugins (authors), events in Developer API; code-tour
  walkthroughs "Loading a plugin" and "Running a plugin command"; SDK shown separately on the
  crate map.

### Changed
- A command's edits are one undo step and the cursor is kept valid after a command.
- CI installs the `wasm32-wasip2` target for plugin tests.

## [0.5.0] - 2026-11-26 — M3 developer API and configuration

### Added
- Command registry (`Commands`): `register(name, description, handler)`, `alias`, `get`.
  `Editor::run_command(line)` runs `:name[!] [arg]`; a handler's `Err` becomes the message.
- Built-in commands (`write`, `quit`, `wq`, `xit`, `edit` and short names) are registered
  through the same API, so they can be replaced.
- Key mappings (`Keymap`) for normal, insert, and visual mode: map keys to keys or to a
  `:` command, or disable a key. Mapped keys are not re-mapped. Prefix waiting with timeout.
- Config file `config.toml` (`$VOX_CONFIG`, `$XDG_CONFIG_HOME/vox/`, `~/.config/vox/`):
  `leader`, `timeout_ms`, `[keymap.normal|insert|visual]`. Errors are shown on start-up;
  valid settings still apply.
- `Config` / `Value` in core (format-neutral); `vox-config-file` adapter parses TOML.
- Public `Editor` API: `commands`, `commands_mut`, `keymap_mut`, `apply_config`,
  `set_message`, `quit`, `has_pending_keys`, `key_timeout`, `timeout`.
- Tests for commands, mappings, config validation, and TOML parsing.
- Docs: Configuration and Developer API pages; 3 new code-tour walkthroughs.

### Changed
- `vox-term` run loop waits at most `timeout_ms` while a mapping is incomplete.
- Editor events (planned for M3) moved to M4, where plugins will use them.

## [0.4.0] - 2026-11-26 — M2.5 dependency isolation

No user-visible behaviour change.

### Added
- `vox_core::storage::TextStorage` trait, `SimpleStorage` (std), `storage::conformance` checks.
- `vox_view::CharWidth` trait and `Monospace` fallback.
- `vox-editor` crate: editor logic with own `Key`/`KeyCode`/`Modifiers`, `Surface` screen
  model, and `Services` injection (storage, char width, clipboard).
- Adapter crates, named by purpose: `vox-storage` (text storage, uses ropey), `vox-width`
  (char cell widths, uses unicode-width), `vox-clipboard` (system clipboard, uses arboard),
  `vox-term` (terminal I/O with row-diff drawing, uses crossterm).
- `scripts/check-deps.sh` (in CI): core crates must depend only on vox crates.
- Docs: "Dependencies and adapters" page; architecture updated.
- Docs: interactive **Code tour** (crate map, function explorer with call graph, 8 step-by-step
  walkthroughs), generated from source by `scripts/gen-code-tour.py`; API reference with
  private items under `api/` via `scripts/build-docs.sh`. CI checks the tour data is current.
- `///` descriptions on all internal functions.

### Changed
- `Document::new`/`open` take a `StorageFactory`; `Document::text()` replaced by
  `char`, `len_chars`, and `Display`.
- `vox-tui` split: logic → `vox-editor` (`App` renamed `Editor`), terminal → `vox-term`.
- Text storage now uses LF-only line breaks (lone `\r`, U+2028 etc. no longer split lines).

### Removed
- `ratatui` and `anyhow` dependencies.

## [0.3.0] - 2026-11-26 — M2 Vim grammar

### Added
- Vim grammar parser: `["x][count] operator [count] motion`, counts multiply.
- Operators `d`, `c`, `y` with any motion; `dd`, `cc`, `yy`; `x` with count.
- Motions `e`, `^`, `n`, `N`; counts on all motions; `NG` / `Ngg` go to line N.
- `p` / `P` (charwise and linewise, with count).
- `I`, `A` insert entry.
- Visual mode: `v` (charwise), `V` (linewise), selection highlight, `d`/`x`/`c`/`y`.
- Search: `/text`, `/` repeats, `n`/`N`, wraparound with messages, `E486`.
- Registers: unnamed + system clipboard (`"+`, `"*`) via `arboard` (X11 and Wayland).
- `Clipboard` trait and `App::set_clipboard` for tests/embedding.
- Vim rules: `dw` on last word stops at line end; `cw` acts like `ce`; `yw` on last word of the file includes it.
- Tests: Vim golden tests (`tests/vim.rs`), parser tests, search/motion tests, selection render tests.
- Docs: grammar, motions, operators, registers, visual mode, search.

### Changed
- `u` and `Ctrl-r` accept a count.
- `Esc` in normal mode cancels a pending command.

## [0.2.0] - 2026-11-26 — M1 core editing

### Added
- Cursor with Normal and Insert modes; bar cursor in Insert mode.
- Motions: `h j k l`, arrows, `w`, `b`, `0`/`Home`, `$`/`End`, `gg`, `G`; `j`/`k` keep the wanted column.
- Insert entry: `i`, `a`, `o`, `O`. Insert mode: text, `Enter`, `Backspace` (joins lines), `Tab`, arrows, `Esc`.
- `x` deletes the char under the cursor.
- Linear undo/redo (`u`, `Ctrl-r`); one insert session = one undo step.
- Commands: `:w [file]`, `:wq`, `:x`, `:e [file]`, `:e!`, `:q!`.
- Modified flag (`[+]` in statusline); `:q` and `:e` refuse with `E37` when modified.
- Line ending (`\n`/`\r\n`) and final-newline preservation on save.
- Horizontal scrolling; tabs render to 4-column tab stops; wide chars supported.
- `parse_keys`: Vim key notation parser (`"ihi<Esc>"`, `<C-r>`).
- Tests: document/undo/motion unit tests, key-sequence tests, more render snapshots.
- Docs: keybindings, commands, architecture, testing pages updated.

### Changed
- `j`/`k`, `Ctrl-d`/`Ctrl-u`, `Ctrl-f`/`Ctrl-b`, `PageUp`/`PageDown` move the cursor; the viewport follows.
- Statusline shows `line:col` instead of `top/total`.
- `Document::line_count` no longer special-cases a trailing newline (it is stored outside the rope).

## [0.1.0] - 2026-11-26 — M0 skeleton

### Added
- Cargo workspace with `vox-core`, `vox-view`, `vox-tui`, `vox` crates.
- `vox-core`: rope-backed `Document` (open file, missing file = empty buffer, line access).
- `vox-view`: `Viewport` with clamped scrolling.
- `vox-tui`: line-number gutter, `~` filler rows, statusline (mode, file, position), command line.
- Normal-mode scrolling: `j`/`k`, arrows, `Ctrl-d`/`Ctrl-u`, `Ctrl-f`/`Ctrl-b`, `PageUp`/`PageDown`, `gg`, `G`.
- Command line: `:q`, `:q!`, `:quit`, `:qa`; unknown commands show `E492`.
- Unit tests and `insta` render snapshot tests.
- Docs: README, ARCHITECTURE, CONTRIBUTING, mdBook scaffold.
- CI: fmt, clippy, test, docs build.
