# CLAUDE.md — Axygen Shot

## Build / Test / Lint

```
cargo build                          # debug build
cargo build --release                # release build (LTO + strip)
cargo test                           # all unit + integration tests
cargo clippy -- -D warnings          # lint (zero warnings policy)
cargo fmt -- --check                 # format check
```

## Project structure

- `src/main.rs` — entry point, AttachConsole, mode dispatch
- `src/cli.rs` — clap derive struct, Mode enum
- `src/config.rs` — TOML parsing, walk-up discovery, merge, init
- `src/window_resolver.rs` — WindowEnumerator trait, Win32 impl, resolve/list
- `src/capture.rs` — PrintWindow → HBITMAP → PNG
- `src/storage.rs` — filename generation, sanitization, file write
- `src/clipboard.rs` — CF_UNICODETEXT and/or CF_DIB
- `src/audio.rs` — MessageBeep wrappers
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

- DPI awareness MUST be set before any window enumeration
- PrintWindow PW_RENDERFULLCONTENT = 0x00000002 (undocumented flag)
- AttachConsole must happen before any stdout/stderr output
- CF_DIB (not CF_BITMAP) for clipboard — device-independent
- windows crate feature flags may need adjustment — check docs.rs
- All modules are stateless — no global mutable state
