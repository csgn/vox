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
  Each `Command` also carries metadata: category, `Param`s (with a `Complete` source),
  returns, force, examples. `help.rs` turns it into the `:help` dialog (`HelpView`) and data
  (`:commands info`); `:?TOPIC` is rewritten to `:help TOPIC` in `exec_command`.
- `buffer.rs`: open files. The active buffer is the editor's own `doc` / `cursor` / `view`;
  the others are parked (`Buffer`) and swapped in by `switch_buffer`, so the rest of the
  editor only ever sees one document. `open_buffer`, `add_buffer`, `close_buffer`, `buffers()`.
- `structure.rs`: `Structure` trait (units around a line, innermost first) and the
  `Heuristic` implementation (braces, indentation, keywords). `focus.rs`: the focused `Unit`
  on `Editor` (`unit_focus`), `:narrow` / `:widen`, `follow_focus` keeps it on its text after
  edits (called from `with_change_events`); `render.rs` dims lines outside it.
- `agent.rs`: `EditorMode { Text, Agent }`, `:mode`; leaving agent mode removes owner
  `agent`'s commands and mappings.
- `assist.rs`: the `Assistant` trait (`ask(&AssistRequest) -> AssistReply`, run in a
  background job), `diff` (line LCS → `Hunk`s), the pending `Proposal`, `:ai` commands and
  keys (owner `agent`, registered by `register_agent` when agent mode starts). `render.rs`
  `text_rows` interleaves removed (`-`) and added (`+`) lines with the buffer's lines.
- `complete.rs`: `Editor::complete(line) -> Completion` (pure): command names, subcommands,
  then the current parameter's `Complete` source (files via `std::fs`, plugins, setting
  keys, values). `prompt.rs`: the `:`/`/` prompt's keys — `Tab` cycling, the menu, history —
  with state in `PromptState`; `render.rs` draws it per `[ui] cmdline` / `searchline`.
- `Config::from_value(&Value)`: validates settings from a format-neutral `Value` tree (the TOML
  file is parsed by the `vox-config-file` adapter); `Editor::apply_config` installs them.
- `Editor::exec(Cmd)`: operators turn a motion into a range (exclusive / inclusive /
  linewise), write the register, then delete / change / yank.
- `Surface`: cells (`symbol`, `Style { dim, reversed }`), cursor, cursor shape.
  `Editor::render()` fills it. `Display` prints rows for snapshot tests.

## UI layer (`vox-editor/src/ui/`)
- `layout::compute(size, &UiConfig, side, bottom, modal) -> Layout`: text area, side panel,
  bottom panel, modal, and the area notifications may use.
- `View` trait + `ListView`, `TextView`, `Confirm`, `Input`, `TreeView` (drill-down columns
  with breadcrumb and filter; `help::command_tree` feeds it for `:commands`). Views return `Outcome`s that name
  command lines; the editor runs them.
- Key routing in `Editor::handle_key`: modal first, then a focused panel (`<C-w>` and `:`
  still work), then mappings and the Vim grammar.
- Notifications expire through `next_deadline()` / `tick()` with an injected clock.

## Owners, log, developer mode
- `owner.rs`: owner names. `Commands` and `Keymap` keep per-name stacks with owners;
  `Editor::with_owner` / `remove_owned` add and remove by owner. `apply_config` replaces the
  `config` owner's mappings, so it also reloads.
- `log.rs`: `Log` ring buffer (2000 entries) and the live `LogView`; views expose
  `as_log_view()` so the editor can refresh them.
- `:reload config` is registered by `vox-config-file::register_commands`; `:reload plugins`
  by `vox-plugin-host::PluginHost::register_commands` (the adapters own the file and plugin
  access). The plugin host runs every plugin call under the owner `plugin:NAME`.

## Scopes
- `scope.rs` (core, std only): `Scope { kind, dir }` with `config_path`, `plugin_dir`,
  `lock_path`; `find_project_root` (upward search for `.vox/`); `list_plugins`;
  `setup_local(global, local, choice)` copies without overwriting.
- The binary (`crates/vox/src/cli.rs`, `main.rs`) parses `--local-config [DIR]` /
  `--vanilla`, builds the active scope, and loads config and plugins only from its paths.
  The editor only stores the scope (for `:scope`, the badge, and the set-up dialog).
- `vox run …` (`cli::Run`, `main::run_commands`) builds the same editor without a terminal:
  `set_interactive(false)`, no clipboard, no update check. Each command goes through
  `Editor::execute`; the loop calls `tick` until `jobs_running() == 0`, prints the reply
  (stdout) and new `history()` entries (stderr), and stops at the first error.

## Plugin packages (`vox-editor/src/plugin/`)
- `version.rs`: `Version`, `VersionReq` (`>=`, `^`, ranges).
- `manifest.rs`: `PluginManifest::from_value` (from the neutral `Value`), requirements,
  typed settings, `resolve_settings`.
- `health.rs`: `check` (API, vox, plugins, programs) with levels; `program_on_path`.
- `package.rs`: `discover` (folders + single `.wasm`), `assess` (repeats until plugin-on-plugin
  requirements are stable), `report` for `:checkhealth`.
- `template.rs`: the standard project for `:plugin new`.
- `lock.rs`: `Lockfile` (`vox.lock`) as a value tree; read/write via injected parser and
  formatter.
- `install.rs`: `install` (path or git; package or project built with `cargo`),
  `install_locked`, `uninstall`, `check_lock`; uses `git` / `cargo` through std `process`.
- `sha256.rs`: checksums, std only.
- `manager.rs`: `ManagerView`, the plugin manager list; refreshes from `Editor::plugin_infos`
  and `vox.lock` through `View::refresh` after every command and finished job.
- `manage.rs`: `:plugins`, `:plugin add|remove|enable|disable|files`, `:plugins sync`; installs run as editor jobs
  (`Editor::spawn_job`: a thread plus a channel drained in `tick`; `JobStatus` per running
  job, drawn with the notifications).
- The host takes a `ValueParser` (TOML from `vox-config-file`) so core never parses TOML.

## Settings (`vox-editor/src/settings/`)
- `mod.rs`: vox settings schema, `Config::to_value`, `ConfigEdit`, `apply_edits`.
- `dialog.rs`: `:settings` commands, forms, the key-mapping list.
- `commands.rs`: `:settings list|get|set|unset` (dotted keys, no dialog).
- Saving goes through `Editor::save_config_edits` → injected `ConfigWriter` → `apply_config`.

## Updates (`vox-editor/src/update.rs`)
- `UpdateSource { repo, current, install_root }` is set by the binary; `install_root` comes
  from the running binary's path (`ROOT/bin/vox`), `None` for builds that must not be replaced.
- `:update check` runs `git ls-remote --tags` as a job and offers the newest `vX.Y.Z` tag in
  a `Confirm`; yes runs `:update install VERSION` (`cargo install --git --tag --root`).
- `check_on_start` does the same quietly at start-up unless `check_updates = false`.

## vox-term
- `Terminal::enter()` (raw mode, alternate screen, panic-safe restore), `draw(&Surface)` with
  per-row diffing, `read_key()`, and `run(&mut Editor)`.
