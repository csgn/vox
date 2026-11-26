# Dependencies and adapters

Core crates (`vox-core`, `vox-view`, `vox-editor`) use only `std`. Third-party libraries
live in **adapter** crates. `scripts/check-deps.sh` enforces this in CI.

## Why
- Core logic stays testable without terminals, clipboards, or large libraries.
- Any library can be replaced by writing a new adapter; core does not change.
- Core API never leaks third-party types to plugins or other users.

## Adding a third-party library
1. **Define the abstraction in core.** A trait (or plain data type) that says what vox needs,
   in vox terms. Keep it minimal. Example: `vox_view::CharWidth`.
2. **Provide a std stand-in** in core when tests need one. Example: `vox_view::Monospace`.
3. **Create an adapter crate** named by its purpose (`crates/vox-<purpose>`, e.g.
   `vox-width`, never `vox-<library>`). It depends on the library and the core crate and only
   implements the trait. Example: `vox-width` implements `CharWidth` with `unicode-width`.
4. **Test the adapter**: if core ships a conformance check (e.g.
   `vox_core::storage::conformance`), call it from the adapter's tests.
5. **Wire it in the binary** (`crates/vox/src/main.rs`), e.g. through `Services`.
6. Run `scripts/check-deps.sh`.

## Current adapters
| Adapter | Purpose | Implements | Library |
|---|---|---|---|
| `vox-storage` | fast text storage for large files | `vox_core::storage::TextStorage` | ropey (LF-only lines) |
| `vox-width` | terminal cell width of each char | `vox_view::CharWidth` | unicode-width |
| `vox-clipboard` | system copy/paste | `vox_editor::Clipboard` | arboard |
| `vox-term` | terminal input and drawing | terminal I/O for `Key` / `Surface` | crossterm |
| `vox-config-file` | read `config.toml`; write settings changes keeping comments | produces `vox_editor::Value`; implements `ConfigWriter` | toml, toml_edit |
| `vox-plugin-host` | run plugins in a sandbox | uses the `Editor` API for each plugin request | wasmtime, wasmtime-wasi (one project: the runtime and its system-interface part) |

`vox-plugin-sdk` is not part of vox: plugin authors compile it into their plugins. It uses
`wit-bindgen` to generate glue code from `wit/vox.wit`.

`ropey` stores text as a *rope*: a balanced tree of small text chunks, so inserting or
deleting in a large file does not copy the whole file.
