# Axygen Shot MVP — Implementation Design Spec

> **Phase:** 1 (MVP — CLI mode)
> **Stack:** Rust, windows crate v0.62+, clap v4, toml v0.8, png v0.17
> **Scope:** All PRD features EXCEPT `--watch` mode (tray, hotkey, daemon)
> **PRD Reference:** `docs/PRD.md`
> **ADR Reference:** `docs/ADR-001-tech-stack.md`

## Overview

Axygen Shot (`shot.exe`) is a portable Windows CLI tool that captures a specific application window as a PNG screenshot, saves it with a structured filename, copies it to the clipboard, and plays an audio confirmation. Designed for a totally blind developer — all feedback is non-visual.

This spec covers Phase 1 (MVP): CLI mode with all supporting modules. Phase 2 (watch mode with tray icon and global hotkey) will be a separate spec.

## Phase 1 Scope

### Included (User Stories from PRD)

- **Configuration:** Stories 1–12 — `shot.toml` parsing, walk-up discovery, validation
- **Initialization:** Stories 13–15 — `--init` creates template config + `.gitignore` entry
- **CLI Mode:** Stories 16–26 — capture, labels, CLI overrides, `--quiet`/`--verbose`
- **Screenshot Capture:** Stories 35–39 — single-window capture via `PrintWindow`
- **File Naming & Storage:** Stories 40–44 — timestamped filenames, auto-create directory
- **Clipboard:** Stories 45–47 — path, image, or both
- **Discovery:** Stories 48–49 — `--list-windows`
- **Terminal Output:** Stories 50–56 — `key: value` format, `--check`, `--version`
- **Feedback:** Stories 57–58, 60 — success/error sounds, stderr messages
- **Edge Cases:** Stories 61–63 — minimized, capture-failed, storage-failed

### Excluded (Phase 2)

- **Watch Mode:** Stories 27–34 — `--watch`, tray icon, `RegisterHotKey`, daemon detachment
- **Watch Feedback:** Story 59 — startup sound
- Future stories 64–67

## Project Structure

```
axygen-shot/
├── Cargo.toml
├── build.rs                    # Embed Windows application manifest
├── shot.manifest               # DPI awareness declaration
├── src/
│   ├── main.rs                 # Entry point: AttachConsole, parse CLI, dispatch
│   ├── cli.rs                  # clap derive struct, Mode enum, validation
│   ├── config.rs               # TomlConfig, CaptureConfig, find/merge/validate
│   ├── window_resolver.rs      # WindowEnumerator trait, Win32Enumerator, resolve()
│   ├── capture.rs              # PrintWindow → HBITMAP → PNG bytes
│   ├── storage.rs              # Filename generation, sanitization, file write
│   ├── clipboard.rs            # CF_UNICODETEXT and/or CF_DIB
│   ├── audio.rs                # MessageBeep wrappers
│   └── errors.rs               # ShotError enum (thiserror), output formatting
├── tests/
│   ├── cli_integration.rs      # CLI argument combinations
│   ├── config_integration.rs   # Walk-up with temp directories
│   ├── init_integration.rs     # --init file creation
│   └── storage_integration.rs  # End-to-end file write
├── CLAUDE.md                   # Agent instructions (Claude Code)
└── AGENTS.md                   # Agent instructions (Copilot Agent)
```

## Cargo.toml

```toml
[package]
name = "axygen-shot"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "shot"

[dependencies]
clap = { version = "4", features = ["derive"] }
toml = "0.8"
serde = { version = "1", features = ["derive"] }
png = "0.17"
thiserror = "2"
chrono = { version = "0.4", features = ["clock"], default-features = false }

[dependencies.windows]
version = "0.62"
features = [
    "Win32_System_Console",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_Threading",
    "Win32_Graphics_Gdi",
    "Win32_System_DataExchange",
    "Win32_System_Memory",
    "Win32_UI_Shell",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_HiDpi",
    "Win32_Media_Audio",
    "Win32_Foundation",
    "Win32_Security",
]

[dev-dependencies]
tempfile = "3"
assert_cmd = "2"

[profile.release]
lto = true
strip = true
codegen-units = 1
```

**Note:** The exact `windows` crate feature flag names must be verified at first `cargo build`. If a feature is not found, check `docs.rs/windows` for the correct module path. The list above reflects the expected structure — agent should fix any discrepancies.

## Data Types

### cli.rs

```rust
use clap::{Parser, ValueEnum};

#[derive(Parser)]
#[command(name = "shot", version, about = "Screenshot tool for developers")]
pub struct CliArgs {
    /// Optional label for the screenshot filename
    pub label: Option<String>,

    /// Target process name (e.g. "myapp.exe")
    #[arg(long)]
    pub process: Option<String>,

    /// Target window title substring
    #[arg(long)]
    pub title: Option<String>,

    /// Output subfolder (relative path)
    #[arg(long)]
    pub folder: Option<String>,

    /// Clipboard mode
    #[arg(long, value_enum)]
    pub clipboard: Option<ClipboardMode>,

    /// Watch mode hotkey override (Phase 2)
    #[arg(long)]
    pub hotkey: Option<String>,

    /// Suppress stdout output
    #[arg(long)]
    pub quiet: bool,

    /// Print diagnostic info to stderr
    #[arg(long)]
    pub verbose: bool,

    // --- Mode flags (mutually exclusive) ---
    /// Create shot.toml template in CWD
    #[arg(long)]
    pub init: bool,

    /// Watch mode — Phase 2, reject with error in Phase 1
    #[arg(long)]
    pub watch: bool,

    /// Validate config and check window
    #[arg(long)]
    pub check: bool,

    /// List all visible windows
    #[arg(long)]
    pub list_windows: bool,

}

pub enum Mode {
    Capture,
    Init,
    Check,
    ListWindows,
    Version,
    Watch, // Phase 2 — returns error in Phase 1
}
```

**Note on --init flags:** PRD stories 14 specify `--init --process=myapp.exe` and `--init --title="MyApp"`. The existing `--process` and `--title` flags are reused: during `--init` mode, their values are written to the template instead of being used for capture. No separate `--init-process`/`--init-title` flags needed.

**Note on ClipboardMode:** The `ClipboardMode` enum is defined in `config.rs` (canonical location) and re-exported. `cli.rs` imports it from `config` for the clap `ValueEnum` derive. This avoids circular dependencies.

### config.rs

```rust
use serde::Deserialize;
use std::path::PathBuf;

/// Canonical definition — imported by cli.rs for clap ValueEnum derive
#[derive(Clone, Copy, Default, Deserialize)]
pub enum ClipboardMode {
    #[default]
    Path,
    Image,
    Both,
}

#[derive(Deserialize)]
pub struct TomlConfig {
    pub process: Option<String>,
    pub title: Option<String>,
    #[serde(default = "default_folder")]
    pub folder: String,
    #[serde(default)]
    pub clipboard: ClipboardMode,
    pub hotkey: Option<String>,
}

fn default_folder() -> String {
    "screenshots".to_string()
}

/// Merged configuration — everything needed for a capture
pub struct CaptureConfig {
    pub process: Option<String>,
    pub title: Option<String>,
    pub folder: String,
    pub clipboard: ClipboardMode,
    pub label: Option<String>,
    pub quiet: bool,
    pub verbose: bool,
    /// Where shot.toml was found, or CWD if CLI-only
    pub project_root: PathBuf,
}
```

### window_resolver.rs

```rust
#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub process_name: String,
    pub pid: u32,
}

/// Injectable window enumeration — enables testing without Win32
pub trait WindowEnumerator {
    fn enumerate(&self) -> Vec<WindowInfo>;
    fn get_foreground(&self) -> Option<isize>;
}

pub struct Win32Enumerator;
// Implements WindowEnumerator using EnumWindows + GetWindowThreadProcessId + OpenProcess
```

### capture.rs

```rust
pub struct CaptureResult {
    pub png_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
```

### storage.rs

```rust
pub struct SavedFile {
    pub path: PathBuf,
    pub filename: String,
}
```

## Error Handling

### errors.rs

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShotError {
    #[error("No window matching process \"{0}\" or title \"{1}\"")]
    WindowNotFound(Option<String>, Option<String>),

    #[error("Target window is minimized; restore it and try again")]
    WindowMinimized,

    #[error("{0}")]
    CaptureFailed(String),

    #[error("{0}")]
    StorageFailed(String),

    #[error("{0}")]
    ConfigError(String),

    #[error("{0}")]
    ClipboardError(String),

    #[error("{0}")]
    InitError(String),

    #[error("{0}")]
    ArgError(String),
}

impl ShotError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::WindowNotFound(..) => "window-not-found",
            Self::WindowMinimized => "window-minimized",
            Self::CaptureFailed(_) => "capture-failed",
            Self::StorageFailed(_) => "storage-failed",
            Self::ConfigError(_) => "config-error",
            Self::ClipboardError(_) => "clipboard-error",
            Self::InitError(_) => "init-error",
            Self::ArgError(_) => "arg-error",
        }
    }
}

/// Format error for stderr (PRD story 51)
pub fn format_error(err: &ShotError) -> String {
    format!("status: error\ncode: {}\nmessage: {}", err.code(), err)
}

/// Format success for stdout (PRD story 50)
pub fn format_success(file: &std::path::Path, window_title: &str, pid: u32, width: u32, height: u32) -> String {
    format!(
        "status: ok\nfile: {}\nwindow: {} (PID {})\nsize: {}x{}",
        file.display(), window_title, pid, width, height
    )
}
```

**Exit codes:** 0 for success, 1 for any error. The error code is communicated via the `code:` field in stderr output, not via distinct exit codes.

## Module Interfaces

### cli.rs — Public API

```rust
/// Parse command line arguments
pub fn parse() -> CliArgs;

/// Determine which mode to run based on flags
/// Enforces mutual exclusivity (PRD story 56)
pub fn determine_mode(args: &CliArgs) -> Result<Mode, ShotError>;
```

Mode determination priority: `--version` → `--init` → `--check` → `--list-windows` → `--watch` → Capture (default). If multiple mode flags set → `ShotError::ArgError`.

`--verbose` takes precedence over `--quiet` (PRD story 24/52).

### config.rs — Public API

```rust
/// Walk up from `start_dir` looking for shot.toml.
/// Stops at .git boundary (file or directory) or filesystem root.
/// Returns (parsed config, project_root directory) or None if not found.
pub fn find_config(start_dir: &Path) -> Result<Option<(TomlConfig, PathBuf)>, ShotError>;

/// Merge CLI args with optional TOML config.
/// CLI values override config values.
/// Returns error if neither config nor CLI provides process/title.
pub fn merge(cli: &CliArgs, toml: Option<(TomlConfig, PathBuf)>) -> Result<CaptureConfig, ShotError>;

/// Validate folder path: must be relative, no "..", no leading "\", no UNC, no device paths.
pub fn validate_folder(folder: &str) -> Result<(), ShotError>;

/// Create shot.toml template in CWD (--init mode).
/// Fails if shot.toml already exists.
/// After creating shot.toml, appends "screenshots/" to .gitignore.
pub fn init(process: Option<&str>, title: Option<&str>) -> Result<(), ShotError>;
```

Config walk-up: starts at CWD, checks for `shot.toml` in each directory, walks to parent, stops when `.git` entry (file OR directory) is found or at filesystem root. Lists all searched directories in error message if not found (PRD story 11). **`--init` never walks up** — always creates in CWD (PRD Implementation Decisions, Config module).

### window_resolver.rs — Public API

```rust
/// Resolve a single window matching the given criteria.
/// When both process and title provided: OR logic (match either).
/// When multiple matches: prefer foreground window, else first in z-order.
pub fn resolve(
    enumerator: &dyn WindowEnumerator,
    process: Option<&str>,
    title: Option<&str>,
) -> Result<WindowInfo, ShotError>;

/// List all visible top-level windows, excluding:
/// - Empty titles
/// - Invisible windows (WS_VISIBLE not set)
/// - Shell windows (Shell_TrayWnd, Progman class names)
pub fn list_all(enumerator: &dyn WindowEnumerator) -> Vec<WindowInfo>;
```

Title matching: case-insensitive, substring match anywhere in window title (PRD story 3).

### capture.rs — Public API

```rust
/// Capture a window as PNG.
/// Checks: window must be visible and not minimized.
/// Uses PrintWindow with PW_RENDERFULLCONTENT (0x00000002).
/// Converts HBITMAP to PNG bytes.
pub fn capture_window(hwnd: isize) -> Result<CaptureResult, ShotError>;
```

Returns `ShotError::WindowMinimized` if `IsIconic` returns true. Returns `ShotError::CaptureFailed` if `PrintWindow` fails (with message suggesting elevation or GPU rendering issue per PRD story 62).

### storage.rs — Public API

```rust
/// Save PNG bytes to disk.
/// Builds path: project_root / folder / YYYY-MM-DD_HHMMSS_<label>.png
/// Creates directory if it doesn't exist.
/// If label is None, uses sanitized window_title.
pub fn save(
    png_bytes: &[u8],
    project_root: &Path,
    folder: &str,
    label: Option<&str>,
    window_title: &str,
) -> Result<SavedFile, ShotError>;

/// Build the filename: YYYY-MM-DD_HHMMSS_<sanitized-label-or-title>.png
pub fn build_filename(label: Option<&str>, window_title: &str) -> String;

/// Sanitize invalid filename characters: : \ / * ? " < > | → -
pub fn sanitize_filename(s: &str) -> String;
```

### clipboard.rs — Public API

```rust
/// Write to clipboard based on mode.
/// Path mode: CF_UNICODETEXT with absolute file path.
/// Image mode: CF_DIB with device-independent bitmap.
/// Both mode: both formats simultaneously.
pub fn write_clipboard(
    mode: ClipboardMode,
    file_path: &Path,
    png_bytes: &[u8],
    width: u32,
    height: u32,
) -> Result<(), ShotError>;
```

Uses `CF_DIB` (not `CF_BITMAP`) because it's device-independent and works with browsers, Electron apps, and AI chat interfaces (PRD Implementation Decisions).

For `CF_DIB`: convert PNG to BITMAPINFOHEADER + raw pixel data. Allocate via `GlobalAlloc(GMEM_MOVEABLE, ...)`, write via `GlobalLock`/`GlobalUnlock`, set via `SetClipboardData`.

### audio.rs — Public API

```rust
/// Play success sound (SystemAsterisk) — PRD story 57
pub fn play_success();

/// Play error sound (SystemHand) — PRD story 58
pub fn play_error();
```

Both use `MessageBeep`. Fire-and-forget — errors are silently ignored.

## Data Flow — Capture Mode

```
1. main()
   ├── AttachConsole(ATTACH_PARENT_PROCESS) or AllocConsole()
   ├── cli::parse()
   ├── cli::determine_mode() → Mode::Capture
   │
   ├── config::find_config(cwd)
   │   └── walks up directories, parses shot.toml if found
   ├── config::merge(cli_args, toml_config)
   │   └── CLI overrides TOML, validates folder
   │
   ├── window_resolver::resolve(Win32Enumerator, process, title)
   │   ├── Win32Enumerator::enumerate()
   │   │   └── EnumWindows → GetWindowThreadProcessId → OpenProcess
   │   ├── filter by process name and/or title substring (OR logic)
   │   ├── Win32Enumerator::get_foreground()
   │   └── prefer foreground match, else first in z-order
   │
   ├── capture::capture_window(hwnd)
   │   ├── IsIconic check → WindowMinimized error
   │   ├── GetWindowRect → dimensions
   │   ├── CreateCompatibleDC, CreateCompatibleBitmap
   │   ├── PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)
   │   ├── GetDIBits → raw pixel data
   │   └── png::Encoder → PNG bytes
   │
   ├── storage::save(png_bytes, project_root, folder, label, title)
   │   ├── sanitize label or window title
   │   ├── build filename with timestamp
   │   ├── create directory if needed
   │   └── write PNG file
   │
   ├── clipboard::write_clipboard(mode, file_path, png_bytes, w, h)
   │   ├── OpenClipboard, EmptyClipboard
   │   ├── if path/both: SetClipboardData(CF_UNICODETEXT, ...)
   │   ├── if image/both: SetClipboardData(CF_DIB, ...)
   │   └── CloseClipboard
   │
   ├── audio::play_success()
   └── print format_success() to stdout (unless --quiet)

   On error at any step:
   ├── audio::play_error()
   ├── print format_error() to stderr
   └── exit(1)
```

## Data Flow — Other Modes

### --init

```
1. cli::determine_mode() → Mode::Init
2. Check shot.toml doesn't exist in CWD → error if it does
3. Generate template with inline comments
4. If --process or --title provided, uncomment and fill those lines
5. Write shot.toml to CWD
6. Append "screenshots/" to .gitignore (create if needed, skip if line already exists)
7. Print status to stdout
```

### --check

```
1. cli::determine_mode() → Mode::Check
2. config::find_config() + config::merge() — same as capture
3. window_resolver::resolve() — find the window
4. Print structured output to stdout:
   status: ok
   config: C:\path\to\shot.toml  (or "none (CLI args)")
   window: running (myapp.exe, PID 1234)
   folder: C:\path\to\screenshots
```

### --list-windows

```
1. cli::determine_mode() → Mode::ListWindows
2. window_resolver::list_all(Win32Enumerator)
3. For each window, print:
   title: <window title>
   process: <process name>
   pid: <process id>
   ---
```

### --version

```
1. Handled by clap's built-in --version flag
2. Prints: shot 0.1.0
```

## DPI Awareness

The executable must declare `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2` to capture screenshots at physical pixel resolution. Two mechanisms:

1. **Application manifest** (preferred) — `shot.manifest` embedded via `build.rs`:
   ```xml
   <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
   <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
     <application xmlns="urn:schemas-microsoft-com:asm.v3">
       <windowsSettings>
         <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">
           PerMonitorV2
         </dpiAwareness>
       </windowsSettings>
     </application>
   </assembly>
   ```

2. **Runtime fallback** — call `SetProcessDpiAwarenessContext` at startup if manifest embedding fails during build.

`build.rs` uses the `embed-resource` crate or direct `#[link_args]` to embed the manifest. If this proves complex, the runtime API call is sufficient for MVP.

## Console Attachment

`main.rs` starts with:

```rust
#![windows_subsystem = "windows"]

fn main() {
    // Attach to parent console for stdout/stderr
    // If no parent console (launched from Explorer), allocate one
    unsafe {
        use windows::Win32::System::Console::*;
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            let _ = AllocConsole();
        }
    }

    // Parse CLI, dispatch mode, handle errors
    match run() {
        Ok(()) => {}
        Err(e) => {
            audio::play_error();
            eprintln!("{}", errors::format_error(&e));
            std::process::exit(1);
        }
    }
}
```

**Trade-off (from PRD):** The very first bytes of stderr output may be lost before `AttachConsole` completes. Accepted limitation.

## Testing Strategy

### Unit Tests (in-module `#[cfg(test)]`)

**config.rs tests:**
- Valid TOML string → correct `TomlConfig` struct
- Missing both `process` and `title` → descriptive error
- `folder = "../outside"` → validation error
- `folder = "\\\\server\\share"` → validation error (UNC)
- `folder = "\\\\.\\pipe\\name"` → validation error (device path)
- `folder = "\\absolute"` → validation error
- Unknown key in TOML → does not panic (serde default behavior)
- Walk-up finds `shot.toml` in parent directory (tempdir)
- Walk-up stops at `.git` boundary (tempdir with `.git` dir)
- Walk-up stops at `.git` file (worktree scenario, tempdir with `.git` file)

**cli.rs tests:**
- `["shot"]` with no args → default mode is Capture
- `["shot", "my-label"]` → label = Some("my-label")
- `["shot", "my label"]` → label = Some("my label") (quoted)
- `["shot", "--clipboard=image"]` → clipboard override
- `["shot", "--verbose", "--quiet"]` → verbose wins (Mode logic test)
- `["shot", "--init", "--check"]` → ArgError (exclusive modes)
- `["shot", "--version"]` → Mode::Version

**window_resolver.rs tests (with MockEnumerator):**
- Process name match → correct WindowInfo returned
- Title substring match → correct WindowInfo
- Title match is case-insensitive
- Title match works mid-title ("MyApp" matches "Foo — MyApp — Bar")
- Both process and title → OR logic (either matches)
- No match → ShotError::WindowNotFound
- Multiple matches, foreground among them → foreground preferred
- Multiple matches, none foreground → first in list returned

**storage.rs tests:**
- `sanitize_filename("foo:bar\\baz")` → `"foo-bar-baz"`
- `build_filename(Some("login"), "ignored")` → contains "login"
- `build_filename(None, "My App: v2")` → contains "My-App--v2"
- Timestamp format in filename matches `YYYY-MM-DD_HHMMSS`
- save() creates directory if absent (tempdir)
- save() writes valid PNG bytes to correct path
- Two saves with same timestamp don't panic (different labels or overwrite)

**errors.rs tests:**
- `format_error` output contains all three fields: `status: error`, `code:`, `message:`
- `format_success` output contains all four fields: `status: ok`, `file:`, `window:`, `size:`
- Error code strings match PRD codes (`window-not-found`, `window-minimized`, etc.)

### Integration Tests (tests/ directory)

**init_integration.rs:**
- `--init` creates `shot.toml` with inline comments
- `--init --process=myapp.exe` produces pre-filled file
- `--init --title="MyApp"` produces pre-filled file
- `--init` fails if `shot.toml` already exists
- `--init` creates/appends `.gitignore` with `screenshots/`

**config_integration.rs:**
- Walk-up from subdirectory finds `shot.toml` in parent
- Walk-up stops at `.git` boundary

### Smoke Tests (agent manual execution)

After implementation, agent verifies:
1. `cargo build --release` → zero warnings
2. `cargo test` → all pass
3. `cargo clippy -- -D warnings` → no diagnostics
4. `shot.exe --version` → prints version
5. `shot.exe --list-windows` → `key: value` lines, no blank titles
6. `shot.exe --init` in temp directory → creates `shot.toml`
7. `shot.exe --check --process=notepad.exe` (with Notepad open) → `status: ok`

## Agent Instruction Files

### CLAUDE.md

```markdown
# CLAUDE.md — Axygen Shot

## Build / Test / Lint

cargo build                          # debug build
cargo build --release                # release build (LTO + strip)
cargo test                           # all unit + integration tests
cargo clippy -- -D warnings          # lint (zero warnings policy)
cargo fmt -- --check                 # format check

## Project structure

src/main.rs            — entry point, AttachConsole, mode dispatch
src/cli.rs             — clap derive struct, Mode enum
src/config.rs          — TOML parsing, walk-up discovery, merge, init
src/window_resolver.rs — WindowEnumerator trait, Win32 impl, resolve/list
src/capture.rs         — PrintWindow → HBITMAP → PNG
src/storage.rs         — filename generation, sanitization, file write
src/clipboard.rs       — CF_UNICODETEXT and/or CF_DIB
src/audio.rs           — MessageBeep wrappers
src/errors.rs          — ShotError enum, format_error/format_success

## Contracts

stdout output format is a contract — field names and order must not change:
  status: ok
  file: <absolute path>
  window: <title> (PID <pid>)
  size: <width>x<height>

stderr error format:
  status: error
  code: <error-code>
  message: <human-readable>

## Gotchas

- DPI awareness MUST be set before any window enumeration
- PrintWindow PW_RENDERFULLCONTENT = 0x00000002 (undocumented flag)
- AttachConsole must happen before any stdout/stderr output
- CF_DIB (not CF_BITMAP) for clipboard — device-independent
- windows crate feature flags may need adjustment — check docs.rs
- All modules are stateless — no global mutable state
```

### AGENTS.md

```markdown
# AGENTS.md — Axygen Shot

Refer to CLAUDE.md for full details. Key commands:

## Quick Reference

Build:  cargo build
Test:   cargo test
Lint:   cargo clippy -- -D warnings
Format: cargo fmt -- --check

## Module Map

cli → config → window_resolver → capture → storage → clipboard → audio
All modules in src/. Each file = one module.
WindowResolver uses trait-based DI (WindowEnumerator trait).

## stdout/stderr contract

Do NOT change field names or order in format_success/format_error output.
See CLAUDE.md for exact format.
```

## Implementation Notes

### windows crate feature flags

The feature flags listed in `Cargo.toml` are best-guess names for v0.62. The agent must verify them during first build. Common resolution pattern:
- If `Win32_Media_Audio` doesn't exist, try `Win32_Media` or search docs.rs
- The `windows` crate documents all features in its crate-level docs

### PNG encoding from HBITMAP

The capture flow is:
1. `CreateCompatibleDC(None)` — create memory DC
2. `CreateCompatibleBitmap(screen_dc, width, height)` — create bitmap
3. `SelectObject(mem_dc, bitmap)` — select into DC
4. `PrintWindow(hwnd, mem_dc, PW_RENDERFULLCONTENT)` — capture
5. `GetDIBits(mem_dc, bitmap, ...)` — get raw pixel data (BGRA)
6. Convert BGRA → RGBA, flip vertically (DIB is bottom-up)
7. `png::Encoder` → write PNG bytes to `Vec<u8>`

### CF_DIB clipboard format

For clipboard image mode:
1. Build `BITMAPINFOHEADER` struct (40 bytes)
2. Append raw pixel data (BGRA, bottom-up — DIB format)
3. `GlobalAlloc(GMEM_MOVEABLE, header_size + pixel_data_size)`
4. `GlobalLock` → copy header + pixels
5. `GlobalUnlock`
6. `SetClipboardData(CF_DIB, global_handle)`

### --init template content

```toml
# shot.toml — Axygen Shot configuration
#
# At least one of 'process' or 'title' must be uncommented.

# process: name of your app's .exe file (recommended).
# You already know this — it's in your Cargo.toml, .csproj, Makefile, etc.
# process = "myapp.exe"

# title: substring of the window title (alternative or complement to process).
# Matched as "title contains substring" — position-independent, works with dynamic titles.
# title   = "MyApp"

folder    = "screenshots"   # output subfolder (default: "screenshots")
clipboard = "path"          # "path" | "image" | "both" (default: "path")
# hotkey  = "Win+F12"       # watch mode hotkey (default: "Win+F12")
```

When `--process` or `--title` is provided with `--init`, uncomment and fill the corresponding line.

## Open Questions

- [ ] Exact `windows` crate v0.62 feature flag names — verify at first build
- [ ] Whether `embed-resource` crate is needed for manifest, or `#[link(name = "...")]` suffices
- [ ] Minimum Windows version required by `PW_RENDERFULLCONTENT` (documented as Windows 8.1+)
- [ ] Whether `chrono` is needed or `std::time::SystemTime` + manual formatting suffices
