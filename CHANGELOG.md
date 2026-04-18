# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Split into two binaries: `shot.exe` (console subsystem, CLI) and
  `shot-watch.exe` (windows subsystem, tray daemon). Both must reside in the
  same directory. See [ADR-002](docs/ADR-002-dual-binaries.md).
- `shot.exe --watch` now spawns `shot-watch.exe` as a detached daemon process
  instead of re-launching itself.
- Extracted shared code into `axygen_shot` library crate (`src/lib.rs`).

### Added

- Scoop package manager support: `scoop install axygen-shot` via
  [ruslan-rv-ua bucket](https://github.com/ruslan-rv-ua/scoop-bucket).
- GitHub Actions workflow (`update-scoop.yml`) to update Scoop manifest on new
  releases.

### Fixed

- Shell prompt-before-output race condition in cmd.exe and PowerShell caused by
  GUI-subsystem executable with `AttachConsole`. The console subsystem `shot.exe`
  is now waited on synchronously by the shell.

## [0.1.0] - 2025-07-17

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
