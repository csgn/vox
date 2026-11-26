# Architecture

## Crates (current)

```
vox (bin) ──> vox-tui ──> vox-view
         └──> vox-core <──┘
```

| Crate | Responsibility | Depends on |
|---|---|---|
| `vox-core` | Text model: `Document` (rope, file load/save, edits, linear undo/redo), `Position`, `motion` fns, literal `search`. No UI. | ropey |
| `vox-view` | `Viewport` (scroll, follow cursor), char-column → screen-cell mapping (tabs, wide chars). No rendering. | unicode-width |
| `vox-tui` | `App` (modes, cursor, registers, `:` commands), Vim grammar parser, key notation parser, rendering (`ratatui`), event loop. | vox-core, vox-view, ratatui, arboard |
| `vox` | Binary: argument parsing, terminal init/restore. | vox-core, vox-tui, ratatui |

## Rules
- `vox-core` and `vox-view` never depend on terminal crates.
- Rendering is a pure function of state (`impl Widget for &App`) so it is snapshot-testable.
- Key handling is separate from the event loop (`App::handle_key`) so it is unit-testable without a terminal.

## Planned crates
`vox-api` (+ config), `vox-plugin-host`, `vox-plugin-sdk`, `vox-claude`. Added only when a milestone needs them; see the roadmap in `docs/src/dev/roadmap.md`.
