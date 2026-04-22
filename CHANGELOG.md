# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project follows Semantic Versioning.

## [0.2.0] - 2026-04-22

### Added
- Added process provenance metadata to listeners, including executable path, working directory, parent process, project root, and origin summary.
- Added security-oriented listener heuristics for bind scope, risk level, workspace relation, and recommended action.
- Added runtime and service inference for common listener types such as Go, Node, Python, Docker, PostgreSQL, Redis, Electron, macOS app, and system service processes.
- Added a compact-safe inspect flow that opens with `Enter` or `i` and surfaces port, identity, provenance, guidance, and action details.
- Added Unicode-safe truncation utilities and regression tests for UI text handling.
- Added key-handling regression tests covering navigation, filtering, inspect/help overlays, sorting, mark/unmark flows, confirm dialogs, and `Ctrl+C`.

### Changed
- Bumped crate version from `0.1.0` to `0.2.0`.
- Reworked the table layout to adapt to compact, standard, and wide terminal widths.
- Updated visible columns to emphasize scope, action, risk, and inferred type instead of only raw process stats.
- Made `Tab` cycle only through sort columns that are visible in the current layout, avoiding hidden-column sort jumps.
- Unified TUI and `--json` filtering so both modes match against the same fields.
- Improved clipboard handling to report real failures instead of showing false success.
- Improved kill behavior to verify process termination more accurately before reporting success.

### Fixed
- Fixed stale or misleading sort behavior when the terminal layout changed and the previous sort column was no longer visible.
- Fixed unsafe string truncation that could panic on multibyte UTF-8 text.
- Fixed workspace-origin heuristics that could incorrectly classify `/`-based processes as belonging to the current workspace.
- Fixed process detection edge cases for app bundle paths outside `.app/Contents/MacOS/`.

### Verified
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
