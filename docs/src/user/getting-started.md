# Getting started

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/csgn/vox/master/install.sh | sh
```

Builds the latest release with `cargo` (needs `git` and [Rust](https://rustup.rs)) into
`~/.cargo/bin`. Options: `VOX_VERSION=v0.16.3`, `VOX_INSTALL_ROOT=DIR` (installs to `DIR/bin`).

Or from a checkout:

```sh
cargo build --release
./target/release/vox FILE
```

- `vox` with no argument opens an empty unnamed buffer. Save it with `:w NAME`.
- `vox FILE` where `FILE` does not exist opens an empty buffer bound to that path; `:w` creates it.
- `vox --local-config [DIR] FILE` uses the project's own config and plugins instead of your
  global ones; `vox --vanilla FILE` uses none. See [Project setups](scopes.md).
- `vox --help`, `vox --version`.
- Quit with `:q` (refuses if there are unsaved changes; `:q!` discards them).

## Files
- Line endings (`\n` or `\r\n`) are detected from the first line and used for new lines.
- Whether the file ends with a newline is preserved on save. New files get a final newline.
- Tabs are shown 4 columns wide (aligned to tab stops).
