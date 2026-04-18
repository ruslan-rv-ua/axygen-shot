# Інтеграція зі Scoop: покроковий гайд

Цей документ пояснює, як налаштувати будь-який Windows-застосунок для встановлення та оновлення через [Scoop](https://scoop.sh/) — пакетний менеджер для Windows.

Інфраструктура побудована на двох репозиторіях:

- **scoop-bucket** ([ruslan-rv-ua/scoop-bucket](https://github.com/ruslan-rv-ua/scoop-bucket)) — зберігає маніфести пакетів і автоматичні CI/CD воркфлоу.
- **репозиторій застосунку** — ваш проєкт; у ньому живе release workflow, який після збірки повідомляє scoop-bucket про нову версію.

---

## Зміст

1. [Як Scoop працює](#як-scoop-працює)
2. [Що вже готово в scoop-bucket](#що-вже-готово-в-scoop-bucket)
3. [Крок 1: Створити маніфест](#крок-1-створити-маніфест)
4. [Крок 2: Створити release workflow](#крок-2-створити-release-workflow)
5. [Крок 3: Налаштувати токен](#крок-3-налаштувати-токен)
6. [Крок 4: Тестовий реліз](#крок-4-тестовий-реліз)
7. [Як виглядає повний цикл оновлення](#як-виглядає-повний-цикл-оновлення)
8. [Альтернатива: окремий workflow для Scoop](#альтернатива-окремий-workflow-для-scoop)
9. [Діагностика проблем](#діагностика-проблем)
10. [Чеклист](#чеклист)

---

## Як Scoop працює

Scoop — це CLI-пакетний менеджер для Windows. Він встановлює програми без прав адміністратора в теку `~/scoop/`.

Ключові концепції:

| Термін | Значення |
|--------|----------|
| **Bucket** | Git-репозиторій із JSON-маніфестами пакетів. Scoop клонує його локально. |
| **Маніфест** | JSON-файл із описом пакету: URL для завантаження, хеш, шляхи до exe. |
| **Shim** | Маленький exe-файл у `~/scoop/shims/`, що проксує виклик до реального exe. Додається в `PATH` автоматично. |
| **`checkver`** | Механізм перевірки нових версій через GitHub Releases API. |
| **`autoupdate`** | Шаблон URL для автоматичного оновлення маніфесту на нову версію. |

Типовий цикл для користувача:

```powershell
# Одноразове додавання bucket'а
scoop bucket add ruslan-rv-ua https://github.com/ruslan-rv-ua/scoop-bucket

# Встановлення
scoop install myapp

# Оновлення до останньої версії
scoop update myapp

# Видалення
scoop uninstall myapp
```

---

## Що вже готово в scoop-bucket

Вам **не потрібно** писати CI-воркфлоу для scoop-bucket — вони вже є:

| Файл | Призначення |
|------|-------------|
| `.github/workflows/update-scoop-manifest.yml` | Універсальний оновлювач. Отримує dispatch-подію від будь-якого застосунку, перевіряє SHA256, оновлює маніфест, пушить у `main`. |
| `.github/workflows/ci.yml` | Валідує маніфести через `scoop info`. При помилці — автоматичний revert останнього коміту. |
| `examples/release-template.yml` | Шаблон release workflow для копіювання в репозиторій застосунку. |
| `DEVELOPMENT.md` | Детальна документація для розробника (українською). |

Щоб додати новий застосунок, потрібно:
1. Створити маніфест у `bucket/` (scoop-bucket)
2. Створити release workflow (репозиторій застосунку)
3. Налаштувати токен

---

## Крок 1: Створити маніфест

Маніфест — JSON-файл у `bucket/` директорії scoop-bucket. Ім'я файлу = ім'я пакету в Scoop (lowercase).

### Мінімальний маніфест

Створіть `bucket/{appname}.json`:

```json
{
  "version": "1.0.0",
  "description": "Короткий опис застосунку англійською",
  "homepage": "https://github.com/ruslan-rv-ua/MyApp",
  "license": "MIT",
  "architecture": {
    "64bit": {
      "url": "https://github.com/ruslan-rv-ua/MyApp/releases/download/v1.0.0/MyApp-1.0.0-windows-x64.zip",
      "hash": "sha256-хеш-архіву"
    }
  },
  "bin": "MyApp.exe",
  "checkver": "github",
  "autoupdate": {
    "architecture": {
      "64bit": {
        "url": "https://github.com/ruslan-rv-ua/MyApp/releases/download/v$version/MyApp-$version-windows-x64.zip"
      }
    },
    "hash": {
      "url": "$url.sha256"
    }
  }
}
```

### Ключові поля

| Поле | Обов'язкове | Опис |
|------|:-----------:|------|
| `version` | ✅ | Поточна версія. Оновлюється автоматично через dispatch. |
| `description` | ✅ | Один рядок, англійською. Видно у `scoop search`. |
| `homepage` | ✅ | URL головної сторінки проєкту. |
| `license` | ✅ | SPDX-ідентифікатор ліцензії (`MIT`, `Apache-2.0`, тощо). |
| `architecture.64bit.url` | ✅ | Пряме посилання на ZIP-архів. |
| `architecture.64bit.hash` | ✅ | SHA256-хеш архіву (64 hex chars, lowercase). |
| `bin` | ✅ | Один exe або масив exe-файлів для створення shim'ів. |
| `checkver` | рекомендовано | `"github"` — перевіряє версію через GitHub Releases. |
| `autoupdate` | рекомендовано | Шаблон URL для автоматичного оновлення. `$version` замінюється на нову версію. |

### Необов'язкові поля

| Поле | Коли потрібно |
|------|--------------|
| `extract_dir` | Якщо ZIP містить піддиректорію (наприклад, `MyApp/MyApp.exe` замість `MyApp.exe`). |
| `persist` | Якщо застосунок зберігає конфігурацію у своїй директорії і вона має зберігатися між оновленнями. |
| `shortcuts` | Якщо потрібен ярлик на робочому столі або в меню Пуск (для GUI-застосунків). |
| `pre_install` / `post_install` | PowerShell-скрипти до/після встановлення. |
| `depends` | Залежності від інших Scoop-пакетів. |

### Кілька exe-файлів

Якщо застосунок містить декілька exe-файлів:

```json
"bin": [
  "myapp.exe",
  "myapp-daemon.exe"
]
```

Кожен отримає свій shim у `PATH`.

### Як порахувати SHA256

```powershell
# Для локального файлу:
(Get-FileHash -Path MyApp.zip -Algorithm SHA256).Hash.ToLower()

# Для файлу з GitHub Release:
$url = "https://github.com/.../MyApp-1.0.0-windows-x64.zip"
$temp = [System.IO.Path]::GetTempFileName()
Invoke-WebRequest -Uri $url -OutFile $temp
(Get-FileHash -Path $temp -Algorithm SHA256).Hash.ToLower()
Remove-Item $temp
```

### Як валідувати маніфест локально

```powershell
# Перевірити синтаксис JSON
python -c "import json; json.load(open('bucket/myapp.json'))"

# Перевірити через Scoop (потрібен встановлений Scoop)
scoop info myapp
```

---

## Крок 2: Створити release workflow

Release workflow живе у репозиторії застосунку і робить три речі:

1. Збирає застосунок
2. Публікує GitHub Release з ZIP + SHA256
3. Відправляє `repository_dispatch` у scoop-bucket для оновлення маніфесту

### Використання готового шаблону

У scoop-bucket є готовий шаблон: [`examples/release-template.yml`](https://github.com/ruslan-rv-ua/scoop-bucket/blob/main/examples/release-template.yml).

1. Скопіюйте його у `.github/workflows/release.yml` вашого репозиторію.
2. Змініть **`APP_NAME`** на ім'я вашого застосунку (єдине обов'язкове місце).
3. Замініть **TODO-блок збірки** на кроки збірки вашого проєкту (є коментовані приклади для C/C++, Rust, Node.js).
4. Замініть **TODO-рядки Copy-Item** у кроці `Create release package` на реальні шляхи до файлів.

### Що робить шаблон

```
git tag v1.2.3 && git push origin v1.2.3
         │
         ▼
   release.yml запускається
         │
         ├── Збирає проєкт
         ├── Пакує у ZIP: MyApp-1.2.3-windows-x64.zip
         ├── Рахує SHA256, зберігає у .sha256 файл
         ├── Публікує GitHub Release (з автогенерованими release notes)
         │
         └── repository_dispatch → scoop-bucket
              event-type: update-myapp
              payload: { app, version, hash, url }
```

### Тригери

| Тригер | Коли |
|--------|------|
| `push tags: v*` | Основний шлях: `git tag v1.2.3 && git push origin v1.2.3` |
| `workflow_dispatch` | Ручний перезапуск (якщо автоматичний завершився з помилкою) |

### Pre-release

Теги з суфіксами `-alpha`, `-beta`, `-rc` автоматично позначаються як pre-release. Для pre-release крок оновлення Scoop bucket **пропускається**.

### Важливі деталі

- **`APP_NAME`** визначає все: ім'я ZIP, ідентифікатор dispatch-події (приводиться до lowercase автоматично), ім'я маніфесту.
- **`extract_dir`** у маніфесті має збігатися з назвою директорії всередині ZIP. У шаблоні ZIP створюється з піддиректорією `$appName`. Якщо ваш ZIP плоский (файли на рівні кореня) — видаліть `extract_dir` з маніфесту.
- **SHA256 файл** (`*.zip.sha256`) публікується поряд із ZIP у GitHub Release. Scoop використовує його для автоматичних перевірок через `autoupdate.hash.url`.

---

## Крок 3: Налаштувати токен

`SCOOP_BUCKET_TOKEN` — персональний токен доступу (PAT), який дозволяє workflow у вашому репозиторії відправляти `repository_dispatch` події у scoop-bucket. Стандартний `GITHUB_TOKEN` не підходить — він має доступ лише до поточного репозиторію.

### Створити токен (одноразово)

> Якщо ви вже маєте `SCOOP_BUCKET_TOKEN` для іншого застосунку — використовуйте той самий. Один токен для всіх.

1. GitHub → аватар → **Settings** → **Developer settings** → **Personal access tokens** → **Fine-grained tokens** → **Generate new token**

2. Заповніть:

   | Поле | Значення |
   |------|----------|
   | Token name | `scoop-bucket-dispatch` |
   | Expiration | 1 рік (GitHub нагадає про оновлення) |
   | Resource owner | `ruslan-rv-ua` |
   | Repository access | **Only select repositories** → `scoop-bucket` |

3. **Permissions → Repository permissions:**
   - **Contents**: Read and write
   - **Metadata**: Read-only (додається автоматично)

4. Натисніть **Generate token**.

> ⚠️ GitHub покаже значення токена (`github_pat_...`) **лише один раз**. Збережіть його у менеджері паролів одразу.

### Додати секрет у репозиторій застосунку

1. GitHub → репозиторій застосунку → **Settings** → **Secrets and variables** → **Actions** → **New repository secret**

2. Заповніть:

   | Поле | Значення |
   |------|----------|
   | Name | `SCOOP_BUCKET_TOKEN` |
   | Secret | вставити `github_pat_...` |

3. Натисніть **Add secret**.

Після цього `${{ secrets.SCOOP_BUCKET_TOKEN }}` у workflow автоматично підставить значення. Воно ніколи не з'являється у логах.

---

## Крок 4: Тестовий реліз

### Крок 4.1: Перший реліз

```powershell
git tag v1.0.0
git push origin v1.0.0
```

Що має відбутися:
1. `release.yml` збирає проєкт
2. Публікує GitHub Release
3. Відправляє dispatch у scoop-bucket
4. `update-scoop-manifest.yml` оновлює маніфест у `bucket/`
5. `ci.yml` валідує маніфест → ✅ або автоматичний revert

### Крок 4.2: Перевірка встановлення

```powershell
scoop bucket add ruslan-rv-ua https://github.com/ruslan-rv-ua/scoop-bucket
scoop install myapp
myapp --version
```

### Крок 4.3: Перевірка оновлення

Зробіть ще один реліз (наприклад, `v1.0.1`), потім:

```powershell
scoop update myapp
myapp --version   # має показати 1.0.1
```

---

## Як виглядає повний цикл оновлення

```
1. Розробник:  git tag v1.2.3 && git push origin v1.2.3
2. release.yml:
   ├── Збирає → пакує → публікує GitHub Release
   └── repository_dispatch: update-myapp { version: "1.2.3", hash: "abc...", url: "..." }
3. update-scoop-manifest.yml (scoop-bucket):
   ├── Завантажує ZIP → перевіряє SHA256
   ├── Оновлює bucket/myapp.json (version, url, hash)
   └── Push → main
4. ci.yml (scoop-bucket):
   ├── scoop info myapp → валідує структуру
   └── ✅ або revert
5. Користувач:
   └── scoop update myapp → завантажує нову версію
```

---

## Альтернатива: окремий workflow для Scoop

Якщо ваш release workflow вже існує і ви не хочете його чіпати, можна створити **окремий workflow** з ручним запуском. Він завантажує ZIP з існуючого GitHub Release, рахує SHA256 і відправляє dispatch:

```yaml
name: Update Scoop

on:
  workflow_dispatch:
    inputs:
      version:
        description: 'Version (without v prefix, e.g. 1.2.0)'
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
          URL="https://github.com/${{ github.repository }}/releases/download/v${{ inputs.version }}/MYAPP-${{ inputs.version }}-windows-x64.zip"
          TEMP=$(mktemp)

          HTTP_CODE=$(curl -fsSL --write-out "%{http_code}" -o "$TEMP" "$URL")
          if [ "$HTTP_CODE" != "200" ]; then
            echo "ERROR: Download failed (HTTP $HTTP_CODE). Is release v${{ inputs.version }} published?"
            exit 1
          fi

          HASH=$(sha256sum "$TEMP" | awk '{print $1}')
          echo "hash=$HASH" >> "$GITHUB_OUTPUT"
          echo "url=$URL" >> "$GITHUB_OUTPUT"
          echo "SHA256: $HASH"

      - name: Dispatch to scoop-bucket
        uses: peter-evans/repository-dispatch@v3
        with:
          token: ${{ secrets.SCOOP_BUCKET_TOKEN }}
          repository: ruslan-rv-ua/scoop-bucket
          event-type: update-myapp
          client-payload: |
            {
              "app": "myapp",
              "version": "${{ inputs.version }}",
              "hash": "${{ steps.release.outputs.hash }}",
              "url": "${{ steps.release.outputs.url }}"
            }
```

Замініть `MYAPP`, `myapp` і шаблон URL на реальні значення вашого застосунку.

**Процес з окремим workflow:**
1. Запустіть `release.yml` → створюється GitHub Release
2. Запустіть `update-scoop.yml` з тією ж версією → оновлюється маніфест

---

## Діагностика проблем

### CI падає з "Invalid manifest" і робить revert

**Причина:** Невалідна структура JSON або відсутні обов'язкові поля.

**Вирішення:**
1. Перевірте JSON синтаксис: `python -c "import json; json.load(open('bucket/myapp.json'))"`
2. Перевірте через Scoop: `scoop info myapp`
3. Після виправлення запустіть оновлення вручну: GitHub → scoop-bucket → Actions → Update Scoop manifest → Run workflow

### Hash mismatch

**Причина:** SHA256, що передається в dispatch, не збігається з реальним вмістом ZIP за вказаним URL.

**Вирішення:**
1. Завантажте ZIP вручну і порахуйте хеш:
   ```powershell
   Invoke-WebRequest -Uri "URL" -OutFile temp.zip
   (Get-FileHash temp.zip -Algorithm SHA256).Hash.ToLower()
   ```
2. Порівняйте з хешем з dispatch payload
3. Перевірте як рахується хеш у `release.yml`

### Download failed (HTTP 404)

**Причина:** GitHub Release ще не опублікований на момент dispatch, або URL сформовано з помилкою.

**Вирішення:**
1. Перевірте, що Release існує на GitHub
2. Перевірте URL вручну в браузері
3. Спробуйте запустити оновлення вручну через кілька хвилин

### repository_dispatch не спрацьовує

**Причина:** Проблема з токеном.

**Вирішення:**
1. Перевірте що `SCOOP_BUCKET_TOKEN` не прострочений
2. Перевірте що токен має доступ до `scoop-bucket` з правами `Contents: write`
3. Перевірте що `event-type` відповідає паттерну `update-*` у `update-scoop-manifest.yml`

### Scoop каже "Hash check failed" при встановленні

**Причина:** Локальний кеш Scoop застарілий.

**Вирішення:**
```powershell
scoop update           # оновити метадані bucket'а
scoop cache rm myapp   # очистити кеш ZIP
scoop install myapp    # встановити заново
```

### Manifest file not found

**Причина:** Назва файлу у `bucket/` не збігається з `APP_NAME` (приведеним до lowercase) у `release.yml`.

**Вирішення:** Ім'я маніфесту має бути строго lowercase: `bucket/myapp.json` (не `bucket/MyApp.json`).

---

## Чеклист

### Для нового застосунку

- [ ] Створено `bucket/{appname}.json` у scoop-bucket (валідний JSON, правильний SHA256)
- [ ] Додано секцію у `README.md` scoop-bucket
- [ ] Створено `.github/workflows/release.yml` (або `update-scoop.yml`) у репозиторії застосунку
- [ ] Додано секрет `SCOOP_BUCKET_TOKEN` у репозиторій застосунку
- [ ] Зроблено тестовий реліз
- [ ] Перевірено `scoop install {appname}` локально
- [ ] Перевірено оновлення: другий реліз → `scoop update {appname}`

### При кожному релізі

- [ ] GitHub Release створено з ZIP + SHA256
- [ ] Маніфест у scoop-bucket оновлено (автоматично або вручну)
- [ ] CI у scoop-bucket пройшов ✅
