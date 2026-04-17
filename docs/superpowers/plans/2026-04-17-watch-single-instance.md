# Watch Mode Single-Instance Enforcement Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce a single running `--watch` daemon and report startup success/failure back to the parent process via the stdout/stderr contract.

**Architecture:** The parent process creates two named Win32 events (`ok`/`err`) keyed to its own PID before forking the daemon. The daemon acquires a global named mutex (single-instance guard), then signals `ok` or `err` at each startup checkpoint. The parent blocks for up to 3 s on both events, reads the result from a temp file on error, and prints the correct `status:` line. On the Restart tray path (no parent waiting), `MessageBoxW` is retained as the sole feedback channel.

**Tech Stack:** Rust, windows crate v0.62, Win32 (CreateMutexW, CreateEventW, WaitForMultipleObjects, OpenEventW, SetEvent, GetCurrentProcessId, GetLastError)

**Spec:** `docs/superpowers/specs/2026-04-17-watch-single-instance-design.md`

---

## Chunk 1: Foundations

### Task 1: Add new error variants to `errors.rs`

**Files:**
- Modify: `src/errors.rs`

- [ ] **Step 1: Write the failing tests**

  Add to the `#[cfg(test)]` block in `src/errors.rs`:

  ```rust
  #[test]
  fn watch_already_running_code() {
      assert_eq!(ShotError::WatchAlreadyRunning.code(), "watch-already-running");
  }

  #[test]
  fn tray_error_code() {
      assert_eq!(ShotError::TrayError("x".into()).code(), "tray-error");
  }

  #[test]
  fn watch_timeout_code() {
      assert_eq!(ShotError::WatchTimeout.code(), "timeout");
  }

  #[test]
  fn watch_already_running_format() {
      let e = ShotError::WatchAlreadyRunning;
      let output = format_error(&e);
      assert!(output.contains("status: error"));
      assert!(output.contains("code: watch-already-running"));
      assert!(output.contains("daemon is already running"));
      assert_eq!(format_error_message(&e), "daemon is already running");
  }

  #[test]
  fn tray_error_format() {
      let e = ShotError::TrayError("cannot add icon".into());
      let output = format_error(&e);
      assert!(output.contains("code: tray-error"));
      assert!(output.contains("cannot add icon"));
      assert_eq!(format_error_message(&e), "Tray icon error: cannot add icon");
  }

  #[test]
  fn watch_timeout_format() {
      let e = ShotError::WatchTimeout;
      let output = format_error(&e);
      assert!(output.contains("code: timeout"));
      assert!(output.contains("daemon did not respond within 3s"));
      assert_eq!(format_error_message(&e), "daemon did not respond within 3s");
  }
  ```

- [ ] **Step 2: Verify tests fail**

  Run: `just test`
  Expected: FAIL — `WatchAlreadyRunning`, `TrayError`, and `WatchTimeout` not defined.

- [ ] **Step 3: Add the variants**

  In the `ShotError` enum, add after `HotkeyError`:

  ```rust
  #[error("daemon is already running")]
  WatchAlreadyRunning,

  #[error("{0}")]
  TrayError(String),

  #[error("daemon did not respond within 3s")]
  WatchTimeout,
  ```

  In `code()`, add after `Self::HotkeyError(_) => "hotkey-error"`:

  ```rust
  Self::WatchAlreadyRunning => "watch-already-running",
  Self::TrayError(_) => "tray-error",
  Self::WatchTimeout => "timeout",
  ```

  In `format_error_message()`, add after the `HotkeyError` arm:

  ```rust
  ShotError::WatchAlreadyRunning => "daemon is already running".into(),
  ShotError::TrayError(s) => format!("Tray icon error: {}", s),
  ShotError::WatchTimeout => "daemon did not respond within 3s".into(),
  ```

- [ ] **Step 4: Verify tests pass**

  Run: `just test`
  Expected: all tests PASS.

- [ ] **Step 5: Commit**

  ```
  git add src/errors.rs
  git commit -m "feat(errors): add WatchAlreadyRunning, TrayError, WatchTimeout variants"
  ```

---

### Task 2: Add `--daemon-parent-pid` to `cli.rs`

**Files:**
- Modify: `src/cli.rs`

- [ ] **Step 1: Write failing tests**

  Add to `#[cfg(test)]` block in `src/cli.rs`:

  ```rust
  #[test]
  fn daemon_parent_pid_parsed() {
      let args = CliArgs::parse_from(["shot", "--watch", "--daemon-parent-pid=12345"]);
      assert_eq!(args.daemon_parent_pid, Some(12345u32));
  }

  #[test]
  fn daemon_parent_pid_defaults_none() {
      let args = CliArgs::parse_from(["shot", "--watch"]);
      assert_eq!(args.daemon_parent_pid, None);
  }
  ```

- [ ] **Step 2: Verify tests fail**

  Run: `just test`
  Expected: FAIL — field `daemon_parent_pid` not found.

- [ ] **Step 3: Add the field to `CliArgs`**

  After the `list_windows` field, add:

  ```rust
  /// Internal: parent PID for watch-mode IPC startup feedback (hidden from --help)
  #[arg(long, hide = true)]
  pub daemon_parent_pid: Option<u32>,
  ```

- [ ] **Step 4: Verify tests pass**

  Run: `just test`
  Expected: PASS.

- [ ] **Step 5: Commit**

  ```
  git add src/cli.rs
  git commit -m "feat(cli): add hidden --daemon-parent-pid arg for watch IPC"
  ```

---

### Task 3: Update `main.rs` and stub `watch::run` signature

**Files:**
- Modify: `src/main.rs`, `src/watch.rs`

- [ ] **Step 1: Update `watch::run` and `run_daemon` signatures**

  In `src/watch.rs`, change:

  ```rust
  pub fn run(cfg: &CaptureConfig) -> Result<(), ShotError> {
  ```

  to:

  ```rust
  pub fn run(cfg: &CaptureConfig, parent_pid: Option<u32>) -> Result<(), ShotError> {
  ```

  Change the tail call at the end of `run()` from `run_daemon(cfg)` to:

  ```rust
  run_daemon(cfg, parent_pid)
  ```

  Change `run_daemon` signature from:

  ```rust
  fn run_daemon(cfg: &CaptureConfig) -> Result<(), ShotError> {
  ```

  to:

  ```rust
  fn run_daemon(cfg: &CaptureConfig, parent_pid: Option<u32>) -> Result<(), ShotError> {
  ```

  Add `let _ = parent_pid;` at the top of `run_daemon` body to suppress the unused warning (will be removed in Task 6).

- [ ] **Step 2: Update `main.rs` call site**

  In `src/main.rs`, change the `Watch` arm:

  ```rust
  cli::Mode::Watch => {
      let cwd = std::env::current_dir()
          .map_err(|e| ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
      let toml = config::find_config(&cwd)?;
      let cfg = config::merge(&args, toml)?;
      watch::run(&cfg, args.daemon_parent_pid)
  }
  ```

- [ ] **Step 3: Run CI**

  Run: `just ci`
  Expected: PASS (no regressions).

- [ ] **Step 4: Commit**

  ```
  git add src/watch.rs src/main.rs
  git commit -m "feat(watch): thread parent_pid through run/run_daemon signatures"
  ```

---

## Chunk 2: IPC Implementation

### Task 4: Temp file helpers (with unit tests)

**Files:**
- Modify: `src/watch.rs`

These helpers are pure file I/O — testable without Win32.

- [ ] **Step 1: Write failing tests**

  Add to the `#[cfg(test)]` block in `src/watch.rs`:

  ```rust
  #[test]
  fn temp_file_roundtrip_hotkey_error() {
      let pid: u32 = 9_999_999;
      write_temp_file(pid, "hotkey-error", "RegisterHotKey failed");
      let err = read_and_delete_temp_file(pid);
      assert!(matches!(err, ShotError::HotkeyError(_)));
      assert!(err.to_string().contains("RegisterHotKey failed"));
      assert!(!temp_file_path(pid).exists());
  }

  #[test]
  fn temp_file_watch_already_running() {
      let pid: u32 = 9_999_998;
      write_temp_file(pid, "watch-already-running", "daemon is already running");
      let err = read_and_delete_temp_file(pid);
      assert!(matches!(err, ShotError::WatchAlreadyRunning));
      assert!(!temp_file_path(pid).exists());
  }

  #[test]
  fn temp_file_tray_error() {
      let pid: u32 = 9_999_997;
      write_temp_file(pid, "tray-error", "Shell_NotifyIcon failed");
      let err = read_and_delete_temp_file(pid);
      assert!(matches!(err, ShotError::TrayError(_)));
      assert!(err.to_string().contains("Shell_NotifyIcon"));
      assert!(!temp_file_path(pid).exists());
  }

  #[test]
  fn temp_file_missing_falls_back() {
      let pid: u32 = 9_999_996;
      let _ = std::fs::remove_file(temp_file_path(pid));
      let err = read_and_delete_temp_file(pid);
      assert!(matches!(err, ShotError::HotkeyError(_)));
  }
  ```

- [ ] **Step 2: Verify tests fail**

  Run: `just test`
  Expected: FAIL — functions not defined.

- [ ] **Step 3: Implement helpers**

  Add near the top of `src/watch.rs` (after imports, before `parse_hotkey`):

  ```rust
  fn temp_file_path(parent_pid: u32) -> std::path::PathBuf {
      let mut path = std::env::temp_dir();
      path.push(format!("shot-err-{}.txt", parent_pid));
      path
  }

  /// Write error code and message to temp file for the parent process to read.
  fn write_temp_file(parent_pid: u32, code: &str, message: &str) {
      let path = temp_file_path(parent_pid);
      let _ = std::fs::write(&path, format!("{}\n{}", code, message));
  }

  /// Read and delete the temp file written by the daemon. Returns the ShotError to report.
  fn read_and_delete_temp_file(parent_pid: u32) -> crate::errors::ShotError {
      use crate::errors::ShotError;
      let path = temp_file_path(parent_pid);
      let content = std::fs::read_to_string(&path).unwrap_or_default();
      let _ = std::fs::remove_file(&path);
      let mut lines = content.lines();
      let code = lines.next().unwrap_or("");
      let message = lines.next().unwrap_or("daemon failed to start");
      match code {
          "watch-already-running" => ShotError::WatchAlreadyRunning,
          "hotkey-error" => ShotError::HotkeyError(message.to_string()),
          "tray-error" => ShotError::TrayError(message.to_string()),
          _ => ShotError::HotkeyError(format!("daemon failed to start: {}", message)),
      }  }
  ```

- [ ] **Step 4: Verify tests pass**

  Run: `just test`
  Expected: PASS.

- [ ] **Step 5: Commit**

  ```
  git add src/watch.rs
  git commit -m "feat(watch): temp file helpers for IPC error passing"
  ```

---

### Task 5: Parent-side IPC — named events + wait

**Files:**
- Modify: `src/watch.rs`

Replace the `!is_detached()` branch in `run()` with event-based startup feedback.

**New imports to add** (extend the existing `use windows::...` blocks in `watch.rs`):

```rust
use windows::Win32::System::Threading::{
    CreateEventW, GetCurrentProcessId, WaitForMultipleObjects,
    // existing imports stay: CreateProcessW, PROCESS_CREATION_FLAGS, ...
};
```

> **Implementor note:** `WaitForMultipleObjects` in windows crate v0.62 may have the signature
> `unsafe fn WaitForMultipleObjects(ncount: u32, lphandles: *const HANDLE, bwaitall: bool, dwmilliseconds: u32) -> u32`
> (raw pointer form). The return value is: `0` = first handle (ok), `1` = second handle (err), `258` = timeout, `0xFFFFFFFF` = failure. Verify with `cargo doc --open` against your local crate version.

- [ ] **Step 1: Update `launch_daemon` to accept and embed parent PID**

  Change the function signature from `fn launch_daemon() -> Result<u32, ShotError>` to `fn launch_daemon(parent_pid: u32) -> Result<u32, ShotError>`.

  After the `for arg in std::env::args().skip(1)` loop, append the IPC arg (no quoting needed — it's a pure integer):

  ```rust
  cmd_line.push(format!(" --daemon-parent-pid={}", parent_pid).as_str());
  ```

- [ ] **Step 2: Replace the `!is_detached()` branch in `run()`**

  Replace the existing branch:

  ```rust
  if !is_detached() {
      let my_pid = unsafe { GetCurrentProcessId() };

      let ok_name = wide_string(&format!("Local\\axygen-shot-ok-{}", my_pid));
      let err_name = wide_string(&format!("Local\\axygen-shot-err-{}", my_pid));

      let ok_event = unsafe {
          CreateEventW(None, false, false, windows::core::PCWSTR(ok_name.as_ptr()))
              .map_err(|e| ShotError::HotkeyError(format!("Cannot create ok event: {}", e)))?
      };
      let err_event = unsafe {
          CreateEventW(None, false, false, windows::core::PCWSTR(err_name.as_ptr()))
              .map_err(|e| {
                  let _ = windows::Win32::Foundation::CloseHandle(ok_event);
                  ShotError::HotkeyError(format!("Cannot create err event: {}", e))
              })?
      };

      let child_pid = match launch_daemon(my_pid) {
          Ok(pid) => pid,
          Err(e) => {
              unsafe {
                  let _ = windows::Win32::Foundation::CloseHandle(ok_event);
                  let _ = windows::Win32::Foundation::CloseHandle(err_event);
              }
              return Err(e);
          }
      };

      // Block up to 3 s for the daemon to signal ok (index 0) or err (index 1).
      // WAIT_OBJECT_0 = 0, WAIT_OBJECT_0+1 = 1, WAIT_TIMEOUT = 258, WAIT_FAILED = 0xFFFFFFFF
      let handles = [ok_event, err_event];
      let wait_result = unsafe {
          WaitForMultipleObjects(2, handles.as_ptr(), false, 3000)
      };

      unsafe {
          let _ = windows::Win32::Foundation::CloseHandle(ok_event);
          let _ = windows::Win32::Foundation::CloseHandle(err_event);
      }

      return match wait_result {
          0 => {
              if !cfg.quiet || cfg.verbose {
                  println!("status: ok\nwatch: started (PID {})", child_pid);
              }
              Ok(())
          }
          1 => Err(read_and_delete_temp_file(my_pid)),
          _ => Err(ShotError::WatchTimeout),
      };
  }
  ```

  > **Note on `wait_result` type:** In windows crate v0.62, `WaitForMultipleObjects` returns `u32`; the snippet above is correct as-is. Do NOT add `.0` — that would be a compile error on a `u32`.

- [ ] **Step 3: Build to catch type errors**

  Run: `cargo build 2>&1`
  Expected: clean build.

- [ ] **Step 4: Run CI**

  Run: `just ci`
  Expected: PASS.

- [ ] **Step 5: Commit**

  ```
  git add src/watch.rs
  git commit -m "feat(watch): parent-side IPC — named events, wait for daemon feedback"
  ```

  > **⚠️ NOTE:** After this commit the system is in an intermediate broken state — the daemon still has `let _ = parent_pid;` (from Task 3) and never signals the events, so every `shot --watch` invocation will timeout after 3 s. Do NOT merge between Task 5 and Task 6; they must be committed together as a logical unit.

---

### Task 6: Daemon-side IPC— mutex + signaling + restart fix

**Files:**
- Modify: `src/watch.rs`

**New imports to add:**

```rust
use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::{
    CreateMutexW, EVENT_MODIFY_STATE, OpenEventW, SetEvent,
};
// GetLastError: use windows::Win32::Foundation::GetLastError (or fully-qualified)
// EVENT_MODIFY_STATE is typed as EVENT_ACCESS_RIGHTS in v0.62 — pass directly to OpenEventW.
// If it won't compile, fall back to: windows::Win32::System::Threading::EVENT_ACCESS_RIGHTS(0x0002)
```

- [ ] **Step 1: Add `open_parent_events` helper**

  ```rust
  fn open_parent_events(
      parent_pid: Option<u32>,
  ) -> (windows::Win32::Foundation::HANDLE, windows::Win32::Foundation::HANDLE) {
      use windows::Win32::Foundation::HANDLE;
      let Some(pid) = parent_pid else {
          return (HANDLE::default(), HANDLE::default());
      };
      let ok_name = wide_string(&format!("Local\\axygen-shot-ok-{}", pid));
      let err_name = wide_string(&format!("Local\\axygen-shot-err-{}", pid));
      unsafe {
          // EVENT_MODIFY_STATE is typed as EVENT_ACCESS_RIGHTS in v0.62 — pass directly.
          // Fallback if it won't compile: windows::Win32::System::Threading::EVENT_ACCESS_RIGHTS(0x0002)
          let access = windows::Win32::System::Threading::EVENT_MODIFY_STATE;
          let ok = OpenEventW(access, false, windows::core::PCWSTR(ok_name.as_ptr()))
              .unwrap_or_default();
          let err = OpenEventW(access, false, windows::core::PCWSTR(err_name.as_ptr()))
              .unwrap_or_default();
          (ok, err)
      }
  }
  ```

- [ ] **Step 2: Add `signal_ok` helper**

  ```rust
  fn signal_ok(ok_event: windows::Win32::Foundation::HANDLE) {
      if ok_event.0 != 0 {
          unsafe {
              let _ = SetEvent(ok_event);
          }
      }
  }
  ```

- [ ] **Step 3: Add `handle_startup_err` helper**

  ```rust
  /// On the IPC path (parent_pid is Some): writes temp file, signals err event, exits the process.
  /// On the restart path (parent_pid is None): shows MessageBoxW and returns.
  /// Caller must `return Err(err)` after this call — it is only reached on the restart path.
  fn handle_startup_err(
      err_event: windows::Win32::Foundation::HANDLE,
      parent_pid: Option<u32>,
      err: &ShotError,
  ) {
      if let Some(pid) = parent_pid {
          write_temp_file(pid, err.code(), &crate::errors::format_error_message(err));
          if err_event.0 != 0 {
              unsafe {
                  let _ = SetEvent(err_event);
              }
          }
          std::process::exit(1);
      } else {
          message_box(
              &crate::errors::format_error_message(err),
              "Axygen Shot — Error",
              MB_ICONERROR,
          );
      }
  }
  ```

- [ ] **Step 4: Update `run_daemon` — single-instance mutex**

  At the very start of `run_daemon` (before `parse_hotkey`), remove `let _ = parent_pid;` and add:

  > **Ordering note:** This block must go before `parse_hotkey` (line ~212). The `DAEMON_CFG` global assignment happens later, after `parse_hotkey` — it is unaffected by this insertion.

  ```rust
  let (ok_event, err_event) = open_parent_events(parent_pid);

  // Single-instance guard: only one daemon may run system-wide.
  let mutex_name = wide_string("Global\\axygen-shot-daemon");
  let _mutex_handle = unsafe {
      match CreateMutexW(None, true, windows::core::PCWSTR(mutex_name.as_ptr())) {
          Ok(h) => {
              // Call GetLastError IMMEDIATELY — before any other Win32 call.
              let last_err = windows::Win32::Foundation::GetLastError();
              if last_err == ERROR_ALREADY_EXISTS || last_err == ERROR_ACCESS_DENIED {
                  let err = ShotError::WatchAlreadyRunning;
                  handle_startup_err(err_event, parent_pid, &err);
                  return Err(err); // Only reached on restart path
              }
              h
          }
          Err(e) => {
              let err = ShotError::HotkeyError(format!("Cannot create instance mutex: {}", e));
              handle_startup_err(err_event, parent_pid, &err);
              return Err(err);
          }
      }
  };
  ```

  > **IMPORTANT:** `_mutex_handle` must remain in scope for the entire `run_daemon` function body. Do NOT wrap it in a block or drop it. The mutex is released by Windows when the process exits.

- [ ] **Step 5: Update `run_daemon` — wrap `parse_hotkey` with IPC signaling**

  Change from:

  ```rust
  let (modifiers, vk) = parse_hotkey(&cfg.hotkey)?;
  ```

  To:

  ```rust
  let (modifiers, vk) = match parse_hotkey(&cfg.hotkey) {
      Ok(v) => v,
      Err(e) => {
          handle_startup_err(err_event, parent_pid, &e);
          return Err(e);
      }
  };
  ```

- [ ] **Step 6: Update `run_daemon` — message window failure**

  Replace the existing window creation failure branch (which currently returns an error without a MessageBox) with:

  ```rust
  let hwnd = match hwnd {
      Ok(h) if !h.0.is_null() => h,
      _ => {
          let err = ShotError::HotkeyError("Cannot create message window".into());
          handle_startup_err(err_event, parent_pid, &err);
          return Err(err);
      }
  };
  ```

- [ ] **Step 7: Update `run_daemon` — RegisterHotKey failure**

  Replace the existing block:

  ```rust
  if let Err(e) = hotkey_result {
      message_box(
          &format!(
              "Cannot register hotkey '{}': {}\n\nThe hotkey may be in use by another application.",
              cfg.hotkey, e
          ),
          "Axygen Shot — Error",
          MB_ICONERROR,
      );
      unsafe {
          let _ = DestroyWindow(hwnd);
      }
      return Err(ShotError::HotkeyError(format!(
          "RegisterHotKey failed for '{}': {}",
          cfg.hotkey, e
      )));
  }
  ```

  With:

  ```rust
  if let Err(e) = hotkey_result {
      unsafe { let _ = DestroyWindow(hwnd); }
      let err = ShotError::HotkeyError(format!(
          "RegisterHotKey failed for '{}': {}",
          cfg.hotkey, e
      ));
      handle_startup_err(err_event, parent_pid, &err);
      return Err(err);
  }
  ```

- [ ] **Step 8: Update `run_daemon` — tray failure**

  Replace the existing block:

  ```rust
  if !tray_ok.as_bool() {
      unsafe {
          let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
          let _ = DestroyWindow(hwnd);
      }
      message_box(
          "Cannot create tray icon. The system tray may not be available.",
          "Axygen Shot — Error",
          MB_ICONERROR,
      );
      return Err(ShotError::HotkeyError("Shell_NotifyIcon failed".into()));
  }
  ```

  With:

  ```rust
  if !tray_ok.as_bool() {
      unsafe {
          let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
          let _ = DestroyWindow(hwnd);
      }
      let err = ShotError::TrayError(
          "cannot create tray icon — system tray may not be available".into(),
      );
      handle_startup_err(err_event, parent_pid, &err);
      return Err(err);
  }
  ```

- [ ] **Step 9: Add `signal_ok` call after tray success, then close event handles**

  Immediately after the tray success block and before `audio::play_startup()`:

  ```rust
  // Signal the parent that daemon started successfully.
  signal_ok(ok_event);
  // Close event handles — no longer needed after signaling.
  unsafe {
      if ok_event.0 != 0 {
          let _ = windows::Win32::Foundation::CloseHandle(ok_event);
      }
      if err_event.0 != 0 {
          let _ = windows::Win32::Foundation::CloseHandle(err_event);
      }
  }
  ```

- [ ] **Step 10: Update `restart()` to strip `--daemon-parent-pid=...`**

  In the `restart()` function, change the arg loop from:

  ```rust
  for arg in std::env::args().skip(1) {
      cmd_line.push(" ");
      cmd_line.push(quote_arg(&arg).as_str());
  }
  ```

  To:

  ```rust
  for arg in std::env::args().skip(1) {
      if arg.starts_with("--daemon-parent-pid") {
          continue; // Internal IPC arg — must not be forwarded to restarted daemon
      }
      cmd_line.push(" ");
      cmd_line.push(quote_arg(&arg).as_str());
  }
  ```

- [ ] **Step 11: Build**

  Run: `cargo build 2>&1`
  Expected: clean build, no unused variable warnings.

- [ ] **Step 12: Run CI**

  Run: `just ci`
  Expected: all tests PASS, zero clippy warnings.

- [ ] **Step 13: Commit**

  ```
  git add src/watch.rs
  git commit -m "feat(watch): daemon-side IPC — mutex, event signaling, restart fix

  - CreateMutexW 'Global\\axygen-shot-daemon' for single-instance enforcement
  - Handle ERROR_ALREADY_EXISTS and ERROR_ACCESS_DENIED (elevation mismatch)
  - GetLastError called immediately after CreateMutexW before any other Win32 call
  - open_parent_events: OpenEventW for ok/err, no-op handles on restart path
  - handle_startup_err: IPC path signals event + exits; restart path uses MessageBoxW
  - signal_ok after successful tray creation; close event handles
  - restart() strips --daemon-parent-pid= to prevent stale event references"
  ```

---

## Chunk 3: Verification

### Task 7: Manual verification

Since IPC involves Win32 handles and a background daemon, unit tests cannot cover the full flow. Verify these scenarios manually after building.

- [ ] **Step 1: Build release**

  Run: `just build`
  Expected: clean build.

- [ ] **Step 2: Normal first launch**

  From a terminal in a project directory with a valid `shot.toml`:

  ```
  shot --watch
  ```

  Expected stdout:
  ```
  status: ok
  watch: started (PID <n>)
  ```

  Expected: tray icon appears, startup sound plays, process exits.

- [ ] **Step 3: Double launch — expect error**

  While the daemon from Step 2 is still running:

  ```
  shot --watch
  ```

  Expected stderr:
  ```
  status: error
  code: watch-already-running
  message: daemon is already running
  ```

  Expected: process exits with code 1. No new tray icon appears. No MessageBox popup.

- [ ] **Step 4: Verify tray Restart works**

  Right-click tray → Restart.
  Expected: old tray icon disappears, new one appears after a moment. Hotkey still works. No MessageBox popup.

- [ ] **Step 5: Verify tray Exit works**

  Right-click tray → Exit.
  Expected: tray icon disappears. Process exits. A subsequent `shot --watch` starts a fresh daemon successfully.

- [ ] **Step 6: Final CI run**

  Run: `just ci`
  Expected: PASS.

- [ ] **Step 7: Commit any fixes found during verification**

  ```
  git add -p
  git commit -m "fix(watch): <description>"
  ```
