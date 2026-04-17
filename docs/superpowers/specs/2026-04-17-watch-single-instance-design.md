# Watch Mode: Single-Instance Enforcement & Parent Feedback

**Date:** 2026-04-17
**Status:** Approved for implementation

---

## Problem

Running `shot --watch` twice produces a false-positive result:

1. The second invocation prints `status: ok\nwatch: started (PID …)` immediately (the parent process exits before the daemon starts).
2. The second daemon attempts `RegisterHotKey` and fails (hotkey already taken).
3. The failure surfaces only as a `MessageBoxW` popup in the background — never reaches the caller's stdout/stderr.

Additionally, there is no mechanism preventing two daemons from running simultaneously with different hotkeys, leaving two tray icons.

## Goal

- Enforce exactly one running daemon globally.
- Report daemon startup success/failure back to the parent process via the stdout/stderr contract.
- Preserve the `Restart` tray action without regression.

---

## Design

### Components

#### 1. Named Mutex — single-instance guard

When `run_daemon` starts, it calls:

```rust
CreateMutexW(NULL, bInitialOwner=TRUE, "Global\\axygen-shot-daemon")
```

- Call `GetLastError()` **immediately in the same `unsafe` block**, before any other Win32 call. Any intervening call may overwrite the last-error slot.
- If `GetLastError() == ERROR_ALREADY_EXISTS` **or** `== ERROR_ACCESS_DENIED` → treat as "daemon already running" (the `ACCESS_DENIED` case occurs when a non-elevated caller tries to create a mutex already held by an elevated process). Daemon signals the err event (if available) and exits cleanly. No `MessageBoxW`.
- The mutex handle is held for the lifetime of the daemon process. Windows releases it automatically on process exit (crash or clean shutdown — the kernel object is destroyed when the last handle closes, so no "abandoned mutex" scenario).
- `"Global\\"` prefix means cross-session mutual exclusion: only one daemon runs system-wide regardless of Windows session. This is intentional — on a single machine, one daemon is sufficient. On a multi-user RDS/TS server, user B cannot run a daemon while user A's is active. Acceptable trade-off for a developer tool.

#### 2. Named Events — startup feedback to parent

**Parent side** (in `watch::run`, before `launch_daemon`):

```rust
pid = GetCurrentProcessId()
ok_event  = CreateEventW(NULL, manual_reset=FALSE, initial=FALSE, "Local\\axygen-shot-ok-{pid}")
err_event = CreateEventW(NULL, manual_reset=FALSE, initial=FALSE, "Local\\axygen-shot-err-{pid}")
```

After `CreateProcessW`:

```rust
WaitForMultipleObjects([ok_event, err_event], wait_all=FALSE, timeout=3000ms)
```

- `WAIT_OBJECT_0` (ok): print `status: ok\nwatch: started (PID …)`.
- `WAIT_OBJECT_0 + 1` (err): read error message from temp file, print to stderr, exit 1.
- `WAIT_TIMEOUT`: print `status: error / code: timeout / message: daemon did not respond`, exit 1.

Both event handles are closed after the wait.

**Daemon side** (in `run_daemon`):

```rust
parent_pid = // from --daemon-parent-pid=N arg (passed to run_daemon, not via CaptureConfig)
// Restart path: parent_pid is None → skip OpenEventW entirely (no parent waiting)
let (ok_event, err_event) = if let Some(pid) = parent_pid {
    (
        OpenEventW(EVENT_MODIFY_STATE, FALSE, "Local\\axygen-shot-ok-{pid}"),
        OpenEventW(EVENT_MODIFY_STATE, FALSE, "Local\\axygen-shot-err-{pid}"),
    )
} else {
    (NULL, NULL)
};
```

The daemon calls `signal_ok(ok_event)` or `signal_err_or_popup(err_event, parent_pid, message)` at each exit point. If the handle is `NULL` (restart path), `signal_ok` is a no-op and `signal_err_or_popup` falls back to `MessageBoxW`.

**MessageBoxW policy:** `signal_err` and `MessageBoxW` are mutually exclusive per path:

```rust
fn signal_err_or_popup(event: HANDLE, parent_pid: Option<u32>, msg: &str) {
    if parent_pid.is_some() {
        // Normal start: parent is waiting — communicate via event + temp file.
        // Do NOT call MessageBoxW (would block and risk parent timeout).
        write_temp_file_and_signal(event, msg);
    } else {
        // Restart path: no parent process — MessageBoxW is the only feedback channel.
        message_box(msg, "Axygen Shot — Error", MB_ICONERROR);
    }
}
```

All existing `MessageBoxW` calls in `run_daemon` (RegisterHotKey failure, tray failure, mutex failure) are replaced with `signal_err_or_popup`.

#### 3. Error message passing — temp file

Before signaling the err event, the daemon writes:

```
%TEMP%\shot-err-{parent_pid}.txt
```

Content: single line with the error message (UTF-8). The parent reads and deletes this file after waking on the err event. If the file is absent, a generic fallback message is used.

#### 4. Internal CLI arg `--daemon-parent-pid`

`launch_daemon` appends `--daemon-parent-pid={pid}` (single-token `=` form) to the reconstructed command line. This arg is:

- Parsed by `cli.rs` with `hide = true` (invisible in `--help`).
- Stored in `CliArgs` as `daemon_parent_pid: Option<u32>` — **not** in `CaptureConfig`.
- Filtered out in `restart()` using `starts_with("--daemon-parent-pid")` — the single-token `=` form ensures this predicate removes the whole arg without leaving an orphaned value.

`daemon_parent_pid` is **not** added to `CaptureConfig`. It is IPC lifecycle plumbing, not capture configuration. Instead, `watch::run` reads it from `CliArgs` before calling `run_daemon`, and passes it explicitly:

```rust
pub fn run(cfg: &CaptureConfig, parent_pid: Option<u32>) -> Result<(), ShotError>
```

`run_daemon` receives it as a second parameter and uses it only for event signaling. `CaptureConfig`, `merge()`, and `do_capture()` remain unaware of IPC.

**Why not an env var?** `CreateProcessW` with `lpEnvironment = NULL` inherits the parent environment. A daemon restarting via the tray would pass the env var to the new daemon, which would incorrectly try to open stale event handles. The CLI arg approach requires explicit filtering in `restart()` but is more deterministic.

---

## Error Scenarios

| Scenario | Daemon action | Parent output |
|---|---|---|
| Normal first start | `signal_ok` | `status: ok\nwatch: started (PID …)` |
| Mutex already exists (double launch) | write temp file, `signal_err` | `status: error\ncode: watch-already-running\nmessage: daemon is already running` |
| `RegisterHotKey` fails | write temp file, `signal_err` | `status: error\ncode: hotkey-error\nmessage: …` |
| `Shell_NotifyIconW` fails | write temp file, `signal_err` | `status: error\ncode: tray-error\nmessage: …` |
| Daemon crashes / timeout | — | `status: error\ncode: timeout\nmessage: daemon did not respond within 3s` |
| Restart failure (no parent waiting) | `MessageBoxW` (no parent) | *(no parent process)* |

---

## Sequence Diagrams

### Normal first launch

```
parent                          daemon
  │                               │
  ├─ CreateEventW(ok, err)        │
  ├─ CreateProcessW ─────────────►│
  ├─ WaitForMultipleObjects       │
  │  (blocking, ≤3s)             ├─ CreateMutex → OK
  │                               ├─ RegisterHotKey → OK
  │                               ├─ Shell_NotifyIconW → OK
  │                               ├─ SetEvent(ok) ──────────────────────────►│
  ├─ WAIT_OBJECT_0 ◄──────────────┤                                           │
  ├─ print "status: ok"           ├─ message loop …
  └─ exit                         │
```

### Double launch

```
parent2                         daemon2
  │                               │
  ├─ CreateEventW(ok2, err2)      │
  ├─ CreateProcessW ─────────────►│
  ├─ WaitForMultipleObjects       │
  │  (blocking, ≤3s)             ├─ CreateMutex → ERROR_ALREADY_EXISTS
  │                               ├─ write %TEMP%\shot-err-{pid2}.txt
  │                               ├─ SetEvent(err2) ────────────────────────►│
  ├─ WAIT_OBJECT_0+1 ◄────────────┤                                           │
  ├─ read temp file               ├─ exit
  ├─ print "status: error / already running"
  └─ exit 1
```

---

## Out of Scope

- `--once` flag (separate feature, separate spec).
- Multiple daemons per user session (deliberately prevented by this design).
- Cross-user daemon detection (`"Global\\"` mutex enforces system-wide single instance; on RDS/TS servers user B cannot start a daemon while user A's is active — acceptable trade-off for a developer tool).

---

## Files Changed

| File | Change |
|---|---|
| `src/watch.rs` | Add mutex creation, `signal_err_or_popup`/`signal_ok` helpers, update `launch_daemon` to create/wait events (and append `--daemon-parent-pid={pid}`), update `restart` to strip `--daemon-parent-pid=…`, update `run_daemon` signature to accept `parent_pid: Option<u32>` |
| `src/cli.rs` | Add `daemon_parent_pid: Option<u32>` with `hide = true` |
| `src/main.rs` | Read `args.daemon_parent_pid` and pass to `watch::run(cfg, parent_pid)` |
