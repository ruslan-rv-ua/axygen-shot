# Scoop Integration Implementation Plan

> [!WARNING]
> **Історичний запис. Реалізоване тут згодом замінено.**
>
> Описаний нижче механізм — workflow `update-scoop.yml`, який слав
> `repository_dispatch` до `scoop-bucket`, і секрет `SCOOP_BUCKET_TOKEN` для
> цього — прибрано. Маніфест тепер оновлює сам bucket: його workflow
> `Excavator` читає `checkver` і `autoupdate` у `bucket/axygen-shot.json`,
> запускається вручну з
> [Actions](https://github.com/ruslan-rv-ua/scoop-bucket/actions), плюс раз на
> добу о 04:20 UTC.
>
> Документ не переписано навмисно: він точно описує те, що було вирішено й
> зроблено на вказану дату. Читайте його як запис, а не як інструкцію.


> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable installation and updates of axygen-shot via Scoop package manager.

**Architecture:** Two repositories involved — `scoop-bucket` (manifest + README) and `axygen-shot` (workflow + READMEs). The `update-scoop.yml` workflow dispatches to scoop-bucket which auto-updates the manifest.

**Tech Stack:** GitHub Actions, Scoop manifests (JSON), `peter-evans/repository-dispatch@v3`

**Spec:** `docs/superpowers/specs/2025-07-18-scoop-integration-design.md`

---

## Chunk 1: scoop-bucket changes

### Task 1: Create Scoop manifest

**Files:**
- Create: `C:\dev\scoop-bucket\bucket\axygen-shot.json`

- [ ] **Step 1: Create manifest file**

Create `C:\dev\scoop-bucket\bucket\axygen-shot.json`:

```json
{
  "version": "0.1.0",
  "description": "Portable Windows CLI screenshot tool for blind developers",
  "homepage": "https://github.com/ruslan-rv-ua/axygen-shot",
  "license": "MIT",
  "architecture": {
    "64bit": {
      "url": "https://github.com/ruslan-rv-ua/axygen-shot/releases/download/v0.1.0/axygen-shot-0.1.0-windows-x86_64.zip",
      "hash": "9c48d725712a2b7b1980181bcd83b0b9b17c87a294b6e8be1a56ce5f59d8b646"
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

- [ ] **Step 2: Validate JSON syntax**

Run: `cd C:\dev\scoop-bucket && python -c "import json; json.load(open('bucket/axygen-shot.json'))"`
Expected: no output (success)

- [ ] **Step 3: Commit manifest**

```
cd C:\dev\scoop-bucket
git add bucket/axygen-shot.json
git commit -m "Add axygen-shot manifest"
```

---

### Task 2: Update scoop-bucket README

**Files:**
- Modify: `C:\dev\scoop-bucket\README.md`

- [ ] **Step 1: Add Axygen Shot section**

Add before the `---` separator that precedes the `## Update all apps` section (after the QuickSnippets section), the following block:

```markdown
---

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

- [ ] **Step 2: Commit README update**

```
cd C:\dev\scoop-bucket
git add README.md
git commit -m "Add Axygen Shot to README"
```

---

### Task 3: Push scoop-bucket to GitHub

- [ ] **Step 1: Push to origin**

```
cd C:\dev\scoop-bucket
git push origin main
```

- [ ] **Step 2: Verify CI passes**

Check GitHub Actions for `scoop-bucket` — `ci.yml` should validate the new manifest.
The manifest uses the real v0.1.0 hash, so CI should pass.

---

## Chunk 2: axygen-shot changes

### Task 4: Create update-scoop workflow

**Files:**
- Create: `C:\dev\axygen-shot\.github\workflows\update-scoop.yml`

- [ ] **Step 1: Create workflow file**

Create `C:\dev\axygen-shot\.github\workflows\update-scoop.yml`:

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

- [ ] **Step 2: Validate YAML syntax**

Run: `cd C:\dev\axygen-shot && python -c "import yaml; yaml.safe_load(open('.github/workflows/update-scoop.yml'))"`
Expected: no output (success). If `yaml` module not available, use: `python -c "import json; print('YAML syntax check skipped')"` and verify manually.

- [ ] **Step 3: Commit workflow**

```
cd C:\dev\axygen-shot
git add .github/workflows/update-scoop.yml
git commit -m "ci: add Update Scoop workflow for manual manifest dispatch"
```

---

### Task 5: Update axygen-shot README.md

**Files:**
- Modify: `C:\dev\axygen-shot\README.md` (lines 21–25)

- [ ] **Step 1: Replace Installation section**

Replace the existing content at lines 21–25:

```
## Installation

Download `shot.exe` and `shot-watch.exe` from the
[latest release](https://github.com/ruslan-rv-ua/axygen-shot/releases/latest)
and place them in the same directory, anywhere on your `PATH`.
```

With:

```
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

The `Requirements:` line and `> **Why two files?**` block that follow remain unchanged.

- [ ] **Step 2: Commit**

```
cd C:\dev\axygen-shot
git add README.md
git commit -m "docs: add Scoop installation instructions to README"
```

---

### Task 6: Update axygen-shot README_UK.md

**Files:**
- Modify: `C:\dev\axygen-shot\README_UK.md` (lines 22–26)

- [ ] **Step 1: Replace Встановлення section**

Replace the existing content at lines 22–26:

```
## Встановлення

Завантажте `shot.exe` та `shot-watch.exe` з
[останнього релізу](https://github.com/ruslan-rv-ua/axygen-shot/releases/latest)
і помістіть обидва файли в одну теку, що є у вашому `PATH`.
```

With:

```
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

The `Вимоги:` line and `> **Чому два файли?**` block remain unchanged.

- [ ] **Step 2: Commit**

```
cd C:\dev\axygen-shot
git add README_UK.md
git commit -m "docs: add Scoop installation instructions to README_UK"
```

---

### Task 7: Push and verify

- [ ] **Step 1: Run axygen-shot CI locally**

```
cd C:\dev\axygen-shot
just ci
```

Expected: all tests pass, lint clean, format clean (README changes don't affect build).

- [ ] **Step 2: Push to remote**

```
cd C:\dev\axygen-shot
git push origin feature/scoop
```

---

## Post-implementation: Manual steps (not automated)

These steps require GitHub UI access and are documented here for the operator:

1. **Add `SCOOP_BUCKET_TOKEN` secret** to `axygen-shot` repository:
   - GitHub → `ruslan-rv-ua/axygen-shot` → Settings → Secrets → Actions → New repository secret
   - Name: `SCOOP_BUCKET_TOKEN`
   - Value: same PAT used for BrowserSelector/QuickSnippets/Marka

2. **Test the workflow** by running `update-scoop.yml` manually:
   - GitHub → `axygen-shot` → Actions → Update Scoop → Run workflow → version: `0.1.0`
   - Verify `scoop-bucket/bucket/axygen-shot.json` gets updated with correct hash

3. **Test Scoop installation** locally:
   ```powershell
   scoop bucket add ruslan-rv-ua https://github.com/ruslan-rv-ua/scoop-bucket
   scoop install axygen-shot
   shot --version
   shot --help
   ```
