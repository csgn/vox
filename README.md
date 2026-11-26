# vox

Vim-like modal terminal editor written in Rust. Planned: built-in Claude integration (via the `claude` CLI) and Rust plugins compiled to WASM.

Status: **v0.5.0** — Vim editing, a developer API (commands, key mappings), and a config file
(`~/.config/vox/config.toml`). Editor logic is `std`-only; third-party libraries live in small
adapter crates (see [ARCHITECTURE.md](ARCHITECTURE.md)).

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

- User guide, developer guide, and an interactive **code tour** (crate map, function explorer,
  walkthroughs): `./scripts/build-docs.sh`, then `python3 -m http.server -d docs/book 3000`
- [ARCHITECTURE.md](ARCHITECTURE.md), [CONTRIBUTING.md](CONTRIBUTING.md), [CHANGELOG.md](CHANGELOG.md)
