# Testing

```sh
cargo test --workspace
```

| Kind | Location | Tool |
|---|---|---|
| Unit | `#[cfg(test)] mod tests` in each crate | std |
| Render snapshots | `crates/vox-tui/tests/render.rs`, `snapshots/*.snap` | `insta` + ratatui `TestBackend` |

Key handling is tested by calling `App::handle_key` with synthetic `KeyEvent`s; no terminal needed.

Update snapshots after an intended UI change: `cargo insta review` (requires `cargo install cargo-insta`) or `INSTA_UPDATE=always cargo test`.
