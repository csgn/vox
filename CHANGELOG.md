# Changelog

All notable changes to this project are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [SemVer](https://semver.org/).

## [Unreleased]

## [0.16.2] - 2026-11-27 — test release

No changes. Released to test the updater (`:update install`, progress box).

## [0.16.1] - 2026-11-27 — job progress

### Added
- Running jobs (plugin install, sync, vox update) show a box with a spinner, the time so
  far, the title, and the latest step until they finish (`Editor::job_status`, `JobStatus`).
- `:update install` reports cargo's progress (`compiling (N): CRATE`) as it builds.
- `vox run` prints job progress to stderr (`vox: progress: …`).

### Changed
- Job progress no longer pops up as notifications that disappear; the "started" message is
  only in `:messages`.

## [0.16.0] - 2026-11-27 — M14 vox run

### Added
- `vox run [--local-config [DIR] | --vanilla] [--json] [--file FILE] COMMAND... | -`: run
  editor commands without a terminal. Waits for background jobs; replies go to stdout
  (text, or one JSON line each with `--json`), messages to stderr; the first failure stops
  with exit status 1. `-` reads command lines from stdin. `vox run --help`.
- `Value::to_json`.

### Changed
- Bad command-line arguments exit with status 2 (was 1).

## [0.15.0] - 2026-11-27 — M13 updater

### Added
- Updater: at start-up vox checks the newest `vX.Y.Z` tag of its repository in the
  background (`git ls-remote`) and asks whether to install a newer release; yes runs
  `cargo install --locked --force --git … --tag vX.Y.Z --root ROOT vox` as a job, into the
  running binary's install root. Builds not installed with cargo are never replaced.
- `:update check`, `:update install [VERSION]`.
- `check_updates = false` turns the start-up check off (also in `:settings`); `--vanilla`
  never checks.
- `Editor::spawn_quiet_job`, `Editor::set_update_source`, `vox_editor::update`.

## [0.14.0] - 2026-11-27 — M12 command API

### Added
- M12 command API: `Editor::execute(line)` runs a command and returns its reply or error;
  `Editor::reply(Value)` returns data from a command; `run_command` shows a reply in the
  message line or a modal. `Value::table`, `Value::to_text`, `From<&str | String | i64 | bool>`.
- Non-interactive mode: `Editor::set_interactive(false)`. Commands that open a view fail
  with "needs the editor UI"; `:plugin add` without a source and `:plugin remove` without `!`
  fail instead of asking.
- Data commands: `:commands list`, `:scope show`, `:plugin list`, `:plugin info NAME`,
  `:settings list`, `:settings get KEY`, `:settings set KEY VALUE`, `:settings unset KEY`,
  `:settings key list`. Keys are dotted config paths (`ui.side_width`, `plugins.NAME.KEY`).
- `install.sh`: `curl -fsSL …/install.sh | sh` builds and installs the latest tagged release
  with `cargo install` (`VOX_VERSION`, `VOX_REPO`, `VOX_INSTALL_ROOT`).

## [0.13.0] - 2026-11-27 — M11 plugin manager

### Added
- `:plugins` / `<leader>p`: plugin manager for the active scope. Lists status, version,
  source (from `vox.lock`), locked-but-missing plugins; details show description, first
  problem, and what the plugin adds. Keys: `a` add, `e` settings, `E` files, `Space` on/off,
  `D` delete, `r` reload, `h` health, `s` sync, `c` copy from global (project mode), `?` help.
  Dialogs go back to it. Vanilla mode explains that plugins are off.
- `[ui] plugin_manager = "modal" | "side"` (also in `:settings`).
- Commands: `:plugin enable NAME`, `:plugin disable NAME` (write `[plugins.NAME] enabled`),
  `:plugin files NAME`, `:plugins help`; `:plugin add` without an argument asks for the
  source.
- `View::refresh`, `Editor::refresh_views`: open views update after commands and jobs.
- `Editor::plugin_infos` / `set_plugin_infos` (kept up to date by the plugin host).
- Default mappings owned by `defaults`, re-made when the leader changes.

### Changed
- `:plugin remove NAME` asks first; `:plugin remove! NAME` does not.
- A `!` after a subcommand forces it (`:plugin remove! x`, same as `:plugin! remove x`).
- Saving settings drops a table the edit left empty (e.g. `[plugins.NAME]` after turning the
  plugin back on).

## [0.12.0] - 2026-11-27 — M10 lockfile and installer

### Added
- `:plugin add PATH` and `:plugin add GIT_URL[@REF]`: install into the active scope from a
  packaged plugin or a plugin project (built with `cargo` for `wasm32-wasip2`); git sources
  are cloned and the exact commit is recorded. Runs in the background with progress
  notifications; the plugin loads when ready.
- `:plugin remove NAME`: uninstall and unload.
- `vox.lock` per scope (`version = 1`, `[[plugin]]` with name, version, source path or git
  url/ref/rev, `sha256:` checksum of `plugin.wasm`). Path sources near the project are stored
  relative to it.
- `:plugins sync`: install what the lock lists (missing or changed plugins); start-up notice
  when installed plugins differ from the lock.
- Project set-up writes `.vox/.gitignore` (installed plugins and build files) and copies the
  global lock entries of copied plugins.
- Background jobs: `Editor::spawn_job`, `jobs_running`; run loop wakes up while jobs run.
- Core: `plugin::lock`, `plugin::install`, `plugin::sha256` (std only), `plugin::manage`;
  `ValueFormatter`; `vox-config-file::format`.
- Docs: installing and the lockfile in Plugins and Project setups; code-tour walkthroughs.

### Changed
- `:reload plugins NAME` loads a plugin that is installed but not yet known.

## [0.11.0] - 2026-11-27 — M9 settings dialog

### Added
- `:settings` dialog: vox settings, key mappings, and each plugin's settings. Changes are
  saved to the active scope's `config.toml` and applied immediately; vanilla mode applies
  for the session only.
- Saving keeps the file's comments, order, and formatting (`vox-config-file::write_edits`
  with `toml_edit`), writes atomically, and never writes invalid values.
- `Form` view: typed fields, inline editing, validation messages, reset to default, unsaved
  changes warning, `*` on changed fields.
- Key-mapping list: add / change / delete your config's mappings; other owners' mappings are
  read-only. Commands `:settings key add|delete`.
- `:settings vox`, `:settings keys`, `:settings plugin NAME`, `:settings NAME`.
- Core: `settings::vox_settings` schema, `Config::to_value`, `ConfigEdit`, `apply_edits`;
  `Editor::save_config_edits`, `config`, `set_config_writer`, `plugin_settings`;
  `Outcome::Submit`; `Input::returning` / `Confirm::returning`.
- Docs: Settings dialog page; code-tour walkthrough.

### Fixed
- A plugin built for an older plugin API (e.g. an old single `text_tools.wasm`) failed with
  a cryptic link error; vox now reads the API version from the plugin's imports and reports
  `built for plugin API 0.1, vox provides 0.2: rebuild ...` in `:checkhealth`.
- Key mappings on uppercase letters and shifted symbols (e.g. `Q`, `?`) did not trigger in a
  real terminal: the terminal adapter now drops the Shift flag for char keys.

## [0.10.0] - 2026-11-27 — M8 plugin packages and health

### Added
- Standard plugin project layout and installed package layout (`NAME/{plugin.toml,
  plugin.wasm, README.md}`); single `.wasm` files still load.
- `plugin.toml` manifest: name, version, description, authors, `api`, `vox`; `[[requires]]`
  (plugin + version, vox version, program on PATH) with level error / warning / info;
  typed `[settings.KEY]` (string, integer with min/max, bool, choice).
- Health checks before loading (package, API version, requirements; plugin-on-plugin
  requirements re-checked until stable) and the plugin's own `health()` after `init`;
  `error` = not loaded. Statuses: ok, info, warning, failed, disabled.
- `:checkhealth [NAME]` report; start-up notifications for failed / warning plugins.
- Plugin settings from `[plugins.NAME]` in `config.toml`, checked against the manifest
  (bad values: warning + default); `enabled = false` turns a plugin off.
- Plugin API 0.2 (`wit/vox.wit`): `setting(key)`, optional `health()` export.
- SDK: `Plugin` trait with optional `on_event` / `health`; `SettingValue::as_text` /
  `as_integer` / `as_bool`.
- `:plugin new NAME [DIR]` (template project) and `scripts/build-plugin.sh DIR [--install
  DEST]`.
- Core `vox_editor::plugin`: `Version`, `VersionReq`, `PluginManifest`, health, package
  discovery and assessment, template (std only).
- `plugins/text-tools` 0.2.0: manifest, settings `case` and `count_on_save`.

- Subcommands: a command name with a space (`"reload plugins"`) is a subcommand of its root;
  `:ROOT` alone lists the subcommands when there is no root command.

### Changed
- Related commands are subcommands now: `:log clear`, `:log refresh`, `:reload config`,
  `:reload plugins [NAME]`, `:scope setup`, `:scope init`, `:scope pick` (were `:log-clear`,
  `:log-refresh`, `:reload-config`, `:reload-plugins`, `:scope-init`, `:scope-pick`).
- Plugin API 0.1 → 0.2: plugins must be rebuilt with the current SDK (vox reports the
  mismatch in `:checkhealth`).
- `PluginHost::new` takes a `ValueParser`; `load_dir` / `reload_all` return `PluginInfo`s.
- Owner and log source of a packaged plugin is `plugin:NAME` with the folder name.

## [0.9.0] - 2026-11-26 — M7 project setups

### Added
- Scopes: global (default, `~/.config/vox`), local (`--local-config [DIR]`, the project's
  `.vox/` only; global config and plugins are not loaded), vanilla (`--vanilla`: no config,
  no plugins). Exactly one is active; status line shows `LOCAL` / `VANILLA`.
- Project root: `DIR`, else the nearest folder upward containing `.vox/`, else the current
  folder.
- Project set-up dialog on first local start (and `:scope setup`): start empty, copy global
  config, choose what to copy (checklist), copy everything, not now. Copies never overwrite.
- Commands: `:scope`, `:scope setup`, `:scope-pick`, `:scope-init`.
- `CheckList` view (multi-select).
- Nested dialogs: a modal opened from a modal keeps the first one underneath; `Esc`/`q` goes
  back one step (`Esc: back` hint on the border), a final choice closes the chain
  (`Editor::modal_depth`).
- Command line: `--help`, `--version`, errors for unknown options and conflicting modes.
- Core `vox_editor::scope` (std only): `Scope`, `find_project_root`, `list_plugins`,
  `setup_local`.
- Docs: Project setups page; code-tour walkthrough.

### Changed
- The global folder comes from `$VOX_CONFIG_DIR` (a folder). **`$VOX_CONFIG` (a file path)
  is no longer read.**
- `vox-config-file`: `global_dir()` replaces `default_path()` / `plugin_dir()`.

## [0.8.0] - 2026-11-26 — M6 logging and developer mode

### Added
- Log: `Editor::log(level, source, text)`, 2000-entry ring buffer, `Level::Debug`.
  Logged: commands run (debug), command errors, config load and warnings, plugin load /
  failure / disable, plugin log lines.
- `:log [LEVEL] [SOURCE]`: live log view in the bottom panel (`f` level, `s` source,
  `c` clear); `:log-clear`, `:log-refresh`.
- Developer mode (`developer_mode = true` or `:devmode`): `DEV` badge, `<leader>dl` log,
  `<leader>dr` reload plugins, `<leader>dc` reload config.
- `:reload-config` (vox-config-file) and `:reload-plugins [NAME]` (vox-plugin-host);
  reloading revives disabled plugins.
- Ownership: commands, aliases, mappings, and subscriptions record an owner (`builtin`,
  `config`, `devmode`, `plugin:NAME`, `user`); `Editor::with_owner`, `remove_owned`.
  A newer owner hides an older registration; removing it restores the older one.
- Plugin API (`wit/vox.wit`): `notify(level, text)`, `log(level, text)`, `level` enum.
- `:keymaps` shows each mapping's owner.
- Docs: Developer mode page; owners and log in Developer API; plugin development loop;
  code-tour walkthroughs.

### Changed
- `Editor::apply_config` replaces earlier config mappings (so it doubles as reload).
- Config errors at start-up are notifications (and log lines) instead of the message line.
- `Keymap::iter` yields the owner too.

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
