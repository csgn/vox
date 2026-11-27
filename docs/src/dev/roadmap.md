# Roadmap

vox is not a Neovim clone. The goal is a small editor that does a few things well:
Vim-style editing, a simple developer API, and Rust/WASM plugins that are easy to manage.
Everything else waits until those work.

Built one milestone at a time; each ships with tests, docs, and a changelog entry.

| Milestone | Version | Scope | Status |
|---|---|---|---|
| M0 | 0.1.0 | Skeleton: open, render, scroll, `:q` | done |
| M1 | 0.2.0 | Cursor, insert/normal, `hjkl w b 0 $ gg G`, `i a o O x`, undo/redo, `:w :wq :e` | done |
| M2 | 0.3.0 | `d c y` × motions, `dd cc yy p P`, counts, visual (char/line), `/ n N`, clipboard | done |
| M2.5 | 0.4.0 | Dependency isolation: std-only core, adapter crates; code tour | done |
| M3 | 0.5.0 | Developer API (commands, keymaps), config file, configurable keys | done |
| M4 | 0.6.0 | WASM plugins: WIT contract, sandboxed host, Rust SDK, example plugin, events | done |
| M5 | 0.7.0 | UI layer: side/bottom panels, modal, notifications, focus; configurable in `[ui]` | done |
| M6 | 0.8.0 | Logging and developer mode: log panel, reload config/plugins shortcuts | done |
| M7 | 0.9.0 | Scopes and start-up modes: global (default), `--local-config [DIR]` (project's `.vox/` only), `--vanilla` (no config, no plugins); set-up dialog to copy global config / chosen plugins into a project | done |
| M8 | 0.10.0 | Plugin packages: standard project structure, `plugin.toml` manifest (with typed settings), health checks (error / warning / info) for plugins, vox version, programs, custom checks | done |
| M9 | 0.11.0 | Settings editor: `:settings` dialog to edit vox settings, key mappings, and each plugin's settings; saves to the active scope's `config.toml`, keeping comments | done |
| M10 | 0.12.0 | Lockfile and installer: `vox.lock` per scope (version, path or git URL + commit, checksum), install from path or git, `:plugins sync` | done |
| M11 | 0.13.0 | Plugin manager (modal or side panel), scope-aware: list, add (installs into the active scope only), edit settings / open files, enable/disable, delete, reload, health, sync | done |
| M12 | 0.14.0 | Command API: `Editor::execute` returns a result, `Editor::reply` lets commands return data (`Value`); non-interactive mode (missing arguments and confirmations are errors, views are refused); data commands: `plugin list`, `plugin info`, `settings list/get/set/unset`, `settings key list`, `commands list`, `scope show` | done |
| M13 | 0.15.0 | Updater: at start-up check the newest `vX.Y.Z` tag of vox's repository (`git ls-remote`, background job), ask to install it (`cargo install --git --tag`); `:update check`, `:update install [VERSION]`, `check_updates = false` | done |
| M14 | 0.16.0 | Headless CLI: `vox run [--local-config [DIR] \| --vanilla] [--json] [--file FILE] CMD... \| -` runs commands without a terminal, waits for background jobs, prints replies as text or JSON (`Value::to_json`), messages to stderr; exit status 0 / 1 (a command failed) / 2 (bad arguments) | done |
| M15 | 0.17.0 | Command metadata and help: commands declare category, parameters, result, examples; `:help CMD` / `:?CMD` dialog; `help` in `vox run` | done |
| M16 | 0.18.0 | Command browser: `:commands` as a tree (category → command → subcommand) with sub-panels, breadcrumb path, filter, run, help | done |
| M17 | 0.19.0 | Prompt and completion: `:` and `/` prompt in the status line, a floating box, or a modal (configurable); completion menu while typing, `Tab` completes; argument hints; prompt history | done |
| M18 | 0.20.0 | Buffers: several open files (`:e`, `:ls`, `:b`, `:bn`/`:bp`, `:bd`, `:wa`), `vox a b`; `:q` checks every buffer | done |
| M19 | 0.21.0 | Editor modes and focus: `text` / `agent` mode (`:mode`, key, `vox --agent`, `editor_mode`); focus on one unit (selection, block, function, class, file), the rest dimmed | done |
| M20 | 0.22.0 | Asking the AI: `<leader>a` / `:ai` dialog on the focused unit; the reply shown as an inline diff to accept or decline (or applied directly, per scope); backend trait with a test backend | done |
| M21 | 0.23.0 | Claude backend: an adapter implementing the assistant trait with the Claude API | planned |

## Scopes

| Scope | Start with | Config | Plugins | Lockfile |
|---|---|---|---|---|
| global | `vox` | `~/.config/vox/config.toml` | `~/.config/vox/plugins/` | `~/.config/vox/vox.lock` |
| local | `vox --local-config [DIR]` | `<project>/.vox/config.toml` | `<project>/.vox/plugins/` | `<project>/.vox/vox.lock` |
| vanilla | `vox --vanilla` | none | none | none |

Exactly one scope is active. Without `DIR`, the project is the nearest folder upward that
contains `.vox/` (like git).

## Next: command UX (M15–M17)

Before Claude integration, commands must be easy to find, type, and understand. Three
milestones, in this order, because the browser and completion both need the metadata.

### M15 — command metadata and help

- `Command` gains optional metadata, all with defaults so existing registrations keep
  working:
  - `category`: where the command is shown (`File`, `Editing`, `Search`, `View`, `Plugins`,
    `Settings`, `Developer`, `Help`). Plugin commands default to category `Plugin: NAME`;
    user/config commands to `User`.
  - `params`: name, description, required or optional, repeated or not, and a
    *completion source*: `file`, `dir`, `command`, `plugin`, `setting-key`, `version`,
    one of fixed values, or none. M17 uses the sources.
  - `returns`: what the reply holds (for data commands), e.g. "table: name, version, …".
  - `examples`: command lines with a short note.
  - `usage` is generated from params: `plugin add SOURCE [--global]`.
- Registration through a builder (`register(...).category(..).param(..)`); the plain
  `register(name, description, handler)` stays.
- Plugins: a new WIT function to describe a command (category, params, returns,
  examples) after `register-command`; the SDK wraps it. Old plugins still load.
- `:help CMD` (alias `:?CMD`, typed in the `:` prompt, e.g. `:?plugin add`) opens a help
  dialog: breadcrumb path (`Commands › plugin › add`), description, usage, parameters table,
  result, examples, aliases, owner, and the subcommands as a list — `Enter` opens a
  subcommand's help, `Backspace`/`h` goes back to the parent (hint shown). A key runs the
  command (asking for missing arguments).
- `:help` with no argument opens M16's browser once it exists (until then, the list).
- `:commands list` and `vox run help CMD` return the same metadata as data (`--json`).
- All built-in commands get metadata; a test fails if a built-in has none.
- Normal-mode `?` stays free for Vim's backward search; help is reached from the prompt.

### M16 — command browser

- `:commands` opens a tree instead of one flat list: **categories → root commands →
  subcommands**. Each level is a sub-panel (columns side by side when wide, one at a time
  when narrow) with a breadcrumb at the top: `Commands › Plugins › plugin › add`.
- Keys: `j`/`k` move, `l`/`Enter` go in (or run a leaf), `h`/`Backspace` go back to the
  parent, `?` opens the M15 help for the selection, `/` filters across all levels (shows
  matches with their full path), `q`/`Esc` close. Hints in the footer.
- The detail area shows the selection's description and usage.
- Opens as a modal or in the side panel: `[ui] commands = "modal" | "side"` (like
  `plugin_manager`).
- A reusable tree/drill-down view in `ui/`, so `:keymaps`, `:settings`, and the plugin
  manager can use it later.

### M17 — prompt and completion

- Prompt placement, configurable separately for commands and search:
  `[ui] cmdline = "statusline" | "float" | "modal"` and `[ui] searchline = …` (default
  `"statusline"`, as today). `float` is a one-line box centred near the top; `modal` is a
  box with the input and the completion list. Also in `:settings`.
- Completion menu while typing (`[ui] completion = "auto" | "tab" | "off"`):
  - first word: commands and aliases; after a root: its subcommands; then arguments from
    the parameter's completion source (files, plugins, setting keys, …);
  - each entry shows its description; the selected one's usage is shown as a hint
    (`plugin add SOURCE [--global]`, current parameter highlighted);
  - prefix matches first, then fuzzy (subsequence) matches.
- Keys: `Tab` completes the selection (or the common prefix), `Shift-Tab` goes back,
  `Up`/`Down` (or `<C-n>`/`<C-p>`) move in the menu, `Enter` runs, `Esc` leaves the prompt
  (with an entry selected, it first goes back to what was typed). With the menu closed, `Up`/`Down` walk the prompt history
  (kept per session).
- `/` gets history only (no completion).
- Completion is a pure function of the editor state (`Editor::complete(line, cursor)`),
  tested without a terminal and reusable by `vox run` / Claude later.

## Next: editor modes (M18–M21)

vox gets two editor modes. **text** is the Vim-style editor with no AI anywhere. **agent** is
the same editor plus inline AI help, in the style of Colab: while coding, press a key, a small
dialog asks what to do, the AI's change appears in the buffer as a diff, and you accept or
decline it. There is no chat panel: the editor stays the UI.

### M18 — buffers
`Editor.doc` stays the active document; other open files are kept as buffers (document,
cursor, view) and swapped in on switch. `:e FILE` opens or switches; `:ls` / `:buffers`,
`:buffer N|NAME`, `:bnext`, `:bprevious`, `:bdelete[!]`, `:wall`, `:buffer list` (data).
`:q` refuses while any buffer has unsaved changes. The status line shows `[N/M]`.

### M19 — editor modes and focus
- Mode: `:mode text|agent|toggle`, `<leader>m`, `vox --agent`, `editor_mode` per scope (also
  in `:settings`). Agent-only commands and keys have their own owner and are removed in text
  mode. The status line shows `AGENT`.
- Focus: you (and later the AI) work on one unit — selection, block, function, class, or file.
  `<leader>f` focuses the smallest unit at the cursor and widens on repeat, `<leader>F`
  narrows, `:narrow KIND`, `:widen`. Everything outside is dimmed. Units come from a core
  `Structure` trait with a heuristic (braces, indentation, keywords like `fn`/`def`/`class`);
  tree-sitter can replace it later.

### M20 — asking the AI and reviewing its change
- `<leader>a` / `:ai INSTRUCTION` opens a small dialog; the request carries the focused unit
  (else the selection, else the line) and the file for context. The reply may only change
  that unit.
- The reply is shown inline: removed lines `-` and added lines `+` in place. `ga` / `gx`
  accept / decline a hunk, `gA` / `gX` all, `]c` / `[c` move between hunks. Accepting is one
  undo step. `ai_apply = "review" | "direct"` per scope. Editing the text while a change is
  pending drops it (simpler and safer than locking the lines).
- A core `Assistant` trait (std-only) runs as a background job; tests use a fake backend.

### M21 — Claude backend
An adapter crate implements `Assistant` with the Claude API. Planned in detail after M20.

## Later (only if needed)
MCP server on top of `vox run` (other agents driving vox), remote control of a running vox (`vox run --attach` over a Unix socket, buffer commands; only if Claude needs the open buffer), plugin registry, plugin-defined panels, splits, fuzzy finder,
tree-sitter highlighting, themes, LSP, git signs, macros, marks, named registers.
