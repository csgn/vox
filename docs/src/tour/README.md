# Code tour

An interactive map of the vox source code: which crates exist, what is inside them, which
functions call which, and what happens step by step when you use the editor.

Everything on these pages is generated from the source by `scripts/gen-code-tour.py`, so it
matches the code. Links marked with a file path open the source code in the API reference.

- **This page** — the crate map. Click a crate to see its purpose, files, types, functions,
  and which crates use it.
- **[Function explorer](functions.md)** — every function: what it does, its signature, what it
  calls, and who calls it. Click through the call graph.
- **[Walkthroughs](walkthroughs.md)** — step through real scenarios (start-up, `dw`, typing,
  `:w`, drawing a frame, undo, search, clipboard) one function at a time.

## How the pieces fit

vox is split into three layers:

| Layer | Crates | Rule |
|---|---|---|
| **core** | `vox-core`, `vox-view`, `vox-editor` | All editor logic. Only the Rust standard library. |
| **adapters** | `vox-storage`, `vox-width`, `vox-clipboard`, `vox-term` | Each wraps one third-party library behind a trait that core defines. |
| **binary** | `vox` | Picks the adapters and starts the editor. |

Core never names a third-party library. When it needs something from the outside world
(storing text, measuring char widths, the clipboard, the terminal) it defines a trait, and an
adapter implements it. Tests use small std-only stand-ins instead of adapters.

## Crate map

<link rel="stylesheet" href="tour.css">
<div id="vt-map"></div>
<script src="data.js"></script>
<script src="tour.js"></script>
