# Contributing

## Before pushing
```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Definition of done (per change)
- Tests for new behaviour (unit, or snapshot for rendering).
- Docs updated (`docs/`, rustdoc for public items).
- `CHANGELOG.md` `[Unreleased]` entry.

## Snapshot tests
Render tests use [insta](https://insta.rs). After an intended UI change:
```sh
cargo insta review   # or: INSTA_UPDATE=always cargo test
```
Commit the updated `.snap` files.
