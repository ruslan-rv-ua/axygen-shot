# Spec: Scoop інтеграція для axygen-shot

**Дата:** 2025-07-18
**Статус:** Затверджено

---

## Мета

Дозволити встановлення та оновлення axygen-shot через Scoop:

```powershell
scoop bucket add ruslan-rv-ua https://github.com/ruslan-rv-ua/scoop-bucket
scoop install axygen-shot
scoop update axygen-shot
```

## Архітектура

Два артефакти в двох репозиторіях:

1. **scoop-bucket** — маніфест `bucket/axygen-shot.json`
2. **axygen-shot** — воркфлоу `.github/workflows/update-scoop.yml`

Потік оновлення:

```
Ручний запуск update-scoop.yml (version: "X.Y.Z")
  → Завантажує ZIP з GitHub Release vX.Y.Z
  → Рахує SHA256
  → repository_dispatch → scoop-bucket
    → update-scoop-manifest.yml оновлює bucket/axygen-shot.json
    → ci.yml перевіряє маніфест (revert при помилці)
```

---

## 1. Маніфест: `bucket/axygen-shot.json`

Файл створюється в локальному репо `C:\dev\scoop-bucket` і пушиться на GitHub.

```json
{
  "version": "0.1.0",
  "description": "Portable Windows CLI screenshot tool for blind developers",
  "homepage": "https://github.com/ruslan-rv-ua/axygen-shot",
  "license": "MIT",
  "architecture": {
    "64bit": {
      "url": "https://github.com/ruslan-rv-ua/axygen-shot/releases/download/v0.1.0/axygen-shot-0.1.0-windows-x86_64.zip",
      "hash": "placeholder — перезаписується першим запуском update-scoop.yml"
    }
  },
  "bin": [
    "shot.exe",
    "shot-watch.exe"
  ],
  "checkver": "github",
  "autoupdate": {
    "architecture": {
      "64bit": {
        "url": "https://github.com/ruslan-rv-ua/axygen-shot/releases/download/v$version/axygen-shot-$version-windows-x86_64.zip"
      }
    },
    "hash": {
      "url": "$url.sha256"
    }
  }
}
```

### Рішення

- **Назва пакету:** `axygen-shot` (збігається з GitHub repo)
- **`bin`:** обидва exe (`shot.exe`, `shot-watch.exe`) — обидва доступні з PATH
- **Без `extract_dir`:** поточний ZIP плоский (файли на рівні кореня, без піддиректорії)
- **Без `persist`:** `shot.toml` живе у проєктних директоріях, не в директорії застосунку
- **Без `shortcuts`:** CLI-інструмент, не GUI-застосунок
- **`checkver: "github"`:** використовує GitHub Releases для автоматичної перевірки версій
- **`autoupdate.hash.url`:** `$url.sha256` — SHA256 файл публікується поруч із ZIP у GitHub Release

### SHA256 файл: формат

Поточний `release.yml` генерує SHA256 у форматі `"hash  filename"`. Scoop's `autoupdate.hash.url` парсить цей формат коректно (перше слово = hash). Зміни не потрібні.

---

## 2. Воркфлоу: `.github/workflows/update-scoop.yml`

Окремий від `release.yml`. Тригер — тільки `workflow_dispatch`.

```yaml
name: Update Scoop

on:
  workflow_dispatch:
    inputs:
      version:
        description: 'Version (without v prefix, e.g. 0.2.0)'
        required: true
        type: string

jobs:
  update:
    name: Update Scoop manifest
    runs-on: ubuntu-latest
    steps:
      - name: Download release ZIP and compute hash
        id: release
        run: |
          URL="https://github.com/ruslan-rv-ua/axygen-shot/releases/download/v${{ inputs.version }}/axygen-shot-${{ inputs.version }}-windows-x86_64.zip"
          TEMP=$(mktemp)

          HTTP_CODE=$(curl -fsSL --write-out "%{http_code}" -o "$TEMP" "$URL")
          if [ "$HTTP_CODE" != "200" ]; then
            echo "ERROR: Download failed (HTTP $HTTP_CODE). Is release v${{ inputs.version }} published?"
            exit 1
          fi

          HASH=$(sha256sum "$TEMP" | awk '{print $1}')
          echo "hash=$HASH" >> "$GITHUB_OUTPUT"
          echo "url=$URL" >> "$GITHUB_OUTPUT"
          echo "Downloaded: $URL"
          echo "SHA256: $HASH"

      - name: Dispatch to scoop-bucket
        uses: peter-evans/repository-dispatch@v3
        with:
          token: ${{ secrets.SCOOP_BUCKET_TOKEN }}
          repository: ruslan-rv-ua/scoop-bucket
          event-type: update-axygen-shot
          client-payload: |
            {
              "app": "axygen-shot",
              "version": "${{ inputs.version }}",
              "hash": "${{ steps.release.outputs.hash }}",
              "url": "${{ steps.release.outputs.url }}"
            }
```

### Рішення

- **Тільки `workflow_dispatch`:** ручний запуск, без автоматичних тригерів
- **Вхід:** тільки `version` — воркфлоу сам будує URL з шаблону та рахує hash
- **Запуск на `ubuntu-latest`:** не потребує Windows (тільки curl + sha256sum)
- **Перевірка доступності:** якщо Release не знайдено — зрозуміле повідомлення про помилку
- **`event-type: update-axygen-shot`:** відповідає паттерну `update-*` у scoop-bucket

---

## 3. Необхідний секрет

`SCOOP_BUCKET_TOKEN` — Fine-grained PAT з правом `Contents: write` на `scoop-bucket`.

**Якщо токен вже існує** (використовується для BrowserSelector/QuickSnippets/Marka) — додати його як секрет `SCOOP_BUCKET_TOKEN` у репозиторій `axygen-shot`.

---

## 4. Оновлення README

Секція `## Installation` / `## Встановлення` вже існує в обох файлах. Потрібно **оновити** їх, додавши Scoop як основний спосіб і залишивши ручний як альтернативу.

**README.md** (оновлена секція `## Installation`):

```markdown
## Installation

### Via Scoop (recommended)

```powershell
scoop bucket add ruslan-rv-ua https://github.com/ruslan-rv-ua/scoop-bucket
scoop install axygen-shot
```

### Manual

Download `shot.exe` and `shot-watch.exe` from the
[latest release](https://github.com/ruslan-rv-ua/axygen-shot/releases/latest)
and place them in the same directory, anywhere on your `PATH`.
```

**README_UK.md** (оновлена секція `## Встановлення`):

```markdown
## Встановлення

### Через Scoop (рекомендовано)

```powershell
scoop bucket add ruslan-rv-ua https://github.com/ruslan-rv-ua/scoop-bucket
scoop install axygen-shot
```

### Вручну

Завантажте `shot.exe` та `shot-watch.exe` з
[останнього релізу](https://github.com/ruslan-rv-ua/axygen-shot/releases/latest)
і помістіть обидва файли в одну теку, що є у вашому `PATH`.
```

Блок `> **Why two files?** / **Чому два файли?**` та `Requirements:` залишаються після секції без змін.

---

## 5. Оновлення scoop-bucket README

Додати секцію `### Axygen Shot` до `README.md` у `scoop-bucket`, за зразком існуючих (BrowserSelector, QuickSnippets):

```markdown
### [Axygen Shot](https://github.com/ruslan-rv-ua/axygen-shot)

A portable Windows CLI screenshot tool designed for blind developers. Captures
a single window to PNG, copies to clipboard, with audio feedback.

**Key features:**
- Single-window capture via PrintWindow/BitBlt
- Watch mode with global hotkey and system tray icon
- Dual binaries: `shot.exe` (CLI) + `shot-watch.exe` (tray daemon)
- Audio feedback via Windows system sounds
- Full screen reader support

```powershell
scoop install axygen-shot
```
```

---

## Обсяг змін

| Репозиторій | Файл | Дія |
|-------------|------|-----|
| scoop-bucket | `bucket/axygen-shot.json` | Створити |
| scoop-bucket | `README.md` | Додати секцію Axygen Shot |
| axygen-shot | `.github/workflows/update-scoop.yml` | Створити |
| axygen-shot | `README.md` | Оновити секцію Installation |
| axygen-shot | `README_UK.md` | Оновити секцію Встановлення |

---

## Передумови

- GitHub Release `v0.1.0` повинен існувати з ZIP та SHA256 файлом
- Секрет `SCOOP_BUCKET_TOKEN` повинен бути доданий до axygen-shot репозиторію
- Воркфлоу `update-scoop-manifest.yml` та `ci.yml` вже існують у scoop-bucket (перевірено)

## Послідовність використання

1. Запустити `release.yml` з версією (наприклад `0.2.0`) → створюється GitHub Release
2. Запустити `update-scoop.yml` з тією ж версією → оновлюється маніфест у scoop-bucket

## Обмеження

- Маніфест оновлюється тільки вручну (без автоматичного тригеру після Release)
- Тільки 64-bit Windows (x86_64)
- Без підтримки `persist` (shot.toml — не у директорії застосунку)
