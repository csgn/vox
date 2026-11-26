# Roadmap

vox is not a Neovim clone. The goal is a small editor that does four things well first:
Vim-style editing, a simple developer API, Rust/WASM plugins, and built-in Claude.
Everything else waits until those work.

Built one milestone at a time; each ships with tests, docs, and a changelog entry.

| Milestone | Version | Scope | Status |
|---|---|---|---|
| M0 | 0.1.0 | Skeleton: open, render, scroll, `:q` | done |
| M1 | 0.2.0 | Cursor, insert/normal, `hjkl w b 0 $ gg G`, `i a o O x`, undo/redo, `:w :wq :e` | done |
| M2 | 0.3.0 | `d c y` × motions, `dd cc yy p P`, counts, visual (char/line), `/ n N`, clipboard | |
| M3 | 0.4.0 | Developer API (commands, keymaps, events), TOML config, configurable keys | |
| M4 | 0.5.0 | WASM plugins: WIT contract, host, Rust SDK, example plugin | |
| M5 | 0.6.0 | Claude via `claude` CLI: chat pane, selection/buffer context, confirmed edits | |

## Later (only if needed)
Splits, buffer list, fuzzy finder, tree-sitter highlighting, themes, LSP, git signs,
macros, marks, named registers, plugin hot reload, diff review view.
