# Testing

```sh
cargo test --workspace
```

| Kind | Location | Tool |
|---|---|---|
| Unit | `#[cfg(test)] mod tests` in each crate | std |
| Key sequences | `crates/vox-tui/tests/editing.rs` | `parse_keys` |
| Render snapshots | `crates/vox-tui/tests/render.rs`, `snapshots/*.snap` | `insta` + ratatui `TestBackend` |

## Key-sequence tests
Write the keys in Vim notation and check text and cursor:
```rust
assert_eq!(run("a\nb", "oX<Esc>"), ("a\nX\nb".into(), (1, 0)));
```
No terminal is needed; `App::handle_key` is called directly.

## Snapshots
Update after an intended UI change: `cargo insta review` (requires `cargo install cargo-insta`)
or `INSTA_UPDATE=always cargo test`. Review the diff before committing.
