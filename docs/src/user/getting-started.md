# Getting started

```sh
cargo build --release
./target/release/vox FILE
```

- `vox` with no argument opens an empty unnamed buffer. Save it with `:w NAME`.
- `vox FILE` where `FILE` does not exist opens an empty buffer bound to that path; `:w` creates it.
- Quit with `:q` (refuses if there are unsaved changes; `:q!` discards them).

## Files
- Line endings (`\n` or `\r\n`) are detected from the first line and used for new lines.
- Whether the file ends with a newline is preserved on save. New files get a final newline.
- Tabs are shown 4 columns wide (aligned to tab stops).
