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

- If `GetLastError() == ERROR_ALREADY_EXISTS` → daemon signals the err event (if available) and exits cleanly. No `MessageBoxW`.
- The mutex handle is held for the lifetime of the daemon process. Windows releases it automatically on process exit.
- `"Global\\"` prefix ensures the mutex is visible across sessions (Terminal Server / Fast User Switching safe).

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
parent_pid = cfg.daemon_parent_pid  // from --daemon-parent-pid arg
ok_event  = OpenEventW(EVENT_MODIFY_STATE, FALSE, "Local\\axygen-shot-ok-{parent_pid}")
err_event = OpenEventW(EVENT_MODIFY_STATE, FALSE, "Local\\axygen-shot-err-{parent_pid}")
// Both may be NULL (e.g., Restart path) — handled gracefully
```

The daemon calls `signal_ok(ok_event)` or `signal_err(err_event, message)` at each exit point. If the handle is `NULL`, the call is a no-op.

#### 3. Error message passing — temp file

Before signaling the err event, the daemon writes:

```
%TEMP%\shot-err-{parent_pid}.txt
```

Content: single line with the error message (UTF-8). The parent reads and deletes this file after waking on the err event. If the file is absent, a generic fallback message is used.

#### 4. Internal CLI arg `--daemon-parent-pid`

`launch_daemon` appends `--daemon-parent-pid {pid}` to the reconstructed command line. This arg is:

- Parsed by `cli.rs` with `hide = true` (invisible in `--help`).
- Stored in `CaptureConfig` as `daemon_parent_pid: Option<u32>`.
- Filtered out in `restart()` — the `restart` function skips any arg that starts with `--daemon-parent-pid` when reconstructing the command line.

**Why not an env var?** `CreateProcessW` with `lpEnvironment = NULL` inherits the parent environment. A daemon restarting via the tray would pass the env var to the new daemon, which would incorrectly try to open stale event handles. The CLI arg approach requires explicit filtering in `restart()` but is more deterministic.

---

## Error Scenarios

| Scenario | Daemon action | Parent output |
|---|---|---|
| Normal first start | `signal_ok` | `status: ok\nwatch: started (PID …)` |
| Mutex already exists (double launch) | write temp file, `signal_err` | `status: error\ncode: watch-already-running\nmessage: daemon is already running` |
| `RegisterHotKey` fails | write temp file, `signal_err` | `status: error\ncode: hotkey-error\nmessage: …` |
| `Shell_NotifyIconW` fails | write temp file, `signal_err` | `status: error\ncode: hotkey-error\nmessage: …` |
| Daemon crashes / timeout | — | `status: error\ncode: timeout\nmessage: daemon did not respond within 3s` |
| Restart (no parent waiting) | `OpenEventW` returns NULL, no-op | *(no parent process)* |

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
- Cross-user daemon detection (`"Global\\"` mutex handles this implicitly).

---

## Files Changed

| File | Change |
|---|---|
| `src/watch.rs` | Add mutex creation, event signaling, `signal_ok`/`signal_err` helpers, update `launch_daemon` to create/wait events, update `restart` to strip `--daemon-parent-pid` |
| `src/cli.rs` | Add `daemon_parent_pid: Option<u32>` with `hide = true` |
| `src/config.rs` | Add `daemon_parent_pid: Option<u32>` to `CaptureConfig`, propagate from `CliArgs` |
