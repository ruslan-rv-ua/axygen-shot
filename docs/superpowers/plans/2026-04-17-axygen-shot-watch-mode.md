# Watch Mode (Phase 2) Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add watch mode to Axygen Shot — a background daemon with global hotkey, system tray icon, and capture-on-keypress.

**Architecture:** Single new module `src/watch.rs` handles the complete daemon lifecycle (detection, launcher, message loop, tray, capture dispatch). Minimal changes to existing modules: add `hotkey` field to config, add `HotkeyError` variant, add audio functions, wire up `Mode::Watch` in main. Daemon detection uses `GetConsoleWindow()` — no flags or env vars.

**Tech Stack:** Rust, windows crate v0.62 (Win32_System_Console, existing feature flags), clap v4

**Spec:** `docs/superpowers/specs/2026-04-17-axygen-shot-watch-mode-design.md`

---

## Chunk 1: Foundation Changes (existing modules)

### Task 1: Add `HotkeyError` variant and `format_error_message()` to errors.rs

**Files:**
- Modify: `src/errors.rs`

**Context:** The `ShotError` enum currently has 8 variants. Watch mode needs `HotkeyError(String)` for hotkey registration failures. The `format_error_message()` function extracts just the human-readable message string (without key:value formatting) — needed for `MessageBox` dialogs in daemon mode.

**Existing code you need to know:**

```rust
// src/errors.rs — current ShotError enum:
#[derive(Error, Debug)]
pub enum ShotError {
    #[error("{0}")] WindowNotFound(String),
    #[error("Target window is minimized; restore it and try again")] WindowMinimized,
    #[error("{0}")] CaptureFailed(String),
    #[error("{0}")] StorageFailed(String),
    #[error("{0}")] ConfigError(String),
    #[error("{0}")] ClipboardError(String),
    #[error("{0}")] InitError(String),
    #[error("{0}")] ArgError(String),
}

// ShotError::code() maps each variant to a string code like "window-not-found"
// format_error() produces "status: error\ncode: ...\nmessage: ..." for stderr
```

- [ ] **Step 1: Write the failing tests**

Add these tests to the existing `#[cfg(test)] mod tests` block in `src/errors.rs`:

```rust
#[test]
fn hotkey_error_code() {
    assert_eq!(ShotError::HotkeyError("x".into()).code(), "hotkey-error");
}

#[test]
fn format_error_message_returns_human_readable() {
    let cases = vec![
        (ShotError::WindowNotFound("test".into()), "Window not found: test"),
        (ShotError::WindowMinimized, "Target window is minimized. Restore it and try again."),
        (ShotError::CaptureFailed("x".into()), "Capture failed: x"),
        (ShotError::StorageFailed("x".into()), "Could not save screenshot: x"),
        (ShotError::ClipboardError("x".into()), "Clipboard error: x"),
        (ShotError::ConfigError("x".into()), "Configuration error: x"),
        (ShotError::ArgError("x".into()), "Argument error: x"),
        (ShotError::InitError("x".into()), "Init error: x"),
        (ShotError::HotkeyError("x".into()), "Hotkey error: x"),
    ];
    for (err, expected) in cases {
        assert_eq!(format_error_message(&err), expected, "Failed for {:?}", err);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib errors -- --nocapture`
Expected: FAIL — `HotkeyError` variant and `format_error_message` don't exist

- [ ] **Step 3: Implement HotkeyError and format_error_message**

Add `HotkeyError` variant to `ShotError`:

```rust
#[error("{0}")]
HotkeyError(String),
```

Add its code mapping in `code()`:

```rust
Self::HotkeyError(_) => "hotkey-error",
```

Add the `format_error_message` function (public, after `format_error`):

```rust
/// Human-readable message only (no key:value format). Used by watch mode MessageBox.
pub fn format_error_message(err: &ShotError) -> String {
    match err {
        ShotError::WindowNotFound(s) => format!("Window not found: {}", s),
        ShotError::WindowMinimized => "Target window is minimized. Restore it and try again.".into(),
        ShotError::CaptureFailed(s) => format!("Capture failed: {}", s),
        ShotError::StorageFailed(s) => format!("Could not save screenshot: {}", s),
        ShotError::ClipboardError(s) => format!("Clipboard error: {}", s),
        ShotError::ConfigError(s) => format!("Configuration error: {}", s),
        ShotError::ArgError(s) => format!("Argument error: {}", s),
        ShotError::InitError(s) => format!("Init error: {}", s),
        ShotError::HotkeyError(s) => format!("Hotkey error: {}", s),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib errors -- --nocapture`
Expected: ALL PASS (existing 10 + new 2 = 12 tests)

- [ ] **Step 5: Run full test suite to check for breakage**

Run: `cargo test`
Expected: ALL 80+ tests pass. No breakage — the new variant doesn't affect existing match arms because `format_error()` uses `err.code()` and `err` (Display), not explicit matching.

- [ ] **Step 6: Commit**

```bash
git add src/errors.rs
git commit -m "feat: add HotkeyError variant and format_error_message()

Phase 2 (watch mode) needs HotkeyError for hotkey registration failures
and format_error_message() for MessageBox dialogs in daemon mode."
```

---

### Task 2: Add `play_startup()` and `play_busy()` to audio.rs

**Files:**
- Modify: `src/audio.rs`

**Context:** Audio module uses raw FFI to `MessageBeep` (the `windows` crate v0.62 does NOT expose `MessageBeep`). Currently has `play_success()` (SystemAsterisk = 0x40) and `play_error()` (SystemHand = 0x10). Watch mode needs two more sounds: startup (SystemExclamation = 0x30, PRD story 59) and busy (MB_ICONQUESTION = 0x20, for capture serialization).

**Current audio.rs (complete file):**

```rust
#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBeep(uType: u32) -> i32;
}

const MB_ICONASTERISK: u32 = 0x00000040;
const MB_ICONHAND: u32 = 0x00000010;

pub fn play_success() {
    unsafe { let _ = MessageBeep(MB_ICONASTERISK); }
}

pub fn play_error() {
    unsafe { let _ = MessageBeep(MB_ICONHAND); }
}
```

- [ ] **Step 1: Add the two new functions**

Add constants and functions after the existing ones:

```rust
const MB_ICONEXCLAMATION: u32 = 0x00000030;
const MB_ICONQUESTION: u32 = 0x00000020;

/// Play startup sound (SystemExclamation) — PRD story 59.
/// Used when watch mode successfully registers its hotkey.
pub fn play_startup() {
    unsafe {
        let _ = MessageBeep(MB_ICONEXCLAMATION);
    }
}

/// Play busy sound (MB_ICONQUESTION) — spec: capture serialization.
/// Used when hotkey fires during an active capture.
pub fn play_busy() {
    unsafe {
        let _ = MessageBeep(MB_ICONQUESTION);
    }
}
```

- [ ] **Step 2: Run full test suite**

Run: `cargo test`
Expected: ALL PASS (no tests for audio — it's fire-and-forget FFI)

- [ ] **Step 3: Commit**

```bash
git add src/audio.rs
git commit -m "feat: add play_startup() and play_busy() to audio module

SystemExclamation for watch mode startup (PRD story 59).
MB_ICONQUESTION for capture serialization busy signal."
```

---

### Task 3: Add `hotkey` field to `CaptureConfig` and merge logic

**Files:**
- Modify: `src/config.rs`

**Context:** `CaptureConfig` is the merged config struct used by all capture operations. It currently has: `process`, `title`, `folder`, `clipboard`, `label`, `quiet`, `verbose`, `project_root`. Watch mode needs `hotkey: String` (default `"Win+F12"`). The `TomlConfig` already has `pub hotkey: Option<String>` (added in Phase 1 with `#[allow(dead_code)]`). The `CliArgs` already has `pub hotkey: Option<String>`. The merge function at `config.rs:88` (`pub fn merge(...)`) needs to include hotkey in the output. There are 7+ tests that construct `CaptureConfig` indirectly via `merge()` — they will all continue to work. But if any tests assert on the whole struct or construct `CaptureConfig` directly, they need the new field.

**Key locations in config.rs:**
- Line 28-29: `TomlConfig.hotkey` — already exists with `#[allow(dead_code)]`
- Line 88: `pub fn merge(cli: &CliArgs, toml: Option<(TomlConfig, PathBuf)>) -> Result<CaptureConfig, ShotError>`
- Line 121-131: The `Ok(CaptureConfig { ... })` construction in merge()
- Line 214-224: `pub struct CaptureConfig { ... }`

- [ ] **Step 1: Write the failing tests**

Add these tests in the existing `#[cfg(test)] mod tests` block (after `merge_default_clipboard_is_path`):

```rust
#[test]
fn merge_hotkey_default() {
    let cli = CliArgs::parse_from(["shot", "--process=x.exe"]);
    let cfg = merge(&cli, None).unwrap();
    assert_eq!(cfg.hotkey, "Win+F12");
}

#[test]
fn merge_hotkey_from_toml() {
    let toml_str = "process = \"x.exe\"\nhotkey = \"Ctrl+F5\"";
    let toml_config: TomlConfig = toml::from_str(toml_str).unwrap();
    let cli = CliArgs::parse_from(["shot"]);
    let cfg = merge(&cli, Some((toml_config, PathBuf::from("."))))
        .unwrap();
    assert_eq!(cfg.hotkey, "Ctrl+F5");
}

#[test]
fn merge_hotkey_cli_overrides_toml() {
    let toml_str = "process = \"x.exe\"\nhotkey = \"Ctrl+F5\"";
    let toml_config: TomlConfig = toml::from_str(toml_str).unwrap();
    let cli = CliArgs::parse_from(["shot", "--hotkey=Win+F11"]);
    let cfg = merge(&cli, Some((toml_config, PathBuf::from("."))))
        .unwrap();
    assert_eq!(cfg.hotkey, "Win+F11");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config -- merge_hotkey --nocapture`
Expected: FAIL — `CaptureConfig` has no `hotkey` field

- [ ] **Step 3: Add hotkey field and merge logic**

In `CaptureConfig` struct (line ~215), add:

```rust
pub hotkey: String,
```

Remove `#[allow(dead_code)]` from `TomlConfig.hotkey` (line 28).

In `merge()` (line ~121), add to the `CaptureConfig { ... }` construction:

```rust
hotkey: cli.hotkey.clone()
    .or(toml_config.as_ref().and_then(|t| t.hotkey.clone()))
    .unwrap_or_else(|| "Win+F12".to_string()),
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib config -- --nocapture`
Expected: ALL PASS (existing 32 + new 3 = 35 tests)

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add src/config.rs
git commit -m "feat: add hotkey field to CaptureConfig with merge logic

Default: Win+F12. CLI --hotkey overrides TOML hotkey.
Removes #[allow(dead_code)] from TomlConfig.hotkey."
```

---

### Task 4: Add `Win32_System_Console` feature flag to Cargo.toml

**Files:**
- Modify: `Cargo.toml`

**Context:** `Win32_System_Console` was removed in Phase 1 when we dropped `AttachConsole`. Watch mode needs `GetConsoleWindow()` from this feature for daemon detection. The feature flag list is at `Cargo.toml:20-34`.

- [ ] **Step 1: Add feature flag**

Add `"Win32_System_Console",` to the windows features list (after `"Win32_System_Ole",`):

```toml
features = [
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
    "Win32_Storage_Xps",
    "Win32_System_Ole",
    "Win32_System_Console",
]
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "feat: re-add Win32_System_Console for GetConsoleWindow

Required by watch mode daemon detection (GetConsoleWindow)."
```

---

## Chunk 2: Hotkey Parsing (pure logic, fully testable)

### Task 5: Implement hotkey parsing with TDD

**Files:**
- Create: `src/watch.rs`

**Context:** This task creates `src/watch.rs` with ONLY the hotkey parsing logic (`parse_hotkey`, `parse_vk`). The daemon lifecycle, message loop, tray — all come in later tasks. This task is pure logic with no Win32 dependencies, fully unit-testable.

The `parse_hotkey` function takes a string like `"Win+F12"` and returns `(HOT_KEY_MODIFIERS, u32)` — the modifier flags and virtual key code needed by `RegisterHotKey`. The `parse_vk` helper converts a key name to its VK_* constant.

**Win32 constants needed:**
- `MOD_WIN = 0x0008`, `MOD_CONTROL = 0x0002`, `MOD_SHIFT = 0x0004`, `MOD_ALT = 0x0001`
- `MOD_NOREPEAT = 0x4000` — added to all results
- `VK_F1 = 0x70` through `VK_F24 = 0x87`
- `VK_SNAPSHOT = 0x2C` (PrintScreen)
- VK for A–Z = `0x41`–`0x5A`, for 0–9 = `0x30`–`0x39`

**Important:** You MUST also register the module in `src/main.rs` by adding `mod watch;` to the module declarations at the top. Without this, the module won't be compiled or tested.

- [ ] **Step 1: Create src/watch.rs with parse_hotkey, parse_vk, and tests**

Create `src/watch.rs`. Start with the test module, then implement:

```rust
use crate::errors::ShotError;

// Win32 hotkey modifier flags
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;
const MOD_NOREPEAT: u32 = 0x4000;

/// Parse a hotkey string like "Win+F12" into (modifiers, virtual_key_code).
/// Modifiers are OR'd together. MOD_NOREPEAT is always added.
/// At least one modifier required. Exactly one key required.
pub fn parse_hotkey(s: &str) -> Result<(u32, u32), ShotError> {
    let mut modifiers: u32 = 0;
    let mut vk: Option<u32> = None;

    for token in s.split('+') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        match token.to_lowercase().as_str() {
            "win" => modifiers |= MOD_WIN,
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "shift" => modifiers |= MOD_SHIFT,
            "alt" => modifiers |= MOD_ALT,
            key => {
                if vk.is_some() {
                    return Err(ShotError::ArgError(
                        format!("Invalid hotkey '{}': multiple keys specified", s),
                    ));
                }
                vk = Some(parse_vk(key, s)?);
            }
        }
    }

    let vk = vk.ok_or_else(|| {
        ShotError::ArgError(format!("Invalid hotkey '{}': no key specified", s))
    })?;

    if modifiers == 0 {
        return Err(ShotError::ArgError(format!(
            "Invalid hotkey '{}': at least one modifier required (Win, Ctrl, Shift, Alt)",
            s,
        )));
    }

    Ok((modifiers | MOD_NOREPEAT, vk))
}

fn parse_vk(key: &str, full_hotkey: &str) -> Result<u32, ShotError> {
    let lower = key.to_lowercase();

    // F1–F24
    if lower.starts_with('f') {
        if let Ok(n) = lower[1..].parse::<u32>() {
            if (1..=24).contains(&n) {
                return Ok(0x6F + n); // VK_F1 = 0x70
            }
        }
    }

    // Single character: A–Z or 0–9
    if lower.len() == 1 {
        let ch = lower.as_bytes()[0];
        if ch.is_ascii_lowercase() {
            return Ok((ch - b'a' + b'A') as u32);
        }
        if ch.is_ascii_digit() {
            return Ok(ch as u32);
        }
    }

    // Named keys
    match lower.as_str() {
        "printscreen" | "prtsc" => Ok(0x2C), // VK_SNAPSHOT
        _ => Err(ShotError::ArgError(format!(
            "Invalid hotkey key '{}' in '{}'. Valid: F1-F24, A-Z, 0-9, PrintScreen",
            key, full_hotkey,
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win_f12() {
        let (mods, vk) = parse_hotkey("Win+F12").unwrap();
        assert_eq!(mods, MOD_WIN | MOD_NOREPEAT);
        assert_eq!(vk, 0x7B); // VK_F12
    }

    #[test]
    fn ctrl_shift_a() {
        let (mods, vk) = parse_hotkey("Ctrl+Shift+A").unwrap();
        assert_eq!(mods, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT);
        assert_eq!(vk, 0x41); // VK_A
    }

    #[test]
    fn alt_printscreen() {
        let (mods, vk) = parse_hotkey("Alt+PrintScreen").unwrap();
        assert_eq!(mods, MOD_ALT | MOD_NOREPEAT);
        assert_eq!(vk, 0x2C); // VK_SNAPSHOT
    }

    #[test]
    fn case_insensitive() {
        let (m1, v1) = parse_hotkey("Win+F12").unwrap();
        let (m2, v2) = parse_hotkey("win+f12").unwrap();
        assert_eq!((m1, v1), (m2, v2));
    }

    #[test]
    fn ctrl_shift_f5() {
        let (mods, vk) = parse_hotkey("ctrl+shift+f5").unwrap();
        assert_eq!(mods, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT);
        assert_eq!(vk, 0x74); // VK_F5
    }

    #[test]
    fn no_modifier_errors() {
        let err = parse_hotkey("F12").unwrap_err();
        assert!(err.to_string().contains("at least one modifier"));
    }

    #[test]
    fn no_key_errors() {
        let err = parse_hotkey("Win+").unwrap_err();
        assert!(err.to_string().contains("no key specified"));
    }

    #[test]
    fn two_keys_errors() {
        let err = parse_hotkey("Win+F12+F5").unwrap_err();
        assert!(err.to_string().contains("multiple keys"));
    }

    #[test]
    fn invalid_key_errors() {
        let err = parse_hotkey("Win+InvalidKey").unwrap_err();
        assert!(err.to_string().contains("Invalid hotkey key"));
    }

    #[test]
    fn empty_string_errors() {
        assert!(parse_hotkey("").is_err());
    }

    #[test]
    fn win_digit_0() {
        let (mods, vk) = parse_hotkey("Win+0").unwrap();
        assert_eq!(mods, MOD_WIN | MOD_NOREPEAT);
        assert_eq!(vk, 0x30); // VK_0
    }

    #[test]
    fn win_z() {
        let (mods, vk) = parse_hotkey("Win+Z").unwrap();
        assert_eq!(mods, MOD_WIN | MOD_NOREPEAT);
        assert_eq!(vk, 0x5A); // VK_Z
    }

    #[test]
    fn prtsc_alias() {
        let (mods, vk) = parse_hotkey("Alt+PrtSc").unwrap();
        assert_eq!(mods, MOD_ALT | MOD_NOREPEAT);
        assert_eq!(vk, 0x2C);
    }

    #[test]
    fn f1_and_f24_boundaries() {
        let (_, vk1) = parse_hotkey("Win+F1").unwrap();
        assert_eq!(vk1, 0x70); // VK_F1
        let (_, vk24) = parse_hotkey("Win+F24").unwrap();
        assert_eq!(vk24, 0x87); // VK_F24
    }

    #[test]
    fn control_alias() {
        let (mods, _) = parse_hotkey("Control+F1").unwrap();
        assert_eq!(mods & MOD_CONTROL, MOD_CONTROL);
    }
}
```

- [ ] **Step 2: Register module in main.rs**

Add `mod watch;` to the module declarations at the top of `src/main.rs` (after `mod window_resolver;`).

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib watch -- --nocapture`
Expected: ALL 15 tests PASS

- [ ] **Step 4: Run full test suite**

Run: `cargo test`
Expected: ALL PASS (previous 80+ tests + 15 new = 95+)

- [ ] **Step 5: Commit**

```bash
git add src/watch.rs src/main.rs
git commit -m "feat: hotkey parsing (parse_hotkey, parse_vk) with 15 tests

Parses strings like 'Win+F12' into (modifiers, virtual_key_code).
Supports: Win/Ctrl/Shift/Alt modifiers, F1-F24/A-Z/0-9/PrintScreen keys.
Pure logic, no Win32 dependencies."
```

---

## Chunk 3: Watch Mode Daemon (Win32 integration)

### Task 6: Implement daemon detection and launcher phase

**Files:**
- Modify: `src/watch.rs`

**Context:** This task adds the `run()` entry point and the launcher phase to `watch.rs`. When `shot --watch` runs from a terminal, `GetConsoleWindow()` returns a valid HWND. The launcher validates config, re-launches itself with `DETACHED_PROCESS | CREATE_NO_WINDOW`, prints status, and exits. The re-launched process has no console (`GetConsoleWindow() == NULL`) and will enter daemon mode (implemented in Task 7).

**Important Win32 API notes for the windows crate v0.62:**
- `GetConsoleWindow()` is in `windows::Win32::System::Console`
- `CreateProcessW` is in `windows::Win32::System::Threading`
- Process creation flags: `PROCESS_CREATION_FLAGS` type, constants like `CREATE_NEW_PROCESS_GROUP`, `DETACHED_PROCESS`, `CREATE_NO_WINDOW` are in `windows::Win32::System::Threading`
- `STARTUPINFOW` and `PROCESS_INFORMATION` are in `windows::Win32::System::Threading`

**The function signatures this task creates:**

```rust
pub fn run(cfg: &CaptureConfig) -> Result<(), ShotError>  // Main entry point
fn is_detached() -> bool                                   // Check for console
fn launch_daemon() -> Result<u32, ShotError>               // Re-launch, return child PID
```

- [ ] **Step 1: Implement is_detached() and launch_daemon()**

Add to `src/watch.rs` (above the `parse_hotkey` function):

```rust
use crate::config::CaptureConfig;
use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::System::Console::GetConsoleWindow;
use windows::Win32::System::Threading::{
    CreateProcessW, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTUPINFOW,
};
use windows::core::PWSTR;

const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
const DETACHED_PROCESS: u32 = 0x00000008;
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn is_detached() -> bool {
    unsafe { GetConsoleWindow().is_null() }
}

fn launch_daemon() -> Result<u32, ShotError> {
    let exe = std::env::current_exe()
        .map_err(|e| ShotError::HotkeyError(format!("Cannot find own executable: {}", e)))?;

    // Build command line: exe path + all original args
    let mut cmd_line = OsString::new();
    cmd_line.push("\"");
    cmd_line.push(exe.as_os_str());
    cmd_line.push("\"");
    for arg in std::env::args().skip(1) {
        cmd_line.push(" ");
        cmd_line.push(&arg);
    }
    let mut cmd_wide: Vec<u16> = cmd_line.encode_wide().chain(std::iter::once(0)).collect();

    let flags = PROCESS_CREATION_FLAGS(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS | CREATE_NO_WINDOW);
    let mut si = STARTUPINFOW::default();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    let mut pi = PROCESS_INFORMATION::default();

    unsafe {
        CreateProcessW(
            None,
            PWSTR(cmd_wide.as_mut_ptr()),
            None,
            None,
            false,
            flags,
            None,
            None,
            &si,
            &mut pi,
        )
        .map_err(|e| ShotError::HotkeyError(format!("Cannot launch daemon: {}", e)))?;

        // Close handles we don't need
        let _ = windows::Win32::Foundation::CloseHandle(pi.hProcess);
        let _ = windows::Win32::Foundation::CloseHandle(pi.hThread);

        Ok(pi.dwProcessId)
    }
}
```

- [ ] **Step 2: Implement run() entry point**

Add the public `run()` function:

```rust
/// Entry point for watch mode. Handles both launcher (has console) and daemon (no console).
pub fn run(cfg: &CaptureConfig) -> Result<(), ShotError> {
    if !is_detached() {
        // Launcher phase: we have a console (terminal)
        let child_pid = launch_daemon()?;
        if !cfg.quiet || cfg.verbose {
            println!("status: ok\nwatch: started (PID {})", child_pid);
        }
        return Ok(());
    }

    // Daemon phase: no console, we ARE the background process
    run_daemon(cfg)
}

fn run_daemon(_cfg: &CaptureConfig) -> Result<(), ShotError> {
    // Placeholder — implemented in Task 7
    Ok(())
}
```

- [ ] **Step 3: Wire up Mode::Watch in main.rs**

In `src/main.rs`, replace:

```rust
cli::Mode::Watch => Err(ShotError::ArgError(
    "--watch is not available in Phase 1".into(),
)),
```

With:

```rust
cli::Mode::Watch => {
    let cwd = std::env::current_dir()
        .map_err(|e| ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
    let toml = config::find_config(&cwd)?;
    let cfg = config::merge(args, toml)?;
    watch::run(&cfg)
}
```

Note: Watch mode loads config in the same way as Capture mode. The `--watch` flag doesn't skip config validation — a valid process/title is required.

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: No errors (the daemon placeholder returns Ok immediately)

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: ALL PASS. The existing `watch_mode` test in cli.rs (`fn watch_mode()`) tests that `--watch` produces `Mode::Watch`, which is unchanged.

- [ ] **Step 6: Commit**

```bash
git add src/watch.rs src/main.rs
git commit -m "feat: daemon detection and launcher phase for watch mode

GetConsoleWindow() detects if running from terminal (launcher) or
as detached daemon. Launcher re-launches self with DETACHED_PROCESS |
CREATE_NO_WINDOW, prints PID, and exits. Mode::Watch wired in main.rs."
```

---

### Task 7: Implement daemon message loop, tray icon, and hotkey registration

**Files:**
- Modify: `src/watch.rs`

**Context:** This is the core task — implementing the actual daemon. It replaces the `run_daemon` placeholder from Task 6 with a full Win32 message loop, tray icon, hotkey registration, context menu, and capture dispatch.

**Critical Win32 API details (windows crate v0.62):**

1. **Hidden message-only window:** Create with `CreateWindowExW` using `HWND_MESSAGE` as parent. This window receives messages but is invisible. You need to register a window class first with `RegisterClassExW` and provide a `wndproc`.

2. **Shell_NotifyIcon:** In `windows::Win32::UI::Shell`. Uses `NOTIFYICONDATAW` struct. Set `uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP`. Tooltip is `szTip` field (max 128 UTF-16 chars). Callback message stored in `uCallbackMessage`.

3. **RegisterHotKey:** In `windows::Win32::UI::Input::KeyboardAndMouse`. First param is HWND, second is ID (use `1`), third is modifiers (`HOT_KEY_MODIFIERS`), fourth is VK code.

4. **MessageBoxW:** In `windows::Win32::UI::WindowsAndMessaging`. Use `PCWSTR` for text params.

5. **Context menu:** `CreatePopupMenu`, `AppendMenuW`, `TrackPopupMenu`, `DestroyMenu`. Must call `SetForegroundWindow` before `TrackPopupMenu` for proper tray behavior, and `PostMessageW(WM_NULL)` after.

6. **Getting the tray callback:** The tray sends `WM_APP + 1` to the window. `lParam` contains the actual message (e.g., `WM_RBUTTONUP`).

**Important about `unsafe` in Rust 2024 edition:** Each individual unsafe Win32 call needs its own `unsafe { }` block inside an `unsafe fn`. The `unsafe_op_in_unsafe_fn` lint is deny by default.

**The capture pipeline calls the SAME functions as `run_capture` in main.rs but with MessageBox error handling instead of stderr:**

```rust
// Actual signatures from Phase 1 source code:
window_resolver::resolve(&enumerator, cfg.process.as_deref(), cfg.title.as_deref()) -> Result<WindowInfo, ShotError>
capture::capture_window(window.hwnd) -> Result<CaptureResult, ShotError>
storage::save(&result.png_bytes, &cfg.project_root, &cfg.folder, label, &window.title) -> Result<SavedFile, ShotError>
clipboard::write_clipboard(cfg.clipboard, &saved.path, &result.png_bytes, result.width, result.height) -> Result<(), ShotError>
```

- [ ] **Step 1: Implement run_daemon with all Win32 components**

Replace the `run_daemon` placeholder in `src/watch.rs` with the full implementation. This is a large function — here's the complete structure:

```rust
use crate::{audio, capture, clipboard, errors, storage, window_resolver};
use crate::window_resolver::Win32Enumerator;
use std::mem;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{w, PCWSTR};

const WM_TRAY_CALLBACK: u32 = WM_APP + 1;
const ID_EXIT: usize = 1001;
const ID_RESTART: usize = 1002;
const HOTKEY_ID: i32 = 1;

// Store config pointer for wndproc access (single-threaded, safe for daemon)
static mut DAEMON_CFG: Option<*const CaptureConfig> = None;
static mut CAPTURING: bool = false;

fn run_daemon(cfg: &CaptureConfig) -> Result<(), ShotError> {
    // Parse hotkey before creating any windows
    let (modifiers, vk) = parse_hotkey(&cfg.hotkey)?;

    // Store config for wndproc access
    unsafe { DAEMON_CFG = Some(cfg as *const CaptureConfig) };

    // Register window class
    let class_name = wide_string("ShotWatchClass");
    let wc = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wndproc),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    unsafe { RegisterClassExW(&wc) };

    // Create hidden message-only window
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE::default(),
            0, 0, 0, 0,
            HWND_MESSAGE,
            None,
            None,
            None,
        )
    };
    if hwnd == HWND::default() {
        return Err(ShotError::HotkeyError("Cannot create message window".into()));
    }

    // Register global hotkey
    let ok = unsafe {
        RegisterHotKey(Some(hwnd), HOTKEY_ID, HOT_KEY_MODIFIERS(modifiers), vk)
    };
    if let Err(e) = ok {
        message_box(
            &format!("Cannot register hotkey '{}': {}\n\nThe hotkey may be in use by another application.", cfg.hotkey, e),
            "Axygen Shot — Error",
            MB_ICONERROR,
        );
        return Err(ShotError::HotkeyError(format!(
            "RegisterHotKey failed for '{}': {}", cfg.hotkey, e
        )));
    }

    // Add tray icon
    let tooltip = build_tooltip(cfg);
    let mut nid = NOTIFYICONDATAW {
        cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAY_CALLBACK,
        hIcon: unsafe { LoadIconW(None, IDI_APPLICATION).unwrap_or_default() },
        ..Default::default()
    };
    copy_to_wide_buf(&tooltip, &mut nid.szTip);

    let tray_ok = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
    if !tray_ok.as_bool() {
        unsafe { let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID); }
        message_box(
            "Cannot create tray icon. The system tray may not be available.",
            "Axygen Shot — Error",
            MB_ICONERROR,
        );
        return Err(ShotError::HotkeyError("Shell_NotifyIcon failed".into()));
    }

    // Play startup sound (PRD story 59)
    audio::play_startup();

    // Message loop
    unsafe {
        let mut msg: MSG = mem::zeroed();
        loop {
            match GetMessageW(&mut msg, None, 0, 0).0 {
                -1 | 0 => break,
                _ => {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        // Cleanup (runs after PostQuitMessage → GetMessageW returns 0)
        // Note: cleanup is here instead of WM_DESTROY for simplicity —
        // nid is local to run_daemon, not accessible from wndproc.
        Shell_NotifyIconW(NIM_DELETE, &nid);
        let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
    }

    Ok(())
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            if unsafe { CAPTURING } {
                audio::play_busy();
            } else {
                unsafe { CAPTURING = true };
                if let Some(cfg_ptr) = unsafe { DAEMON_CFG } {
                    let cfg = unsafe { &*cfg_ptr };
                    do_capture(cfg);
                }
                unsafe { CAPTURING = false };
            }
            LRESULT(0)
        }
        msg if msg == WM_TRAY_CALLBACK => {
            let mouse_msg = (lparam.0 & 0xFFFF) as u32;
            if mouse_msg == WM_RBUTTONUP {
                show_context_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as usize;
            match id {
                ID_EXIT => {
                    unsafe { PostQuitMessage(0) };
                }
                ID_RESTART => {
                    restart();
                    unsafe { PostQuitMessage(0) };
                }
                _ => {}
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn do_capture(cfg: &CaptureConfig) {
    let result = (|| -> Result<(), ShotError> {
        let enumerator = Win32Enumerator;
        let window = window_resolver::resolve(
            &enumerator,
            cfg.process.as_deref(),
            cfg.title.as_deref(),
        )?;
        let capture_result = capture::capture_window(window.hwnd)?;
        let saved = storage::save(
            &capture_result.png_bytes,
            &cfg.project_root,
            &cfg.folder,
            None, // No label in watch mode (PRD story 34)
            &window.title,
        )?;
        clipboard::write_clipboard(
            cfg.clipboard,
            &saved.path,
            &capture_result.png_bytes,
            capture_result.width,
            capture_result.height,
        )?;
        Ok(())
    })();

    match result {
        Ok(()) => audio::play_success(),
        Err(e) => {
            audio::play_error();
            let msg = errors::format_error_message(&e);
            message_box(&msg, "Axygen Shot — Error", MB_ICONERROR);
        }
    }
}

fn show_context_menu(hwnd: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let _ = AppendMenuW(menu, MENU_ITEM_FLAGS(0), ID_RESTART, w!("Restart"));
        let _ = AppendMenuW(menu, MENU_ITEM_FLAGS(0), ID_EXIT, w!("Exit"));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(menu, TPM_RIGHTALIGN, pt.x, pt.y, 0, hwnd, None);
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
    }
}

fn restart() {
    let exe = std::env::current_exe().ok();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(exe) = exe {
        let mut cmd_line = OsString::new();
        cmd_line.push("\"");
        cmd_line.push(exe.as_os_str());
        cmd_line.push("\"");
        for arg in &args {
            cmd_line.push(" ");
            cmd_line.push(arg);
        }
        let mut cmd_wide: Vec<u16> = cmd_line.encode_wide().chain(std::iter::once(0)).collect();
        let flags = PROCESS_CREATION_FLAGS(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS | CREATE_NO_WINDOW);
        let mut si = STARTUPINFOW::default();
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        let mut pi = PROCESS_INFORMATION::default();
        unsafe {
            let _ = CreateProcessW(None, PWSTR(cmd_wide.as_mut_ptr()), None, None, false, flags, None, None, &si, &mut pi);
            let _ = windows::Win32::Foundation::CloseHandle(pi.hProcess);
            let _ = windows::Win32::Foundation::CloseHandle(pi.hThread);
        }
    }
}

// --- Helpers ---

fn message_box(text: &str, title: &str, flags: MESSAGEBOX_STYLE) {
    let text_w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(text_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            flags,
        );
    }
}

fn build_tooltip(cfg: &CaptureConfig) -> String {
    let target = if let Some(ref p) = cfg.process {
        p.clone()
    } else if let Some(ref t) = cfg.title {
        format!("'{}'", t)
    } else {
        "?".to_string()
    };
    let tip = format!("shot — watching {}", target);
    // szTip is 128 UTF-16 chars max (including null). copy_to_wide_buf handles truncation.
    tip
}

fn wide_string(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn copy_to_wide_buf(s: &str, buf: &mut [u16]) {
    let wide: Vec<u16> = s.encode_utf16().collect();
    let len = wide.len().min(buf.len() - 1);
    buf[..len].copy_from_slice(&wide[..len]);
    buf[len] = 0;
}
```

**IMPORTANT notes for the implementer:**

- The `windows` crate v0.62 has specific API quirks. If any import path doesn't compile, check `docs.rs` for the exact location. Common issues:
  - `HOT_KEY_MODIFIERS` might need to come from a specific submodule
  - `MENU_ITEM_FLAGS` might be named differently (try `MF_STRING`)
  - `MESSAGEBOX_STYLE` might need explicit import
  - The `w!` macro for wide string literals comes from `windows::core::w`
- **Rust 2024 edition:** Each `unsafe` call inside an `unsafe fn` needs its own `unsafe { }` block
- Some Win32 functions return `Result<(), Error>` in the windows crate, others return `BOOL`. Check each one.
- The static muts (`DAEMON_CFG`, `CAPTURING`) are safe here because the daemon is strictly single-threaded (one message loop thread, no other threads).

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors. Fix any import path issues (the windows crate v0.62 moves things around).

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: ALL PASS (existing tests + hotkey parsing tests from Task 5)

- [ ] **Step 4: Manual smoke test**

Open a terminal. Start Notepad. Run:

```
cargo run -- --watch --process=notepad.exe
```

Expected:
1. Terminal prints `status: ok\nwatch: started (PID XXXX)` and returns to prompt
2. System tray shows new icon with tooltip "shot — watching notepad.exe"
3. Press Win+F12 → screenshot captured, success beep
4. Right-click tray icon → menu with "Restart" and "Exit"
5. Click "Exit" → daemon stops, tray icon disappears

If hotkey is already registered:
```
cargo run -- --watch --process=notepad.exe --hotkey=Win+F11
```

- [ ] **Step 5: Commit**

```bash
git add src/watch.rs
git commit -m "feat: full watch mode daemon — tray icon, hotkey, message loop

Implements PRD stories 27-34, 59:
- GetConsoleWindow daemon detection
- RegisterHotKey with configurable hotkey
- Shell_NotifyIcon tray with tooltip
- Context menu: Restart + Exit
- Capture pipeline with MessageBox errors
- Startup sound (SystemExclamation)
- Capture serialization (busy sound)"
```

---

## Chunk 4: Documentation and Manual Testing

### Task 8: Update documentation (CLAUDE.md, AGENTS.md, manual testing checklist)

**Files:**
- Modify: `CLAUDE.md`
- Modify: `AGENTS.md`
- Modify: `docs/manual-testing-checklist.md`

**Context:** Documentation needs to reflect the new watch mode module. CLAUDE.md describes the project structure and gotchas. AGENTS.md has a quick reference. The manual testing checklist needs watch mode test cases.

- [ ] **Step 1: Update CLAUDE.md**

Add `src/watch.rs` to the project structure section:

```
- `src/watch.rs` — watch mode daemon: hotkey parsing, tray icon, message loop
```

Add watch mode gotcha:

```
- Watch mode daemon detected via GetConsoleWindow() — no console = daemon mode
- RegisterHotKey requires at least one modifier (Win, Ctrl, Shift, Alt)
- Tray tooltip max 128 UTF-16 chars (szTip field limit)
```

- [ ] **Step 2: Update AGENTS.md**

Add `watch` to the module map:

```
cli → config → window_resolver → capture → storage → clipboard → audio
                                                                    ↑
watch (daemon: hotkey + tray + message loop) ────────────────────────┘
```

- [ ] **Step 3: Add watch mode section to manual testing checklist**

Add a new section `## 14. Watch Mode` to `docs/manual-testing-checklist.md` with these tests:

| # | Тест | Команда | Очікуваний результат |
|---|------|---------|---------------------|
| 1 | Запуск watch mode | `shot --watch --process=notepad.exe` | `status: ok` + PID, термінал повертає промпт |
| 2 | Tray icon з'являється | (перевірити після запуску) | Іконка в треї, tooltip "shot — watching notepad.exe" |
| 3 | Hotkey capture | Натиснути Win+F12 | Скріншот збережено, beep успіху |
| 4 | Capture serialization | Швидко натиснути Win+F12 двічі | Другий натиск — busy sound |
| 5 | Exit через tray | ПКМ → Exit | Daemon зупиняється, іконка зникає |
| 6 | Restart через tray | ПКМ → Restart | Daemon перезапускається, нова іконка |
| 7 | Hotkey override | `shot --watch --process=notepad.exe --hotkey=Win+F11` | Працює Win+F11 замість F12 |
| 8 | Hotkey conflict | Запустити два --watch з однаковим hotkey | Другий — MessageBox помилки |
| 9 | Startup sound | Запустити --watch | SystemExclamation при старті |
| 10 | Quiet mode | `shot --watch --process=notepad.exe --quiet` | Без stdout, daemon працює |
| 11 | Немає конфігу | `shot --watch` (без shot.toml, без --process/--title) | Помилка в stderr |
| 12 | hotkey з shot.toml | Створити shot.toml з `hotkey = "Win+F11"` | Працює Win+F11 |
| 13 | Window not found | Вказати неіснуючий процес, натиснути hotkey | Error beep + MessageBox |
| 14 | Window minimized | Мінімізувати вікно, натиснути hotkey | Error beep + MessageBox "minimized" |

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md AGENTS.md docs/manual-testing-checklist.md
git commit -m "docs: update for watch mode (module map, gotchas, manual tests)

Added watch.rs to CLAUDE.md project structure and gotchas.
Added 14 manual test cases for watch mode to checklist."
```

---

### Task 9: End-to-end manual verification

**Files:** None (verification only)

**Context:** This is the final verification pass. Run through key scenarios to confirm watch mode works end-to-end.

- [ ] **Step 1: Build release binary**

Run: `just build`
Check: binary size reported (should be slightly larger than 674 KB due to new watch code)

- [ ] **Step 2: Verify all tests pass**

Run: `just ci`
Expected: lint OK, fmt OK, all tests pass

- [ ] **Step 3: Smoke test watch mode**

1. Open Notepad
2. Run: `.\target\release\shot.exe --watch --process=notepad.exe`
3. Verify: `status: ok` printed, prompt returned
4. Press Win+F12
5. Verify: screenshot saved, success beep
6. Right-click tray icon → "Exit"
7. Verify: daemon stopped

- [ ] **Step 4: Smoke test hotkey override**

1. Run: `.\target\release\shot.exe --watch --process=notepad.exe --hotkey=Ctrl+Shift+F5`
2. Press Ctrl+Shift+F5
3. Verify: screenshot captured

- [ ] **Step 5: Verify CLI mode still works**

Run: `.\target\release\shot.exe --process=notepad.exe`
Expected: screenshot captured (same as before Phase 2)

- [ ] **Step 6: Final commit (if any fixes needed)**

```bash
git add -A
git commit -m "fix: address issues found during manual verification"
```
