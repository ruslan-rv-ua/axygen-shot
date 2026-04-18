# Axygen Shot

[![CI](https://github.com/nickolay-kondratyev/axygen-shot/actions/workflows/ci.yml/badge.svg)](https://github.com/nickolay-kondratyev/axygen-shot/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/nickolay-kondratyev/axygen-shot)](https://github.com/nickolay-kondratyev/axygen-shot/releases/latest)

A portable Windows CLI tool for capturing application windows as screenshots.
Designed primarily for blind developers who need a fast, keyboard-driven workflow
to capture, save, and hand off screenshots to AI assistants for visual description.

**[Українська версія](README_UK.md)**

## What it does

- Captures a specific application window by process name or title substring
- Saves PNG screenshots with timestamped filenames to a project subfolder
- Copies the result to the clipboard (file path, image, or both)
- Plays audio confirmation so you know it worked without looking at the screen
- Runs as a background daemon with a global hotkey for repeated captures

## Installation

Download `shot.exe` from the
[latest release](https://github.com/nickolay-kondratyev/axygen-shot/releases/latest)
and place it anywhere on your `PATH`.

Requirements: Windows 10 or later (x86-64).

## Quick start

```
cd my-project
shot --init --process=myapp.exe
shot
```

This creates `shot.toml` in the current directory, captures the window of
`myapp.exe`, saves the screenshot to `screenshots/`, copies the file path to
the clipboard, and plays a confirmation sound.

## Configuration

`shot.toml` lives at the project root (next to `.git`). The tool searches upward
from the current directory to find it.

```toml
# shot.toml
process   = "myapp.exe"          # match by process name
# title   = "MyApp"              # or match by title substring (OR logic)
folder    = "screenshots"        # output subfolder (default: "screenshots")
clipboard = "path"               # "path" | "image" | "both" (default: "path")
# hotkey  = "Win+F12"            # watch mode hotkey (default: "Win+F12")
```

All fields except `process` or `title` are optional with sensible defaults.

## Usage

```
shot                        Capture once and exit
shot my-label               Capture with a custom filename label
shot --watch                Run as background daemon with global hotkey
shot --list-windows         List all capturable windows
shot --check                Validate configuration without capturing
shot --init                 Create shot.toml template in current directory
shot --init --process=app   Create pre-filled shot.toml
shot --version              Print version
shot --help                 Print help
```

### CLI overrides

Any config value can be overridden on the command line:

```
shot --process=other.exe --clipboard=both --folder=snaps "my label"
```

### Watch mode

```
shot --watch
```

Starts a background daemon that listens for a global hotkey (default `Win+F12`).
A system tray icon appears with a right-click menu offering Restart and Exit.
Only one daemon runs at a time per system (enforced by a named mutex).

## Output

On success, the tool prints to stdout:

```
status: ok
file: C:\projects\myapp\screenshots\2025-01-15_143022345_myapp-main-window.png
clipboard: path
```

On failure, it prints to stderr and exits with a non-zero code.
Audio feedback accompanies both outcomes.

## Building from source

Requires Rust 1.87 or later.

```
cargo build --release
```

The release binary is at `target\release\shot.exe`.

## Development

```
cargo test           # run all tests
cargo clippy -- -D warnings   # lint
cargo fmt -- --check          # format check
```

Or use the justfile:

```
just ci              # lint + format check + test
just build           # release build
just size            # release build + show binary size
```

## Project structure

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point, mode dispatch |
| `src/cli.rs` | CLI argument parsing (clap) |
| `src/config.rs` | TOML config discovery, validation, init |
| `src/window_resolver.rs` | Window enumeration and matching |
| `src/capture.rs` | Window capture via PrintWindow/BitBlt |
| `src/storage.rs` | Filename generation and PNG file saving |
| `src/clipboard.rs` | Clipboard operations (path and image) |
| `src/audio.rs` | Audio feedback via MessageBeep |
| `src/watch.rs` | Watch mode daemon, hotkey, tray icon |
| `src/errors.rs` | Error types and formatting |

## License

See LICENSE file.

---

This project was developed entirely with the assistance of AI coding agents.
