# AGENTS.md — Axygen Shot

Refer to CLAUDE.md for full details. Key commands:

## Quick Reference

```
Build:  cargo build
Test:   cargo test
Lint:   cargo clippy -- -D warnings
Format: cargo fmt -- --check
```

## Module Map

```
cli → config → window_resolver → capture → storage → clipboard → audio
```

All modules in `src/`. Each file = one module.
WindowResolver uses trait-based DI (WindowEnumerator trait).

## stdout/stderr contract

Do NOT change field names or order in format_success/format_error output.
See CLAUDE.md for exact format.
