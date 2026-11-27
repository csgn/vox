# vox

Vim-like modal terminal editor written in Rust. Planned: built-in Claude integration (via the `claude` CLI) and Rust plugins compiled to WASM.

Status: **v0.20.0** — buffers (`:e`, `:ls`, `:b`, `:bn`, `:bd`, `:wa`, `vox a b`); completion at the `:` prompt (menu, `Tab`, history; prompt in the status line, a float, or a modal); command browser (`:commands`: categories, breadcrumb, filter); command help (`:help CMD`, `:?CMD`, metadata for every command); `vox run` (editor commands from a shell, `--json`); updater (`:update check|install`, asks at start-up); command API (`:settings set|get`, `:plugin list|info`, data replies); plugin manager (`:plugins`, `<leader>p`); plugin installer and `vox.lock` (`:plugin add PATH|GIT_URL`,
`:plugins sync`); `:settings` dialog (edits config.toml, keeps comments); plugin packages
with manifests, settings, and health checks; project
setups (`--local-config`, `--vanilla`); Vim editing, panels / modal / notifications, developer mode (log, reload), a developer API (commands,
key mappings, events), a config file
(`~/.config/vox/config.toml`), and sandboxed WebAssembly plugins written in Rust
(`~/.config/vox/plugins/*.wasm`, see `plugins/text-tools`). Editor logic is `std`-only; third-party libraries live in small
adapter crates (see [ARCHITECTURE.md](ARCHITECTURE.md)).

## About this project

vox is a personal, non-commercial project for learning and experimenting, developed with the
help of AI tools (Claude Code).

- It is not affiliated with or endorsed by Vim, Neovim, Anthropic, or any other project or
  company.
- vox borrows familiar ideas from modal editors (Vim-style keys, commands, plugins). It does
  not aim to copy any project, and no code from other editors was knowingly copied.
- Names and trademarks belong to their owners.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/csgn/vox/master/install.sh | sh
```

Builds the latest release with `cargo` (needs `git` and [Rust](https://rustup.rs)) into
`~/.cargo/bin`. Options: `VOX_VERSION=v0.16.0`, `VOX_INSTALL_ROOT=DIR` (installs to `DIR/bin`).

## Build from a checkout

```sh
cargo build --release
./target/release/vox path/to/file
./target/release/vox --local-config path/to/file   # project's own config/plugins
./target/release/vox --vanilla path/to/file        # no config, no plugins
```

## Keys

Normal: motions `h j k l w b e 0 ^ $ gg G n N` · operators `d c y` + motion, `dd cc yy` · `x p P` · counts (`3dw`, `2d3w`, `5G`) · `v V` visual · `/` search · `"+` clipboard · `u` / `Ctrl-r` · `i a I A o O`.
Commands: `:w [file]` `:q` `:q!` `:wq` `:x` `:e [file]` `:e!`.

Full list: `docs/src/user/keybindings.md`.

## Docs

- Website: <https://csgn.github.io/vox/>: user guide, developer guide, an interactive
  **code tour** (crate map, function explorer, walkthroughs), and the API reference.
- Build locally: `./scripts/build-docs.sh`, then `python3 -m http.server -d docs/book 3000`
- [ARCHITECTURE.md](ARCHITECTURE.md), [CONTRIBUTING.md](CONTRIBUTING.md), [CHANGELOG.md](CHANGELOG.md)
