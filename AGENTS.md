# AGENTS.md — Axygen Shot

Refer to CLAUDE.md for full details. Key commands:

## Quick Reference

```
Build fast: just build-fast
Build min:  just build
Test:       just test
Lint:       just lint
Format:     just fmt-check
CI:         just ci
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
