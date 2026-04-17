# Axygen Shot Watch Mode — Implementation Design Spec

> **Phase:** 2 (Watch Mode)
> **Stack:** Rust, windows crate v0.62+, clap v4
> **Scope:** PRD stories 9, 23a, 27–34, 59
> **PRD Reference:** `docs/PRD.md`
> **Phase 1 Spec:** `docs/superpowers/specs/2026-04-17-axygen-shot-mvp-design.md`
> **Depends on:** Phase 1 MVP (fully implemented)

## Overview

Phase 2 adds **watch mode** to Axygen Shot: a background daemon that registers a global hotkey and captures the target window on keypress. The daemon shows a system tray icon with a tooltip and right-click menu (Restart, Exit). All errors are reported via `MessageBox` (accessible to screen readers).

## Phase 2 Scope

### Included (User Stories from PRD)

- **Story 9:** `hotkey` field in `shot.toml`
- **Story 23a:** `--hotkey=Win+F11` CLI override
- **Story 27:** `shot.exe --watch` — registers global hotkey, waits in background
- **Story 28:** Tray icon when watch mode is active
- **Story 29:** Configured hotkey triggers capture
- **Story 30:** Right-click tray icon → "Exit"
- **Story 31:** Config read once at startup; tray menu has "Restart"
- **Story 32:** `MessageBox` if no config/CLI args at watch startup
- **Story 33:** Tray tooltip with meaningful text
- **Story 34:** No per-capture label in watch mode (uses window title)
- **Story 59:** Startup sound (`SystemExclamation`) when hotkey registered

### Not Included

- Stories 64–67 (Future/Out of Scope)
- `--verbose` diagnostics (separate enhancement)

## Project Structure Changes

```
src/
├── main.rs          # MODIFIED: Mode::Watch dispatches to watch::run()
├── watch.rs         # NEW: daemon lifecycle, message loop, tray, hotkey
├── audio.rs         # MODIFIED: add play_startup()
├── cli.rs           # NO CHANGES (--watch, --hotkey already parsed)
├── config.rs        # MODIFIED: merge hotkey from CLI to CaptureConfig
└── ... (other modules unchanged)
```

## New Module: `watch.rs`

### Public API

```rust
/// Entry point for watch mode. Called from main.rs when Mode::Watch.
/// Handles both the "launcher" (has console) and "daemon" (no console) roles.
pub fn run(cfg: &CaptureConfig) -> Result<(), ShotError>;
```

### Internal Components

#### Daemon Detection

```rust
fn is_detached() -> bool {
    unsafe { GetConsoleWindow().is_null() }
}
```

Uses `GetConsoleWindow()` from `Win32_System_Console`. When the process has no console (launched with `DETACHED_PROCESS | CREATE_NO_WINDOW`), this returns `NULL` — indicating we are the daemon instance.

**Why this approach:**
- Checks actual OS state, not user-supplied flags
- Works correctly for all launch contexts: terminal, Task Scheduler, Explorer, IDE
- Cannot be accidentally triggered by users
- 1 function call, zero configuration

#### Launcher Phase (has console)

When `is_detached()` returns `false`:

1. Validate config is complete (process/title present) — if not, print error to stderr and exit
   - **Design decision (PRD story 32 deviation):** PRD specifies MessageBox for missing config at watch startup. Since the launcher runs in a console, stderr is more appropriate and consistent with CLI mode. The daemon phase uses MessageBox for all errors (no console available).
2. Re-launch self with same args using `CreateProcessW`:
   - Flags: `CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS | CREATE_NO_WINDOW`
   - Inherits current working directory
   - Passes all original argv unchanged
3. Print to stdout:
   ```
   status: ok
   watch: started (PID <child_pid>)
   ```
4. Exit with code 0

If `--quiet` is set, suppress stdout output (same as CLI mode).

#### Daemon Phase (no console)

When `is_detached()` returns `true`:

1. Parse and validate hotkey string → `(modifiers: u32, vk: u32)`
2. Create a hidden message-only window (`HWND_MESSAGE` parent) for message dispatch
3. Call `RegisterHotKey(hwnd, 1, modifiers | MOD_NOREPEAT, vk)`
   - On failure: `MessageBox` with error text, exit code 1
4. Call `Shell_NotifyIcon(NIM_ADD, ...)` with:
   - Icon: `IDI_APPLICATION` (default system icon)
   - Tooltip: `"shot — watching <process_or_title>"` (max 128 chars per Win32 API)
   - Callback message: `WM_APP + 1`
   - On failure: `MessageBox` with error text, exit code 1
5. Play startup sound: `MessageBeep(MB_ICONEXCLAMATION)` — `SystemExclamation`
6. Enter message loop

#### Message Loop

```rust
loop {
    let mut msg: MSG = zeroed();
    match GetMessageW(&mut msg, None, 0, 0).0 {
        -1 => break, // Error
         0 => break, // WM_QUIT
         _ => {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
```

Window procedure handles:

| Message | Action |
|---------|--------|
| `WM_HOTKEY` | If not capturing: set `capturing = true`, run capture pipeline, play sound, set `capturing = false`. If already capturing: `MessageBeep(MB_ICONQUESTION)` (busy sound). |
| `WM_APP + 1` (tray callback) | If `lParam == WM_RBUTTONUP`: show context menu at cursor position |
| `WM_COMMAND` + `ID_EXIT` (1001) | `PostQuitMessage(0)` |
| `WM_COMMAND` + `ID_RESTART` (1002) | Re-launch self with same args using `CREATE_NEW_PROCESS_GROUP \| DETACHED_PROCESS \| CREATE_NO_WINDOW` (spawns new daemon directly), then `PostQuitMessage(0)` |
| `WM_DESTROY` | Cleanup: `Shell_NotifyIcon(NIM_DELETE)`, `UnregisterHotKey` |

#### Capture Pipeline (in daemon)

On `WM_HOTKEY`, runs the same pipeline as CLI mode but with watch-specific error handling:

```rust
fn do_capture(cfg: &CaptureConfig) {
    let result = (|| -> Result<(), ShotError> {
        let (hwnd, info) = window_resolver::resolve(cfg)?;
        let capture_result = capture::capture_window(hwnd)?;
        let label = None; // No per-capture label in watch mode (PRD story 34)
        let saved = storage::save(&capture_result.png_bytes, cfg, label, &info.title)?;
        clipboard::write_clipboard(
            &cfg.clipboard,
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
```

The capture pipeline reuses existing modules: `window_resolver::resolve()` → `capture::capture_window()` → `storage::save()` → `clipboard::write_clipboard()`.

**No per-capture label** in watch mode (PRD story 34). Window title is used as the filename label.

**Capture serialization:** A `bool` flag (`capturing`) prevents concurrent captures. If hotkey fires during an active capture, play `MessageBeep(MB_ICONQUESTION)` (busy/question sound) and ignore the event. Since everything is single-threaded, no atomic/mutex needed.

#### Context Menu

```rust
fn show_context_menu(hwnd: HWND) {
    let Ok(menu) = CreatePopupMenu() else {
        return; // Silently fail — tray icon still works for Exit via other means
    };
    AppendMenuW(menu, MF_STRING, ID_RESTART, w!("Restart"));
    AppendMenuW(menu, MF_STRING, ID_EXIT, w!("Exit"));

    let mut pt = POINT::default();
    GetCursorPos(&mut pt);

    // Required for TrackPopupMenu to work correctly from tray
    SetForegroundWindow(hwnd);
    TrackPopupMenu(menu, TPM_RIGHTALIGN, pt.x, pt.y, 0, hwnd, None);
    PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0)); // Dismiss fix
    DestroyMenu(menu);
}
```

#### Restart Logic

"Restart" re-launches the exe with the original command-line args:

```rust
fn restart(hwnd: HWND) {
    // Re-launch with original args (will go through launcher → daemon cycle)
    let exe = std::env::current_exe().unwrap();
    let args: Vec<String> = std::env::args().skip(1).collect();
    Command::new(exe)
        .args(&args)
        .creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS | CREATE_NO_WINDOW)
        .spawn()
        .ok();
    PostQuitMessage(0);
}
```

Note: Restart spawns a new daemon directly (with `DETACHED_PROCESS`) — no intermediate console launcher needed since the new instance will also detect `GetConsoleWindow() == NULL` and proceed as daemon.

### Cleanup

On `WM_DESTROY` or loop exit:

```rust
Shell_NotifyIcon(NIM_DELETE, &nid);  // Remove tray icon
UnregisterHotKey(hwnd, 1);           // Release hotkey
```

## Hotkey Parsing

### Format

```
"<Modifier>[+<Modifier>]+<Key>"
```

Case-insensitive. `+` separator. Examples: `"Win+F12"`, `"Ctrl+Shift+F5"`, `"Alt+PrintScreen"`.

### Supported Modifiers

| String | Win32 Constant |
|--------|----------------|
| `Win` | `MOD_WIN` |
| `Ctrl` / `Control` | `MOD_CONTROL` |
| `Shift` | `MOD_SHIFT` |
| `Alt` | `MOD_ALT` |

### Supported Keys

| String | Win32 VK |
|--------|----------|
| `F1`–`F24` | `VK_F1`–`VK_F24` |
| `PrintScreen` / `PrtSc` | `VK_SNAPSHOT` |
| `A`–`Z` | `0x41`–`0x5A` |
| `0`–`9` | `0x30`–`0x39` |

### Validation

- At least one modifier required (Windows `RegisterHotKey` requires it)
- Exactly one key required
- Unknown tokens → `ShotError::ArgError("Invalid hotkey: unknown token '<tok>'. Valid modifiers: Win, Ctrl, Shift, Alt. Valid keys: F1-F24, A-Z, 0-9, PrintScreen")`

### Default

`"Win+F12"` — used when neither config nor CLI specifies a hotkey.

### Implementation

```rust
pub fn parse_hotkey(s: &str) -> Result<(HOT_KEY_MODIFIERS, u32), ShotError> {
    let mut modifiers = HOT_KEY_MODIFIERS(0);
    let mut vk: Option<u32> = None;

    for token in s.split('+') {
        let token = token.trim();
        match token.to_lowercase().as_str() {
            "win" => modifiers |= MOD_WIN,
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "shift" => modifiers |= MOD_SHIFT,
            "alt" => modifiers |= MOD_ALT,
            key => {
                if vk.is_some() {
                    return Err(ShotError::ArgError(
                        format!("Invalid hotkey '{}': multiple keys specified", s)
                    ));
                }
                vk = Some(parse_vk(key)?);
            }
        }
    }

    let vk = vk.ok_or_else(|| ShotError::ArgError(
        format!("Invalid hotkey '{}': no key specified", s)
    ))?;

    if modifiers.0 == 0 {
        return Err(ShotError::ArgError(
            format!("Invalid hotkey '{}': at least one modifier required (Win, Ctrl, Shift, Alt)", s)
        ));
    }

    Ok((modifiers | MOD_NOREPEAT, vk))
}

fn parse_vk(key: &str) -> Result<u32, ShotError> {
    let lower = key.to_lowercase();
    // F1-F24
    if lower.starts_with('f') {
        if let Ok(n) = lower[1..].parse::<u32>() {
            if (1..=24).contains(&n) {
                return Ok(0x6F + n); // VK_F1 = 0x70
            }
        }
    }
    // A-Z
    if lower.len() == 1 {
        let ch = lower.as_bytes()[0];
        if ch.is_ascii_lowercase() {
            return Ok((ch - b'a' + b'A') as u32);
        }
        if ch.is_ascii_digit() {
            return Ok(ch as u32); // VK_0..VK_9 = 0x30..0x39
        }
    }
    match lower.as_str() {
        "printscreen" | "prtsc" => Ok(0x2C), // VK_SNAPSHOT
        _ => Err(ShotError::ArgError(
            format!("Invalid hotkey key '{}'. Valid: F1-F24, A-Z, 0-9, PrintScreen", key)
        )),
    }
}
```

## Changes to Existing Modules

### `main.rs`

Replace the `Mode::Watch` error with actual dispatch:

```rust
Mode::Watch => {
    watch::run(&cfg)?;
}
```

### `config.rs`

Add `hotkey` to `CaptureConfig` and merge logic:

```rust
pub struct CaptureConfig {
    // ... existing fields ...
    pub hotkey: String, // Default: "Win+F12"
}
```

In `merge()`:
```rust
hotkey: cli.hotkey.clone()
    .or(toml.as_ref().and_then(|t| t.hotkey.clone()))
    .unwrap_or_else(|| "Win+F12".to_string()),
```

### `audio.rs`

Add startup sound:

```rust
pub fn play_startup() {
    unsafe { MessageBeep(0x00000030) }; // MB_ICONEXCLAMATION = SystemExclamation
}
```

Add busy sound:

```rust
pub fn play_busy() {
    unsafe { MessageBeep(0x00000020) }; // MB_ICONQUESTION
}
```

### `errors.rs`

Add watch-specific error variants:

```rust
pub enum ShotError {
    // ... existing variants ...
    HotkeyError(String),    // Hotkey registration failed
}
```

Add `format_error_message()` — returns just the message string (for `MessageBox`):

```rust
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

Note: `do_capture` pseudo-code uses simplified call signatures for readability. Implementers should match the actual function signatures from Phase 1 source code.

### `Cargo.toml`

Re-add `Win32_System_Console` feature flag (removed in Phase 1 when AttachConsole was dropped):

```toml
"Win32_System_Console",
```

Add new feature flags for watch mode:

```toml
"Win32_UI_WindowsAndMessaging",  # Already present — TrackPopupMenu, etc.
"Win32_UI_Shell",                # Already present — Shell_NotifyIcon
```

No new feature flags needed — all required APIs are covered by existing flags plus the re-added `Win32_System_Console`.

## MessageBox Helper

```rust
fn message_box(text: &str, title: &str, flags: MESSAGEBOX_STYLE) {
    let text_w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            flags,
        );
    }
}
```

## Testing Strategy

### Unit Tests (in `watch.rs`)

Hotkey parsing is the only pure-logic component testable without Win32:

```rust
#[cfg(test)]
mod tests {
    // parse_hotkey tests:
    // - "Win+F12" → (MOD_WIN | MOD_NOREPEAT, VK_F12)
    // - "Ctrl+Shift+A" → (MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, 0x41)
    // - "Alt+PrintScreen" → (MOD_ALT | MOD_NOREPEAT, VK_SNAPSHOT)
    // - case insensitive: "win+f12" == "Win+F12"
    // - "ctrl+shift+f5" → correct modifiers + VK_F5
    // - "F12" (no modifier) → error
    // - "Win+" (no key) → error
    // - "Win+F12+F5" (two keys) → error
    // - "Win+InvalidKey" → error with helpful message
    // - "" (empty) → error
    // - "Win+0" → (MOD_WIN | MOD_NOREPEAT, 0x30)
    // - "Win+Z" → (MOD_WIN | MOD_NOREPEAT, 0x5A)
    // - "PrtSc" alias works
}
```

### Integration Tests

Watch mode integration tests are limited because they require:
- A real Windows desktop (tray, hotkey registration)
- Background processes

Recommended approach: **manual testing only** for daemon lifecycle. Add items to `docs/manual-testing-checklist.md`.

### Config Tests

Add tests for hotkey merge in `config.rs`:
- Config has hotkey, no CLI → use config value
- CLI has hotkey, no config → use CLI value
- Both have hotkey → CLI wins
- Neither has hotkey → default "Win+F12"

**Note:** Adding `hotkey: String` to `CaptureConfig` will require updating existing test fixtures that construct `CaptureConfig` (there are 7+ tests in `config.rs` that create `CaptureConfig` structs). Each needs a `hotkey` field added.

## Error Codes

| Code | When | How Reported |
|------|------|--------------|
| `hotkey-error` | `RegisterHotKey` fails | MessageBox |
| `window-not-found` | Target window not found during capture | MessageBox |
| `window-minimized` | Target window is minimized | MessageBox |
| `capture-failed` | `PrintWindow` fails | MessageBox |
| `storage-failed` | Cannot write PNG file | MessageBox |
| `config-error` | Invalid config at startup | MessageBox |
| `arg-error` | Invalid hotkey format | stderr (launcher) or MessageBox (daemon) |

## Windows Crate Feature Flags

After Phase 2, the full list (14 flags):

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

## Sequence Diagram: Normal Capture in Watch Mode

```
User presses Win+F12
        │
        ▼
  [WM_HOTKEY received]
        │
        ├─ capturing == true? → play_busy() → done
        │
        ├─ capturing = true
        │
        ▼
  window_resolver::resolve(cfg)
        │
        ├─ Err → play_error() + MessageBox → capturing = false → done
        │
        ▼
  capture::capture_window(hwnd)
        │
        ├─ Err → play_error() + MessageBox → capturing = false → done
        │
        ▼
  storage::save(png, cfg, None)  // label = None in watch mode
        │
        ├─ Err → play_error() + MessageBox → capturing = false → done
        │
        ▼
  clipboard::write_clipboard(mode, path, png, w, h)
        │
        ├─ Err → play_error() + MessageBox → capturing = false → done
        │
        ▼
  play_success()
  capturing = false
```

## Known Limitations

1. **Tray icon is generic** — uses `IDI_APPLICATION` (default system icon). Not visually distinctive but tooltip is meaningful for screen reader users.
2. **Single-threaded capture** — tray is briefly unresponsive (~500ms) during capture. PRD explicitly accepts this.
3. **No capture label** — watch mode uses window title as label (PRD story 34).
4. **Config not reloaded** — config is read once at startup. Changes require Restart from tray menu (PRD story 31).
5. **`GetConsoleWindow()` for daemon detection** — if launched from a context with no console (e.g., Task Scheduler), goes directly to daemon mode. This is desirable behavior.
