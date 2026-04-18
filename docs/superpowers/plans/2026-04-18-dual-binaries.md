# План: Dual Binaries — розділення `shot.exe` (CLI) та `shot-watch.exe` (daemon)

> **Дата:** 2026-04-18
> **Статус:** Запропоновано
> **Автор:** Claude (Opus 4.7) за запитом користувача
> **Pre-read:** [ADR-001-tech-stack.md](../../ADR-001-tech-stack.md) (секція "SUBSYSTEM:WINDOWS + AttachConsole")

## Мотивація

`shot.exe` зараз збирається з `#![windows_subsystem = "windows"]` (GUI-підсистема) і викликає `AttachConsole(ATTACH_PARENT_PROCESS)` для виводу у батьківську консоль. Це створює **race condition у cmd.exe та PowerShell**:

1. Оболонка читає PE-заголовок дочірнього процесу. Для GUI-підсистеми вона **не чекає** на завершення.
2. Оболонка одразу друкує новий prompt і викликає `ReadConsole` на stdin.
3. `shot.exe` пізніше друкує свій output через `AttachConsole` — нижче вже намальованого prompt.
4. Користувач бачить "output нижче осиротілого prompt" і мусить натиснути Enter.

Цей артефакт відтворюється стабільно у cmd і PowerShell (5.1 та 7). Перебудова PE-підсистеми в рантаймі неможлива — це константа PE-заголовка.

Канонічне рішення (python.exe/pythonw.exe, devenv.com/devenv.exe, wezterm.exe/wezterm-gui.exe) — **два бінарі з різними subsystem-флагами**:

- `shot.exe` — subsystem=**console** (CUI). Використовується для всіх CLI-команд (`shot`, `shot --init`, `shot --check`, `shot --list-windows`, `shot --watch`). Оболонка чекає на нього синхронно → вивід завжди перед promt'ом. `AttachConsole` більше не потрібен.
- `shot-watch.exe` — subsystem=**windows** (GUI). Це фоновий daemon: tray icon, hotkey, message loop. Запускається **тільки** через `shot.exe --watch` (який робить `CreateProcessW` з `DETACHED_PROCESS`), або напряму з ярлика/Startup. У нього немає консолі і нема console-спалаху.

## Критерії успіху

| # | Перевірка | Як верифікувати |
|---|-----------|-----------------|
| 1 | Prompt з'являється після output у cmd і PowerShell | Ручний тест у пустій папці: `.\shot` → очікуване `status: error`, prompt одразу нижче, **без** потреби натискати Enter |
| 2 | Watch-daemon стартує як раніше | `shot --watch --process=notepad.exe` → `status: ok\nwatch: started (PID X)` у stdout, tray icon з'являється |
| 3 | Hotkey (Win+F12 default) захоплює вікно | Як і було |
| 4 | Restart з tray — спрацьовує | ПКМ → Restart → нова іконка |
| 5 | `shot-watch.exe`, запущений напряму з Explorer, не показує консоль | Подвійний клік на `shot-watch.exe` → tray icon, 0 вспишок консолі |
| 6 | `shot --list-windows`, `--init`, `--check`, `--version`, `--help` — усі працюють синхронно у терміналі | Manual checklist §1–4 |
| 7 | `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt -- --check` — зелені | `just ci` |
| 8 | CI билдить обидва бінарі | `.github/workflows/ci.yml` і `release.yml` оновлені |
| 9 | Release zip містить обидва бінарі | `release.yml` копіює `shot.exe` + `shot-watch.exe` |

## Архітектура

### Поточна структура (до зміни)

```
src/main.rs  ── #![windows_subsystem = "windows"]
             ── AttachConsole(ATTACH_PARENT_PROCESS)
             ── mod cli, config, watch, ...
             ── pub fn main() { run() }
             
src/watch.rs ── pub fn run(cfg, parent_pid)
                  ├─ if !is_detached(): parent-side (create events, launch_daemon, wait)
                  └─ else:              daemon-side (mutex, hotkey, tray, loop)
```

### Нова структура

```
src/lib.rs            ── pub mod cli, config, watch, ... (спільні модулі)
                          (нічого executable — лише re-exports)

src/main.rs           ── bin "shot" (CUI)
                          use axygen_shot::*;
                          НЕМАЄ #![windows_subsystem]  (за замовчуванням console)
                          НЕМАЄ AttachConsole
                          match mode { ..., Watch => watch::spawn_daemon(...) }

src/bin/shot-watch.rs ── bin "shot-watch" (GUI)
                          use axygen_shot::*;
                          #![windows_subsystem = "windows"]
                          тонкий entry-point: parse args → watch::run_daemon(...)

src/watch.rs          ── pub fn spawn_daemon(cfg, parent_pid_arg) -> parent-side
                          pub fn run_daemon(cfg, parent_pid) -> daemon-side
                          (is_detached() видалено — розділення явне, не через runtime-сніфінг)
```

### Чому потрібен `lib.rs`

Стандартний Cargo-патерн для проекту з ≥2 бінарями: спільний код виноситься у бібліотечний крейт. Обидва бінарі роблять `use axygen_shot::*;`. Без `lib.rs` довелось би або дублювати `mod foo;` у кожному бінарі (6+ модулів × 2 бінарі), або хакати через `path = "..."`.

### IPC (залишається без змін)

Механізм parent→child startup-feedback (named events `Local\axygen-shot-ok-<pid>` / `Local\axygen-shot-err-<pid>`, tempfile з кодом помилки) лишається **ідентичним**. Просто "parent" тепер — завжди `shot.exe`, "child" — завжди `shot-watch.exe`. Logic очищується: поточна гілка `if !is_detached()` зникає.

## Файли, що будуть змінені

### Cargo manifest та build

| Файл | Зміна |
|------|-------|
| `Cargo.toml` | Додати `[lib]` і другий `[[bin]]` `shot-watch`. Lib name: `axygen_shot`. |
| `build.rs` | Не змінюється — `embed-resource` v3 за замовчуванням впроваджує ресурс у всі bin-target'и пакета → обидва exe отримають DPI-manifest. Перевірити після builds. |
| `shot.rc` | Не змінюється. |
| `shot.manifest` | Не змінюється (однаковий DPI-manifest для обох exe). |

### Сирці

| Файл | Зміна |
|------|-------|
| `src/lib.rs` | **НОВИЙ.** Оголошує: `pub mod audio; pub mod capture; pub mod cli; pub mod clipboard; pub mod config; pub mod errors; pub mod storage; pub mod watch; pub mod window_resolver;` |
| `src/main.rs` | 1) Видалити `#![windows_subsystem = "windows"]` (рядок 1). 2) Видалити блок `AttachConsole` (рядки 19–23). 3) Замінити `mod X;` на `use axygen_shot::*;`. 4) У `Mode::Watch` викликати `watch::spawn_daemon(&cfg)` замість `watch::run(&cfg, args.daemon_parent_pid)`. |
| `src/bin/shot-watch.rs` | **НОВИЙ.** `#![windows_subsystem = "windows"]` + `use axygen_shot::*;` + parse CLI args через `cli::parse()` + викликати `watch::run_daemon(&cfg, args.daemon_parent_pid)`. На помилці — `format_error_message` у MessageBox (бо без консолі). |
| `src/watch.rs` | 1) Зробити `fn run_daemon` → `pub fn run_daemon` (зараз private, [watch.rs:345](../../src/watch.rs#L345)). 2) Розділити `pub fn run` на `pub fn spawn_daemon(cfg) -> Result<u32, ShotError>` (parent-side, зараз [watch.rs:160-208](../../src/watch.rs#L160-L208)) та залишити `run_daemon` як є (daemon-side). 3) Видалити `pub fn run` (стара обгортка) та `fn is_detached` ([watch.rs:212-214](../../src/watch.rs#L212-L214)) і імпорт `GetConsoleWindow` ([watch.rs:13](../../src/watch.rs#L13)). 4) У `launch_daemon` ([watch.rs:243](../../src/watch.rs#L243)) змінити `current_exe()` на sibling-пошук `shot-watch.exe`. Додати перевірку існування файлу. 5) Відфільтрувати `--watch` з forward-ованих args у `launch_daemon` (shot-watch.exe не потребує цього прапора — він завжди запускає daemon). 6) `restart()` ([watch.rs:600](../../src/watch.rs#L600)) лишається на `current_exe()` (воно дорівнює `shot-watch.exe` коли викликається з daemon'а). |
| `src/cli.rs` | Не змінюється. `daemon_parent_pid` лишається hidden CLI arg, тепер парситься на обох exe. `cli::parse()` вже `pub` ([cli.rs:70-72](../../src/cli.rs#L70-L72)). |
| Інші src/*.rs | Не змінюються. Публічність перевірена: `config::find_config`, `config::merge`, `config::CaptureConfig` — pub. `errors::ShotError`, `errors::format_error_message` — pub. `cli::parse`, `cli::CliArgs`, `cli::Mode`, `cli::determine_mode` — pub. `window_resolver::*`, `capture::*`, `storage::*`, `clipboard::*`, `audio::*` — pub. |
| `src/main.rs` у Cargo | Явно вказати `path = "src/main.rs"` у `[[bin]] name = "shot"` (зараз є, лишається). |

### Build / CI / Release

| Файл | Зміна |
|------|-------|
| `justfile` | Див. секцію "Зміни у justfile" нижче. |
| `.github/workflows/ci.yml` | "Show binary size" step показує розмір обох exe. |
| `.github/workflows/release.yml` | "Create distribution zip" step копіює **обидва** `shot.exe` і `shot-watch.exe` у `$distDir`. |

### Зміни у justfile

Повний аудит кожної recipe:

| Recipe | Поточний вміст | Статус | Дія |
|--------|----------------|--------|-----|
| `default` | `@just --list` | ✅ OK | Без змін |
| `build-fast` | `cargo build` | ✅ OK | `cargo build` без `--bin` будує всі bin-target'и. Без змін. |
| `build` | `cargo build --release` | ✅ OK | Те ж. Без змін. |
| `test` | `cargo test` | ✅ OK | Будує всі bins + test targets. Без змін. |
| `lint` | `cargo clippy -- -D warnings` | ⚠️ Pre-existing разходження | CI використовує `--all-targets`, justfile — ні. Це **не зачіпається dual-binary рефактором**, окреме питання. Без змін у рамках цього плану. |
| `fmt-check` | `cargo fmt -- --check` | ✅ OK | Працює на всіх `.rs`. Без змін. |
| `fmt` | `cargo fmt` | ✅ OK | Без змін. |
| `ci` | `ci: lint fmt-check test` | ✅ OK | Композиція, без змін. |
| `clean` | `cargo clean` | ✅ OK | Без змін. |
| `size` | Показує лише `target\release\shot.exe` | ❌ Змінити | Показувати обидва exe у таблиці. |
| `run` | `cargo run --release -- {{ARGS}}` | ❌ **Поламається** | З двома bins `cargo run` без `--bin` видає помилку "could not determine which binary to run". Явний `--bin shot`. |
| `run-watch` | (не існує) | ➕ Додати | Зручність для ручного тестування daemon'а. |

Оновлений `justfile` буде виглядати так (дифф-орієнтоване зображення):

```make
# Build release and show binary size
size: build
    Get-ChildItem target\release\shot.exe, target\release\shot-watch.exe `
        | Format-Table Name, @{N='Size'; E={"$([math]::Round($_.Length / 1KB)) KB"}} -AutoSize

# Run shot.exe (CLI) with arguments (e.g., just run -- --list-windows)
run *ARGS:
    cargo run --release --bin shot -- {{ARGS}}

# Run shot-watch.exe (tray daemon) directly — for debugging, prefer `just run -- --watch`
run-watch *ARGS:
    cargo run --release --bin shot-watch -- {{ARGS}}
```

### Документація

| Файл | Зміна |
|------|-------|
| `README.md` | Installation: згадати обидва файли в релізному zip. Project structure table: додати `src/bin/shot-watch.rs` рядок. Додати секцію "Why two binaries" (коротко, 3–4 речення з посиланням на ADR-002). |
| `README_UK.md` | Дзеркально до README.md. |
| `CLAUDE.md` | Секція "Project structure": додати `src/bin/shot-watch.rs` + `src/lib.rs`. Секція "Gotchas": замінити рядок про `AttachConsole` на **"`shot.exe` is CUI; `shot-watch.exe` is GUI. Do not call AttachConsole."** та видалити "Watch mode daemon detected via GetConsoleWindow() — no console = daemon mode" (тепер розділ чіткий). |
| `AGENTS.md` | Module map: додати рядок про розділення `shot` / `shot-watch`. stdout contract — без змін. |
| `CHANGELOG.md` | Під `[Unreleased]`: додати секції `### Changed` ("Split into two executables: `shot.exe` (CLI) and `shot-watch.exe` (tray daemon) to fix Windows shell prompt-before-output race"), `### Fixed` ("PowerShell/cmd prompt no longer appears before program output"). |
| `docs/manual-testing-checklist.md` | 1) §1.1 check 3 — "Промпт терміналу повертається одразу" — підтверджено; додати примітку "Це було виправлено переходом на dual binaries, див. ADR-002." 2) §14 (Watch mode): додати новий блок "14.0 Перевірка dual-binary" — двокліком запустити `shot-watch.exe` з релізної папки → tray icon, без консольного спалаху. 3) Усюди, де `$shot = "...\shot.exe"`, лишити як є — CLI-режими дзеркальні. |
| `docs/ADR-001-tech-stack.md` | Додати **banner на початку**: "> **Status update (2026-04-18):** Superseded partially by ADR-002 — the `SUBSYSTEM:WINDOWS + AttachConsole` pattern described below was replaced with dual binaries. See ADR-002." Не редагувати історичний текст. |
| `docs/ADR-002-dual-binaries.md` | **НОВИЙ.** Короткий ADR (~150 рядків) з: проблема (prompt race), альтернативи (table з `.com` stub, console-only, тощо), рішення (dual binaries), наслідки, джерела. |
| `docs/PRD.md` | У секції Solution замінити "single portable Windows executable" → "two portable Windows executables (`shot.exe` for CLI, `shot-watch.exe` for the tray daemon)". Додати один реченням "чому" з посиланням на ADR-002. Секція 263 (таблиця Naming Conventions): додати рядок для `shot-watch.exe`. |
| `docs/superpowers/specs/2026-04-17-axygen-shot-mvp-design.md` | Не редагувати (історична специфікація MVP). |
| `docs/superpowers/plans/2026-04-17-*.md` | Не редагувати (історичні плани виконаних фаз). |

### Tests

| Файл | Зміна |
|------|-------|
| `tests/init_integration.rs` | Не змінюється — `Command::cargo_bin("shot")` продовжує працювати (bin `shot` існує). |
| `tests/config_integration.rs` | Те ж саме. |
| (опційно) `tests/dual_binary_smoke.rs` | **НОВИЙ.** Smoke-тест: `Command::cargo_bin("shot-watch").unwrap().arg("--help").assert().success()` — переконатись, що обидва exe будуються і парсять CLI. |

## Кроки реалізації (за порядком)

> **Інваріант:** після кожного етапу `just ci` (lint + fmt-check + test) має проходити зелено. Крім того, на кожному етапі явно вказана функціональна перевірка (manual або command). Кожен етап — **один git commit** (див. секцію "Git стратегія" нижче).

> **Важливо про порядок:** Етапи 2-4 **критично впорядковані**. Якщо поміняти місцями "додати shot-watch.exe" і "прибрати is_detached()", виникне infinite spawn loop (`shot.exe --watch` → спавнить `current_exe()` = `shot.exe --watch` → знов спавнить → ...). Поточний порядок підтримує коректну поведінку на кожному проміжному стані.

### Етап 1 — Рефакторинг у бібліотечний крейт

**Мета:** Винести модулі у `src/lib.rs`, щоб їх могли використовувати обидва майбутні бінарі. Поведінка не змінюється.

**Кроки:**

1.1. Створити `src/lib.rs` з вмістом:
```rust
pub mod audio;
pub mod capture;
pub mod cli;
pub mod clipboard;
pub mod config;
pub mod errors;
pub mod storage;
pub mod watch;
pub mod window_resolver;
```

1.2. У `Cargo.toml` перед блоком `[[bin]]` додати:
```toml
[lib]
name = "axygen_shot"
path = "src/lib.rs"
```

1.3. У [src/main.rs](../../src/main.rs) замінити рядки 3-14 (`mod ...;` декларації + `use errors::ShotError;` + `use window_resolver::Win32Enumerator;`) на:
```rust
use axygen_shot::{cli, config, errors, storage, window_resolver, audio, capture, clipboard, watch};
use axygen_shot::errors::ShotError;
use axygen_shot::window_resolver::Win32Enumerator;
```

**Verify:**
```powershell
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
cargo run --bin shot -- --help
```
Очікування: усі команди exit=0, остання друкує help-текст у терміналі.

**Commit:** `refactor: extract modules into axygen_shot library crate`

---

### Етап 2 — Додати бінар `shot-watch.exe` (без перемикання клієнта)

**Мета:** Створити новий exe, що запускає daemon напряму. Поки що `shot.exe --watch` продовжує спавнити сам себе (через `current_exe()` + `is_detached()`-диспатч). Новий exe існує паралельно і не ламає поточну поведінку.

**Кроки:**

2.1. У [src/watch.rs:345](../../src/watch.rs#L345) змінити сигнатуру:
```diff
- fn run_daemon(cfg: &CaptureConfig, parent_pid: Option<u32>) -> Result<(), ShotError> {
+ pub fn run_daemon(cfg: &CaptureConfig, parent_pid: Option<u32>) -> Result<(), ShotError> {
```

2.2. Створити `src/bin/shot-watch.rs`:
```rust
#![windows_subsystem = "windows"]

use axygen_shot::{cli, config, errors, watch};

fn main() {
    let args = cli::parse();
    let result = (|| -> Result<(), errors::ShotError> {
        let cwd = std::env::current_dir()
            .map_err(|e| errors::ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
        let toml = config::find_config(&cwd)?;
        let cfg = config::merge(&args, toml)?;
        watch::run_daemon(&cfg, args.daemon_parent_pid)
    })();
    if result.is_err() {
        // Помилки вже відправлено батьку через handle_startup_err (temp file + named event)
        // або показано у MessageBox (якщо parent_pid=None). Просто exit 1.
        std::process::exit(1);
    }
}
```

2.3. У `Cargo.toml` додати після існуючого `[[bin]] shot`:
```toml
[[bin]]
name = "shot-watch"
path = "src/bin/shot-watch.rs"
```

**Verify:**
```powershell
cargo build --release
Test-Path target\release\shot.exe        # True
Test-Path target\release\shot-watch.exe  # True
cargo test
```
Очікування: обидва exe створені, тести зелені. На цьому етапі `shot --watch` ще не використовує `shot-watch.exe` — це наступний крок.

**Commit:** `feat: add shot-watch binary with GUI subsystem`

---

### Етап 3 — Перенацілити `launch_daemon` на `shot-watch.exe`

**Мета:** З цього етапу `shot.exe --watch` спавнить новий `shot-watch.exe` замість самого себе. Dispatch через `is_detached()` лишається у `watch::run`, але daemon-гілка стає dead code (недосяжна у нормальному потоці — лише якщо хтось вручну запустить shot.exe detached).

**Кроки:**

3.1. У [src/watch.rs:243-257](../../src/watch.rs#L243-L257), у функції `launch_daemon`, замінити:
```rust
fn launch_daemon(parent_pid: u32) -> Result<u32, ShotError> {
    let exe = std::env::current_exe()
        .map_err(|e| ShotError::HotkeyError(format!("Cannot find own executable: {}", e)))?;

    let mut cmd_line = OsString::new();
    cmd_line.push("\"");
    cmd_line.push(exe.as_os_str());
    cmd_line.push("\"");
    for arg in std::env::args().skip(1) {
        cmd_line.push(" ");
        cmd_line.push(quote_arg(&arg).as_str());
    }
```
на:
```rust
fn launch_daemon(parent_pid: u32) -> Result<u32, ShotError> {
    let self_exe = std::env::current_exe()
        .map_err(|e| ShotError::HotkeyError(format!("Cannot find own executable: {}", e)))?;
    let exe = self_exe.with_file_name("shot-watch.exe");
    if !exe.exists() {
        return Err(ShotError::HotkeyError(format!(
            "shot-watch.exe not found next to {} — check installation",
            self_exe.display()
        )));
    }

    let mut cmd_line = OsString::new();
    cmd_line.push("\"");
    cmd_line.push(exe.as_os_str());
    cmd_line.push("\"");
    for arg in std::env::args().skip(1) {
        if arg == "--watch" {
            continue; // shot-watch.exe always runs the daemon; no --watch flag needed
        }
        cmd_line.push(" ");
        cmd_line.push(quote_arg(&arg).as_str());
    }
```

**Verify:**
```powershell
cargo build --release
# Manual: run shot --watch --process=notepad.exe (with Notepad open).
# Expected: "status: ok\nwatch: started (PID X)" — where PID belongs to shot-watch.exe (verify in Task Manager).
# Expected: tray icon appears.
# Press Win+F12 → screenshot captured.
# Right-click tray → Exit → daemon stops.
Get-Process shot-watch -ErrorAction SilentlyContinue  # None after Exit
```

**Commit:** `feat: spawn shot-watch.exe instead of re-launching shot.exe`

---

### Етап 4 — Прибрати `is_detached()` dispatch, ввести `spawn_daemon`

**Мета:** Явно розділити parent-side і daemon-side. `main.rs` викликає `spawn_daemon` прямо; `shot-watch.rs` викликає `run_daemon` прямо. Runtime-сніфінг через `GetConsoleWindow` більше не потрібен.

**Кроки:**

4.1. У [src/watch.rs:159-210](../../src/watch.rs#L159-L210), замінити цілу функцію `pub fn run` на:
```rust
/// Parent-side: called from shot.exe on `--watch`.
/// Creates IPC events, spawns shot-watch.exe, waits up to 3s for startup signal.
/// Returns the child PID on success.
pub fn spawn_daemon(cfg: &CaptureConfig) -> Result<u32, ShotError> {
    let _ = cfg; // cfg is not used directly — the child re-reads config from CWD/args
    let my_pid = unsafe { GetCurrentProcessId() };

    let ok_name = wide_string(&format!("Local\\axygen-shot-ok-{}", my_pid));
    let err_name = wide_string(&format!("Local\\axygen-shot-err-{}", my_pid));

    let ok_event = unsafe {
        CreateEventW(None, false, false, windows::core::PCWSTR(ok_name.as_ptr()))
            .map_err(|e| ShotError::HotkeyError(format!("Cannot create ok event: {}", e)))?
    };
    let err_event = unsafe {
        CreateEventW(None, false, false, windows::core::PCWSTR(err_name.as_ptr())).map_err(
            |e| {
                let _ = windows::Win32::Foundation::CloseHandle(ok_event);
                ShotError::HotkeyError(format!("Cannot create err event: {}", e))
            },
        )?
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

    let wait_result = unsafe { WaitForMultipleObjects(&[ok_event, err_event], false, 3000) };

    unsafe {
        let _ = windows::Win32::Foundation::CloseHandle(ok_event);
        let _ = windows::Win32::Foundation::CloseHandle(err_event);
    }

    match wait_result.0 {
        0 => Ok(child_pid),
        1 => Err(read_and_delete_temp_file(my_pid)),
        _ => Err(ShotError::WatchTimeout),
    }
}
```

4.2. Видалити `fn is_detached()` ([watch.rs:212-214](../../src/watch.rs#L212-L214)).

4.3. Видалити імпорт `GetConsoleWindow` ([watch.rs:13](../../src/watch.rs#L13)):
```diff
- use windows::Win32::System::Console::GetConsoleWindow;
```

4.4. У [src/main.rs](../../src/main.rs), функція `run()`, гілка `Mode::Watch`: замінити `watch::run(&cfg, args.daemon_parent_pid)` → `watch::spawn_daemon(&cfg)`. Після `?` отриманий `child_pid` — надрукувати повідомлення:
```rust
cli::Mode::Watch => {
    let cwd = std::env::current_dir()
        .map_err(|e| ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
    let toml = config::find_config(&cwd)?;
    let cfg = config::merge(&args, toml)?;
    let child_pid = watch::spawn_daemon(&cfg)?;
    if !cfg.quiet || cfg.verbose {
        println!("status: ok\nwatch: started (PID {})", child_pid);
    }
    Ok(())
}
```
(Це переміщує `println!` з `watch::run` у `main.rs` — контракт stdout без змін.)

**Verify:**
```powershell
cargo build --release
cargo test
cargo clippy -- -D warnings
# Manual: shot --watch --process=notepad.exe → "status: ok\nwatch: started (PID X)"
# Tray icon appears. Win+F12 captures. Right-click Exit ends daemon.
```

**Commit:** `refactor: replace is_detached dispatch with explicit spawn_daemon`

---

### Етап 5 — Перемкнути `shot.exe` на console subsystem

**Мета:** Головне виправлення — `shot.exe` більше не GUI-підсистема і не використовує `AttachConsole`. Оболонка чекає на нього синхронно → prompt з'являється після output без потреби в Enter.

**Кроки:**

5.1. У [src/main.rs](../../src/main.rs):
- Видалити рядок 1: `#![windows_subsystem = "windows"]`
- Видалити рядки 17-23 (коментар + блок `AttachConsole`):
```rust
    // Attach to parent console for stdout/stderr when launched from terminal.
    // No-op when launched from GUI (Explorer, Task Scheduler, shortcuts).
    unsafe {
        let _ = windows::Win32::System::Console::AttachConsole(
            windows::Win32::System::Console::ATTACH_PARENT_PROCESS,
        );
    }
```

Функція `main` має стати:
```rust
fn main() {
    match run() {
        Ok(()) => {}
        Err(e) => {
            audio::play_error();
            eprintln!("{}", errors::format_error(&e));
            std::process::exit(1);
        }
    }
}
```

**Verify (ГОЛОВНИЙ ACCEPTANCE TEST):**
```powershell
cargo build --release
cd $env:TEMP
mkdir shot-accept-test; cd shot-accept-test
C:\dev\axygen-shot\target\release\shot.exe
# Очікування: negative test (немає --process/--title/shot.toml) → error output,
# prompt з'являється ОДРАЗУ нижче, БЕЗ потреби натискати Enter.
# Повторити у cmd.exe для впевненості.

# Positive test:
C:\dev\axygen-shot\target\release\shot.exe --list-windows
# Очікування: list вікон, exit 0, prompt одразу.
```

**Commit:** `fix: switch shot.exe to console subsystem to fix prompt race`

---

### Етап 6 — Cleanup: прибрати непотрібні features

**Мета:** `Win32_System_Console` більше не використовується (ми видалили `AttachConsole` і `GetConsoleWindow`). Прибрати з `Cargo.toml`.

**Кроки:**

6.1. Grep (діагностика): `Grep "Win32::System::Console" src/` — має повернути 0 збігів.

6.2. У [Cargo.toml](../../Cargo.toml) у `[dependencies.windows].features` видалити рядок `"Win32_System_Console",`.

**Verify:**
```powershell
cargo build --release  # має компілюватись без помилок
cargo test
```
Якщо compile error — щось ще використовує Console feature; діагностувати через `cargo build 2>&1 | Select-String "Console"` і або повернути feature, або прибрати останнє використання.

**Commit:** `chore: drop unused Win32_System_Console feature`

---

### Етап 7 — Smoke-тест для `shot-watch.exe`

**Мета:** Регресійний тест, що `shot-watch.exe` будується і парсить CLI (без запуску daemon'а).

**Кроки:**

7.1. Створити `tests/shot_watch_smoke.rs`:
```rust
use assert_cmd::Command;

/// Verifies shot-watch.exe builds and clap parses --help.
/// We cannot assert on stdout because the binary uses windows-subsystem=windows
/// and stdout may not appear in pipe under some configurations.
#[test]
fn shot_watch_help_exits_zero() {
    Command::cargo_bin("shot-watch")
        .unwrap()
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn shot_watch_version_exits_zero() {
    Command::cargo_bin("shot-watch")
        .unwrap()
        .arg("--version")
        .assert()
        .success();
}
```

**Verify:**
```powershell
cargo test --test shot_watch_smoke
```

**Commit:** `test: add shot-watch smoke tests`

---

### Етап 8 — Оновити justfile, CI, release

**Кроки:**

8.1. [justfile](../../justfile): recipe `size` — замінити рядок 43 на:
```make
size: build
    Get-ChildItem target\release\shot.exe, target\release\shot-watch.exe `
        | Format-Table Name, @{N='Size'; E={"$([math]::Round($_.Length / 1KB)) KB"}} -AutoSize
```

8.2. [justfile](../../justfile): recipe `run` — замінити рядок 47:
```make
run *ARGS:
    cargo run --release --bin shot -- {{ARGS}}

run-watch *ARGS:
    cargo run --release --bin shot-watch -- {{ARGS}}
```

8.3. [.github/workflows/ci.yml](../../.github/workflows/ci.yml): у step "Show binary size" замінити тіло на:
```yaml
      - name: Show binary sizes
        run: |
          foreach ($name in 'shot.exe', 'shot-watch.exe') {
            $exe = Get-Item "target\release\$name"
            Write-Output "$name size: $([math]::Round($exe.Length / 1KB)) KB"
          }
```

8.4. [.github/workflows/release.yml](../../.github/workflows/release.yml): у step "Create distribution zip" додати копіювання `shot-watch.exe`:
```yaml
          Copy-Item target\release\shot.exe $distDir\
          Copy-Item target\release\shot-watch.exe $distDir\
```

**Verify:**
```powershell
just ci
just size   # показує обидва
just run -- --list-windows   # працює
# (`just run-watch` не перевіряти у CI — лишити для ручного тестування)
```

**Commit:** `build: update justfile and CI for dual binaries`

---

### Етап 9 — Документація

**Кроки:** Див. таблицю "Документація" у секції "Файли, що будуть змінені". Конкретний текст кожного правки — на розсуд виконавця, з дотриманням пунктів.

**Obligatorily створити:**
- `docs/ADR-002-dual-binaries.md` (~150 рядків). Скелет:
  ```markdown
  # ADR-002: Dual Binaries — shot.exe (CLI) + shot-watch.exe (daemon)
  
  > **Тип задачі:** 🏗️ Архітектурне рішення
  > **Статус:** Прийнято
  > **Дата:** 2026-04-18
  > **Супроводжує:** ADR-001 (частково перекриває розділ "SUBSYSTEM:WINDOWS + AttachConsole")
  
  ## TL;DR
  
  Розділено один `shot.exe` (GUI subsystem + AttachConsole) на два бінарі:
  `shot.exe` (console subsystem, CLI) + `shot-watch.exe` (windows subsystem, tray daemon).
  Причина: GUI-subsystem exe з AttachConsole створює prompt-before-output race у
  cmd.exe та PowerShell.
  
  ## Проблема
  [опис race condition — див. "Мотивація" у плані 2026-04-18-dual-binaries.md]
  
  ## Розглянуті варіанти
  [table: dual binaries | .com stub | console-only + flash | FreeConsole hack]
  
  ## Рішення
  Dual binaries. Обґрунтування: прецеденти (python.exe/pythonw.exe,
  devenv.com/devenv.exe, wezterm.exe/wezterm-gui.exe).
  
  ## Наслідки
  - (+) Prompt у shell працює синхронно.
  - (+) `shot-watch.exe` можна запускати з ярлика без console-спалаху.
  - (−) Два exe у дистрибутиві (~1.4 MB total).
  - (−) Installer має розміщувати їх разом.
  
  ## Джерела
  [той самий список, що у плані]
  ```

**Verify:** перечитати всі оновлені документи — чи узгоджені з новою архітектурою.

**Commit:** `docs: document dual-binary architecture (ADR-002, README, CLAUDE.md)`

---

### Етап 10 — CHANGELOG і фінальна валідація

**Кроки:**

10.1. У [CHANGELOG.md](../../CHANGELOG.md), під `## [Unreleased]`, додати:
```markdown
### Changed

- Split executable into two binaries: `shot.exe` (CLI, console subsystem) and
  `shot-watch.exe` (tray daemon, windows subsystem). Release zip now contains
  both files; they must be kept in the same directory.

### Fixed

- PowerShell / cmd.exe prompt no longer appears before program output when
  running `shot.exe`. Previously `shot.exe` used `windows_subsystem = "windows"`
  with `AttachConsole`, which caused a race where the shell printed its next
  prompt before the tool's stdout was visible (forcing the user to press Enter
  to redraw). See ADR-002.
```

10.2. Повний прогон:
```powershell
just ci
just build
just size
# Manual: усі 14 пунктів §14 (Watch mode) з docs/manual-testing-checklist.md
# Manual: головний acceptance test з Етапу 5 (prompt race)
# Manual: double-click shot-watch.exe з Explorer → tray icon без console flash
```

**Commit:** `chore: update CHANGELOG for dual-binary release`

---

## Git стратегія

Кожен етап — один focused commit. 10 комітів у серії. Переваги:

- Легко робити `git bisect` якщо регресія знайдена
- Code review по окремих PR'ах можливий
- Відкат будь-якого етапу через `git revert <sha>`

Рекомендована гілка: `feature/dual-binaries`. Після всіх коммітів — PR у `develop`.

## Ризики та мітигація

| Ризик | Ймовірність | Вплив | Мітигація |
|-------|-------------|-------|-----------|
| `embed-resource` не впроваджує manifest у другий bin | Низька | Середній | Перевірити через `dumpbin /headers target\release\shot-watch.exe` → шукати `16.00 optional header magic` + resource section. Якщо нема — додати явний виклик `embed_resource` per-bin у build.rs. |
| `shot-watch.exe` не знайдено поруч з `shot.exe` при нестандартному PATH-виклику | Середня | Низький | Чітка error message з абсолютним шляхом. Документувати у README що обидва exe мають бути разом. |
| Хтось запустить `shot.exe` з `--daemon-parent-pid=X` вручну | Дуже низька | Низький | На `shot.exe` прапор не робить нічого (mode дорівнює Capture/Watch, а PID ігнорується бо не викликається run_daemon). Можна лишити як є. |
| Користувачі з існуючими скриптами `shot --watch` | Низька | Низький | Не ламається: `shot.exe --watch` → spawn `shot-watch.exe` → ідентична поведінка на виході. |
| Розмір release zip збільшиться (~×2) | Висока | Низький | Два exe ≈ 1.4 MB total. Все ще портативно. Документувати в CHANGELOG. |
| `Win32_System_Console` feature все ще потрібен для чогось | Середня | Низький | `cargo build` скаже при видаленні. Якщо потрібен — лишити. |

## Відкат

Кожен етап — окремий commit (або navd-група commit'ів). Відкат:
- Етап 5 ламає UX GUI-запуску shot.exe → `git revert` останнього commit'а повертає GUI-subsystem + AttachConsole.
- Етап 3–4 можна залишити (другий бінар не ламає існуюче).

## Зафіксовані рішення

Відкритих питань нема — усі рекомендації прийняті:

- **Назва другого бінаря:** `shot-watch.exe`. Обґрунтування: зрозуміла дефіс-нотація для screen reader'ів (головна аудиторія PRD — незряча), відповідає wezterm-стилю (`wezterm.exe` / `wezterm-gui.exe`), явно промовляється. `shotw.exe` (Python-стиль) відкидається через неоднозначну озвучку.
- **Release profile:** один спільний `[profile.release]` для обох бінарів. Окремий профіль для daemon'а — overkill.
- **Мінімальна підтримувана Windows:** не змінюється (Windows 10+, x86_64). Dual binaries не зачіпають вимог.
- **Іконка/ресурси для `shot-watch.exe`:** той самий `shot.rc` / `shot.manifest` для обох exe через типову поведінку `embed-resource` v3. Окрема іконка у Task Manager — не потрібна в MVP.
- **Внутрішній flag `--daemon-parent-pid`:** лишається hidden CLI arg, фізично парситься на обох бінарях через спільний [src/cli.rs](../../src/cli.rs), але використовується лише `shot-watch.exe`. На `shot.exe` просто ігнорується (mode дорівнює Capture/Watch/etc.).

## Рекомендації для агента кодування (Claude Opus 4.6)

Цей план написаний так, щоб його міг виконати і людина, і LLM-агент (Claude Code, GitHub Copilot). Нижче — інструкція для Opus 4.6, яка оптимізує під безпечне, покрокове виконання.

### Режим виконання

- Стартувати з `superpowers:executing-plans` (якщо доступна). Вона включає checkpoint-дисципліну: після кожного етапу обов'язково пускати `Verify`-команди з плану, і тільки потім коммітити.
- Паралельно тримати `superpowers:verification-before-completion` — не дозволяє репортити "done", поки verify не повернув exit=0 у реальному shell (а не "виглядає ок").
- Використати `TodoWrite` на старті: створити 10 todo-айтемів, по одному на етап. Після кожного успішного verify → позначити `completed`, одразу перейти до наступного. Не батчити.

### Порядок виконання

- Один етап — одна ітерація. **Не поєднувати** Етапи 2, 3, 4 в одному коміті, навіть якщо здається що можна. Критичне попередження про infinite spawn loop на початку секції "Кроки реалізації" — це реальний ризик.
- Між етапами завжди повертатися до `just ci` (або еквівалент) перед `git commit`. Якщо lint/тест червоний — виправити **перед** коммітом, не "потім".
- Якщо можливо — **один етап за сесію**. Це економить контекст і дає користувачу точку контролю. Якщо сесія велика, групувати по 2-3 суміжні етапи максимум (наприклад: 1; 2+3; 4+5; 6-8; 9; 10).

### Git workflow

- Створити гілку: `git checkout -b feature/dual-binaries` перед Етапом 1.
- Коміт після кожного етапу за HEREDOC-шаблоном з CLAUDE.md (з `Co-Authored-By: Claude Opus 4.6`).
- Не робити `--amend`. Не робити `git push --force`. Якщо помилка після коміту — новий коміт "fix: ...".
- `git push origin feature/dual-binaries` лише після того, як користувач явно попросить PR.

### Робота з кодом

- **Не вигадувати типи Win32.** Якщо `windows` crate v0.62 не експортує потрібний тип (часта проблема з Xps, Ole, Console модулями) — зупинитися, показати точну compile-помилку користувачу, запитати. Не пробувати "ймовірний" шлях імпорту.
- **Не чіпати style.** Імпорти групуються так, як зараз у `src/main.rs` (std → windows → axygen_shot). Error messages форматуються через `ShotError` варіанти — дивитись існуючі виклики у [src/watch.rs](../../src/watch.rs).
- **Не рефакторити сусідній код.** Видаляти тільки те, що явно перераховано в етапі. Якщо помічена "можливість" (dead code, duplicate) — **згадати в кінцевому звіті**, не робити.
- Борьба з borrow checker / lifetimes у `launch_daemon` навколо `OsString` і `CreateProcessW` — якщо 2 спроби не компілюються, зупинитися і показати користувачу.

### Тестування і верифікація

- **Етап 5 — критичний.** Не вважати його завершеним без live-ручного тесту в PowerShell з empty folder. Опис у секції Verify саме там. Агент **не може** його автоматизувати (prompt race видно лише в інтерактивному shell) — має ЯВНО сказати користувачу: "Етап 5 вимагає ручного тесту. Будь ласка, запустіть [команди] і підтвердіть що prompt з'являється після output."
- Smoke-тест (Етап 7) — автоматизований, його запустити агенту.
- `cargo test` — full suite на кожному етапі, не тільки змінений.

### Комунікація з користувачем

- На початку кожного етапу — **короткий (1-2 речення) анонс**: "Починаю Етап N: <що саме>."
- В кінці — "Етап N завершено. Verify: <результати>. Комміт: `<sha short>`."
- Не писати довгі резюме змін — користувач читає діфф. Писати тільки: яке рішення прийнято (якщо було на розсуд виконавця), що сюрпризне, що НЕ зроблено і чому.
- Якщо у плані щось виявилось неточним або суперечить реальному коду — **не мовчати, не адаптувати "як зручно"**. Зупинитися, описати розходження, дочекатися рішення.

### Після всіх 10 етапів

- Не створювати PR автоматично. Підсумувати коротко (≤10 рядків): список коммітів, посилання на гілку, що треба зробити користувачу (manual acceptance test з Етапу 5, потім `just ci` ще раз локально, потім `git push` + PR).
- Оновити `MEMORY.md` feedback-записом, якщо по ходу отримано нові правила від користувача.

## Джерела

- [Raymond Chen — GUI vs console subsystem](https://devblogs.microsoft.com/oldnewthing/20090101-00/?p=19643)
- [AttachConsole — Microsoft Learn](https://learn.microsoft.com/en-us/windows/console/attachconsole)
- [WezTerm CLI — dual-binary pattern](https://wezterm.org/cli/general.html)
- [Python docs — python.exe vs pythonw.exe](https://docs.python.org/3/using/windows.html)
- [devenv.com vs devenv.exe](https://learn.microsoft.com/en-us/visualstudio/ide/reference/devenv-command-line-switches)
- [Rust RFC 1665 — Windows Subsystem](https://rust-lang.github.io/rfcs/1665-windows-subsystem.html)
- Початковий баг-репорт: bug report 2026-04-18 від користувача — "prompt не з'являється одразу у PowerShell/cmd після `shot.exe`".
