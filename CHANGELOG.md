# Changelog

All notable changes to this project are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [SemVer](https://semver.org/).

## [Unreleased]

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
