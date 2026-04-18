# PRD: Axygen Shot — Screenshot Tool for Blind Developers

## Problem Statement

As a totally blind Windows developer, I cannot visually inspect the appearance of the desktop applications I build. The built-in Windows screenshot tools are cumbersome to configure and don't behave as needed. I require a fast, minimal-friction workflow to:

1. Capture the window of the application I am currently developing (not whatever window happens to be active)
2. Save the screenshot automatically with a structured filename
3. Immediately receive confirmation that the capture succeeded (without visual feedback)
4. Hand the screenshot off to an AI assistant for visual description — using any AI tool, without tool-specific integration

## Solution

**Axygen Shot** (`shot.exe` + `shot-watch.exe`) — two portable Windows executables (`shot.exe` for CLI, `shot-watch.exe` for the tray daemon). This dual-binary split fixes a shell prompt race condition inherent to GUI-subsystem executables with `AttachConsole` (see [ADR-002](ADR-002-dual-binaries.md)). Per-project configuration lives in `shot.toml` at the project root and specifies which window to capture. On invocation, the tool finds the target window, captures it, saves it to a local `screenshots/` folder with a timestamped name, plays a confirmation beep, and copies the result to the clipboard.

Two invocation modes:
- **CLI mode** (`shot.exe [label]`): captures once and exits — called from a terminal or shortcut
- **Watch mode** (`shot.exe --watch`): runs in the background as a tray process, captures on a global hotkey

## User Stories

### Configuration

1. As a developer, I want to create a `shot.toml` file in my project root so that `shot.exe` knows which application window to capture for this project.
2. As a developer, I want to identify the target window by process name (e.g. `process = "myapp.exe"`) so that the tool captures my app regardless of window title changes.
3. As a developer, I want to identify the target window by a title substring (e.g. `title = "MyRadio"`) as an alternative to process name, so that any window whose title contains that text is matched — regardless of where in the title the substring appears and regardless of dynamic title changes (e.g. "MyRadio", "MyRadio - Station 1", "Station 1 -- MyRadio" are all matched by `title = "MyRadio"`).
4. As a developer, I want to specify both `process` and `title` in the config so that the tool matches any window that satisfies **either** condition (OR logic), giving me two independent ways to locate the window. This is useful when the process name alone is ambiguous or when the title is more reliable.
5. As a developer, I want to set the output subfolder name in config (defaulting to `screenshots`) so that screenshots are organized within my project.
6. As a developer, I want to set the default clipboard mode in config (`path`, `image`, or `both`) so that pasting into AI chats works without extra steps.
7. As a developer, I want the config file to support comments (via `#`) so that I can document my settings.
8. As a developer, I want the config file to be in TOML format so that it is easy to read and edit with a screen reader.
9. As a developer, I want a `hotkey` field in `shot.toml` so that I can configure the global hotkey for watch mode on a per-project basis.
10. As a developer, I want `shot.exe` to search for `shot.toml` by walking up from the current directory to the nearest `.git` boundary (or filesystem root if no `.git` exists), so that I can invoke the tool from any subdirectory of the project without specifying the config path.
11. As a developer, I want a clear error message listing the directories that were searched when `shot.toml` is not found, so I understand what went wrong.
12. As a developer, I want the `folder` value in `shot.toml` to be validated as a relative path (no absolute paths, no `..` traversal, no leading `\`, no UNC paths like `\\server\share`, no Windows device paths like `\\.\pipe\name`), so that screenshots always stay within the project directory tree.

### Initialization

13. As a developer, I want to run `shot.exe --init` in a new project directory to create a commented `shot.toml` template, so I can start the configuration from a known-good baseline. The template should include inline comments explaining each field. `--init` writes `shot.toml` **first**, then appends `screenshots/` to `.gitignore` (or creates it) in the CWD — in that order, so that if `.gitignore` modification fails, `shot.toml` is still present and the error is clear. Screenshots are not accidentally committed to version control.
14. As a developer, I want to run `shot.exe --init --process=myapp.exe` or `shot.exe --init --title="MyApp"` (or both) to create a pre-filled `shot.toml` without opening a text editor, so the tool is immediately usable after one command.
15. As a developer, I want `--init` to fail with a clear error if `shot.toml` already exists in the current directory, so I don't accidentally overwrite an existing config.

### CLI Mode

16. As a developer, I want to run `shot.exe` from the terminal in my project directory so that it captures the configured window immediately.
17. As a developer, I want to run `shot.exe my-label` so that the saved filename contains my label instead of the window title. Labels with spaces must be quoted (e.g. `shot.exe "login form"`).
18. As a developer, I want to run `shot.exe --process=myapp.exe` to specify the target process on the command line without a config file, so I can use the tool in one-off situations.
19. As a developer, I want to run `shot.exe --title="MyApp"` to specify the target window title on the command line without a config file.
20. As a developer, I want CLI arguments to take precedence over `shot.toml` values so that I can override any config field for a single run.
21. As a developer, I want `shot.toml` to be optional — if neither a config file nor the required CLI arguments (`--process` or `--title`) are provided, I want a descriptive error message and a non-zero exit code.
22. As a developer, I want to run `shot.exe --clipboard=image` to override the clipboard mode for a single capture.
23. As a developer, I want to run `shot.exe --folder=temp` to override the output folder for a single capture. The value must be a relative path (same validation as the config `folder` field).
23a. As a developer, I want to run `shot.exe --watch --hotkey=Win+F11` to override the hotkey for a single watch session without editing `shot.toml`, so I can resolve conflicts when the default hotkey is claimed by another app.
24. As a developer, I want to use `--quiet` to suppress all stdout output (errors still go to stderr), so the tool can be embedded in build scripts without unwanted noise. If both `--quiet` and `--verbose` are passed, `--verbose` takes precedence.
25. As a developer, I want an error message and non-zero exit code if the target window cannot be found, so I know the app is not running.
26. As a developer, I want the tool to exit immediately after capturing, so it doesn't linger in the background unexpectedly.

### Watch Mode

27. As a developer, I want to run `shot.exe --watch` so that the tool registers a global hotkey and waits in the background.
28. As a developer, I want a tray icon to appear when watch mode is active so that I can confirm the tool is running.
29. As a developer, I want to press the configured hotkey to trigger a capture without switching to a terminal.
30. As a developer, I want to right-click the tray icon and select "Exit" so that I can stop the watch mode cleanly.
31. As a developer, I want watch mode to read configuration (from `shot.toml` or CLI args) once at startup, so configuration changes require a restart. The tray right-click menu should include a "Restart" option for convenience.
32. As a developer, I want a `MessageBox` dialog (readable by screen reader) to appear if neither a config file nor required CLI arguments are present when watch mode starts.
33. As a developer, I want the tray icon tooltip to display a meaningful text (e.g. `shot.exe — watching myapp.exe`) so that screen readers can announce it when the user navigates to the tray area.
34. As a developer, I accept that in watch mode it is not possible to specify a per-capture label (since there is no interactive prompt at hotkey press time); all captures in a watch session will use the window title as the label. This is a known limitation.

### Screenshot Capture

35. As a developer, I want the tool to capture only the target application window (not the full screen) so that the AI receives a focused, relevant image.
36. As a developer, I want the capture to work even if the target window is not currently focused or in the foreground.
37. As a developer, I want the capture to work even if the target window is partially obscured by other windows (using PrintWindow API).
38. As a developer, I want the captured screenshot to be saved as PNG for lossless quality.
39. As a developer, I accept that when multiple windows match the configured criteria, the tool prefers the **foreground (currently active) window** if it is among the matches; otherwise the first window found by OS enumeration order is captured. This makes the behavior predictable when both a dev build and a release build are running simultaneously.

### File Naming and Storage

40. As a developer, I want each screenshot saved as `YYYY-MM-DD_HHMMSS_<label-or-window-title>.png` so that files sort chronologically and are self-describing.
41. As a developer, I want characters that are invalid in Windows filenames (`:`, `\`, `/`, `*`, `?`, `"`, `<`, `>`, `|`) to be replaced with `-` in the label or window title portion of the filename, so that the tool never fails due to an unpredictable window title.
42. As a developer, I want the `screenshots/` folder to be created automatically if it does not exist.
43. As a developer, I want the output folder to be resolved relative to the directory where `shot.toml` was found when a config file is used, so screenshots land inside the project.
44. As a developer, I want the output folder to be resolved relative to the CWD when no config file is used (CLI-only mode), so I have clear control over output location.

### Clipboard

45. As a developer, I want the absolute file path copied to the clipboard by default so that I can paste it into any AI chat.
46. As a developer, I want the `--clipboard=image` option to copy the bitmap to the clipboard so that I can paste the image directly into AI tools that accept pasted images.
47. As a developer, I want the `--clipboard=both` option to write both the path (as text) and the bitmap to the clipboard simultaneously so that the receiving application can choose the format it prefers.

### Discovery

48. As a developer, I want to run `shot.exe --list-windows` to print a structured list of all currently visible top-level windows (title, process name, PID) in `key: value` format, so that my screen reader can read it and I can identify the correct `process` or `title` value without any visual tools. This command does not require a config file or `--process`/`--title` arguments.
49. As a developer, I want `--list-windows` output to exclude system-internal windows (no title, invisible, shell/tray windows) so that the list stays short and relevant.

### Terminal Output

50. As a developer, I want successful captures to print structured `key: value` output to stdout so that both my screen reader and AI coding agents can parse the result reliably. All four fields are always present: `status`, `file`, `window`, and `size`.
51. As a developer, I want errors to print `status: error`, `code:`, and `message:` fields to stderr, so that the failure is both human-readable and machine-parseable.
52. As a developer, I want to use `--quiet` to suppress all stdout output (errors still go to stderr), so the tool can be used in scripting contexts without unwanted noise. If both `--quiet` and `--verbose` are passed, `--verbose` takes precedence.
53. As a developer, I want to use `--verbose` to print additional diagnostic lines to **stderr** (not stdout), so that the structured stdout output remains clean and parseable while I can still read diagnostics.
54. As a developer, I want to run `shot.exe --version` to print the current version (e.g. `shot 1.0.0`), so I can confirm which version is installed.
55. As a developer, I want to run `shot.exe --check` to validate the current configuration without taking a screenshot, producing `key: value` output on stdout:
    ```
    status: ok
    config: C:\dev\myapp\shot.toml
    window: running (myapp.exe, PID 1234)
    folder: C:\dev\myapp\screenshots
    ```
    `--check` requires the same target arguments as a normal capture (`--process` or `--title` in config or CLI).
56. As a developer, I want mode flags (`--version`, `--check`, `--list-windows`, `--init`, `--watch`) to be mutually exclusive — specifying more than one at the same time produces an error, so that the tool's behavior is always unambiguous.

### Feedback

57. As a developer, I want a short audio beep (`SystemAsterisk` sound) to play after a successful capture so that I receive non-visual confirmation.
58. As a developer, I want a distinct error sound (`SystemHand`) to play if the capture fails so that I am not left uncertain.
59. As a developer, I want a startup sound (`SystemExclamation`) to play when `--watch` mode starts successfully (hotkey registered), so I know the daemon is ready without looking at the tray area.
60. As a developer, I want error messages written to stderr (CLI mode) or shown in a `MessageBox` (watch mode) so that my screen reader can read them.

### Window Capture Edge Cases

61. As a developer, I want a clear `code: window-minimized` error (with a distinct sound) if the target window is minimized at capture time, so I know to restore it first.
62. As a developer, I want a clear `code: capture-failed` error if `PrintWindow` fails, with a human-readable message identifying the likely cause. Common causes include: the target process is running with administrator privileges while `shot.exe` is not (message: "try running shot.exe as administrator"); the window uses hardware-accelerated rendering (WebView2, Direct3D, OpenGL) that is incompatible with `PrintWindow` (message: "window uses GPU rendering; try disabling hardware acceleration in the target app if possible").
63. As a developer, I want a clear `code: storage-failed` error if the screenshot file cannot be written (disk full, insufficient permissions, invalid path), with a message explaining the cause, so I know the capture succeeded but saving failed.

### Future (Out of Scope for MVP)

64. As a developer, I want to run `shot.exe "describe the navigation bar"` to append a text prompt to the clipboard output alongside the path.
65. As a developer, I want a clipboard template in config so that I can customize the text that surrounds the path (e.g. for different AI tools).
66. As a developer, I want auto-capture triggered when the target process restarts (after a rebuild) so that I don't need to manually invoke the tool after each build.
67. As a developer, I want to specify multiple named targets in `shot.toml` and select one by label on invocation.

## Implementation Decisions

### Modules

- **Config** — Walks up from CWD looking for `shot.toml`, stopping when a `.git` entry (file **or** directory — handles both regular repos and git worktrees) is found, or at the filesystem root. `--init` is an exception: it **never** walks up and always creates `shot.toml` in the current working directory. Parses the file and returns a typed config struct along with the project root directory. Validates that `folder` is a relative path with no `..` segments. Fails fast with a message listing all searched directories if the file is not found and no CLI overrides supply the required fields.
- **WindowResolver** — Enumerates top-level windows via `EnumWindows`. Matches by process name (via `GetWindowThreadProcessId` + `OpenProcess`) and/or window title substring (case-insensitive, anywhere in title). When both `process` and `title` are specified, a window matches if **either** condition is satisfied (OR logic). When multiple windows match, prefers the current foreground window (`GetForegroundWindow`) if it is among the matches, otherwise takes the first found. Sanitizes matched window titles for use in filenames (replaces invalid characters with `-`). Also provides a list-all mode for `--list-windows`. **Must be designed with an injectable window enumeration function** (dependency injection) to enable unit testing without a real Windows desktop environment.
- **Capture** — Given HWND, checks window visibility and minimized state before capture. Prefers the foreground window when multiple windows match the criteria. Uses `PrintWindow` (with the `PW_RENDERFULLCONTENT` flag, value `0x00000002`) to capture the window into a `HBITMAP`, then converts to PNG bytes. **Note:** `PW_RENDERFULLCONTENT` is an undocumented Win32 internal flag present since Windows 8.1; it enables capturing windows rendered via DWM/DirectComposition. It may not be available in future Windows versions — this is an accepted risk for MVP. `IDXGIOutputDuplication` is the documented fallback path and may be used in a future version. **Known limitation:** `PrintWindow` may return a blank or incorrect image for some hardware-accelerated windows (DRM-protected video, sandboxed processes, some Electron configurations). The tool documents this in the `code: capture-failed` error and advises the user accordingly. **`PrintWindow` is a blocking call** — it sends `WM_PRINT` to the target window and waits for the response; capture duration depends on the target app and is typically under 500ms. Captures run on the message loop thread in watch mode — the tray will be briefly unresponsive during capture; this is acceptable for MVP. Returns specific error codes: `window-minimized` if the window is minimized, `capture-failed` if `PrintWindow` returns failure, `storage-failed` if the PNG file cannot be written.
- **Storage** — Given config and optional label, builds the output path, ensures the directory exists, writes the PNG file.
- **ClipboardWriter** — Opens the Win32 clipboard, writes `CF_UNICODETEXT` (path) and/or `CF_DIB` (Device Independent Bitmap) depending on the configured mode. `CF_DIB` is used instead of `CF_BITMAP` because it is device-independent and correctly interpreted by modern browsers, Electron apps, and AI chat interfaces (Claude.ai, VS Code Copilot Chat) when the user pastes the image directly.
- **AudioNotifier** — Calls `MessageBeep` or `PlaySound` for success, a distinct token for failure, and a separate token for watch-mode startup. Different sounds allow the user to distinguish outcomes without visual feedback.
- **CLI** — Parses `argv`: optional positional label; `--init`, `--watch`, `--quiet`, `--verbose`, `--version`, `--check`, `--list-windows` flags; `--process`, `--title`, `--folder`, `--clipboard`, `--hotkey` overrides. CLI values take precedence over config. Mode flags (`--init`, `--watch`, `--check`, `--list-windows`, `--version`) are mutually exclusive — specifying more than one produces an error. Fails with a descriptive error if neither a valid config nor the required target arguments are present.
- **WatchDaemon** — **Detachment mechanism:** Windows has no `fork()`/`setsid()` equivalent. To become independent of the spawning terminal, the launcher detects if it was started from a console and, if so, re-launches itself with `CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS` flags and exits immediately — the daemon runs as the re-launched process. If already detached (second launch), skip re-launch and proceed. Calls `RegisterHotKey` with the configured key combination. If `RegisterHotKey` fails (e.g. hotkey already claimed by another instance), exits immediately with a clear `MessageBox` error. Plays startup sound (`SystemExclamation`) on successful hotkey registration. Runs a Win32 message loop. **Serializes captures**: if a capture is already in progress when the hotkey fires again, the new event is acknowledged with a short distinct "busy" sound (e.g. `MessageBeep(MB_ICONQUESTION)`) and then ignored, so the user knows the keypress was received but not processed. Creates a `Shell_NotifyIcon` tray entry with a descriptive tooltip and a right-click menu offering "Restart" and "Exit".

### Architecture Decisions

- **Console/GUI subsystem:** The EXE is built with `SUBSYSTEM:WINDOWS` (no console window ever allocated by default). In CLI mode, the process calls `AttachConsole(ATTACH_PARENT_PROCESS)` immediately at startup to attach to the parent terminal so that `stdout`/`stderr` output is visible. If `AttachConsole` fails (no parent console, e.g. launched from Explorer or Task Scheduler), CLI mode falls back to `AllocConsole`. In watch mode, no `AttachConsole` call is made — the process remains fully windowless. **Trade-off:** the very first bytes of stderr output may be lost before `AttachConsole` completes; this is an accepted limitation.
- **DPI awareness:** The executable must declare `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2` (via app manifest or `SetProcessDpiAwarenessContext` at startup) to ensure screenshots are captured at physical pixel resolution rather than logical pixel resolution. Without this, screenshots on high-DPI displays (125%, 150%, 200% scaling) will be smaller than expected.
- Terminal output (CLI mode) uses `key: value` format on stdout for success and stderr for errors. Both formats are human-readable for screen readers and machine-parseable by AI coding agents and scripts. `--quiet` suppresses stdout only; errors always go to stderr.
- Success output always includes all four fields on stdout:
  ```
  status: ok
  file: C:\dev\myapp\screenshots\2026-04-17_143022_login-form.png
  window: MyApp (PID 1234)
  size: 1280x720
  ```
- Error output example (stderr):
  ```
  status: error
  code: window-not-found
  message: No window matching process "myapp.exe"
  ```
- Config discovery uses walk-up from CWD, stopping when a `.git` entry (file or directory — both are valid in regular repos and git worktrees respectively) is found, or at the filesystem root. The directory containing `shot.toml` becomes the project root for all relative path resolution.
- `shot.toml` is optional. If not found, all required values (`process` or `title`) must be supplied via CLI arguments. Optional values fall back to defaults.
- The `folder` value (from config or CLI) is validated to be a relative path with no `..` components, no leading `\`, no UNC paths (`\\server\share`), and no Windows device paths (`\\.\...`), preventing path traversal outside the project.
- Filename construction replaces all Windows-invalid characters (`: \ / * ? " < > |`) in the label/window-title segment with `-` before writing to disk.
- `--list-windows` filters out windows with empty titles, invisible windows (`WS_VISIBLE` not set), and shell/tray windows identified by class name (`Shell_TrayWnd`, `Progman`) rather than by process name, so that legitimate `explorer.exe` File Explorer windows remain visible in the list.
- `--verbose` diagnostic output goes to **stderr** (not stdout) so that stdout remains clean and machine-parseable regardless of verbosity level. `--verbose` takes precedence over `--quiet`.
- `--check` output uses the same `key: value` format on stdout as normal captures, but with config and window fields instead of a file path.
- When multiple windows match the configured criteria and none is the foreground window, the **topmost window in z-order returned by `EnumWindows`** is captured. `EnumWindows` enumerates in z-order, which changes every time any window receives focus — this order is **not stable** even within a session. When both the foreground heuristic and z-order are insufficient (e.g. two background builds running simultaneously), the behavior is documented but not guaranteed. Documented in story 39.
- Three distinct Windows system sounds are used for feedback: `SystemAsterisk` (success), `SystemHand` (error), `SystemExclamation` (watch startup).
- Errors in watch mode are surfaced via `MessageBox` (accessible to screen readers) since no console is available.
- All modules are stateless functions; no shared global state.
- Configuration is loaded once at startup, not re-read on each capture.
- The tool does not require elevated privileges.
- No installer required — single EXE, copy to any location.

### Configuration Schema

```toml
# shot.toml

# process: name of your app's .exe file (recommended).
# You already know this — it's in your Cargo.toml, .csproj, Makefile, etc.
# process = "myapp.exe"

# title: substring of the window title (alternative or complement to process).
# Matched as "title contains substring" — position-independent, works with dynamic titles.
# title   = "MyApp"

folder    = "screenshots"   # output subfolder (default: "screenshots")
clipboard = "path"          # "path" | "image" | "both" (default: "path")
# hotkey  = "Win+F12"       # watch mode hotkey (default: "Win+F12")
```

At least one of `process` or `title` must be present.

## Testing Decisions

Good tests verify **external behavior**, not internal implementation. Tests should not call private functions or inspect internal state.

### What to test

- **Config**: given a valid TOML string, returns the correct struct; given missing `process` and `title`, returns a descriptive error; given `folder` with `..`, returns a validation error; walk-up finds `shot.toml` in parent directory; walk-up stops at `.git` boundary; given unknown key, does not panic.
- **WindowResolver**: given a mock window enumeration, returns the correct HWND when process name matches; returns correct HWND when title substring matches; returns correct HWND when either OR condition matches; returns error when no match; title matching is case-insensitive; title matching works when the substring appears mid-title.
- **Storage**: generates correct filename from timestamp + label; creates output directory if absent; sanitizes invalid filename characters in label; sanitizes invalid characters in window title used as label; does not panic if two captures share the same timestamp.
- **ClipboardWriter**: given mode `"path"`, writes only CF_UNICODETEXT; given mode `"both"`, writes both CF_UNICODETEXT and CF_BITMAP formats. (Requires a Windows environment.)
- **CLI argument parsing**: `shot.exe` with no args uses config defaults; `shot.exe my-label` passes label to storage; `shot.exe "my label"` handles quoted multi-word label; `--clipboard=image` overrides config; `--verbose` and `--quiet` together results in verbose winning; missing required args produces non-zero exit code.
- **`--init`**: creates `shot.toml` in CWD with correct template content; `--init --process=x` produces a pre-filled file; `--init` fails if `shot.toml` already exists; `--init --title=x` also works.
- **`--check`**: outputs `status: ok` and expected fields when config is valid and window is running; outputs `status: error` when window is not found.
- **`--list-windows`**: output excludes windows with empty titles; output excludes shell windows (`Shell_TrayWnd`); output includes legitimate `explorer.exe` file windows.

## Out of Scope

- macOS or Linux support
- Full-screen capture
- Screen recording / video
- OCR or AI integration built into the tool itself
- GUI configuration editor
- Multi-monitor handling beyond what `PrintWindow` provides by default

## AI Development Context

Development of this project is AI-assisted, using **Claude Code** and **GitHub Copilot Agent** as primary coding tools. Tech stack will be decided separately and documented in a dedicated ADR.

### Agent-Friendly Conventions
- **Modules map 1-to-1 to source files** — `config`, `window_resolver`, `capture`, `storage`, `clipboard`, `audio`, `cli`, `watch`
- **WindowResolver is testable via DI** — the window enumeration function is injected, so tests pass a mock implementation; no real Windows desktop needed for unit tests
- **All modules are stateless** — no global state; everything is passed as arguments
- **`key: value` stdout format is a contract** — do not change field names or order without updating tests

### Verifiable Acceptance Criteria (for agent self-check)
After implementing a feature, an agent can verify:
1. Build completes with zero warnings
2. All unit tests pass (Config, WindowResolver, Storage, CLI parsing)
3. Linter/static analysis produces no diagnostics
4. `shot.exe --check` (with a real running window) exits 0 and prints `status: ok`
5. `shot.exe --list-windows` produces `key: value` lines (no blank titles, no `Shell_TrayWnd`)
6. `shot.exe --init` in a new directory creates `shot.toml` with inline comments

### Agent Instruction Files (when repo exists)
Once the stack is decided, create two files in the repo root:

**`CLAUDE.md`** (read by Claude Code at every session start):
- Build, test, and lint commands (exact invocations)
- Module names and source file locations
- `key: value` stdout contract — never change field names without updating tests
- Any non-obvious dev environment requirements (e.g. Windows SDK version, linker flags)
- Common gotchas (e.g. DPI awareness must be set before `EnumWindows`)

**`AGENTS.md`** (read by GitHub Copilot Agent and compatible tools):
- Same essentials as `CLAUDE.md` — build/test/lint commands, module map, stdout contract
- Can cross-reference `CLAUDE.md` with `@CLAUDE.md` import syntax if using Claude Code

## Further Notes

### Naming Conventions

| Context | Name |
|---|---|
| Product name | **Axygen Shot** |
| Binary (CLI) | `shot.exe` |
| Binary (tray daemon) | `shot-watch.exe` |
| Scoop manifest | `axygen-shot.json` |
| Scoop install command | `scoop install axygen-shot` |
| GitHub repository | `axygen-shot` |
| Distribution archive | `axygen-shot-1.0.0-win64.zip` |
| Distribution folder | `axygen-shot-1.0.0\` |

"Axygen" is a brand namespace for accessibility-focused developer tools. Future tools may follow the same pattern: Axygen Reader, Axygen Speak, etc. The binary names should remain short and CLI-friendly (e.g., `shot.exe`, `read.exe`) while the product names carry the "Axygen" prefix.

### Technical Notes

- The primary user is totally blind. All feedback must be non-visual: audio signals, clipboard content, exit codes, and stderr messages.
- The tool must work reliably when called from a terminal (VS Code integrated terminal or external cmd/PowerShell) since that is the primary invocation context.
- TOML was chosen over JSON and INI because it supports comments, is widely supported across all candidate implementation languages, and its `key = "value"` syntax is predictable and screen-reader-friendly.
- The implementation language is not yet decided. Any language with Win32 FFI, TOML parsing, and PNG encoding support is viable (Rust, C#, Go, Python with PyInstaller).
