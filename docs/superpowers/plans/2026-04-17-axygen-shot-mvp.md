# Axygen Shot MVP Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `shot.exe` — a portable Windows CLI screenshot tool for blind developers, covering configuration, window capture, storage, clipboard, and audio feedback (Phase 1 MVP).

**Architecture:** Functional pipeline in a single Rust binary. `main.rs` orchestrates: `cli → config → window_resolver → capture → storage → clipboard → audio`. All modules are stateless. `WindowResolver` uses trait-based DI for testability.

**Tech Stack:** Rust (edition 2024), clap v4, toml v0.8, serde v1, png v0.17, thiserror v2, chrono v0.4, windows v0.62 (added in Chunk 3)

**Spec:** `docs/superpowers/specs/2026-04-17-axygen-shot-mvp-design.md`
**PRD:** `docs/PRD.md`
**ADR:** `docs/ADR-001-tech-stack.md`

---

## File Structure

| File | Responsibility |
|------|---------------|
| `Cargo.toml` | Dependencies, binary `shot`, release profile |
| `build.rs` | Embed DPI-awareness manifest (Chunk 4) |
| `shot.manifest` | PerMonitorV2 DPI declaration (Chunk 4) |
| `src/main.rs` | Entry: AttachConsole, parse CLI, dispatch mode |
| `src/errors.rs` | ShotError enum, error codes, format functions |
| `src/config.rs` | ClipboardMode, TomlConfig, CaptureConfig, find/validate/merge/init |
| `src/cli.rs` | CliArgs (clap), Mode enum, parse(), determine_mode() |
| `src/storage.rs` | Filename sanitization, building, PNG file write |
| `src/audio.rs` | MessageBeep wrappers (success/error sounds) |
| `src/window_resolver.rs` | WindowEnumerator trait, Win32Enumerator, resolve(), list_all() |
| `src/capture.rs` | PrintWindow → HBITMAP → PNG bytes |
| `src/clipboard.rs` | CF_UNICODETEXT + CF_DIB clipboard |
| `CLAUDE.md` | Agent instructions (Claude Code) |
| `AGENTS.md` | Agent instructions (GitHub Copilot) |
| `tests/init_integration.rs` | --init file creation tests |
| `tests/config_integration.rs` | Walk-up with temp directories |

### Module dependencies

```
errors ← (used by all modules)
config ← cli, clipboard, main
cli ← main
storage ← main
audio ← main
window_resolver ← main
capture ← main
clipboard ← main
```

**Note:** The `windows` crate is NOT in Cargo.toml until Task 9. Pure Rust logic (errors, config, cli, storage) is built and tested first — this avoids Win32 feature-flag issues during early development.

## Chunk Overview

| Chunk | Tasks | Focus |
|-------|-------|-------|
| 1: Foundation | 1–4 | Scaffold, errors, config types, CLI parsing |
| 2: Config + Storage | 5–7 | Config validation/walk-up/merge/init, storage |
| 3: Win32 Modules | 8–12 | Audio, window resolver, capture, clipboard |
| 4: Orchestration | 13–16 | DPI manifest, main.rs wiring, agent docs, integration + smoke tests |

---

## Chunk 1: Foundation

Tasks 1–4 establish the project skeleton, error handling, config types, and CLI parsing. After this chunk, `cargo build` and `cargo test` pass with 22 unit tests.

**Spec deviations in this chunk:**

1. `WindowNotFound(String)` instead of `WindowNotFound(Option<String>, Option<String>)` — `Option<String>` doesn't impl `Display` (thiserror requirement). Caller pre-formats the message.
2. `Mode` enum omits `Version` variant — `--version` is handled by clap before `determine_mode()` runs. Dead code per YAGNI.

---

### Task 1: Project Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`, `src/errors.rs`, `src/config.rs`, `src/cli.rs`, `src/storage.rs`, `src/audio.rs`, `src/window_resolver.rs`, `src/capture.rs`, `src/clipboard.rs`
- Create: `CLAUDE.md`, `AGENTS.md`
- Create: `.gitignore` (if not present)

- [ ] **Step 1: Verify Rust toolchain**

Run: `rustc --version && cargo --version`
Expected: rustc 1.85+ (edition 2024 requires ≥1.85)

- [ ] **Step 2: Create `Cargo.toml`**

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

[dev-dependencies]
tempfile = "3"
assert_cmd = "2"

[profile.release]
lto = true
strip = true
codegen-units = 1
```

The `windows` crate is intentionally omitted — added in Task 9 when Win32 modules are implemented.

- [ ] **Step 3: Create `src/main.rs`**

```rust
#![allow(dead_code)] // Remove in Task 15 when all modules are wired

mod audio;
mod capture;
mod cli;
mod clipboard;
mod config;
mod errors;
mod storage;
mod window_resolver;

fn main() {
    println!("shot: not yet implemented");
}
```

- [ ] **Step 4: Create empty module stubs**

Create each file with a single placeholder comment:

| File | Content |
|------|---------|
| `src/errors.rs` | `// Implemented in Task 2` |
| `src/config.rs` | `// Implemented in Tasks 3, 5–7` |
| `src/cli.rs` | `// Implemented in Task 4` |
| `src/storage.rs` | `// Implemented in Task 8` |
| `src/audio.rs` | `// Implemented in Task 9` |
| `src/window_resolver.rs` | `// Implemented in Tasks 10–11` |
| `src/capture.rs` | `// Implemented in Task 12` |
| `src/clipboard.rs` | `// Implemented in Task 13` |

- [ ] **Step 5: Create `.gitignore`**

If `.gitignore` does not exist, create it. If it exists, ensure `/target/` is present:

```
/target/
```

- [ ] **Step 6: Create `CLAUDE.md`**

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

- [ ] **Step 7: Create `AGENTS.md`**

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

- [ ] **Step 8: Verify build**

Run: `cargo build`
Expected: Compiles with zero errors.

- [ ] **Step 9: Commit**

```
git add -A
git commit -m "chore: scaffold project structure

Cargo.toml, module stubs, CLAUDE.md, AGENTS.md.
Windows crate intentionally omitted — added when Win32 modules are implemented.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 2: errors.rs — Error Types and Formatting

**Files:**
- Modify: `src/errors.rs`

**Spec deviation:** `WindowNotFound(Option<String>, Option<String>)` → `WindowNotFound(String)`. Reason: `Option<String>` doesn't impl `Display`, which thiserror requires for `#[error("...{0}...")]`. The caller pre-formats the criteria string.

- [ ] **Step 1: Write failing tests**

Replace `src/errors.rs` with:

```rust
use std::path::Path;
use thiserror::Error;

// Implementation: Step 3

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_window_not_found() {
        let e = ShotError::WindowNotFound("test".into());
        assert_eq!(e.code(), "window-not-found");
    }

    #[test]
    fn code_window_minimized() {
        assert_eq!(ShotError::WindowMinimized.code(), "window-minimized");
    }

    #[test]
    fn code_capture_failed() {
        assert_eq!(ShotError::CaptureFailed("x".into()).code(), "capture-failed");
    }

    #[test]
    fn code_storage_failed() {
        assert_eq!(ShotError::StorageFailed("x".into()).code(), "storage-failed");
    }

    #[test]
    fn code_config_error() {
        assert_eq!(ShotError::ConfigError("x".into()).code(), "config-error");
    }

    #[test]
    fn code_clipboard_error() {
        assert_eq!(ShotError::ClipboardError("x".into()).code(), "clipboard-error");
    }

    #[test]
    fn code_init_error() {
        assert_eq!(ShotError::InitError("x".into()).code(), "init-error");
    }

    #[test]
    fn code_arg_error() {
        assert_eq!(ShotError::ArgError("x".into()).code(), "arg-error");
    }

    #[test]
    fn format_error_contains_all_fields() {
        let e = ShotError::CaptureFailed("gpu not available".into());
        let output = format_error(&e);
        assert!(output.contains("status: error"));
        assert!(output.contains("code: capture-failed"));
        assert!(output.contains("message: gpu not available"));
    }

    #[test]
    fn format_success_contains_all_fields() {
        let output = format_success(
            Path::new(r"C:\project\screenshots\test.png"),
            "Notepad",
            1234,
            1920,
            1080,
        );
        assert!(output.contains("status: ok"));
        assert!(output.contains(r"file: C:\project\screenshots\test.png"));
        assert!(output.contains("window: Notepad (PID 1234)"));
        assert!(output.contains("size: 1920x1080"));
    }
}
```

- [ ] **Step 2: Run tests — expect compile error**

Run: `cargo test errors::`
Expected: Compilation error — `ShotError` not defined.

- [ ] **Step 3: Implement ShotError**

Insert above `#[cfg(test)]` in `src/errors.rs`:

```rust
#[derive(Error, Debug)]
pub enum ShotError {
    #[error("{0}")]
    WindowNotFound(String),

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
            Self::WindowNotFound(_) => "window-not-found",
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
pub fn format_success(
    file: &Path,
    window_title: &str,
    pid: u32,
    width: u32,
    height: u32,
) -> String {
    format!(
        "status: ok\nfile: {}\nwindow: {} (PID {})\nsize: {}x{}",
        file.display(),
        window_title,
        pid,
        width,
        height,
    )
}
```

- [ ] **Step 4: Run tests — expect pass**

Run: `cargo test errors::`
Expected: 10 tests pass.

- [ ] **Step 5: Lint and format**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 6: Commit**

```
git add src/errors.rs
git commit -m "feat: implement ShotError with error codes and formatting

10 unit tests. WindowNotFound takes String (spec deviation: Option<String>
doesn't impl Display for thiserror).

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 3: config.rs — Data Types

**Files:**
- Modify: `src/config.rs`

Types only — no functions yet. Enables Task 4 (cli.rs) to import `ClipboardMode`.

- [ ] **Step 1: Write type definitions**

Replace `src/config.rs` with:

```rust
use clap::ValueEnum;
use serde::Deserialize;
use std::path::PathBuf;

/// Clipboard behavior — canonical definition.
/// Imported by cli.rs for clap ValueEnum derive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardMode {
    #[default]
    Path,
    Image,
    Both,
}

/// Raw TOML file representation
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

/// Merged config — everything needed for a capture operation
pub struct CaptureConfig {
    pub process: Option<String>,
    pub title: Option<String>,
    pub folder: String,
    pub clipboard: ClipboardMode,
    pub label: Option<String>,
    pub quiet: bool,
    pub verbose: bool,
    pub project_root: PathBuf,
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```
git add src/config.rs
git commit -m "feat: add config data types (ClipboardMode, TomlConfig, CaptureConfig)

Types only — functions follow in Tasks 5–7.
ClipboardMode derives serde::Deserialize + clap::ValueEnum.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 4: cli.rs — Argument Parsing and Mode Selection

**Files:**
- Modify: `src/cli.rs`

- [ ] **Step 1: Write failing tests**

Replace `src/cli.rs` with:

```rust
use clap::Parser;
use crate::config::ClipboardMode;
use crate::errors::ShotError;

// Implementation: Step 3

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse tests ---

    #[test]
    fn no_args_defaults() {
        let args = CliArgs::parse_from(["shot"]);
        assert!(args.label.is_none());
        assert!(args.process.is_none());
        assert!(args.title.is_none());
        assert!(!args.quiet);
        assert!(!args.verbose);
    }

    #[test]
    fn positional_label() {
        let args = CliArgs::parse_from(["shot", "my-label"]);
        assert_eq!(args.label.as_deref(), Some("my-label"));
    }

    #[test]
    fn quoted_label_with_spaces() {
        let args = CliArgs::parse_from(["shot", "my label"]);
        assert_eq!(args.label.as_deref(), Some("my label"));
    }

    #[test]
    fn clipboard_override() {
        let args = CliArgs::parse_from(["shot", "--clipboard=image"]);
        assert_eq!(args.clipboard, Some(ClipboardMode::Image));
    }

    #[test]
    fn process_and_title_flags() {
        let args = CliArgs::parse_from([
            "shot",
            "--process=notepad.exe",
            "--title=Untitled",
        ]);
        assert_eq!(args.process.as_deref(), Some("notepad.exe"));
        assert_eq!(args.title.as_deref(), Some("Untitled"));
    }

    // --- determine_mode tests ---

    #[test]
    fn default_mode_is_capture() {
        let args = CliArgs::parse_from(["shot"]);
        assert!(matches!(determine_mode(&args), Ok(Mode::Capture)));
    }

    #[test]
    fn init_mode() {
        let args = CliArgs::parse_from(["shot", "--init"]);
        assert!(matches!(determine_mode(&args), Ok(Mode::Init)));
    }

    #[test]
    fn check_mode() {
        let args = CliArgs::parse_from(["shot", "--check"]);
        assert!(matches!(determine_mode(&args), Ok(Mode::Check)));
    }

    #[test]
    fn list_windows_mode() {
        let args = CliArgs::parse_from(["shot", "--list-windows"]);
        assert!(matches!(determine_mode(&args), Ok(Mode::ListWindows)));
    }

    #[test]
    fn watch_mode() {
        let args = CliArgs::parse_from(["shot", "--watch"]);
        assert!(matches!(determine_mode(&args), Ok(Mode::Watch)));
    }

    #[test]
    fn multiple_mode_flags_error() {
        let args = CliArgs::parse_from(["shot", "--init", "--check"]);
        assert!(determine_mode(&args).is_err());
    }

    #[test]
    fn verbose_and_quiet_both_accepted() {
        // Precedence (verbose wins) is enforced in main.rs, not here
        let args = CliArgs::parse_from(["shot", "--verbose", "--quiet"]);
        assert!(args.verbose);
        assert!(args.quiet);
    }
}
```

- [ ] **Step 2: Run tests — expect compile error**

Run: `cargo test cli::`
Expected: Compilation error — `CliArgs`, `Mode`, `determine_mode` not defined.

- [ ] **Step 3: Implement CliArgs, Mode, parse, determine_mode**

Insert above `#[cfg(test)]` in `src/cli.rs`:

```rust
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

    /// Clipboard mode override
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

    /// Create shot.toml template in CWD
    #[arg(long)]
    pub init: bool,

    /// Watch mode (Phase 2 — returns error)
    #[arg(long)]
    pub watch: bool,

    /// Validate config and check window
    #[arg(long)]
    pub check: bool,

    /// List all visible windows
    #[arg(long)]
    pub list_windows: bool,
}

#[derive(Debug)]
pub enum Mode {
    Capture,
    Init,
    Check,
    ListWindows,
    Watch,
}

/// Parse command-line arguments
pub fn parse() -> CliArgs {
    CliArgs::parse()
}

/// Determine run mode from CLI flags. Error if multiple mode flags set.
pub fn determine_mode(args: &CliArgs) -> Result<Mode, ShotError> {
    let count =
        args.init as u8 + args.check as u8 + args.list_windows as u8 + args.watch as u8;
    if count > 1 {
        return Err(ShotError::ArgError(
            "Multiple mode flags set; use only one of --init, --check, --list-windows, --watch"
                .into(),
        ));
    }
    if args.init {
        return Ok(Mode::Init);
    }
    if args.check {
        return Ok(Mode::Check);
    }
    if args.list_windows {
        return Ok(Mode::ListWindows);
    }
    if args.watch {
        return Ok(Mode::Watch);
    }
    Ok(Mode::Capture)
}
```

- [ ] **Step 4: Run tests — expect pass**

Run: `cargo test cli::`
Expected: 12 tests pass.

- [ ] **Step 5: Lint and format**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: 22 tests pass (10 errors + 12 cli).

- [ ] **Step 7: Commit**

```
git add src/cli.rs
git commit -m "feat: implement CLI argument parsing and mode selection

12 unit tests. CliArgs (clap derive), Mode enum, determine_mode with
mutual exclusivity check. --version handled by clap built-in.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Chunk 2: Config + Storage

Tasks 5–7 complete the config module (validation, walk-up discovery, merge, init) and implement the storage module. After this chunk, `cargo test` passes with 62 unit tests.

**Spec deviations in this chunk:**

1. `init(dir: &Path, ...)` instead of `init(...)` using CWD — makes unit testing possible without mutating global state. Caller passes CWD.
2. Space character added to `sanitize_filename` replacement list — spec examples show "My App: v2" → "My-App--v2" (space → '-'), though the text only lists `: \ / * ? " < > |`.

---

### Task 5: config.rs — Validation and Config Discovery

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add imports and failing tests**

Update the imports at the top of `src/config.rs`. Change `use std::path::PathBuf;` to:

```rust
use std::path::{Path, PathBuf};
```

Add new import below the existing ones:

```rust
use crate::errors::ShotError;
```

Add test module at the end of `src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_folder ---

    #[test]
    fn valid_simple_folder() {
        assert!(validate_folder("screenshots").is_ok());
    }

    #[test]
    fn valid_nested_folder() {
        assert!(validate_folder("sub/folder").is_ok());
    }

    #[test]
    fn reject_parent_traversal() {
        assert!(validate_folder("../outside").is_err());
    }

    #[test]
    fn reject_embedded_parent_traversal() {
        assert!(validate_folder("foo/../bar").is_err());
    }

    #[test]
    fn reject_unc_path() {
        assert!(validate_folder("\\\\server\\share").is_err());
    }

    #[test]
    fn reject_device_path() {
        assert!(validate_folder("\\\\.\\pipe\\name").is_err());
    }

    #[test]
    fn reject_backslash_absolute() {
        assert!(validate_folder("\\absolute").is_err());
    }

    #[test]
    fn reject_drive_letter_absolute() {
        assert!(validate_folder("C:\\path").is_err());
    }

    #[test]
    fn reject_forward_slash_absolute() {
        assert!(validate_folder("/unix-style").is_err());
    }

    // --- find_config ---

    #[test]
    fn find_in_current_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("shot.toml"), "process = \"test.exe\"\n").unwrap();
        let result = find_config(dir.path()).unwrap();
        assert!(result.is_some());
        let (config, root) = result.unwrap();
        assert_eq!(config.process.as_deref(), Some("test.exe"));
        assert_eq!(root, dir.path());
    }

    #[test]
    fn find_in_parent_dir() {
        let parent = tempfile::tempdir().unwrap();
        let child = parent.path().join("subdir");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(parent.path().join("shot.toml"), "process = \"test.exe\"\n").unwrap();
        let result = find_config(&child).unwrap();
        assert!(result.is_some());
        let (_, root) = result.unwrap();
        assert_eq!(root, parent.path());
    }

    #[test]
    fn stops_at_git_directory() {
        let root = tempfile::tempdir().unwrap();
        let child = root.path().join("child");
        let grandchild = child.join("grandchild");
        std::fs::create_dir_all(&grandchild).unwrap();
        std::fs::write(root.path().join("shot.toml"), "process = \"test.exe\"\n").unwrap();
        std::fs::create_dir(child.join(".git")).unwrap();
        let result = find_config(&grandchild).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn stops_at_git_file() {
        let root = tempfile::tempdir().unwrap();
        let child = root.path().join("child");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(root.path().join("shot.toml"), "process = \"test.exe\"\n").unwrap();
        std::fs::write(child.join(".git"), "gitdir: ../other\n").unwrap();
        let result = find_config(&child).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn not_found_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        let result = find_config(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn invalid_toml_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("shot.toml"), "not valid toml [[[").unwrap();
        assert!(find_config(dir.path()).is_err());
    }

    #[test]
    fn unknown_keys_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("shot.toml"),
            "process = \"test.exe\"\nunknown_key = \"value\"\n",
        )
        .unwrap();
        assert!(find_config(dir.path()).unwrap().is_some());
    }
}
```

- [ ] **Step 2: Run tests — expect compile error**

Run: `cargo test config::`
Expected: Compilation error — `validate_folder` and `find_config` not defined.

- [ ] **Step 3: Implement validate_folder and find_config**

Add above the `#[cfg(test)]` module in `src/config.rs`:

```rust
/// Validate folder: must be relative, no "..", no UNC, no device paths.
pub fn validate_folder(folder: &str) -> Result<(), ShotError> {
    if folder.contains("..") {
        return Err(ShotError::ConfigError(
            format!("Folder must not contain '..': \"{}\"", folder),
        ));
    }
    if folder.starts_with('\\') || folder.starts_with('/') || Path::new(folder).is_absolute() {
        return Err(ShotError::ConfigError(
            format!("Folder must be a relative path: \"{}\"", folder),
        ));
    }
    Ok(())
}

/// Walk up from start_dir looking for shot.toml.
/// Stops at .git boundary (file or directory) or filesystem root.
pub fn find_config(start_dir: &Path) -> Result<Option<(TomlConfig, PathBuf)>, ShotError> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let config_path = dir.join("shot.toml");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| ShotError::ConfigError(
                    format!("Cannot read {}: {}", config_path.display(), e),
                ))?;
            let config: TomlConfig = toml::from_str(&content)
                .map_err(|e| ShotError::ConfigError(
                    format!("Invalid TOML in {}: {}", config_path.display(), e),
                ))?;
            return Ok(Some((config, dir)));
        }
        if dir.join(".git").exists() {
            return Ok(None);
        }
        if !dir.pop() {
            return Ok(None);
        }
    }
}
```

- [ ] **Step 4: Run tests — expect pass**

Run: `cargo test config::`
Expected: 16 tests pass (9 validate_folder + 7 find_config).

- [ ] **Step 5: Lint and format**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 6: Commit**

```
git add src/config.rs
git commit -m "feat: implement config validation and walk-up discovery

16 unit tests. validate_folder rejects absolute/UNC/device/.. paths.
find_config walks up directories, stops at .git boundary.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 6: config.rs — Merge and Init

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add imports and failing merge tests**

Add import at the top of `src/config.rs` (with other imports):

```rust
use crate::cli::CliArgs;
```

Add these imports inside the existing `mod tests { ... }` block, after `use super::*;`:

```rust
    use crate::cli::CliArgs;
    use clap::Parser;
```

Add merge tests inside the existing `mod tests { ... }` block:

```rust
    // --- merge ---

    #[test]
    fn merge_cli_overrides_toml_process() {
        let cli = CliArgs::parse_from(["shot", "--process=cli.exe"]);
        let toml_cfg = TomlConfig {
            process: Some("toml.exe".into()),
            title: None,
            folder: "screenshots".into(),
            clipboard: ClipboardMode::Path,
            hotkey: None,
        };
        let dir = tempfile::tempdir().unwrap();
        let config = merge(&cli, Some((toml_cfg, dir.path().to_path_buf()))).unwrap();
        assert_eq!(config.process.as_deref(), Some("cli.exe"));
    }

    #[test]
    fn merge_toml_provides_defaults() {
        let cli = CliArgs::parse_from(["shot"]);
        let toml_cfg = TomlConfig {
            process: Some("toml.exe".into()),
            title: None,
            folder: "output".into(),
            clipboard: ClipboardMode::Image,
            hotkey: None,
        };
        let dir = tempfile::tempdir().unwrap();
        let config = merge(&cli, Some((toml_cfg, dir.path().to_path_buf()))).unwrap();
        assert_eq!(config.process.as_deref(), Some("toml.exe"));
        assert_eq!(config.folder, "output");
        assert_eq!(config.clipboard, ClipboardMode::Image);
    }

    #[test]
    fn merge_no_target_with_config_errors() {
        let cli = CliArgs::parse_from(["shot"]);
        let toml_cfg = TomlConfig {
            process: None,
            title: None,
            folder: "screenshots".into(),
            clipboard: ClipboardMode::Path,
            hotkey: None,
        };
        let dir = tempfile::tempdir().unwrap();
        assert!(merge(&cli, Some((toml_cfg, dir.path().to_path_buf()))).is_err());
    }

    #[test]
    fn merge_no_target_no_config_errors() {
        let cli = CliArgs::parse_from(["shot"]);
        assert!(merge(&cli, None).is_err());
    }

    #[test]
    fn merge_cli_folder_overrides_toml() {
        let cli = CliArgs::parse_from(["shot", "--process=test.exe", "--folder=custom"]);
        let toml_cfg = TomlConfig {
            process: None,
            title: None,
            folder: "screenshots".into(),
            clipboard: ClipboardMode::Path,
            hotkey: None,
        };
        let dir = tempfile::tempdir().unwrap();
        let config = merge(&cli, Some((toml_cfg, dir.path().to_path_buf()))).unwrap();
        assert_eq!(config.folder, "custom");
    }

    #[test]
    fn merge_invalid_folder_errors() {
        let cli = CliArgs::parse_from(["shot", "--process=test.exe", "--folder=../bad"]);
        assert!(merge(&cli, None).is_err());
    }

    #[test]
    fn merge_default_clipboard_is_path() {
        let cli = CliArgs::parse_from(["shot", "--process=test.exe"]);
        let config = merge(&cli, None).unwrap();
        assert_eq!(config.clipboard, ClipboardMode::Path);
    }
```

- [ ] **Step 2: Run merge tests — expect compile error**

Run: `cargo test config::tests::merge`
Expected: Compilation error — `merge` not defined.

- [ ] **Step 3: Implement merge**

Add to `src/config.rs` (after `find_config`, before `#[cfg(test)]`):

```rust
/// Merge CLI args with optional TOML config. CLI values override TOML.
pub fn merge(cli: &CliArgs, toml: Option<(TomlConfig, PathBuf)>) -> Result<CaptureConfig, ShotError> {
    let (toml_config, project_root) = match toml {
        Some((tc, root)) => (Some(tc), root),
        None => (
            None,
            std::env::current_dir()
                .map_err(|e| ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?,
        ),
    };

    let process = cli
        .process
        .clone()
        .or(toml_config.as_ref().and_then(|t| t.process.clone()));
    let title = cli
        .title
        .clone()
        .or(toml_config.as_ref().and_then(|t| t.title.clone()));

    if process.is_none() && title.is_none() {
        return Err(ShotError::ConfigError(
            "No target: provide --process or --title (or set in shot.toml)".into(),
        ));
    }

    let folder = cli.folder.clone().unwrap_or_else(|| {
        toml_config
            .as_ref()
            .map(|t| t.folder.clone())
            .unwrap_or_else(|| "screenshots".to_string())
    });
    validate_folder(&folder)?;

    let clipboard = cli.clipboard.unwrap_or_else(|| {
        toml_config
            .as_ref()
            .map(|t| t.clipboard)
            .unwrap_or_default()
    });

    Ok(CaptureConfig {
        process,
        title,
        folder,
        clipboard,
        label: cli.label.clone(),
        quiet: cli.quiet,
        verbose: cli.verbose,
        project_root,
    })
}
```

- [ ] **Step 4: Run merge tests — expect pass**

Run: `cargo test config::tests::merge`
Expected: 7 tests pass.

- [ ] **Step 5: Write failing init tests**

Add init tests inside the existing `mod tests { ... }` block:

```rust
    // --- init ---

    #[test]
    fn init_creates_shot_toml() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), None, None).unwrap();
        let path = dir.path().join("shot.toml");
        assert!(path.exists());
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("folder"));
        assert!(content.contains("clipboard"));
    }

    #[test]
    fn init_with_process() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), Some("myapp.exe"), None).unwrap();
        let content = std::fs::read_to_string(dir.path().join("shot.toml")).unwrap();
        assert!(content.lines().any(|l| l == r#"process = "myapp.exe""#));
    }

    #[test]
    fn init_with_title() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), None, Some("MyApp")).unwrap();
        let content = std::fs::read_to_string(dir.path().join("shot.toml")).unwrap();
        assert!(content.lines().any(|l| l == r#"title   = "MyApp""#));
    }

    #[test]
    fn init_fails_if_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("shot.toml"), "existing").unwrap();
        assert!(init(dir.path(), None, None).is_err());
    }

    #[test]
    fn init_creates_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path(), None, None).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains("screenshots/"));
    }

    #[test]
    fn init_appends_to_existing_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "node_modules/\n").unwrap();
        init(dir.path(), None, None).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains("node_modules/"));
        assert!(content.contains("screenshots/"));
    }

    #[test]
    fn init_skips_duplicate_gitignore_entry() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "screenshots/\n").unwrap();
        init(dir.path(), None, None).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(content.matches("screenshots/").count(), 1);
    }
```

- [ ] **Step 6: Run init tests — expect compile error**

Run: `cargo test config::tests::init`
Expected: Compilation error — `init` not defined.

- [ ] **Step 7: Implement init**

Add to `src/config.rs` (after `merge`, before `#[cfg(test)]`):

```rust
/// Create shot.toml template. Takes dir param for testability (spec uses CWD).
pub fn init(dir: &Path, process: Option<&str>, title: Option<&str>) -> Result<(), ShotError> {
    let config_path = dir.join("shot.toml");
    if config_path.exists() {
        return Err(ShotError::InitError("shot.toml already exists".into()));
    }

    let process_line = match process {
        Some(p) => format!("process = \"{}\"", p),
        None => "\
# process: name of your app's .exe file (recommended).\n\
# You already know this — it's in your Cargo.toml, .csproj, Makefile, etc.\n\
# process = \"myapp.exe\"".to_string(),
    };
    let title_line = match title {
        Some(t) => format!("title   = \"{}\"", t),
        None => "\
# title: substring of the window title (alternative or complement to process).\n\
# Matched as \"title contains substring\" — position-independent, works with dynamic titles.\n\
# title   = \"MyApp\"".to_string(),
    };

    let template = format!(
        "\
# shot.toml — Axygen Shot configuration
#
# At least one of 'process' or 'title' must be uncommented.

{process_line}

{title_line}

folder    = \"screenshots\"   # output subfolder (default: \"screenshots\")
clipboard = \"path\"          # \"path\" | \"image\" | \"both\" (default: \"path\")
# hotkey  = \"Win+F12\"       # watch mode hotkey (default: \"Win+F12\")
"
    );

    std::fs::write(&config_path, &template)
        .map_err(|e| ShotError::InitError(format!("Cannot write shot.toml: {}", e)))?;

    // Append "screenshots/" to .gitignore
    let gitignore_path = dir.join(".gitignore");
    let entry = "screenshots/";
    let already_present = gitignore_path.exists() && {
        let content = std::fs::read_to_string(&gitignore_path)
            .map_err(|e| ShotError::InitError(format!("Cannot read .gitignore: {}", e)))?;
        content.lines().any(|line| line.trim() == entry)
    };

    if !already_present {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&gitignore_path)
            .map_err(|e| ShotError::InitError(format!("Cannot write .gitignore: {}", e)))?;
        writeln!(file, "{}", entry)
            .map_err(|e| ShotError::InitError(format!("Cannot write .gitignore: {}", e)))?;
    }

    Ok(())
}
```

- [ ] **Step 8: Run all config tests — expect pass**

Run: `cargo test config::`
Expected: 30 tests pass (16 from Task 5 + 7 merge + 7 init).

- [ ] **Step 9: Lint and format**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 10: Commit**

```
git add src/config.rs
git commit -m "feat: implement config merge and init

14 unit tests. merge() combines CLI + TOML with CLI precedence.
init() creates template shot.toml and updates .gitignore.
Spec deviation: init() takes dir param for testability.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 7: storage.rs — File Naming and Saving

**Files:**
- Modify: `src/storage.rs`

- [ ] **Step 1: Write failing tests**

Replace `src/storage.rs` with:

```rust
use std::path::{Path, PathBuf};
use crate::errors::ShotError;

// Implementation: Step 3

#[cfg(test)]
mod tests {
    use super::*;

    // --- sanitize_filename ---

    #[test]
    fn sanitize_replaces_special_chars() {
        assert_eq!(sanitize_filename("foo:bar\\baz"), "foo-bar-baz");
    }

    #[test]
    fn sanitize_preserves_normal_chars() {
        assert_eq!(sanitize_filename("hello-world_2024"), "hello-world_2024");
    }

    #[test]
    fn sanitize_all_special_chars() {
        assert_eq!(
            sanitize_filename("a/b*c?d\"e<f>g|h:i\\j k"),
            "a-b-c-d-e-f-g-h-i-j-k",
        );
    }

    // --- build_filename ---

    #[test]
    fn build_filename_with_label() {
        let name = build_filename(Some("login"), "Ignored Title");
        assert!(name.contains("login"));
        assert!(name.ends_with(".png"));
    }

    #[test]
    fn build_filename_without_label_uses_title() {
        let name = build_filename(None, "My App: v2");
        assert!(name.contains("My-App--v2"));
        assert!(name.ends_with(".png"));
    }

    #[test]
    fn build_filename_has_timestamp_prefix() {
        let name = build_filename(Some("test"), "title");
        // Format: YYYY-MM-DD_HHMMSS_test.png
        assert!(name.chars().nth(4) == Some('-'));  // YYYY-
        assert!(name.chars().nth(7) == Some('-'));  // MM-
        assert!(name.chars().nth(10) == Some('_')); // DD_
    }

    // --- save ---

    #[test]
    fn save_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let result = save(b"fake png", dir.path(), "output", Some("test"), "Title").unwrap();
        assert!(result.path.exists());
        assert!(dir.path().join("output").is_dir());
    }

    #[test]
    fn save_writes_correct_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let data = b"fake png data";
        let result = save(data, dir.path(), "screenshots", Some("test"), "Title").unwrap();
        assert_eq!(std::fs::read(&result.path).unwrap(), data);
    }

    #[test]
    fn save_filename_contains_label() {
        let dir = tempfile::tempdir().unwrap();
        let result = save(b"data", dir.path(), "screenshots", Some("my-label"), "Title").unwrap();
        assert!(result.filename.contains("my-label"));
        assert!(result.filename.ends_with(".png"));
    }

    #[test]
    fn two_saves_same_timestamp_dont_panic() {
        let dir = tempfile::tempdir().unwrap();
        let r1 = save(b"one", dir.path(), "screenshots", Some("alpha"), "Title");
        let r2 = save(b"two", dir.path(), "screenshots", Some("beta"), "Title");
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert_ne!(r1.unwrap().filename, r2.unwrap().filename);
    }
}
```

- [ ] **Step 2: Run tests — expect compile error**

Run: `cargo test storage::`
Expected: Compilation error — `SavedFile`, `sanitize_filename`, etc. not defined.

- [ ] **Step 3: Implement storage module**

Insert above `#[cfg(test)]` in `src/storage.rs`:

```rust
pub struct SavedFile {
    pub path: PathBuf,
    pub filename: String,
}

/// Replace invalid filename characters (and spaces) with hyphens.
/// Characters: : \ / * ? " < > | (space)
pub fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ':' | '\\' | '/' | '*' | '?' | '"' | '<' | '>' | '|' | ' ' => '-',
            _ => c,
        })
        .collect()
}

/// Build filename: YYYY-MM-DD_HHMMSS_<sanitized>.png
pub fn build_filename(label: Option<&str>, window_title: &str) -> String {
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let name_part = match label {
        Some(l) => sanitize_filename(l),
        None => sanitize_filename(window_title),
    };
    format!("{}_{}.png", timestamp, name_part)
}

/// Save PNG bytes to disk. Creates directory if absent.
pub fn save(
    png_bytes: &[u8],
    project_root: &Path,
    folder: &str,
    label: Option<&str>,
    window_title: &str,
) -> Result<SavedFile, ShotError> {
    let dir = project_root.join(folder);
    std::fs::create_dir_all(&dir)
        .map_err(|e| ShotError::StorageFailed(
            format!("Cannot create {}: {}", dir.display(), e),
        ))?;

    let filename = build_filename(label, window_title);
    let path = dir.join(&filename);

    std::fs::write(&path, png_bytes)
        .map_err(|e| ShotError::StorageFailed(
            format!("Cannot write {}: {}", path.display(), e),
        ))?;

    Ok(SavedFile { path, filename })
}
```

- [ ] **Step 4: Run tests — expect pass**

Run: `cargo test storage::`
Expected: 10 tests pass.

- [ ] **Step 5: Run all tests, lint, format**

Run: `cargo test && cargo clippy -- -D warnings && cargo fmt -- --check`
Expected: 62 tests pass. No lint warnings. No format issues.

- [ ] **Step 6: Commit**

```
git add src/storage.rs
git commit -m "feat: implement file naming, sanitization, and save

10 unit tests. sanitize_filename replaces invalid chars + spaces with hyphens.
build_filename generates YYYY-MM-DD_HHMMSS_<label>.png format.
save() creates directories and writes PNG bytes.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Chunk 3: Win32 Modules

Tasks 8–12 add the `windows` crate dependency and implement all Win32-based modules: audio, window resolver (with trait-based DI and 8 unit tests), capture, and clipboard. After this chunk, `cargo test` passes with 70 tests.

**Note:** Win32 modules (audio, capture, clipboard, Win32Enumerator) cannot be unit-tested — they require real hardware/windows. Only `window_resolver::resolve()` and `list_all()` have unit tests via `MockEnumerator`.

---

### Task 8: Add windows Crate + audio.rs

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/audio.rs`

- [ ] **Step 1: Add windows crate to Cargo.toml**

Add the `[dependencies.windows]` section to `Cargo.toml` after the `chrono` dependency:

```toml
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
```

**Note:** Feature flag names are best-guess for v0.62. If any fail, check `docs.rs/windows` and fix. Common issue: `Win32_Media_Audio` might be `Win32_Media` instead.

- [ ] **Step 2: Implement audio.rs**

Replace `src/audio.rs` with:

```rust
use windows::Win32::UI::WindowsAndMessaging::{MessageBeep, MB_ICONASTERISK, MB_ICONHAND};

/// Play success sound (SystemAsterisk) — PRD story 57.
/// Fire-and-forget: errors silently ignored.
pub fn play_success() {
    unsafe {
        let _ = MessageBeep(MB_ICONASTERISK);
    }
}

/// Play error sound (SystemHand) — PRD story 58.
/// Fire-and-forget: errors silently ignored.
pub fn play_error() {
    unsafe {
        let _ = MessageBeep(MB_ICONHAND);
    }
}
```

- [ ] **Step 3: Verify build**

Run: `cargo build`
Expected: Compiles successfully. This validates all windows crate feature flags.

If feature flags fail, check `docs.rs/windows/0.62.0` for correct names and fix `Cargo.toml` before proceeding.

- [ ] **Step 4: Run all existing tests**

Run: `cargo test`
Expected: 62 tests pass (audio has no tests — fire-and-forget functions).

- [ ] **Step 5: Lint and format**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 6: Commit**

```
git add Cargo.toml src/audio.rs
git commit -m "feat: add windows crate and implement audio feedback

MessageBeep wrappers: play_success (SystemAsterisk), play_error (SystemHand).
Fire-and-forget — errors silently ignored (PRD stories 57-58).
windows crate v0.62 feature flags validated by successful build.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 9: window_resolver.rs — Types, Trait, and Pure Logic (TDD)

**Files:**
- Modify: `src/window_resolver.rs`

- [ ] **Step 1: Write types, trait, and failing tests**

Replace `src/window_resolver.rs` with:

```rust
use crate::errors::ShotError;

#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub process_name: String,
    pub pid: u32,
}

/// Injectable window enumeration — enables testing without Win32.
pub trait WindowEnumerator {
    fn enumerate(&self) -> Vec<WindowInfo>;
    fn get_foreground(&self) -> Option<isize>;
}

// resolve() and list_all(): Step 3

#[cfg(test)]
mod tests {
    use super::*;

    struct MockEnumerator {
        windows: Vec<WindowInfo>,
        foreground_hwnd: Option<isize>,
    }

    impl WindowEnumerator for MockEnumerator {
        fn enumerate(&self) -> Vec<WindowInfo> {
            self.windows.clone()
        }
        fn get_foreground(&self) -> Option<isize> {
            self.foreground_hwnd
        }
    }

    fn make_window(hwnd: isize, title: &str, process: &str, pid: u32) -> WindowInfo {
        WindowInfo {
            hwnd,
            title: title.to_string(),
            process_name: process.to_string(),
            pid,
        }
    }

    #[test]
    fn resolve_by_process_name() {
        let mock = MockEnumerator {
            windows: vec![make_window(1, "Title", "notepad.exe", 100)],
            foreground_hwnd: None,
        };
        let result = resolve(&mock, Some("notepad.exe"), None).unwrap();
        assert_eq!(result.hwnd, 1);
        assert_eq!(result.process_name, "notepad.exe");
    }

    #[test]
    fn resolve_by_title_substring() {
        let mock = MockEnumerator {
            windows: vec![make_window(2, "My Document - Notepad", "notepad.exe", 101)],
            foreground_hwnd: None,
        };
        let result = resolve(&mock, None, Some("Document")).unwrap();
        assert_eq!(result.hwnd, 2);
    }

    #[test]
    fn title_match_is_case_insensitive() {
        let mock = MockEnumerator {
            windows: vec![make_window(3, "UPPERCASE TITLE", "app.exe", 102)],
            foreground_hwnd: None,
        };
        let result = resolve(&mock, None, Some("uppercase")).unwrap();
        assert_eq!(result.hwnd, 3);
    }

    #[test]
    fn title_match_works_mid_title() {
        let mock = MockEnumerator {
            windows: vec![make_window(4, "Foo — MyApp — Bar", "app.exe", 103)],
            foreground_hwnd: None,
        };
        let result = resolve(&mock, None, Some("MyApp")).unwrap();
        assert_eq!(result.hwnd, 4);
    }

    #[test]
    fn both_process_and_title_uses_or_logic() {
        let mock = MockEnumerator {
            windows: vec![
                make_window(5, "Unrelated", "notepad.exe", 200),
                make_window(6, "Target Title", "other.exe", 201),
            ],
            foreground_hwnd: None,
        };
        // Process match → finds window 5
        let r1 = resolve(&mock, Some("notepad.exe"), Some("Target")).unwrap();
        assert!(r1.hwnd == 5 || r1.hwnd == 6);
    }

    #[test]
    fn no_match_returns_error() {
        let mock = MockEnumerator {
            windows: vec![make_window(7, "Other", "other.exe", 300)],
            foreground_hwnd: None,
        };
        assert!(resolve(&mock, Some("missing.exe"), None).is_err());
    }

    #[test]
    fn multiple_matches_prefers_foreground() {
        let mock = MockEnumerator {
            windows: vec![
                make_window(8, "Title A", "app.exe", 400),
                make_window(9, "Title B", "app.exe", 401),
            ],
            foreground_hwnd: Some(9),
        };
        let result = resolve(&mock, Some("app.exe"), None).unwrap();
        assert_eq!(result.hwnd, 9);
    }

    #[test]
    fn multiple_matches_no_foreground_returns_first() {
        let mock = MockEnumerator {
            windows: vec![
                make_window(10, "Title A", "app.exe", 500),
                make_window(11, "Title B", "app.exe", 501),
            ],
            foreground_hwnd: Some(99), // Not among matches
        };
        let result = resolve(&mock, Some("app.exe"), None).unwrap();
        assert_eq!(result.hwnd, 10);
    }
}
```

- [ ] **Step 2: Run tests — expect compile error**

Run: `cargo test window_resolver::`
Expected: Compilation error — `resolve` not defined.

- [ ] **Step 3: Implement resolve and list_all**

Add above the `#[cfg(test)]` module in `src/window_resolver.rs`:

```rust
/// Resolve a single window matching the given criteria.
/// When both process and title provided: OR logic (match either).
/// When multiple matches: prefer foreground window, else first in z-order.
pub fn resolve(
    enumerator: &dyn WindowEnumerator,
    process: Option<&str>,
    title: Option<&str>,
) -> Result<WindowInfo, ShotError> {
    let all = enumerator.enumerate();
    let matches: Vec<&WindowInfo> = all
        .iter()
        .filter(|w| {
            let process_match = process
                .map(|p| w.process_name.eq_ignore_ascii_case(p))
                .unwrap_or(false);
            let title_match = title
                .map(|t| w.title.to_lowercase().contains(&t.to_lowercase()))
                .unwrap_or(false);
            process_match || title_match
        })
        .collect();

    if matches.is_empty() {
        let target = format!(
            "process={}, title={}",
            process.unwrap_or("(none)"),
            title.unwrap_or("(none)"),
        );
        return Err(ShotError::WindowNotFound(target));
    }

    let foreground = enumerator.get_foreground();
    if let Some(fg_hwnd) = foreground {
        if let Some(fg_match) = matches.iter().find(|w| w.hwnd == fg_hwnd) {
            return Ok((*fg_match).clone());
        }
    }

    Ok(matches[0].clone())
}

/// List all visible top-level windows.
/// Excludes empty titles and shell windows.
pub fn list_all(enumerator: &dyn WindowEnumerator) -> Vec<WindowInfo> {
    enumerator
        .enumerate()
        .into_iter()
        .filter(|w| !w.title.is_empty())
        .collect()
}
```

- [ ] **Step 4: Run tests — expect pass**

Run: `cargo test window_resolver::`
Expected: 8 tests pass.

- [ ] **Step 5: Lint and format**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 6: Commit**

```
git add src/window_resolver.rs
git commit -m "feat: implement window resolver types, trait, and matching logic

8 unit tests with MockEnumerator. WindowInfo struct, WindowEnumerator trait.
resolve() matches by process name (exact, case-insensitive) or title
(substring, case-insensitive) with OR logic. Prefers foreground window.
list_all() filters out empty-title windows.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 10: window_resolver.rs — Win32Enumerator

**Files:**
- Modify: `src/window_resolver.rs`

- [ ] **Step 1: Add Win32 imports**

Add at the top of `src/window_resolver.rs`:

```rust
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, LPARAM, MAX_PATH};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible,
};
```

- [ ] **Step 2: Implement Win32Enumerator**

Add after the `list_all` function, before `#[cfg(test)]`:

```rust
pub struct Win32Enumerator;

impl WindowEnumerator for Win32Enumerator {
    fn enumerate(&self) -> Vec<WindowInfo> {
        let mut windows: Vec<WindowInfo> = Vec::new();
        unsafe {
            let _ = EnumWindows(
                Some(enum_callback),
                LPARAM(&mut windows as *mut Vec<WindowInfo> as isize),
            );
        }
        windows
    }

    fn get_foreground(&self) -> Option<isize> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                None
            } else {
                Some(hwnd.0 as isize)
            }
        }
    }
}

unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);

    // Skip invisible windows
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1); // Continue enumeration
    }

    // Get window title
    let title_len = GetWindowTextLengthW(hwnd);
    if title_len == 0 {
        return BOOL(1); // Skip empty titles
    }
    let mut title_buf = vec![0u16; (title_len + 1) as usize];
    GetWindowTextW(hwnd, &mut title_buf);
    let title = OsString::from_wide(&title_buf[..title_len as usize])
        .to_string_lossy()
        .to_string();

    if title.is_empty() {
        return BOOL(1);
    }

    // Skip shell windows (taskbar, desktop)
    let mut class_buf = [0u16; 256];
    let class_len = GetClassNameW(hwnd, &mut class_buf);
    if class_len > 0 {
        let class_name = OsString::from_wide(&class_buf[..class_len as usize])
            .to_string_lossy()
            .to_string();
        if class_name == "Shell_TrayWnd" || class_name == "Progman" {
            return BOOL(1);
        }
    }

    // Get process ID
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));

    // Get process name
    let process_name = get_process_name(pid).unwrap_or_default();

    windows.push(WindowInfo {
        hwnd: hwnd.0 as isize,
        title,
        process_name,
        pid,
    });

    BOOL(1) // Continue enumeration
}

fn get_process_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; MAX_PATH as usize];
        let mut size = buf.len() as u32;
        let result = QueryFullProcessImageNameW(handle, PROCESS_NAME_FORMAT(0), &mut buf, &mut size);
        let _ = CloseHandle(handle);
        result.ok()?;
        let path = OsString::from_wide(&buf[..size as usize])
            .to_string_lossy()
            .to_string();
        // Extract filename from full path
        path.rsplit('\\').next().map(|s| s.to_string())
    }
}
```

- [ ] **Step 3: Verify build**

Run: `cargo build`
Expected: Compiles successfully.

**Note:** If Win32 API signatures don't match `windows` v0.62, fix imports based on docs.rs error messages. Common issues: `HWND` inner type, `BOOL` return vs `Result`, `LPARAM` constructor.

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: 70 tests pass (62 existing + 8 window_resolver unit tests).

- [ ] **Step 5: Lint and format**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 6: Commit**

```
git add src/window_resolver.rs
git commit -m "feat: implement Win32Enumerator with EnumWindows

Win32Enumerator implements WindowEnumerator trait using real Win32 APIs.
EnumWindows callback filters invisible/empty-title windows.
get_process_name extracts exe name via OpenProcess + QueryFullProcessImageName.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 11: capture.rs — Window Capture to PNG

**Files:**
- Modify: `src/capture.rs`

- [ ] **Step 1: Implement capture module**

Replace `src/capture.rs` with:

```rust
use crate::errors::ShotError;
use std::mem;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits,
    GetWindowDC, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, IsIconic, PrintWindow, PRINT_WINDOW_FLAGS,
};

/// PW_RENDERFULLCONTENT — undocumented flag for full window capture.
const PW_RENDERFULLCONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(0x00000002);

pub struct CaptureResult {
    pub png_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Capture a window as PNG bytes.
/// Returns WindowMinimized if target is iconic.
/// Returns CaptureFailed if PrintWindow or encoding fails.
pub fn capture_window(hwnd: isize) -> Result<CaptureResult, ShotError> {
    unsafe {
        let hwnd = HWND(hwnd as *mut _);

        // Check if minimized
        if IsIconic(hwnd).as_bool() {
            return Err(ShotError::WindowMinimized);
        }

        // Get window dimensions
        let mut rect = mem::zeroed();
        GetWindowRect(hwnd, &mut rect)
            .map_err(|e| ShotError::CaptureFailed(format!("GetWindowRect failed: {}", e)))?;

        let width = (rect.right - rect.left) as u32;
        let height = (rect.bottom - rect.top) as u32;

        if width == 0 || height == 0 {
            return Err(ShotError::CaptureFailed(
                "Window has zero dimensions".into(),
            ));
        }

        // Create compatible DC and bitmap
        let screen_dc = GetWindowDC(hwnd);
        let mem_dc = CreateCompatibleDC(screen_dc);
        let bitmap = CreateCompatibleBitmap(screen_dc, width as i32, height as i32);
        let old_obj = SelectObject(mem_dc, bitmap);

        // Capture window content
        let print_result = PrintWindow(hwnd, mem_dc, PW_RENDERFULLCONTENT);
        if !print_result.as_bool() {
            // Fallback: try BitBlt
            let blt_result = BitBlt(
                mem_dc,
                0,
                0,
                width as i32,
                height as i32,
                screen_dc,
                0,
                0,
                SRCCOPY,
            );
            if blt_result.is_err() {
                SelectObject(mem_dc, old_obj);
                DeleteObject(bitmap);
                DeleteDC(mem_dc);
                ReleaseDC(hwnd, screen_dc);
                return Err(ShotError::CaptureFailed(
                    "PrintWindow and BitBlt both failed. The window may require elevated \
                     privileges or use GPU-accelerated rendering."
                        .into(),
                ));
            }
        }

        // Get pixel data via GetDIBits
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // Top-down (negative = top-down DIB)
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..mem::zeroed()
            },
            ..mem::zeroed()
        };

        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let lines = GetDIBits(
            mem_dc,
            bitmap,
            0,
            height,
            Some(pixels.as_mut_ptr().cast()),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        // Cleanup GDI resources
        SelectObject(mem_dc, old_obj);
        DeleteObject(bitmap);
        DeleteDC(mem_dc);
        ReleaseDC(hwnd, screen_dc);

        if lines == 0 {
            return Err(ShotError::CaptureFailed("GetDIBits returned 0 lines".into()));
        }

        // Convert BGRA → RGBA
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.swap(0, 2); // B ↔ R
        }

        // Encode as PNG
        let mut png_buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_buf, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|e| ShotError::CaptureFailed(format!("PNG header error: {}", e)))?;
            writer
                .write_image_data(&pixels)
                .map_err(|e| ShotError::CaptureFailed(format!("PNG encode error: {}", e)))?;
        }

        Ok(CaptureResult {
            png_bytes: png_buf,
            width,
            height,
        })
    }
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: Compiles successfully.

**Note:** If Win32 API types differ in `windows` v0.62 (e.g., `HWND` constructor, `BOOL` return), fix based on compiler errors. Common adjustments: `HWND(hwnd as *mut _)` vs `HWND(hwnd)`, `PrintWindow` return type.

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: 70 tests pass (capture has no unit tests — requires real window).

- [ ] **Step 4: Lint and format**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 5: Commit**

```
git add src/capture.rs
git commit -m "feat: implement window capture via PrintWindow → PNG

CaptureResult with png_bytes/width/height. PrintWindow with
PW_RENDERFULLCONTENT, BitBlt fallback. BGRA→RGBA conversion.
PNG encoding via png crate. Checks for minimized/zero-size windows.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 12: clipboard.rs — Path and Image Clipboard

**Files:**
- Modify: `src/clipboard.rs`

- [ ] **Step 1: Implement clipboard module**

Replace `src/clipboard.rs` with:

```rust
use crate::config::ClipboardMode;
use crate::errors::ShotError;
use std::mem;
use std::path::Path;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Gdi::BITMAPINFOHEADER;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::UI::WindowsAndMessaging::{CF_DIB, CF_UNICODETEXT};

/// Write to clipboard based on mode.
pub fn write_clipboard(
    mode: ClipboardMode,
    file_path: &Path,
    png_bytes: &[u8],
    width: u32,
    height: u32,
) -> Result<(), ShotError> {
    unsafe {
        OpenClipboard(None)
            .map_err(|e| ShotError::ClipboardError(format!("OpenClipboard failed: {}", e)))?;
        let _ = EmptyClipboard();

        let result = match mode {
            ClipboardMode::Path => set_path(file_path),
            ClipboardMode::Image => set_image(png_bytes, width, height),
            ClipboardMode::Both => {
                set_path(file_path)?;
                set_image(png_bytes, width, height)
            }
        };

        let _ = CloseClipboard();
        result
    }
}

/// Set CF_UNICODETEXT with absolute file path.
unsafe fn set_path(file_path: &Path) -> Result<(), ShotError> {
    let path_str = file_path
        .to_str()
        .ok_or_else(|| ShotError::ClipboardError("Path is not valid UTF-8".into()))?;
    let wide: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = wide.len() * mem::size_of::<u16>();

    let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len)
        .map_err(|e| ShotError::ClipboardError(format!("GlobalAlloc failed: {}", e)))?;
    let ptr = GlobalLock(hmem);
    if ptr.is_null() {
        return Err(ShotError::ClipboardError("GlobalLock returned null".into()));
    }
    std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, byte_len);
    let _ = GlobalUnlock(hmem);

    SetClipboardData(CF_UNICODETEXT.0 as u32, HANDLE(hmem.0))
        .map_err(|e| ShotError::ClipboardError(format!("SetClipboardData(text) failed: {}", e)))?;

    Ok(())
}

/// Set CF_DIB with device-independent bitmap.
/// Builds BITMAPINFOHEADER + raw BGRA pixel data.
unsafe fn set_image(
    png_bytes: &[u8],
    width: u32,
    height: u32,
) -> Result<(), ShotError> {
    // Decode PNG to get raw RGBA pixels
    let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|e| ShotError::ClipboardError(format!("PNG decode error: {}", e)))?;
    let mut rgba_pixels = vec![0u8; reader.output_buffer_size()];
    reader
        .next_frame(&mut rgba_pixels)
        .map_err(|e| ShotError::ClipboardError(format!("PNG frame error: {}", e)))?;

    // Convert RGBA → BGRA and flip vertically (DIB is bottom-up)
    let stride = (width * 4) as usize;
    let mut bgra_bottomup = vec![0u8; (width * height * 4) as usize];
    for y in 0..height as usize {
        let src_row = &rgba_pixels[y * stride..(y + 1) * stride];
        let dst_row_start = (height as usize - 1 - y) * stride;
        let dst_row = &mut bgra_bottomup[dst_row_start..dst_row_start + stride];
        for (src, dst) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
            dst[0] = src[2]; // B
            dst[1] = src[1]; // G
            dst[2] = src[0]; // R
            dst[3] = src[3]; // A
        }
    }

    // Build DIB: header + pixels
    let header_size = mem::size_of::<BITMAPINFOHEADER>();
    let pixel_size = bgra_bottomup.len();
    let total_size = header_size + pixel_size;

    let header = BITMAPINFOHEADER {
        biSize: header_size as u32,
        biWidth: width as i32,
        biHeight: height as i32, // Positive = bottom-up
        biPlanes: 1,
        biBitCount: 32,
        biCompression: 0, // BI_RGB
        biSizeImage: pixel_size as u32,
        ..mem::zeroed()
    };

    let hmem = GlobalAlloc(GMEM_MOVEABLE, total_size)
        .map_err(|e| ShotError::ClipboardError(format!("GlobalAlloc failed: {}", e)))?;
    let ptr = GlobalLock(hmem);
    if ptr.is_null() {
        return Err(ShotError::ClipboardError("GlobalLock returned null".into()));
    }
    std::ptr::copy_nonoverlapping(
        &header as *const BITMAPINFOHEADER as *const u8,
        ptr as *mut u8,
        header_size,
    );
    std::ptr::copy_nonoverlapping(
        bgra_bottomup.as_ptr(),
        (ptr as *mut u8).add(header_size),
        pixel_size,
    );
    let _ = GlobalUnlock(hmem);

    SetClipboardData(CF_DIB.0 as u32, HANDLE(hmem.0))
        .map_err(|e| ShotError::ClipboardError(format!("SetClipboardData(DIB) failed: {}", e)))?;

    Ok(())
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: Compiles successfully.

**Note:** Win32 API types may vary in `windows` v0.62. Common adjustments:
- `HANDLE` wrapping for `GlobalAlloc` result
- `CF_UNICODETEXT.0 as u32` — check if `SetClipboardData` expects `u32` or `CLIPBOARD_FORMAT`
- Fix any type mismatches based on compiler errors

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: 70 tests pass (clipboard has no unit tests — requires real clipboard).

- [ ] **Step 4: Lint and format**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 5: Commit**

```
git add src/clipboard.rs
git commit -m "feat: implement clipboard with path and image modes

CF_UNICODETEXT for path mode, CF_DIB for image mode, both for combined.
PNG decoded → RGBA → BGRA bottom-up conversion for DIB format.
GlobalAlloc/Lock/Unlock for clipboard memory management.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Chunk 4: Orchestration

Tasks 13–16 tie everything together: DPI manifest, main.rs wiring, agent instruction files, and integration/smoke tests. After this chunk the MVP is functionally complete.

---

### Task 13: DPI Manifest + build.rs

**Files:**
- Create: `shot.manifest`
- Create: `build.rs`
- Modify: `Cargo.toml` (add `embed-resource` build-dependency)

- [ ] **Step 1: Create shot.manifest**

Create `shot.manifest`:

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

- [ ] **Step 2: Create resource file for manifest embedding**

Create `shot.rc`:

```
1 24 "shot.manifest"
```

- [ ] **Step 3: Add embed-resource build-dependency**

Add to `Cargo.toml`:

```toml
[build-dependencies]
embed-resource = "3"
```

- [ ] **Step 4: Create build.rs**

Create `build.rs`:

```rust
fn main() {
    embed_resource::compile("shot.rc", embed_resource::NONE);
}
```

- [ ] **Step 5: Verify build**

Run: `cargo build`
Expected: Compiles successfully with embedded manifest.

**Note:** If `embed-resource` causes issues, fall back to runtime DPI API. Add to the top of `main()`:

```rust
use windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext;
use windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2;
unsafe { let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2); }
```

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: 70 tests pass.

- [ ] **Step 7: Commit**

```
git add shot.manifest shot.rc build.rs Cargo.toml
git commit -m "feat: add DPI-aware manifest via embed-resource

PerMonitorV2 DPI awareness for accurate screenshot dimensions.
Manifest embedded at link time via build.rs + embed-resource crate.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 14: main.rs — Entry Point and Mode Dispatch

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Implement main.rs**

Replace `src/main.rs` with:

```rust
#![windows_subsystem = "windows"]

mod audio;
mod capture;
mod cli;
mod clipboard;
mod config;
mod errors;
mod storage;
mod window_resolver;

use errors::ShotError;
use window_resolver::Win32Enumerator;

fn main() {
    // Attach to parent console for stdout/stderr
    unsafe {
        use windows::Win32::System::Console::*;
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            let _ = AllocConsole();
        }
    }

    match run() {
        Ok(()) => {}
        Err(e) => {
            audio::play_error();
            eprintln!("{}", errors::format_error(&e));
            std::process::exit(1);
        }
    }
}

fn run() -> Result<(), ShotError> {
    let args = cli::parse();
    let mode = cli::determine_mode(&args)?;

    match mode {
        cli::Mode::Init => run_init(&args),
        cli::Mode::Check => run_check(&args),
        cli::Mode::ListWindows => run_list_windows(),
        cli::Mode::Capture => run_capture(&args),
        cli::Mode::Watch => Err(ShotError::ArgError(
            "--watch is not available in Phase 1".into(),
        )),
    }
}

fn run_init(args: &cli::CliArgs) -> Result<(), ShotError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
    config::init(&cwd, args.process.as_deref(), args.title.as_deref())?;
    println!("status: ok\nfile: {}", cwd.join("shot.toml").display());
    Ok(())
}

fn run_check(args: &cli::CliArgs) -> Result<(), ShotError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
    let toml = config::find_config(&cwd)?;
    let config_display = toml
        .as_ref()
        .map(|(_, root)| format!("{}", root.join("shot.toml").display()))
        .unwrap_or_else(|| "none (CLI args)".into());
    let cfg = config::merge(args, toml)?;
    let enumerator = Win32Enumerator;
    let window = window_resolver::resolve(
        &enumerator,
        cfg.process.as_deref(),
        cfg.title.as_deref(),
    )?;
    println!(
        "status: ok\nconfig: {}\nwindow: running ({}, PID {})\nfolder: {}",
        config_display,
        window.process_name,
        window.pid,
        cfg.project_root.join(&cfg.folder).display(),
    );
    Ok(())
}

fn run_list_windows() -> Result<(), ShotError> {
    let enumerator = Win32Enumerator;
    let windows = window_resolver::list_all(&enumerator);
    for w in &windows {
        println!("title: {}\nprocess: {}\npid: {}\n---", w.title, w.process_name, w.pid);
    }
    Ok(())
}

fn run_capture(args: &cli::CliArgs) -> Result<(), ShotError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
    let toml = config::find_config(&cwd)?;
    let cfg = config::merge(args, toml)?;

    let enumerator = Win32Enumerator;
    let window = window_resolver::resolve(
        &enumerator,
        cfg.process.as_deref(),
        cfg.title.as_deref(),
    )?;

    let result = capture::capture_window(window.hwnd)?;

    let saved = storage::save(
        &result.png_bytes,
        &cfg.project_root,
        &cfg.folder,
        cfg.label.as_deref(),
        &window.title,
    )?;

    clipboard::write_clipboard(
        cfg.clipboard,
        &saved.path,
        &result.png_bytes,
        result.width,
        result.height,
    )?;

    audio::play_success();

    if !cfg.quiet {
        println!(
            "{}",
            errors::format_success(&saved.path, &window.title, window.pid, result.width, result.height),
        );
    }

    Ok(())
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: Compiles successfully with no dead_code warnings (all modules now used).

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: 70 tests pass.

- [ ] **Step 4: Lint and format**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 5: Commit**

```
git add src/main.rs
git commit -m "feat: implement main.rs with mode dispatch and full pipeline

Entry point: AttachConsole, parse CLI, dispatch mode (Init/Check/ListWindows/Capture/Watch).
Watch returns ArgError (Phase 2 stub).
Capture pipeline: resolve → capture → save → clipboard → audio → format_success.
Error handler: play_error + format_error to stderr + exit(1).

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 15: CLAUDE.md + AGENTS.md

**Files:**
- Create: `CLAUDE.md`
- Create: `AGENTS.md`

- [ ] **Step 1: Create CLAUDE.md**

Create `CLAUDE.md` (content from spec section "Agent Instruction Files"):

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

- [ ] **Step 2: Create AGENTS.md**

Create `AGENTS.md`:

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

- [ ] **Step 3: Commit**

```
git add CLAUDE.md AGENTS.md
git commit -m "docs: add agent instruction files

CLAUDE.md with build commands, project structure, contracts, and gotchas.
AGENTS.md with quick reference and module map.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 16: Integration Tests + Smoke Test Checklist

**Files:**
- Create: `tests/init_integration.rs`
- Create: `tests/config_integration.rs`

- [ ] **Step 1: Create init integration tests**

Create `tests/init_integration.rs`:

```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn shot_cmd() -> Command {
    Command::cargo_bin("shot").unwrap()
}

#[test]
fn init_creates_shot_toml() {
    let dir = TempDir::new().unwrap();
    shot_cmd()
        .arg("--init")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("status: ok"));
    let toml_path = dir.path().join("shot.toml");
    assert!(toml_path.exists());
    let content = std::fs::read_to_string(&toml_path).unwrap();
    assert!(content.contains('#'), "shot.toml should contain inline comments");
}

#[test]
fn init_with_process_fills_field() {
    let dir = TempDir::new().unwrap();
    shot_cmd()
        .args(["--init", "--process=myapp.exe"])
        .current_dir(dir.path())
        .assert()
        .success();
    let content = std::fs::read_to_string(dir.path().join("shot.toml")).unwrap();
    assert!(content.lines().any(|l| l == r#"process = "myapp.exe""#));
}

#[test]
fn init_with_title_fills_field() {
    let dir = TempDir::new().unwrap();
    shot_cmd()
        .args(["--init", "--title=MyApp"])
        .current_dir(dir.path())
        .assert()
        .success();
    let content = std::fs::read_to_string(dir.path().join("shot.toml")).unwrap();
    assert!(content.lines().any(|l| l == r#"title   = "MyApp""#));
}

#[test]
fn init_fails_if_shot_toml_exists() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("shot.toml"), "existing").unwrap();
    shot_cmd()
        .arg("--init")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("already exists"));
}

#[test]
fn init_creates_gitignore_entry() {
    let dir = TempDir::new().unwrap();
    shot_cmd()
        .arg("--init")
        .current_dir(dir.path())
        .assert()
        .success();
    let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(content.contains("screenshots/"));
}
```

- [ ] **Step 2: Create config integration tests**

Create `tests/config_integration.rs`:

```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn shot_cmd() -> Command {
    Command::cargo_bin("shot").unwrap()
}

#[test]
fn walk_up_finds_config_in_parent() {
    let parent = TempDir::new().unwrap();
    let child = parent.path().join("subdir");
    std::fs::create_dir(&child).unwrap();
    std::fs::write(
        parent.path().join("shot.toml"),
        "process = \"nonexistent-for-test.exe\"\n",
    )
    .unwrap();
    // --check should find the config and try to resolve the window (will fail at resolve)
    let output = shot_cmd()
        .arg("--check")
        .current_dir(&child)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should fail at window resolution, not at config loading
    assert!(
        stderr.contains("window-not-found"),
        "Expected window-not-found error (walk-up should find config), got: {}",
        stderr
    );
}

#[test]
fn walk_up_stops_at_git_boundary() {
    let root = TempDir::new().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    std::fs::write(
        root.path().join("shot.toml"),
        "process = \"test.exe\"\n",
    )
    .unwrap();
    // Create .git boundary in child — should prevent walk-up
    std::fs::create_dir(child.join(".git")).unwrap();
    let output = shot_cmd()
        .arg("--check")
        .current_dir(&child)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should fail with "No target" because config was not found (blocked by .git)
    assert!(
        stderr.contains("No target") || stderr.contains("config-error"),
        "Expected 'No target' error, got: {}",
        stderr
    );
}
```

- [ ] **Step 3: Add predicates dev-dependency**

The integration tests use `predicates` via `assert_cmd`. Add to `Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
assert_cmd = "2"
predicates = "3"
```

- [ ] **Step 4: Build and run integration tests**

Run: `cargo test --test init_integration --test config_integration`
Expected: 7 tests pass (5 init + 2 config).

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: 77 tests pass (70 unit + 7 integration).

- [ ] **Step 6: Lint and format**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 7: Commit**

```
git add tests/ Cargo.toml
git commit -m "test: add integration tests for init and config walk-up

7 integration tests using assert_cmd + tempfile.
init: creates shot.toml, fills --process/--title, fails if exists, creates .gitignore.
config: walk-up finds parent config, stops at .git boundary.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

- [ ] **Step 8: Smoke test checklist (manual)**

Run each command and verify output:

1. `cargo build --release` → zero warnings
2. `cargo test` → all 77 tests pass
3. `cargo clippy -- -D warnings` → no diagnostics
4. `target\release\shot.exe --version` → prints `shot 0.1.0`
5. `target\release\shot.exe --list-windows` → `title:` / `process:` / `pid:` lines, no blank titles
6. In a temp directory: `target\release\shot.exe --init` → creates `shot.toml`
7. With Notepad open: `target\release\shot.exe --check --process=notepad.exe` → `status: ok`

If any smoke test fails, fix and re-run before marking complete.
