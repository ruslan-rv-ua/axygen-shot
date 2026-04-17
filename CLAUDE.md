# CLAUDE.md — Axygen Shot

## Build / Test / Lint

```
just build-fast                      # fast debug build
just build                           # release build, minimum exe size
just test                            # all unit + integration tests
just lint                            # clippy (zero warnings policy)
just fmt-check                       # format check
just ci                              # lint + fmt-check + test
just size                            # build + show binary size
just run -- --list-windows           # run with args
```

## Project structure

- `src/main.rs` — entry point, mode dispatch
- `src/cli.rs` — clap derive struct, Mode enum
- `src/config.rs` — TOML parsing, walk-up discovery, merge, init
- `src/window_resolver.rs` — WindowEnumerator trait, Win32 impl, resolve/list
- `src/capture.rs` — PrintWindow → HBITMAP → PNG
- `src/storage.rs` — filename generation, sanitization, file write
- `src/clipboard.rs` — CF_UNICODETEXT and/or CF_DIB
- `src/audio.rs` — MessageBeep wrappers
- `src/watch.rs` — watch mode daemon: hotkey parsing, tray icon, message loop
- `src/errors.rs` — ShotError enum, format_error/format_success

## Contracts

stdout output format is a contract — field names and order must not change:

```
status: ok
file: <absolute path>
window: <title> (PID <pid>)
size: <width>x<height>
```

stderr error format:

```
status: error
code: <error-code>
message: <human-readable>
```

## Gotchas

- DPI awareness set via manifest (shot.manifest) — PerMonitorV2
- PrintWindow PW_RENDERFULLCONTENT = 0x00000002 (undocumented flag)
- CF_DIB (not CF_BITMAP) for clipboard — device-independent
- GlobalFree and MessageBeep use raw FFI — not exposed in windows crate v0.62
- windows crate v0.62: some APIs moved (PrintWindow → Xps, CF_DIB → Ole)
- All modules are stateless — no global mutable state
- Watch mode daemon detected via GetConsoleWindow() — no console = daemon mode
- RegisterHotKey requires at least one modifier (Win, Ctrl, Shift, Alt)
- Tray tooltip max 128 UTF-16 chars (szTip field limit)
- `[profile.dev]` has `panic = "abort"` to prevent panic UB through extern "system" wndproc
