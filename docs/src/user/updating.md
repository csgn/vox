# Updating vox

At start-up vox looks for a newer release in the background: it reads the `vX.Y.Z` tags of
<https://github.com/csgn/vox.git> with `git ls-remote`. If there is one, it asks:

```text
┌─ Update vox ──────────────────────────┐
│Install vox 0.15.0? You have 0.14.0.   │
│                                       │
│[y] yes   [n] no                       │
└───────────────────────────────────────┘
```

- `y` / `Enter` builds and installs it with
  `cargo install --locked --force --git … --tag vX.Y.Z --root ROOT vox` in the background,
  into the same place as the running vox. A box shows the progress (each crate cargo
  compiles, and the time so far) until it is done. Restart vox to use it.
- `n` / `Esc` does nothing; vox asks again next time.

If a dialog is already open, vox shows a notification instead of asking.
Without network or `git`, the check fails quietly (see `:log update`).

| Command | Action |
|---|---|
| `:update check` | look for a newer release now; asks if there is one |
| `:update install [VERSION]` | install `VERSION` (e.g. `0.15.0`), or the newest release |

vox can only replace itself if it was installed with `cargo install` or `install.sh` (the
binary is in `ROOT/bin/`). A vox run from a source tree (`target/release/vox`) is never
replaced; update it with `git pull` and `cargo build`.

## Turning the check off

```toml
check_updates = false
```

Or `:settings set check_updates false`, or in `:settings` → vox settings. `vox --vanilla`
never checks.

Needs `git` (check) and `cargo` (install).
