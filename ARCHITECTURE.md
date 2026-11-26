# Architecture

## Layers

```
                 ┌──────────── core (std only) ────────────┐
vox (bin) ──────>│ vox-editor ──> vox-view                  │
   │             │      └──────> vox-core                   │
   │             └──────────────────────▲───────────────────┘
   │                                    │ implement core traits
   └──> adapters: vox-storage ─ vox-width ─ vox-clipboard ─ vox-term ─ vox-config-file ─ vox-plugin-host

   plugins (.wasm) ──> vox-plugin-sdk (for plugin authors) ── contract: wit/vox.wit
```

| Crate | Layer | Responsibility | Third-party library used |
|---|---|---|---|
| `vox-core` | core | `Document` over `TextStorage` trait, `Position`, undo/redo, `motion`, literal `search`, `SimpleStorage` | none |
| `vox-view` | core | `Viewport`, `CharWidth` trait (+ `Monospace`), char-column → screen-cell mapping | none |
| `vox-editor` | core | `Editor`: modes, Vim grammar, registers; developer API (command registry, key mappings, events, edit functions, `Config` from a format-neutral `Value`); UI layer (panels, modal, notifications, views); own `Key` types; renders to own `Surface`; `Clipboard` trait; `Services` injection | none |
| `vox-storage` | adapter | `Storage: TextStorage` — fast text storage for large files (tree of text chunks) | ropey |
| `vox-width` | adapter | `UnicodeWidth: CharWidth` — terminal cell width of each char | unicode-width |
| `vox-clipboard` | adapter | `SystemClipboard: Clipboard` — system copy/paste | arboard |
| `vox-term` | adapter | terminal: raw mode, key input → `Key`, draws `Surface` (row diff), run loop with mapping timeout | crossterm |
| `vox-config-file` | adapter | finds and reads `config.toml`, converts it to `Value` | toml |
| `vox-plugin-host` | adapter | runs WebAssembly plugins (time/memory limits, no system access) after core's health checks; passes settings; maps plugin requests to the `Editor` API; `:checkhealth` | wasmtime, wasmtime-wasi |
| `vox-plugin-sdk` | plugin SDK | Rust glue for plugin authors, from `wit/vox.wit` (not linked into vox) | wit-bindgen |
| `vox` | binary | parses the command line (`--local-config`, `--vanilla`), picks the scope, builds `Services`, loads the scope's config and plugins, runs `vox-term` | none directly |

## Rules
- **Core crates are `std`-only.** A third-party crate may only appear in an adapter crate or
  the binary. Enforced by `scripts/check-deps.sh` (CI). Dev-dependencies (e.g. `insta`) are allowed.
- An adapter implements a trait (or converts to a type) defined in core, and nothing else.
- Core API never exposes third-party types.
- Crates are named by **what they do** (`vox-storage`, `vox-width`), never after the library
  they use. Library names appear only in docs and `Cargo.toml`.
- Rendering is pure (`Editor::render() -> Surface`), so it is snapshot-testable without a terminal.
- Key handling (`Editor::handle_key`) is independent of the event loop, so it is unit-testable.

See `docs/src/dev/dependencies.md` for how to add a new dependency.

## Planned crates
`vox-claude`. Added only when a milestone needs them.
