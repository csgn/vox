# vox

Vim-like modal terminal editor written in Rust. Planned: built-in Claude integration (via the `claude` CLI) and Rust plugins compiled to WASM.

Status: **M1 — core editing** (v0.2.0). Cursor, insert/normal mode, basic motions, undo/redo, save.

## Build

```sh
cargo build --release
./target/release/vox path/to/file
```

## Keys

Normal: `h j k l` `w b` `0 $` `gg G` `Ctrl-d/u/f/b` · `i a o O` insert · `x` delete char · `u` / `Ctrl-r` undo/redo · `:` command line.
Insert: type text, `Enter`, `Backspace`, `Tab`, arrows, `Esc` back to normal.
Commands: `:w [file]` `:q` `:q!` `:wq` `:x` `:e [file]` `:e!`.

Full list: `docs/src/user/keybindings.md`.

## Docs

- User and developer guide: `docs/` (mdBook: `mdbook serve docs`)
- [ARCHITECTURE.md](ARCHITECTURE.md), [CONTRIBUTING.md](CONTRIBUTING.md), [CHANGELOG.md](CHANGELOG.md)
