# vox

Vim-like modal terminal editor written in Rust. Planned: built-in Claude integration (via the `claude` CLI) and Rust plugins compiled to WASM.

Status: **M0 — skeleton**. Read-only viewer: open a file, scroll, `:q`.

## Build

```sh
cargo build --release
./target/release/vox path/to/file
```

## Keys (M0)

| Key | Action |
|---|---|
| `j` / `Down`, `k` / `Up` | scroll one line |
| `Ctrl-d` / `Ctrl-u` | scroll half page |
| `Ctrl-f` / `Ctrl-b`, `PageDown` / `PageUp` | scroll full page |
| `gg` / `G` | top / bottom |
| `:q` | quit |

## Docs

- User and developer guide: `docs/` (mdBook: `mdbook serve docs`)
- [ARCHITECTURE.md](ARCHITECTURE.md), [CONTRIBUTING.md](CONTRIBUTING.md), [CHANGELOG.md](CHANGELOG.md)
