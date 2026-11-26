# Testing

```sh
cargo test --workspace
```

| Kind | Location | Tool |
|---|---|---|
| Unit | `#[cfg(test)] mod tests` in each crate | std |
| Key sequences | `crates/vox-tui/tests/editing.rs` | `parse_keys` |
| Vim golden tests | `crates/vox-tui/tests/vim.rs` | `(before, keys, after)` tables |
| Render snapshots | `crates/vox-tui/tests/render.rs`, `snapshots/*.snap` | `insta` + ratatui `TestBackend` |

## Key-sequence tests
Write the keys in Vim notation and check text and cursor:
```rust
assert_eq!(run("a\nb", "oX<Esc>"), ("a\nX\nb".into(), (1, 0)));
```
No terminal is needed; `App::handle_key` is called directly.

## Vim golden tests
`|` marks the cursor (it sits before the char the cursor is on):
```rust
check(&[
    ("foo |bar baz", "dw", "foo |baz"),
    ("a\n|b\nc", "dd", "a\n|c"),
]);
```
All failing cases in a table are reported together. Add a row for every bug fixed.
The system clipboard is replaced by an in-memory one.

## Snapshots
Update after an intended UI change: `cargo insta review` (requires `cargo install cargo-insta`)
or `INSTA_UPDATE=always cargo test`. Review the diff before committing.
