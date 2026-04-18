# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - Unreleased

First public release of Axygen Shot — a portable Windows CLI screenshot tool
designed for blind developers.

### Added

- Single-window capture via `PrintWindow` with `BitBlt` fallback
- `shot.toml` configuration with walk-up discovery to `.git` boundary
- `--init` command to scaffold `shot.toml` and `.gitignore` entry
- `--list-windows` to enumerate capturable windows
- `--check` to validate configuration without capturing
- Clipboard support: path (`CF_UNICODETEXT`), image (`CF_DIB`), or both
- Audio feedback: success/error sounds via `MessageBeep`
- Timestamped filenames with millisecond precision and window title
- `--label` for custom filename suffixes
- `--quiet` and `--verbose` output modes
- Watch mode (`--watch`) with global hotkey, system tray icon, and daemon process
- Daemon single-instance guard via named mutex
- Daemon restart from tray context menu
- DPI-aware manifest (`PerMonitorV2`)
- CI pipeline (GitHub Actions) with lint, format, test, and release build
- Manual release workflow producing zip distribution with SHA-256 checksum
