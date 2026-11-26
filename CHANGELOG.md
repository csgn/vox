# Changelog

All notable changes to this project are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [SemVer](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-11-26 — M1 core editing

### Added
- Cursor with Normal and Insert modes; bar cursor in Insert mode.
- Motions: `h j k l`, arrows, `w`, `b`, `0`/`Home`, `$`/`End`, `gg`, `G`; `j`/`k` keep the wanted column.
- Insert entry: `i`, `a`, `o`, `O`. Insert mode: text, `Enter`, `Backspace` (joins lines), `Tab`, arrows, `Esc`.
- `x` deletes the char under the cursor.
- Linear undo/redo (`u`, `Ctrl-r`); one insert session = one undo step.
- Commands: `:w [file]`, `:wq`, `:x`, `:e [file]`, `:e!`, `:q!`.
- Modified flag (`[+]` in statusline); `:q` and `:e` refuse with `E37` when modified.
- Line ending (`\n`/`\r\n`) and final-newline preservation on save.
- Horizontal scrolling; tabs render to 4-column tab stops; wide chars supported.
- `parse_keys`: Vim key notation parser (`"ihi<Esc>"`, `<C-r>`).
- Tests: document/undo/motion unit tests, key-sequence tests, more render snapshots.
- Docs: keybindings, commands, architecture, testing pages updated.

### Changed
- `j`/`k`, `Ctrl-d`/`Ctrl-u`, `Ctrl-f`/`Ctrl-b`, `PageUp`/`PageDown` move the cursor; the viewport follows.
- Statusline shows `line:col` instead of `top/total`.
- `Document::line_count` no longer special-cases a trailing newline (it is stored outside the rope).

## [0.1.0] - 2026-11-26 — M0 skeleton

### Added
- Cargo workspace with `vox-core`, `vox-view`, `vox-tui`, `vox` crates.
- `vox-core`: rope-backed `Document` (open file, missing file = empty buffer, line access).
- `vox-view`: `Viewport` with clamped scrolling.
- `vox-tui`: line-number gutter, `~` filler rows, statusline (mode, file, position), command line.
- Normal-mode scrolling: `j`/`k`, arrows, `Ctrl-d`/`Ctrl-u`, `Ctrl-f`/`Ctrl-b`, `PageUp`/`PageDown`, `gg`, `G`.
- Command line: `:q`, `:q!`, `:quit`, `:qa`; unknown commands show `E492`.
- Unit tests and `insta` render snapshot tests.
- Docs: README, ARCHITECTURE, CONTRIBUTING, mdBook scaffold.
- CI: fmt, clippy, test, docs build.
