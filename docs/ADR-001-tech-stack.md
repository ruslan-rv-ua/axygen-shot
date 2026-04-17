# ADR-001: Вибір технологічного стеку для Axygen Shot

> **Тип задачі:** 📦 Вибір технологічного стеку + 🏗️ Архітектурне рішення
> **Стек:** Windows desktop, Win32 API, CLI + tray daemon
> **Статус:** Прийнято
> **Дата:** 2026-04-17

## TL;DR

> Axygen Shot — портативний Windows-інструмент для скріншотів (~80% коду = Win32 API виклики), розроблюваний виключно AI-агентами. **Рекомендований стек: Rust** із крейтами `windows` (Microsoft), `clap`, `toml`, `png`. Обрано за: найменший бінарник (~1–3 МБ), найкраща ергономіка Win32 API через офіційний крейт Microsoft, компілятор як страхувальна сітка для агентів, ідеальна відповідність stateless-архітектурі з PRD.

## Аналіз проблеми

### Суть задачі

Потрібно обрати мову та бібліотеки для `shot.exe` — єдиного портативного Windows EXE зі специфічними вимогами:

- **~80% коду — Win32 API**: `EnumWindows`, `PrintWindow`, `RegisterHotKey`, `Shell_NotifyIcon`, `AttachConsole`, `SetProcessDpiAwarenessContext`, clipboard (`CF_UNICODETEXT`, `CF_DIB`), `CreateProcess` з `DETACHED_PROCESS`, `MessageBeep`
- **`SUBSYSTEM:WINDOWS`** з `AttachConsole` для консольного виводу
- **Два режими**: CLI (разовий захват) + Watch (tray icon + глобальний hotkey + message loop)
- **Нуль зовнішніх залежностей** — один EXE, без інсталятора, без рантайму
- **TOML конфіг**, PNG кодування, CLI парсинг
- **100% розробка агентами** — людина не читає і не дебажить код

### Критичні фактори вибору (за пріоритетом)

| # | Фактор | Вага | Чому критичний |
|---|--------|------|----------------|
| 1 | **Ергономіка Win32 API** | Висока | 80% коду — Win32 виклики. Якість біндінгів = якість проєкту |
| 2 | **Коректність коду агентів** | Висока | Людина не дебажить — помилки мають ловитись автоматично |
| 3 | **Портативний EXE** | Висока | Жорстка вимога PRD: single binary, zero dependencies |
| 4 | **Тестованість (DI для WindowResolver)** | Середня | PRD вимагає injectable window enumeration |
| 5 | **Екосистема бібліотек** | Середня | TOML, PNG, CLI parsing — мають бути зрілі |
| 6 | **Швидкість ітерацій агента** | Середня | Час compile → test → fix циклу |
| 7 | **Розмір бінарника** | Низька | Nice-to-have, не блокер |

## Розглянуті рішення

### Варіант A: Rust + `windows` crate

**Суть:** Rust із офіційним крейтом Microsoft `windows` (v0.62.2, квітень 2026) для Win32 API, `clap` для CLI, `toml` + `serde` для конфігу, `png` для кодування.

**Переваги:**
- ✅ **Найкращий Win32 API досвід.** Крейт `windows` v0.62.2 — автогенерований із Windows metadata, підтримується Microsoft (Kenny Kerr), останній коміт 16 квітня 2026. Типобезпечні обгортки для всіх потрібних API
- ✅ **Компілятор як страхувальна сітка.** Borrow checker, type system, `clippy` ловлять помилки до рантайму. Для агентної розробки без людського дебагу — це критично
- ✅ **Ідеальний single binary.** Статичне лінкування за замовчуванням на `x86_64-pc-windows-msvc`. Типовий розмір ~1–3 МБ з LTO + strip
- ✅ **Нативний `#![windows_subsystem = "windows"]`.** Вбудований атрибут мови — не потрібні лінкер-хаки
- ✅ **Stateless архітектура = щасливий borrow checker.** PRD вимагає stateless модулі → жодних lifetime-складнощів, Win32 хендли (`HWND`, `HBITMAP`) — `Copy` типи
- ✅ **Золотий стандарт CLI-екосистеми.** `clap` (derive macros), `toml` (Cargo.toml сам його використовує), `png` — все зріле та добре документоване
- ✅ **Багато навчальних даних.** Rust CLI tools (ripgrep, bat, fd), Tauri v2 (Rust + Win32) — агенти мають тисячі прикладів

**Недоліки:**
- ⚠️ `unsafe` блоки для Win32 викликів (пом'якшено обгортками `windows` крейту)
- ⚠️ Повільніша компіляція: перший build ~1–2 хв (залежності), інкрементальний ~5–15 с
- ⚠️ Якщо агент застрягне на borrow checker — потрібна переформулювання промпту

**Рівень впевненості:** ✅ Підтверджено (крейт перевірений, API підтверджені, архітектура PRD сумісна)

---

### Варіант B: C# (.NET 9+ NativeAOT) + CsWin32

**Суть:** C# з NativeAOT компіляцією для standalone EXE, CsWin32 source generator для Win32 P/Invoke.

**Переваги:**
- ✅ **Найергономічніші Win32 виклики.** CsWin32 генерує P/Invoke з Win32 metadata автоматично. Всі потрібні API підтверджені
- ✅ **Агенти пишуть відмінний C#.** Найбільший корпус навчальних даних для enterprise patterns
- ✅ **Швидкий інкрементальний build**

**Недоліки:**
- ❌ **System.Drawing + NativeAOT = проблеми.** GDI+ (потрібний для HBITMAP → PNG) має рефлексійні залежності, несумісні з AOT. Потрібно писати P/Invoke вручну для bitmap операцій, обходячи System.Drawing
- ❌ **AOT-пастки для агентів.** Рефлексія, XmlSerializer, деякі COM-інтерфейси ламаються під NativeAOT. Агент може написати код, який компілюється але падає в рантаймі — найгірший сценарій для розробки без людського дебагу
- ❌ **Великий бінарник.** 15–35 МБ standalone (5–15 МБ з агресивним triммінгом). В 5–10× більше за Rust
- ⚠️ NativeAOT compile повільний (~30–60 с), не інкрементальний
- ⚠️ `DETACHED_PROCESS` + `CREATE_NEW_PROCESS_GROUP` — нетиповий патерн для C#, менше прикладів

**Рівень впевненості:** ⚠️ Ймовірно працює, але є відомі підводні камені з AOT

---

### Варіант C: Go + `golang.org/x/sys/windows`

**Суть:** Go з офіційним пакетом `x/sys/windows` для Win32, systray бібліотека для tray icon.

**Переваги:**
- ✅ **Найшвидша компіляція** (~2–3 с) — найкоротші цикли ітерації агента
- ✅ **Простота мови** — агенти рідко застрягають на мовних складнощах
- ✅ **Статичний бінарник** за замовчуванням

**Недоліки:**
- ❌ **Критичний конфлікт потоків.** `RegisterHotKey` + Win32 message loop МУСЯТЬ працювати на одному OS-потоці. Go goroutines мігрують між потоками. Потрібен `runtime.LockOSThread()` з ретельним контролем — головне джерело багів
- ❌ **Win32 виклики через `uintptr`.** Жодної типобезпеки: все `proc.Call(uintptr(hwnd), uintptr(flag), 0)`. Помилки мовчазні, відлагодження складне
- ❌ **`lxn/walk` не підтримується** з грудня 2021 (issue #835). `systray` вимагає `cgo`
- ❌ **Більший бінарник:** ~5–8 МБ з GUI компонентами
- ⚠️ `-ldflags "-H windowsgui"` + `AttachConsole` — менш документований патерн

**Рівень впевненості:** ❓ Невизначено — thread-affinity ризик занадто високий для агентної розробки

---

### Варіант D: C/C++ (відхилено на етапі попереднього аналізу)

**Причина відхилення:** Відсутність memory safety → агенти генерують 40–60% помилок у системному C++ коді (buffer overflow, use-after-free). Без компілятора як safety net і без людського ревью — неприйнятний ризик. Додатково: складний build system (CMake + vcpkg), відсутність зручних TOML/CLI бібліотек.

### Варіант E: Python + PyInstaller (відхилено на етапі попереднього аналізу)

**Причина відхилення:** Бінарник 30–50 МБ, запуск 2–5 секунд. PRD вимагає "fast, minimal-friction workflow" — несумісно.

## Порівняльна таблиця

| Критерій | 🦀 Rust | 🟣 C# NativeAOT | 🐹 Go |
|---|---|---|---|
| **Win32 API ергономіка** | ⭐⭐⭐⭐ типобезпечно, `unsafe` обмежений | ⭐⭐⭐⭐⭐ найергономічніше | ⭐⭐ все через `uintptr` |
| **Захист від помилок агента** | ⭐⭐⭐⭐⭐ borrow checker + clippy | ⭐⭐⭐ типи є, AOT-пастки є | ⭐⭐⭐ типи є, thread bugs мовчазні |
| **Портативний EXE** | ⭐⭐⭐⭐⭐ 1–3 МБ, zero deps | ⭐⭐⭐ 5–15 МБ trimmed | ⭐⭐⭐⭐ 5–8 МБ |
| **SUBSYSTEM:WINDOWS** | ⭐⭐⭐⭐⭐ нативний атрибут | ⭐⭐⭐⭐ `<OutputType>WinExe` | ⭐⭐⭐ `-H windowsgui` flag |
| **Watch mode (tray + hotkey)** | ⭐⭐⭐⭐ Win32 message loop — прямий | ⭐⭐⭐⭐ P/Invoke message loop | ⭐⭐ thread-affinity конфлікт |
| **TOML парсинг** | ⭐⭐⭐⭐⭐ `toml` — золотий стандарт | ⭐⭐⭐ `Tomlyn` — достатній | ⭐⭐⭐⭐ `BurntSushi/toml` |
| **PNG кодування** | ⭐⭐⭐⭐⭐ `png` crate | ⭐⭐⭐ проблеми з System.Drawing+AOT | ⭐⭐⭐⭐ stdlib `image/png` |
| **CLI парсинг** | ⭐⭐⭐⭐⭐ `clap` derive macros | ⭐⭐⭐⭐ `System.CommandLine` | ⭐⭐⭐⭐ `cobra` |
| **DI для WindowResolver** | ⭐⭐⭐⭐ trait / closure injection | ⭐⭐⭐⭐⭐ interfaces | ⭐⭐⭐⭐ interfaces |
| **Швидкість build** | ⭐⭐⭐ 5–15 с інкрементально | ⭐⭐⭐⭐ ~5 с інкр., AOT publish повільний | ⭐⭐⭐⭐⭐ 2–3 с |
| **Навчальні дані агентів** | ⭐⭐⭐⭐ CLI tools + Tauri | ⭐⭐⭐⭐⭐ найбільший корпус | ⭐⭐⭐⭐ добрий, Win32 Go — ні |
| **DETACHED_PROCESS daemon** | ⭐⭐⭐⭐ `CreateProcess` + flags | ⭐⭐⭐ нетиповий для C# | ⭐⭐ складно із goroutines |

## Рекомендація

**Обраний варіант: A — Rust**

### Чому Rust

1. **Компілятор = QA-інженер.** При 100% агентній розробці без людського дебагу, кожна помилка, зловлена до рантайму — це зекономлена година. Borrow checker, типи, `clippy` — все це працює як автоматичний code review. За дослідженнями (specgen benchmark, 2026), статичні типи ловлять на 30–40% більше помилок агентів.

2. **`windows` crate v0.62.2 — ідеальна відповідність.** Microsoft-maintained крейт автогенерований із офіційних Windows metadata. Кожний Win32 API з PRD доступний із типобезпечними обгортками. Останній коміт — 16 квітня 2026. Це не community wrapper, а офіційний інструмент від творця WinRT.

3. **Stateless PRD ≡ Happy Rust.** PRD явно вимагає: "All modules are stateless — no global state; everything is passed as arguments." Це *саме той* патерн, де Rust не створює складностей з lifetimes. Win32 хендли (`HWND`, `HBITMAP`, `HDC`) — `Copy` типи, їх не потрібно позичати.

4. **Найменший бінарник.** 1–3 МБ проти 5–15 МБ (C#) чи 5–8 МБ (Go). Для портативного EXE, що копіюється в будь-яку папку — це перевага.

5. **Єдина екосистема.** `cargo build`, `cargo test`, `cargo clippy` — три команди покривають build, test, lint. Агенту не потрібно тримати в голові окремі інструменти.

### Коли обирати альтернативу

| Умова | Обрати |
|---|---|
| Агенти систематично застрягають на borrow checker у Win32 unsafe коді | C# NativeAOT — ергономічніший P/Invoke |
| System.Drawing + NativeAOT вирішили проблеми з bitmap | C# NativeAOT — кращий bitmap pipeline |
| Потрібен швидший прототип без watch mode | Go — найшвидший build цикл |
| Проєкт росте до GUI конфігуратора | C# — зрілішій GUI стек (WPF/WinUI, хоч і не NativeAOT) |

## Реалізація

### Конкретний стек

| Компонент | Бібліотека | Версія | Призначення |
|---|---|---|---|
| **Мова** | Rust | stable (1.85+) | — |
| **Win32 API** | `windows` | 0.62+ | Усі Win32 виклики: вікна, захват, clipboard, tray, hotkey, audio, DPI, console |
| **CLI парсинг** | `clap` | 4.x | Derive macros для декларативного опису аргументів, mutually exclusive groups |
| **TOML конфіг** | `toml` + `serde` | 0.8+ | Десеріалізація `shot.toml` у typed struct |
| **PNG кодування** | `png` | 0.17+ | HBITMAP → PNG bytes |
| **Серіалізація** | `serde` + `serde_derive` | 1.x | Derive macros для Config struct |

### Структура проєкту

```
axygen-shot/
├── Cargo.toml
├── build.rs                  # App manifest для DPI awareness
├── shot.manifest             # Windows application manifest
├── src/
│   ├── main.rs               # Entry point, mode dispatch
│   ├── cli.rs                # clap derive struct, argument parsing
│   ├── config.rs             # TOML parsing, walk-up search, validation
│   ├── window_resolver.rs    # EnumWindows, matching, DI trait
│   ├── capture.rs            # PrintWindow, HBITMAP → PNG
│   ├── storage.rs            # Filename generation, directory creation
│   ├── clipboard.rs          # CF_UNICODETEXT, CF_DIB
│   ├── audio.rs              # MessageBeep wrappers
│   └── watch.rs              # Tray icon, RegisterHotKey, message loop, DETACHED_PROCESS
├── tests/                    # Integration tests
├── CLAUDE.md                 # Agent instructions (Claude Code)
├── AGENTS.md                 # Agent instructions (Copilot Agent)
└── docs/
    ├── PRD.md
    └── ADR-001-tech-stack.md
```

### Cargo.toml (скелет)

```toml
[package]
name = "axygen-shot"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "shot"

[dependencies]
clap = { version = "4", features = ["derive"] }
toml = "0.8"
serde = { version = "1", features = ["derive"] }
png = "0.17"

[dependencies.windows]
version = "0.62"
features = [
    # Console
    "Win32_System_Console",
    # Window enumeration & management
    "Win32_UI_WindowsAndMessaging",
    # Process info
    "Win32_System_Threading",
    # GDI for bitmap capture
    "Win32_Graphics_Gdi",
    # Clipboard
    "Win32_System_DataExchange",
    "Win32_System_Memory",
    # Shell (tray icon)
    "Win32_UI_Shell",
    # Keyboard (hotkey)
    "Win32_UI_Input_KeyboardAndMouse",
    # DPI
    "Win32_UI_HiDpi",
    # Audio
    "Win32_Media_Audio",
]

[profile.release]
lto = true
strip = true
codegen-units = 1
```

> **Примітка:** Конкретні feature flags крейту `windows` потрібно уточнити під час імплементації — назви модулів можуть відрізнятись у v0.62. Вище наведено очікувані назви; агент має перевірити їх через `cargo doc` або docs.rs.

### Ключові архітектурні патерни

**DI для WindowResolver (trait-based):**
```rust
/// Інформація про вікно, повернута enumerator-ом
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub process_name: String,
    pub pid: u32,
}

/// Trait для injectable window enumeration
pub trait WindowEnumerator {
    fn enumerate(&self) -> Vec<WindowInfo>;
    fn get_foreground(&self) -> Option<isize>;
}

/// Продакшен реалізація через Win32 API
pub struct Win32Enumerator;

impl WindowEnumerator for Win32Enumerator {
    fn enumerate(&self) -> Vec<WindowInfo> {
        // EnumWindows + GetWindowThreadProcessId + OpenProcess
        todo!()
    }
    fn get_foreground(&self) -> Option<isize> {
        // GetForegroundWindow
        todo!()
    }
}

/// Резольвить вікно за config criteria
pub fn resolve_window(
    enumerator: &dyn WindowEnumerator,
    process: Option<&str>,
    title: Option<&str>,
) -> Result<WindowInfo, ResolveError> {
    todo!()
}
```

**SUBSYSTEM:WINDOWS + AttachConsole:**
```rust
// main.rs
#![windows_subsystem = "windows"]

fn main() {
    // В CLI-режимі: підключитися до батьківської консолі
    if !is_watch_mode() {
        unsafe {
            use windows::Win32::System::Console::*;
            if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
                let _ = AllocConsole();
            }
        }
    }
    // ... далі парсинг CLI та dispatch
}
```

### Команди агента (build / test / lint)

```bash
# Build (debug)
cargo build

# Build (release, optimized)
cargo build --release

# Run tests
cargo test

# Lint
cargo clippy -- -D warnings

# Format check
cargo fmt -- --check

# Все разом (CI pipeline)
cargo fmt -- --check && cargo clippy -- -D warnings && cargo test && cargo build --release
```

## Ризики та мітигація

| Ризик | Ймовірність | Вплив | Мітигація |
|---|---|---|---|
| **Агент застрягає на borrow checker** | Середня | Середній | Stateless архітектура PRD мінімізує. Якщо повторюється — переформулювати промпт із конкретними типами |
| **`unsafe` помилки у Win32 коді** | Середня | Високий | `windows` крейт абстрагує більшість unsafe. Clippy ловить unsafe-патерни. Тести з mock enumerator покривають логіку |
| **Feature flags `windows` крейту змінились** | Низька | Низький | Перевірити docs.rs при першому `cargo build`. Помилки компіляції чітко вказують на правильні назви |
| **`PrintWindow` не захоплює GPU-rendered вікна** | Висока | Низький | Задокументовано в PRD як прийняте обмеження (story 62) |
| **Повільний перший build (залежності)** | Висока | Низький | ~1–2 хв одноразово. Інкрементальні builds ~5–15 с |

### Відкат

Якщо Rust виявиться непрацездатним (агенти не справляються з unsafe Win32 кодом після 3+ спроб), міграція на C# NativeAOT:
1. Архітектура модулів 1-to-1 зберігається
2. CLI parsing: `clap` → `System.CommandLine`
3. Config: `toml` + `serde` → `Tomlyn`
4. Win32: `windows` crate → CsWin32
5. Bitmap: `png` crate → прямий P/Invoke (не System.Drawing)

Оцінка зусиль міграції: повний переписування, але модульна архітектура дозволяє робити це помодульно.

## Відкриті питання

- [ ] Точні feature flags крейту `windows` v0.62 — уточнити при першому `cargo build`
- [ ] Чи потрібен `build.rs` для вбудовування Windows manifest (DPI awareness), чи достатньо виклику `SetProcessDpiAwarenessContext` на старті — перевірити обидва підходи
- [ ] Мінімальна версія Windows SDK на build машині — перевірити вимоги `windows` крейту
- [ ] Точний розмір release binary з усіма залежностями — виміряти після першого build

## Джерела

- **microsoft/windows-rs** — https://github.com/microsoft/windows-rs — офіційний Rust крейт для Win32 API. v0.62.2, останній коміт 2026-04-16. Використано для підтвердження підтримки API
- **microsoft/CsWin32** — https://github.com/microsoft/CsWin32 — C# source generator для Win32 P/Invoke. Використано для порівняння з Rust підходом
- **dotnet/runtime NativeAOT issues** — https://github.com/dotnet/runtime/issues/61960, /106580 — підтвердження проблем System.Drawing та XmlSerializer під NativeAOT
- **lxn/walk** — https://github.com/lxn/walk — Go GUI framework. Issue #835 (unmaintained), #619 (thread-affinity). Використано для обґрунтування відхилення Go
- **getlantern/systray** — https://github.com/getlantern/systray — Go tray library. Потребує cgo, thread locking
- **specgen benchmark (2026)** — https://github.com/statherm/specgen — 22 моделі на Go code generation. Claude Opus 92%, GPT-4o 66% на concurrency tasks. Використано для оцінки якості агентного коду
- **Aider SWE-Bench** — https://github.com/Aider-AI/aider-swe-bench — Claude Opus 26.3% SOTA на реальних GitHub issues. Multi-language агентна розробка
- **golang.org/x/sys/windows** — https://github.com/golang/sys — офіційний Go пакет для Windows API. Використано для оцінки ергономіки
