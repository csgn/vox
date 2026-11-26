# vox

Vim-like modal terminal editor written in Rust. Planned: built-in Claude integration (via the `claude` CLI) and Rust plugins compiled to WASM.

Status: **M2 — Vim grammar** (v0.3.0). Operators, counts, visual mode, search, clipboard.

## Build

```sh
cargo build --release
./target/release/vox path/to/file
```

## Keys

Normal: motions `h j k l w b e 0 ^ $ gg G n N` · operators `d c y` + motion, `dd cc yy` · `x p P` · counts (`3dw`, `2d3w`, `5G`) · `v V` visual · `/` search · `"+` clipboard · `u` / `Ctrl-r` · `i a I A o O`.
Commands: `:w [file]` `:q` `:q!` `:wq` `:x` `:e [file]` `:e!`.

Full list: `docs/src/user/keybindings.md`.

## Docs

- User and developer guide: `docs/` (mdBook: `mdbook serve docs`)
- [ARCHITECTURE.md](ARCHITECTURE.md), [CONTRIBUTING.md](CONTRIBUTING.md), [CHANGELOG.md](CHANGELOG.md)
