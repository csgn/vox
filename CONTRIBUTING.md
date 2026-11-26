# Contributing

## Before pushing
```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/check-deps.sh   # core crates must stay std-only
# plugin tests build plugins/ for WebAssembly: rustup target add wasm32-wasip2
python3 scripts/gen-code-tour.py   # refresh the docs code tour after code changes
```

## Definition of done (per change)
- Tests for new behaviour (unit, or snapshot for rendering).
- Docs updated (`docs/`, rustdoc for public items).
- `CHANGELOG.md` `[Unreleased]` entry.

## Dependencies
Core crates (`vox-core`, `vox-view`, `vox-editor`) must not depend on third-party crates.
Wrap a library in an adapter crate behind a core trait: see `docs/src/dev/dependencies.md`.

## Docs website
```sh
./scripts/build-docs.sh                      # tour data + mdBook + API reference
python3 -m http.server -d docs/book 3000     # open http://127.0.0.1:3000
```
The **Code tour** pages are generated from the source. After changing code, run
`python3 scripts/gen-code-tour.py` and commit `docs/src/tour/data.js` (CI checks it is current).
Walkthroughs live in `docs/tour-flows.json`; the generator fails if a step names a function that
no longer exists. Give every function a `///` description: the tour shows it.

## Snapshot tests
Render tests use [insta](https://insta.rs). After an intended UI change:
```sh
cargo insta review   # or: INSTA_UPDATE=always cargo test
```
Commit the updated `.snap` files.
