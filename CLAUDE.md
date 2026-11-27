# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

vox: Vim-like modal terminal editor in Rust (edition 2024) with sandboxed Rust→WASM plugins. Not a Neovim clone: scope is Vim-style editing, a small developer API, and easy-to-manage plugins (see `docs/src/dev/roadmap.md`). Next: buffers, then text/agent editor modes with focus and Colab-style inline AI edits (M18–M21); Claude comes as a backend in M21.

## Commands

```sh
cargo build --release && ./target/release/vox FILE   # also: --local-config [DIR], --vanilla
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings  # CI sets RUSTFLAGS=-D warnings
cargo test --workspace
cargo test -p vox-editor --test vim                   # one test file
cargo test -p vox-editor --test vim -- some_name      # one test
cargo doc --workspace --no-deps                       # CI; missing_docs is a workspace lint
./scripts/check-deps.sh                               # core crates must stay std-only
python3 scripts/gen-code-tour.py                      # regenerate docs/src/tour/data.js (CI: --check)
cargo insta review   # or INSTA_UPDATE=always cargo test — after intended UI changes
```

- `vox-plugin-host` tests build `plugins/` for WASM: needs `rustup target add wasm32-wasip2` (plus `git` for install tests).
- `plugins/` is a separate cargo workspace: `scripts/build-plugins.sh`; package one plugin with `scripts/build-plugin.sh DIR [--install DEST]`.
- Docs site: `./scripts/build-docs.sh`, then `python3 -m http.server -d docs/book 3000`.

## Architecture

See `ARCHITECTURE.md` for the crate table. Key points:

- **Core** (`vox-core`, `vox-view`, `vox-editor`) is `std`-only. Third-party crates live only in **adapter** crates (`vox-storage`, `vox-width`, `vox-clipboard`, `vox-term`, `vox-config-file`, `vox-plugin-host`) or the `vox` binary, each implementing a core trait. Core API never exposes third-party types. Enforced by `scripts/check-deps.sh`. Procedure for new deps: `docs/src/dev/dependencies.md`. Things like the plugin manifest, `vox.lock`, sha256, and version parsing are therefore hand-written inside `vox-editor/src/plugin/`.
- Crates/types are named by purpose (`vox-storage`), never after the wrapped library.
- `vox` binary parses CLI, picks the scope, builds `Services` (`crates/vox-editor/src/editor.rs`) with adapter impls, loads config + plugins, runs the `vox-term` loop.
- `Editor::handle_key` is independent of the event loop; `Editor::render() -> Surface` is pure. Both are tested without a terminal.
- Background work (plugin install/sync) runs as jobs reporting via an mpsc channel on `Editor`.
- Developer API in `vox-editor`: command registry (`command.rs`; every command gets metadata via `register(..).category(..).param(..)`, shown by `:help` in `help.rs`), key mappings (`keymap.rs`), events (`event.rs`), `Config` from a format-neutral `Value`. Built-ins in `builtin.rs`. Plugins use the same API through `vox-plugin-host`; things are tracked by owner (`owner.rs`) so a plugin's registrations can be removed on reload.
- Plugin contract: `wit/vox.wit`. Authors use `vox-plugin-sdk` (not linked into vox). Examples: `plugins/text-tools`, `plugins/misbehave` (misbehaving plugin for host limit tests).
- Scopes: exactly one active — global (`~/.config/vox/`), local (`<project>/.vox/`, found upward like git), vanilla (none). Each has `config.toml`, `plugins/`, `vox.lock`.
- UI layer (`vox-editor/src/ui`): panels, modal, notifications, forms/views; settings dialog in `settings/`, plugin manager in `plugin/manager.rs`.

## Conventions

- Commands use subcommands, registered as `"ROOT SUB"` (e.g. `"log clear"`, `"plugin add"`), not hyphenated names. `:cmd! sub` and `:cmd sub!` both force.
- Nested dialogs must allow going back to the parent, with a visible hint.
- Work proceeds one milestone at a time (`docs/src/dev/roadmap.md`). Definition of done per change: tests (unit, `vim.rs` golden row for each bug fix, or insta snapshot for rendering), docs (`docs/src/`, `///` on every function — the code tour shows them), and a `CHANGELOG.md` `[Unreleased]` entry. Commit regenerated `docs/src/tour/data.js` and `.snap` files.
- Walkthroughs in `docs/tour-flows.json` reference function names; the tour generator fails if one is renamed/removed.

## Tests

| Kind | Location |
|---|---|
| Key sequences (`run("a\nb", "oX<Esc>")`) | `crates/vox-editor/tests/editing.rs` |
| Vim golden tables (`"foo \|bar"`, `\|` = cursor) | `crates/vox-editor/tests/vim.rs` |
| Render snapshots | `crates/vox-editor/tests/render.rs`, `tests/snapshots/` |
| Plugins / install / manager (real WASM) | `crates/vox-plugin-host/tests/` |
| `vox run` (real binary) | `crates/vox/tests/run.rs` |

Core tests use `SimpleStorage`, `Monospace` widths, and an in-memory clipboard.
