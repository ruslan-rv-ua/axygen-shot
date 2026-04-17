# Axygen Shot — Чекліст ручного тестування

> **Версія:** 0.1.0 (Phase 1 MVP)
> **Платформа:** Windows 10/11
> **Необхідно:** будь-який запущений додаток з видимим вікном (рекомендовано Notepad або Notepad++)

---

## Підготовка

Перед початком тестування:

1. Зібрати реліз: `just build`
2. Переконатися, що Notepad++ (або інший додаток) запущений
3. Створити тимчасову директорію для тестів:
   ```powershell
   $testDir = New-Item -ItemType Directory -Path "$env:TEMP\shot-test-$(Get-Random)"
   cd $testDir
   ```
4. Зберегти шлях до `shot.exe`:
   ```powershell
   $shot = "C:\dev\axygen-shot\target\release\shot.exe"
   ```

---

## 1. Базові команди

### 1.1 `--version`

```powershell
& $shot --version
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | Виводить версію | `shot 0.1.0` | |
| 2 | Exit code | `0` | |
| 3 | Промпт терміналу повертається одразу | Так | |

### 1.2 `--help`

```powershell
& $shot --help
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | Показує опис | "Screenshot tool for developers" | |
| 2 | Перелічує всі флаги | `--process`, `--title`, `--folder`, `--clipboard`, `--init`, `--check`, `--list-windows`, `--watch`, `--quiet`, `--verbose` | |
| 3 | Показує позиційний аргумент | `[LABEL]` | |
| 4 | Exit code | `0` | |

---

## 2. Ініціалізація (`--init`)

### 2.1 Базова ініціалізація

```powershell
cd $testDir
& $shot --init
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stdout: перший рядок | `status: ok` | |
| 2 | stdout: другий рядок | `file: <шлях до shot.toml>` | |
| 3 | Файл `shot.toml` створено | Існує у поточній директорії | |
| 4 | `shot.toml` містить коментарі | Рядки починаються з `#` | |
| 5 | `shot.toml` — валідний TOML | Парситься без помилок | |
| 6 | Файл `.gitignore` створено | Існує у поточній директорії | |
| 7 | `.gitignore` містить `screenshots/` | Рядок присутній | |
| 8 | Звуковий сигнал | Немає звуку (це не capture) | |
| 9 | Exit code | `0` | |

### 2.2 Ініціалізація з `--process`

```powershell
$dir1 = New-Item -ItemType Directory -Path "$testDir\init-process"
cd $dir1
& $shot --init --process=myapp.exe
Get-Content shot.toml
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | `shot.toml` містить `process = "myapp.exe"` | Розкоментований рядок | |
| 2 | `title` залишається закоментованим | `# title   = "MyApp"` | |

### 2.3 Ініціалізація з `--title`

```powershell
$dir2 = New-Item -ItemType Directory -Path "$testDir\init-title"
cd $dir2
& $shot --init --title=MyApp
Get-Content shot.toml
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | `shot.toml` містить `title   = "MyApp"` | Розкоментований рядок | |
| 2 | `process` залишається закоментованим | `# process = "myapp.exe"` | |

### 2.4 Ініціалізація з обома параметрами

```powershell
$dir3 = New-Item -ItemType Directory -Path "$testDir\init-both"
cd $dir3
& $shot --init --process=app.exe --title=MyApp
Get-Content shot.toml
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | `process = "app.exe"` | Розкоментований | |
| 2 | `title   = "MyApp"` | Розкоментований | |

### 2.5 Повторна ініціалізація (помилка)

```powershell
cd $dir1  # де вже є shot.toml
& $shot --init 2>&1
$LASTEXITCODE
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stderr: `status: error` | Так | |
| 2 | stderr: `code: init-error` | Так | |
| 3 | stderr: містить "already exists" | Так | |
| 4 | Звук помилки | `SystemHand` beep | |
| 5 | Exit code | `1` | |

### 2.6 `.gitignore` — дублювання

```powershell
$dir4 = New-Item -ItemType Directory -Path "$testDir\init-gitignore"
cd $dir4
Set-Content .gitignore "screenshots/`n"
& $shot --init
(Get-Content .gitignore | Where-Object { $_ -eq "screenshots/" }).Count
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | `screenshots/` з'являється лише один раз | Count = 1 | |

### 2.7 `.gitignore` — додавання до існуючого

```powershell
$dir5 = New-Item -ItemType Directory -Path "$testDir\init-gitignore2"
cd $dir5
Set-Content .gitignore "node_modules/"
& $shot --init
Get-Content .gitignore
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | Перший рядок: `node_modules/` | Зберігся | |
| 2 | Останній рядок: `screenshots/` | Додано | |
| 3 | Є порожній рядок між записами | Коректне форматування | |

---

## 3. Перегляд вікон (`--list-windows`)

```powershell
& $shot --list-windows
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | Формат: `title: ...` | Кожне вікно має поле `title` | |
| 2 | Формат: `process: ...` | Кожне вікно має поле `process` | |
| 3 | Формат: `pid: ...` | Кожне вікно має поле `pid` | |
| 4 | Роздільник між вікнами | `---` | |
| 5 | Немає вікон з порожнім title | Усі мають непорожній заголовок | |
| 6 | Немає `Shell_TrayWnd` | Панель завдань відсутня | |
| 7 | Notepad++ присутній | `process: notepad++.exe` (або ваш додаток) | |
| 8 | PID — числа | Коректні числові значення | |
| 9 | Exit code | `0` | |

---

## 4. Перевірка конфігурації (`--check`)

### 4.1 Check з CLI аргументами

```powershell
& $shot --check --process=notepad++.exe
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | `status: ok` | Так | |
| 2 | `config: none (CLI args)` | Без конфіг-файлу | |
| 3 | `window: running (notepad++.exe, PID XXXX)` | Процес і PID вірні | |
| 4 | `folder: <шлях>\screenshots` | Абсолютний шлях | |
| 5 | Exit code | `0` | |

### 4.2 Check з shot.toml

```powershell
cd $dir1  # де є shot.toml з process = "myapp.exe"
& $shot --check 2>&1
$LASTEXITCODE
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stderr: `code: window-not-found` | Вікно `myapp.exe` не знайдено | |
| 2 | Exit code | `1` | |

### 4.3 Check з shot.toml — реальний процес

```powershell
$dir6 = New-Item -ItemType Directory -Path "$testDir\check-real"
cd $dir6
& $shot --init --process=notepad++.exe
& $shot --check
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | `status: ok` | Так | |
| 2 | `config: <шлях до shot.toml>` | Шлях до конфігу | |
| 3 | `window: running (notepad++.exe, PID XXXX)` | Знайдено | |

### 4.4 Check — walk-up пошук конфігу

```powershell
$parent = New-Item -ItemType Directory -Path "$testDir\walkup"
cd $parent
& $shot --init --process=notepad++.exe
$child = New-Item -ItemType Directory -Path "$parent\subdir"
cd $child
& $shot --check
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | `status: ok` | Конфіг знайдено у батьківській директорії | |
| 2 | `config:` вказує на батьківську директорію | `<parent>\shot.toml` | |

### 4.5 Check — walk-up зупиняється на `.git`

```powershell
$root = New-Item -ItemType Directory -Path "$testDir\git-boundary"
cd $root
& $shot --init --process=notepad++.exe
$inner = New-Item -ItemType Directory -Path "$root\inner"
New-Item -ItemType Directory -Path "$inner\.git"
cd $inner
& $shot --check 2>&1
$LASTEXITCODE
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stderr: помилка | Конфіг НЕ знайдено (`.git` заблокував walk-up) | |
| 2 | Exit code | `1` | |

---

## 5. Захоплення скріншоту (Capture)

### 5.1 Базовий захват з CLI

```powershell
cd $testDir
& $shot --process=notepad++.exe
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stdout: `status: ok` | Так | |
| 2 | stdout: `file: <абсолютний шлях до PNG>` | Так | |
| 3 | stdout: `window: <заголовок> (PID XXXX)` | Так | |
| 4 | stdout: `size: <ширина>x<висота>` | Ненульові розміри | |
| 5 | Файл PNG створено | Існує за вказаним шляхом | |
| 6 | Папка `screenshots/` створена автоматично | Так | |
| 7 | Звук успіху | `SystemAsterisk` beep | |
| 8 | Буфер обміну: шлях до файлу | `Get-Clipboard` повертає шлях | |
| 9 | Ім'я файлу: формат `YYYY-MM-DD_HHMMSS_<title>.png` | Коректний формат | |
| 10 | Exit code | `0` | |

### 5.2 Захват з кастомним label

```powershell
& $shot --process=notepad++.exe "login form"
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | Ім'я файлу містить `login-form` | Пробіл замінено на `-` | |
| 2 | Ім'я файлу НЕ містить заголовок вікна | Тільки label | |

### 5.3 Захват з `--title`

```powershell
& $shot --title=Notepad
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | `status: ok` | Вікно знайдено по підрядку заголовка | |
| 2 | Пошук case-insensitive | Працює з будь-яким регістром | |

### 5.4 Захват з `--clipboard=image`

```powershell
& $shot --process=notepad++.exe --clipboard=image
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | `status: ok` | Так | |
| 2 | Буфер обміну: зображення | Можна вставити в Paint/Word | |
| 3 | Буфер обміну: НЕ текст | `Get-Clipboard` не повертає шлях | |

### 5.5 Захват з `--clipboard=both`

```powershell
& $shot --process=notepad++.exe --clipboard=both
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | `status: ok` | Так | |
| 2 | Буфер обміну: і текст, і зображення | Обидва формати | |

### 5.6 Захват з `--folder`

```powershell
& $shot --process=notepad++.exe --folder=captures
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | Файл збережено у папку `captures/` | Не в `screenshots/` | |

### 5.7 Захват з shot.toml

```powershell
cd $dir6  # де є shot.toml з process = "notepad++.exe"
& $shot
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | `status: ok` | Конфіг використано автоматично | |
| 2 | Скріншот збережено відносно директорії конфігу | У `<dir6>\screenshots\` | |

---

## 6. Тиша та діагностика

### 6.1 `--quiet`

```powershell
$output = & $shot --process=notepad++.exe --quiet 2>&1
$output
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stdout: порожній | Нічого не виведено | |
| 2 | Скріншот збережено | Файл існує | |
| 3 | Звук успіху | Грає (звук не залежить від `--quiet`) | |
| 4 | Буфер обміну: оновлений | Шлях скопійовано | |
| 5 | Exit code | `0` | |

### 6.2 `--verbose` перевершує `--quiet`

```powershell
& $shot --process=notepad++.exe --quiet --verbose
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stdout: `status: ok` та інші поля | Вивід НЕ підтиснутий | |

---

## 7. Обробка помилок

### 7.1 Процес не знайдено

```powershell
& $shot --process=nonexistent-app-12345.exe 2>&1
$LASTEXITCODE
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stderr: `status: error` | Так | |
| 2 | stderr: `code: window-not-found` | Так | |
| 3 | stderr: `message:` містить назву процесу | Так | |
| 4 | Звук помилки | `SystemHand` beep | |
| 5 | Exit code | `1` | |

### 7.2 Ні процесу, ні заголовка (без конфігу)

```powershell
$emptyDir = New-Item -ItemType Directory -Path "$testDir\no-config"
cd $emptyDir
& $shot 2>&1
$LASTEXITCODE
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stderr: `status: error` | Так | |
| 2 | stderr: повідомлення вказує що робити | Містить `--process` або `--title` | |
| 3 | Exit code | `1` | |

### 7.3 Невалідний folder (path traversal)

```powershell
& $shot --process=notepad++.exe --folder="..\escape" 2>&1
$LASTEXITCODE
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stderr: `code: config-error` | Так | |
| 2 | Exit code | `1` | |

### 7.4 Невалідний folder (абсолютний шлях)

```powershell
& $shot --process=notepad++.exe --folder="C:\temp" 2>&1
$LASTEXITCODE
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stderr: `code: config-error` | Так | |
| 2 | Exit code | `1` | |

### 7.5 Конфлікт режимів

```powershell
& $shot --init --check 2>&1
$LASTEXITCODE
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stderr: помилка про конфлікт | "Multiple mode flags" | |
| 2 | Exit code | ≠ 0 | |

### 7.6 `--watch` (не реалізовано)

```powershell
& $shot --watch 2>&1
$LASTEXITCODE
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stderr: помилка | "not available in Phase 1" | |
| 2 | Exit code | `1` | |

---

## 8. Іменування файлів

### 8.1 Формат імені

```powershell
& $shot --process=notepad++.exe
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | Шаблон: `YYYY-MM-DD_HHMMSS_<title>.png` | Коректний формат часу | |
| 2 | Спецсимволи замінені на `-` | `:`, `*`, `?`, `"`, `<`, `>`, `\|` → `-` | |
| 3 | Розширення `.png` | Так | |

### 8.2 Кастомний label замінює title

```powershell
& $shot --process=notepad++.exe my-screenshot
Get-ChildItem screenshots\*.png | Sort-Object LastWriteTime | Select-Object -Last 1 -ExpandProperty Name
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | Ім'я містить `my-screenshot` | Так | |
| 2 | Ім'я НЕ містить заголовок вікна | Так | |

---

## 9. Якість скріншоту

### 9.1 Перевірка PNG

```powershell
& $shot --process=notepad++.exe
$lastFile = Get-ChildItem screenshots\*.png | Sort-Object LastWriteTime | Select-Object -Last 1
$lastFile.Length
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | Файл > 0 байт | Не порожній | |
| 2 | Файл < 10 МБ | Розумний розмір | |
| 3 | Відкривається у переглядачі зображень | Валідний PNG | |
| 4 | Зображення відповідає вікну додатку | Вірний вміст | |
| 5 | Розміри збігаються з `size:` у виводі | Ширина × Висота | |

### 9.2 DPI awareness

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | На екрані зі 100% масштабом | Чіткий скріншот | |
| 2 | На екрані зі 125%+ масштабом (якщо доступно) | Фізичні пікселі, не логічні | |

---

## 10. Спеціальні символи

### 10.1 Заголовок з юнікодом

Перейменуйте вікно Notepad++ (відкрийте файл з українською назвою) або використайте додаток з unicode в заголовку.

```powershell
& $shot --title="Axygen" my-test
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | Скріншот збережено | Успішно | |
| 2 | Ім'я файлу валідне | Без crash | |

### 10.2 TOML ін'єкція в `--init`

```powershell
$dir7 = New-Item -ItemType Directory -Path "$testDir\toml-inject"
cd $dir7
& $shot --init '--process="evil" [malicious]'
Get-Content shot.toml
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | TOML валідний | Парситься без помилок | |
| 2 | Значення process екрановане | Спецсимволи не ламають формат | |

---

## 11. Продуктивність

### 11.1 Час захоплення

```powershell
Measure-Command { & $shot --process=notepad++.exe --quiet }
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | Загальний час < 2 секунди | Subsecond бажано | |

### 11.2 Розмір бінарника

```powershell
(Get-Item $shot).Length / 1KB
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | Розмір < 1 МБ | ~674 КБ | |

---

## 12. Мінімізоване вікно

### 12.1 Захват мінімізованого вікна

1. Мінімізуйте Notepad++
2. Виконайте:

```powershell
& $shot --process=notepad++.exe 2>&1
$LASTEXITCODE
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | stderr: `code: window-minimized` | Так | |
| 2 | stderr: "restore it and try again" | Так | |
| 3 | Exit code | `1` | |
| 4 | Звук помилки | `SystemHand` beep | |

3. Відновіть вікно після тесту.

---

## 13. Кілька вікон одного процесу

### 13.1 Foreground preference

1. Відкрийте два вікна одного додатку (наприклад, два екземпляри Notepad)
2. Зфокусуйте одне з них
3. Виконайте:

```powershell
& $shot --process=notepad.exe
```

| # | Перевірка | Очікувано | ✅/❌ |
|---|-----------|-----------|-------|
| 1 | Захоплено foreground вікно | Перевірити візуально | |

---

## 14. Watch Mode

### 14.1 Базовий запуск

```powershell
& $shot --watch --process=notepad.exe
```

| # | Тест | Команда | Очікуваний результат |
|---|------|---------|---------------------|
| 1 | Запуск watch mode | `shot --watch --process=notepad.exe` | `status: ok` + PID, термінал повертає промпт |
| 2 | Tray icon з'являється | (перевірити після запуску) | Іконка в треї, tooltip "shot — watching notepad.exe" |
| 3 | Hotkey capture | Натиснути Win+F12 | Скріншот збережено, beep успіху |
| 4 | Capture serialization | Швидко натиснути Win+F12 двічі | Другий натиск — busy sound |
| 5 | Exit через tray | ПКМ → Exit | Daemon зупиняється, іконка зникає |
| 6 | Restart через tray | ПКМ → Restart | Daemon перезапускається, нова іконка |
| 7 | Hotkey override | `shot --watch --process=notepad.exe --hotkey=Win+F11` | Працює Win+F11 замість F12 |
| 8 | Hotkey conflict | Запустити два --watch з однаковим hotkey | Другий — MessageBox помилки |
| 9 | Startup sound | Запустити --watch | SystemExclamation при старті |
| 10 | Quiet mode | `shot --watch --process=notepad.exe --quiet` | Без stdout, daemon працює |
| 11 | Немає конфігу | `shot --watch` (без shot.toml, без --process/--title) | Помилка в stderr |
| 12 | hotkey з shot.toml | Створити shot.toml з `hotkey = "Win+F11"` | Працює Win+F11 |
| 13 | Window not found | Вказати неіснуючий процес, натиснути hotkey | Error beep + MessageBox |
| 14 | Window minimized | Мінімізувати вікно, натиснути hotkey | Error beep + MessageBox "minimized" |

---

## Прибирання після тестування

```powershell
cd ~
Remove-Item -Recurse -Force $testDir
Remove-Item -Recurse -Force C:\dev\axygen-shot\screenshots -ErrorAction SilentlyContinue
```

---

## Підсумок

| Категорія | Кількість перевірок |
|-----------|---------------------|
| Базові команди | 7 |
| Ініціалізація | 22 |
| Перегляд вікон | 9 |
| Перевірка конфігурації | 13 |
| Захоплення скріншоту | 25 |
| Тиша/діагностика | 6 |
| Обробка помилок | 15 |
| Іменування файлів | 5 |
| Якість скріншоту | 7 |
| Спеціальні символи | 4 |
| Продуктивність | 2 |
| Мінімізоване вікно | 4 |
| Кілька вікон | 1 |
| Watch mode | 14 |
| **Всього** | **134** |
