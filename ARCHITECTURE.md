# Architecture

## Crates (current)

```
vox (bin) ──> vox-tui ──> vox-view
         └──> vox-core <──┘
```

| Crate | Responsibility | Depends on |
|---|---|---|
| `vox-core` | Text model (`Document` over `ropey::Rope`). No IO beyond file load, no UI. | ropey |
| `vox-view` | Viewport/scroll state. No rendering. | — |
| `vox-tui` | `App` state, key handling, rendering (`ratatui`), event loop. | vox-core, vox-view, ratatui |
| `vox` | Binary: argument parsing, terminal init/restore. | vox-core, vox-tui, ratatui |

## Rules
- `vox-core` and `vox-view` never depend on terminal crates.
- Rendering is a pure function of state (`impl Widget for &App`) so it is snapshot-testable.
- Key handling is separate from the event loop (`App::handle_key`) so it is unit-testable without a terminal.

## Planned crates
`vox-keymap`, `vox-api`, `vox-std`, `vox-syntax`, `vox-config`, `vox-plugin-host`, `vox-plugin-sdk`, `vox-claude`. Added per milestone; see the roadmap in `docs/src/dev/roadmap.md`.
