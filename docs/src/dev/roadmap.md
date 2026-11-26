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

## Scopes

| Scope | Start with | Config | Plugins | Lockfile |
|---|---|---|---|---|
| global | `vox` | `~/.config/vox/config.toml` | `~/.config/vox/plugins/` | `~/.config/vox/vox.lock` |
| local | `vox --local-config [DIR]` | `<project>/.vox/config.toml` | `<project>/.vox/plugins/` | `<project>/.vox/vox.lock` |
| vanilla | `vox --vanilla` | none | none | none |

Exactly one scope is active. Without `DIR`, the project is the nearest folder upward that
contains `.vox/` (like git).

## Later (only if needed)
Claude integration (postponed), plugin registry, plugin-defined panels, splits, fuzzy finder,
tree-sitter highlighting, themes, LSP, git signs, macros, marks, named registers.
