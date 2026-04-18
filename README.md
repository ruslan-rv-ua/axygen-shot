# Axygen Shot

[![CI](https://github.com/ruslan-rv-ua/axygen-shot/actions/workflows/ci.yml/badge.svg)](https://github.com/ruslan-rv-ua/axygen-shot/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/ruslan-rv-ua/axygen-shot)](https://github.com/ruslan-rv-ua/axygen-shot/releases/latest)

A portable Windows command-line utility for capturing specific application windows as screenshots. Designed primarily to assist blind developers, it provides a fast, keyboard-driven workflow to capture, save, and pass screenshots to AI coding assistants for visual description.

**[Українська версія](README_UK.md)**

## Features

- Captures a specific window matched by its process name or title substring.
- Automatically saves PNG screenshots with timestamped filenames to a project subfolder.
- Copies the result directly to your clipboard (image itself, file path, or both).
- Plays distinctive audio confirmation tones upon success or failure.
- Can run continuously as a background daemon with a global hotkey for repeated captures.

## Installation

### Via Scoop (Recommended)

```powershell
scoop bucket add ruslan-rv-ua https://github.com/ruslan-rv-ua/scoop-bucket
scoop install axygen-shot
```

### Manual Installation

1. Download the latest release from the [Releases page](https://github.com/ruslan-rv-ua/axygen-shot/releases/latest).
2. Extract the archive. 
3. Move both `shot.exe` and `shot-watch.exe` into the same directory.
4. Ensure that directory is added to your system's `PATH` environment variable.

Requirements: Windows 10 or later (x86-64).

## Quick Start

Navigate to your project directory and initialize the configuration for the application you want to capture (e.g., your web browser or emulator):

```powershell
cd my-project
shot --init --process=myapp.exe
```

This command creates a `shot.toml` file in the current directory. To capture the window, simply run:

```powershell
shot
```

The app will silently capture the window of `myapp.exe`, save the PNG file to the `screenshots/` directory, copy the file path to your clipboard, and play a confirmation sound.

## Configuration

Axygen Shot uses a `shot.toml` configuration file. The tool searches upwards from your current directory to find this file, meaning you should generally place it at the root of your project (e.g., next to `.git`).

### Configuration Options

```toml
# shot.toml
process   = "myapp.exe"          # Match the window by its process name
# title   = "MyApp"              # Alternatively, match by title substring (OR logic)
folder    = "screenshots"        # Directory to save the screenshots (default: "screenshots")
clipboard = "path"               # What to copy to the clipboard: "path", "image", or "both" (default: "path")
# hotkey  = "Win+F12"            # The hotkey used in watch mode (default: "Win+F12")
```

At minimum, you must specify either `process` or `title`. Other fields are strictly optional and use sensible defaults.

## Usage Guide

Axygen Shot can be executed as a one-off command or run continuously in the background.

```text
shot                        Capture the defined window once and exit
shot my-custom-label        Capture and append "my-custom-label" to the filename
shot --watch                Start the background daemon responding to the global hotkey
shot --list-windows         Display a list of all currently capturable windows
shot --check                Validate the configuration without taking a screenshot
shot --init                 Generate an empty shot.toml template in the current directory
shot --init --process=app   Generate a pre-filled shot.toml
shot --version              Print tool version
shot --help                 Print help information
```

### Overriding Settings temporarily

You can override any setting from `shot.toml` directly using command-line arguments:

```powershell
shot --process=other.exe --clipboard=both --folder=snaps "alternative-label"
```

### Watch Mode (Background Daemon)

If you need to make repeated captures without switching your focus away from the target window, use Watch Mode:

```powershell
shot --watch
```

This starts a background daemon waiting for the global hotkey (`Win+F12` by default). The tool will appear as an icon in your system tray, where you can right-click to restart or exit. Only one daemon will run per system to avoid conflicts.

### Output and Feedback

Every operation provides text and audio feedback.

**Success:** Plays a short confirmation tone, exits with code 0, and prints:
```text
status: ok
file: C:\projects\myapp\screenshots\2025-01-15_143022345_myapp-main-window.png
clipboard: path
```

**Failure:** Plays a multi-tone error sound, exits with a non-zero code, and prints human-readable error details to standard error (stderr).

## Development

This section is intended for developers modifying or building the tool from source. 

> **Why two executable files?** `shot.exe` is a standard console application, ensuring shell environments wait for the command synchronously. `shot-watch.exe` is specifically compiled as a Windows GUI application, allowing the background tray daemon to run silently without flashing a black console window. See [ADR-002](docs/ADR-002-dual-binaries.md) for deeper technical context.

### Building from Source

Requires Rust 1.87 or newer.

```cmd
cargo build --release
```
Compiled binaries are output to `target\release\shot.exe` and `target\release\shot-watch.exe`.

### Common Tasks

A `justfile` is provided for convenience:

```cmd
just ci              # Linter, format check, and all unit tests
just build           # Release build
just size            # Release build + show final binary sizes
```
Alternatively, using Cargo directly:
```cmd
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

---
This project was developed entirely with the assistance of AI coding agents.
