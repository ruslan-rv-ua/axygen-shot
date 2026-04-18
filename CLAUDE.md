# CLAUDE.md

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.


---

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
