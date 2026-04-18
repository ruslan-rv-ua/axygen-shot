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

`shot.exe` збирався з `#![windows_subsystem = "windows"]` (GUI-підсистема) і викликав
`AttachConsole(ATTACH_PARENT_PROCESS)` для виводу у батьківську консоль. Це створювало
race condition:

1. Оболонка читає PE-заголовок дочірнього процесу. Для GUI-підсистеми вона **не чекає**
   на завершення.
2. Оболонка одразу друкує новий prompt і викликає `ReadConsole` на stdin.
3. `shot.exe` пізніше друкує свій output через `AttachConsole` — нижче вже
   намальованого prompt.
4. Користувач бачить "output нижче осиротілого prompt" і мусить натиснути Enter.

Цей артефакт відтворюється стабільно у cmd і PowerShell (5.1 та 7). Перебудова
PE-підсистеми в рантаймі неможлива — це константа PE-заголовка.

## Розглянуті варіанти

| Варіант | Плюси | Мінуси | Рішення |
|---------|-------|--------|---------|
| **Dual binaries** (`shot.exe` CUI + `shot-watch.exe` GUI) | Канонічне рішення. Чіткий розподіл. Shell синхронний. | Два файли в дистрибутиві. | ✅ Обрано |
| `.com` stub (shot.com → shot.exe) | Один exe. Shell бачить .com як CUI. | Хак, потребує ручне створення .com stub. Нестандартний для Rust. | ❌ |
| Console-only (CUI для обох) + `FreeConsole` для daemon | Один exe. | Console-спалах при запуску з Explorer/ярлика. FreeConsole не завжди коректний. | ❌ |
| GUI-only + "press Enter" workaround | Один exe. | UX-регресія. Неприйнятно для CLI. | ❌ |

## Рішення

Dual binaries. Обґрунтування: прецеденти (python.exe/pythonw.exe,
devenv.com/devenv.exe, wezterm.exe/wezterm-gui.exe).

- `shot.exe` — subsystem=**console** (CUI). Використовується для всіх CLI-команд.
  Оболонка чекає на нього синхронно → вивід завжди перед prompt'ом.
  `AttachConsole` більше не потрібен.
- `shot-watch.exe` — subsystem=**windows** (GUI). Фоновий daemon: tray icon, hotkey,
  message loop. Запускається через `shot.exe --watch` (який робить `CreateProcessW`
  з `DETACHED_PROCESS`), або напряму з ярлика/Startup.

Обидва бінарі діляться спільним бібліотечним крейтом `axygen_shot` (src/lib.rs).

## Наслідки

- (+) Prompt у shell працює синхронно — головний баг виправлено.
- (+) `shot-watch.exe` можна запускати з ярлика без console-спалаху.
- (+) Явний розподіл parent-side/daemon-side замість runtime-сніфінгу через
  `GetConsoleWindow`.
- (−) Два exe у дистрибутиві (~1.4 MB total). Все ще портативно.
- (−) Обидва exe мають знаходитись в одній директорії (shot.exe шукає shot-watch.exe
  як sibling).

## Джерела

- [Raymond Chen — GUI vs console subsystem](https://devblogs.microsoft.com/oldnewthing/20090101-00/?p=19643)
- [AttachConsole — Microsoft Learn](https://learn.microsoft.com/en-us/windows/console/attachconsole)
- [WezTerm CLI — dual-binary pattern](https://wezterm.org/cli/general.html)
- [Python docs — python.exe vs pythonw.exe](https://docs.python.org/3/using/windows.html)
- [devenv.com vs devenv.exe](https://learn.microsoft.com/en-us/visualstudio/ide/reference/devenv-command-line-switches)
- [Rust RFC 1665 — Windows Subsystem](https://rust-lang.github.io/rfcs/1665-windows-subsystem.html)
