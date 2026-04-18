# Plan: UX помилок при прямому запуску shot-watch.exe

> **Дата:** 2026-07-18
> **Статус:** Draft
> **Залежності:** feature/dual-binaries (ADR-002)

## Проблема

`shot-watch.exe` — GUI-subsystem бінарник (`#![windows_subsystem = "windows"]`).
Коли його запускають напряму (двоклік, ярлик, Task Scheduler) **без** `shot.toml`
або CLI-аргументів `--process`/`--title`, він мовчки завершується з exit code 1.
Жодного зворотного зв'язку: немає консолі (GUI subsystem), немає MessageBox, немає
звуку.

### Де саме розрив

```
shot-watch.exe main()
  ├─ cli::parse()              ← OK
  ├─ config::find_config()     ← повертає None (немає shot.toml)
  ├─ config::merge()           ← Err(ConfigError("No target: ..."))
  └─ result.is_err() → exit(1) ← МОВЧКИ
```

Помилка виникає **до** виклику `watch::run_daemon()`. А `run_daemon()` вже має
`handle_startup_err()`, який показує `MessageBoxW` при `parent_pid == None`
(тобто при прямому запуску). Тобто інфраструктура є — просто config-помилки її
обминають.

### Контракт для незрячих

Головна аудиторія — незрячий розробник зі скрінрідером. Win32 `MessageBoxW`
автоматично зачитується скрінрідером (JAWS, NVDA). Крім того, `MB_ICONERROR`
програє системний звук "Critical Stop". Це найнадійніший канал зворотного зв'язку
для GUI-бінарника без консолі.

## Розглянуті варіанти

| # | Варіант | Плюси | Мінуси | Рішення |
|---|---------|-------|--------|---------|
| A | **MessageBoxW у shot-watch.rs main()** | Мінімальна зміна (3–5 рядків). Скрінрідер зачитує. Системний звук. Консистентно з `handle_startup_err`. | Дублювання виклику MessageBoxW (але він вже є в watch.rs). | ✅ Обрано |
| B | Перенести config parsing в `run_daemon()` | Всі помилки йдуть через `handle_startup_err`. Нуль дублювання. | Міняє сигнатуру `run_daemon(cfg)` → `run_daemon(args)`. Інвазивний рефакторинг. Config parsing для інших режимів дублюватиметься або потрібен новий абстракційний шар. | ❌ |
| C | Витягнути `pub fn show_gui_error()` в `errors.rs` | Один source of truth для GUI-помилок. | Зв'язує `errors.rs` з Win32 UI (зараз це чистий data/format модуль). Порушує SRP. | ❌ |
| D | Лог-файл + Windows Event Log | Персистентний, пошуковий. | Мовчазний — скрінрідер не зачитає. Складність реалізації. Overkill для CLI-утиліти. | ❌ |
| E | Toast-нотифікації | Сучасний UX. | Потребує COM + AppUserModelID. Складний в реалізації. Підтримка скрінрідерами нестабільна. | ❌ |
| F | Зробити `watch::message_box()` pub | Реюз наявного хелпера. | Оголює внутрішню функцію watch-модуля. message_box — занадто загальна назва для pub API. | ❌ |

### Чому Варіант A

1. **Мінімальність.** 3–5 нових рядків в `shot-watch.rs`. Жодних змін в інших модулях.
2. **Консистентність.** Той самий патерн що `handle_startup_err` використовує для
   помилок без parent PID.
3. **Доступність.** `MessageBoxW` + `MB_ICONERROR` — золотий стандарт для
   скрінрідерів на Windows.
4. **Ідемпотентність.** Якщо помилку виправить користувач (додасть shot.toml),
   наступний запуск просто працює. MessageBox не залишає артефактів.

### Edge case: Task Scheduler / unattended контекст

`MessageBoxW` у Session 0 (service context) блокується і ніколи не закриється.
Але `shot-watch.exe` не Service і не має сценарію Session 0. Якщо запускати через
Task Scheduler, він працює в інтерактивній сесії користувача → MessageBox покажеться.

## Рішення

### Зміни в `src/bin/shot-watch.rs`

**УВАГА:** Не можна просто обгорнути весь `result` у MessageBox — це спричинить
подвійний MessageBox для daemon-помилок. `handle_startup_err()` у `run_daemon`
вже показує MessageBox при `parent_pid == None`, а потім повертає `Err(e)`.
Якщо `shot-watch.rs` теж покаже MessageBox для цього `Err(e)` — буде два
однакових діалоги підряд.

**Тому:** розділити config-фазу та daemon-фазу.

Поточний код:

```rust
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
        std::process::exit(1);
    }
}
```

Новий код:

```rust
fn main() {
    let args = cli::parse();

    // Config phase — GUI binary has no console, show error via MessageBox
    // so screen readers can announce it (MB_ICONERROR plays Critical Stop sound).
    let cfg = match (|| -> Result<config::CaptureConfig, errors::ShotError> {
        let cwd = std::env::current_dir()
            .map_err(|e| errors::ShotError::ConfigError(format!("Cannot determine CWD: {}", e)))?;
        let toml = config::find_config(&cwd)?;
        config::merge(&args, toml)
    })() {
        Ok(cfg) => cfg,
        Err(e) => {
            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR};
            use windows::core::PCWSTR;
            let msg = errors::format_error_message(&e);
            let msg_w: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
            let title = "Axygen Shot \u{2014} Error";
            let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe {
                let _ = MessageBoxW(
                    None,
                    PCWSTR(msg_w.as_ptr()),
                    PCWSTR(title_w.as_ptr()),
                    MB_ICONERROR,
                );
            }
            std::process::exit(1);
        }
    };

    // Daemon phase — run_daemon handles its own errors via handle_startup_err
    // (which already shows MessageBox when parent_pid is None).
    if watch::run_daemon(&cfg, args.daemon_parent_pid).is_err() {
        std::process::exit(1);
    }
}
```

**Ключова різниця від наївного підходу:**
- Config-помилки (find_config, merge) → MessageBox у shot-watch.rs ← НОВИЙ
- Daemon-помилки (mutex, hotkey, tray) → MessageBox у handle_startup_err ← ІСНУЮЧИЙ
- Жодних подвійних MessageBox

### Тестування

| # | Тест | Дія | Очікувано |
|---|------|-----|-----------|
| 1 | Без конфігу, без аргументів | Двоклік `shot-watch.exe` | MessageBox: "Configuration error: No target: provide --process or --title (or set in shot.toml)" |
| 2 | З валідним конфігом | Двоклік `shot-watch.exe` з теки де є `shot.toml` | Tray icon з'являється |
| 3 | Запуск через `shot --watch` | `shot --watch --process=notepad.exe` | Працює як раніше (parent PID є, MessageBox не показується) |
| 4 | Скрінрідер | Тест 1 з увімкненим NVDA | NVDA зачитує текст MessageBox і "Critical Stop" звук |

### Не потрібно міняти

- `src/watch.rs` — `handle_startup_err` та `message_box` залишаються private.
  Дублювання MessageBoxW виклику мінімальне (~8 рядків) і виправдане: shot-watch.rs
  обробляє pre-daemon помилки, watch.rs обробляє in-daemon помилки.
- `src/errors.rs` — `format_error_message` вже pub, нічого додавати.
- `Cargo.toml` — `Win32_UI_WindowsAndMessaging` feature вже є.

### Також оновити

- `docs/manual-testing-checklist.md` — §14.0 "Перевірка dual-binary": додати тест
  "двоклік shot-watch.exe без конфігу → MessageBox з помилкою". Зараз тест 1 каже
  "Tray icon з'являється" — це передбачає валідний конфіг. Потрібен окремий рядок
  для сценарію без конфігу.

### Commit

```
fix: show MessageBox for config errors in shot-watch.exe

GUI-subsystem binary has no console, so config errors were silently
swallowed. Now shows a MessageBox with MB_ICONERROR so screen readers
can announce the error and the Critical Stop sound plays.
```
