# Architecture

See `ARCHITECTURE.md` at the repository root for the crate map and layering rules.

Summary:
- `vox-core`: text model, no UI.
- `vox-view`: viewport state.
- `vox-tui`: `App` (state + `handle_key`), `impl Widget for &App` (pure render), `run` (event loop).
- `vox`: binary entry point.
