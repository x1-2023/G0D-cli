# G0D CLI

**g0d** — multi-provider coding agent for Windows, with a **Grok-style fullscreen TUI**.

Repo: [github.com/x1-2023/G0D-cli](https://github.com/x1-2023/G0D-cli)

| | |
| --- | --- |
| Version | **1.14.1** |
| Platforms | Windows x64 (primary) |
| License | See repo root |

---

## Install / update (one line)

```powershell
irm https://raw.githubusercontent.com/x1-2023/G0D-cli/main/install-remote.ps1 | iex
```

This downloads the **latest GitHub Release** asset `g0d-windows-x64.exe`, installs to:

```text
%LOCALAPPDATA%\Programs\g0d\g0d.exe
```

and adds a PATH shim (including `%APPDATA%\npm\g0d.cmd` for CMD discovery).

Open a **new terminal**, then:

```powershell
g0d --version
g0d
```

### Pin a version

```powershell
$script = Join-Path $env:TEMP 'g0d-install-remote.ps1'
Invoke-WebRequest https://raw.githubusercontent.com/x1-2023/G0D-cli/main/install-remote.ps1 -OutFile $script
powershell -ExecutionPolicy Bypass -File $script -Tag v1.14.1
```

### Uninstall binary only

```powershell
irm https://raw.githubusercontent.com/x1-2023/G0D-cli/main/uninstall.ps1 | iex
```

Or from a clone: `.\uninstall.ps1`

### What survives an upgrade?

| Data | Path | Upgrade / reinstall |
| --- | --- | --- |
| Binary | `%LOCALAPPDATA%\Programs\g0d\` | Replaced |
| Config + keys | `%APPDATA%\g0d\config.toml` | **Kept** |
| Sessions | `%APPDATA%\g0d\sessions\` | **Kept** |
| History | `%LOCALAPPDATA%\g0d\history.txt` | **Kept** |

Install/uninstall **never** delete sessions or config. Use `/resume latest` after updating.

---

## Quick start

```powershell
# API key (recommended: env var, not file)
$env:OPENROUTER_API_KEY = "sk-or-v1-..."

# Or persist for the active provider
g0d --key sk-or-v1-...
g0d --provider openrouter --model anthropic/claude-sonnet-4

# Interactive TUI (default)
cd E:\path\to\your-project
g0d

# One-shot agent
g0d "inspect the auth flow, fix validation, run tests, show the diff"
```

Local models: `ollama` (`127.0.0.1:11434/v1`), `lmstudio` (`127.0.0.1:1234/v1`).

---

## Features

- **Grok-style TUI** — header token meter, chat log, rounded input, footer shortcuts
- **Coding agent** — list/glob/search/read, replace/create/write/delete/rename, patch, shell, git
- **Approval** — `ask` vs `always-approve` (**Shift+Tab** toggles; `/approval` persists)
- **Slash menu** — type `/` for command suggestions (Tab / ↑↓ / Enter)
- **Message queue** — type while agent runs; **Enter** queues, **Ctrl+Enter** priority
- **Sessions** — workspace-scoped, titled, crash-safe snapshots, `/resume`, `/export`
- **Context** — auto-compact, `/context` meter, token usage when API reports it
- **Project memory** — `AGENTS.md` / `G0D.md` / `.g0d/instructions.md`
- **Providers** — OpenRouter, Venice, xAI/Grok, Ollama, LM Studio (+ custom)
- **Classic REPL** — `g0d --classic` (reedline + Tab complete)

---

## TUI keybindings

| Key | Action |
| --- | --- |
| `/` | Open slash command menu |
| **Tab** / **↑↓** | Browse suggestions |
| **Enter** | Pick suggestion · send prompt · **queue** if agent busy |
| **Ctrl+Enter** | Send now / queue as **next** |
| **Alt+Enter** | Newline in composer |
| **Shift+Tab** | Toggle **ask** ↔ **always-approve** |
| **Esc** | Close menu · clear input · clear queue |
| **Ctrl+C** | Quit |

---

## Slash commands

| Command | Purpose |
| --- | --- |
| `/help` | Commands + keys |
| `/status` | Provider, model, approval, context |
| `/key <api-key>` | Save key for active provider |
| `/model [id]` | Show / set model |
| `/provider …` | `list` · `default` · `add` · `remove` · `key` |
| `/providers` | List providers |
| `/config [show\|path]` | Config summary or path |
| `/context` | Context meter + project context |
| `/compact [force\|auto]` | Compact older turns |
| `/instructions` | Show loaded project instructions |
| `/language <auto\|vi\|en>` | Reply language |
| `/approval [on\|off\|session …]` | Approval policy |
| `/steps [n\|clear]` | Agent step budget |
| `/session` `/sessions` `/resume` | Session management |
| `/export [path.md]` | Export transcript |
| `/chat` `/godmode` `/snake` `/ultra` | Agent mode labels |
| `/new` `/clear` `/exit` | Session / UI |

---

## CLI usage

```text
g0d [OPTIONS] [QUERY]

g0d                         Grok-style TUI
g0d --classic               Classic reedline REPL
g0d "fix the bug"          One-shot agent
g0d -g "review design"      GODMODE (CLI race)
g0d -p "rewrite prompt"     Parseltongue
g0d -u --tier fast "…"      ULTRAPLINIAN
g0d --approval off          Persist auto-approve
g0d --steps 30              Step budget (1–50)
g0d --resume                Resume latest session
g0d --provider openrouter --model …
g0d --help
```

Piped / headless:

```powershell
Get-Content .\error.log | g0d --headless
```

---

## Config (`%APPDATA%\g0d\config.toml`)

| Key | Default | Purpose |
| --- | --- | --- |
| `approval_mode` | `on` | `on` = ask, `off` = always-approve |
| `max_agent_steps` | `20` | Model/tool turns per query (1–50) |
| `max_context_messages` | `20` | Session message window |
| `context_token_budget` | `80000` | Soft history token budget |
| `auto_compact` | `true` | Compact when thresholds trip |
| `keep_recent_messages` | `8` | Kept after compact |

Override config root: `$env:G0D_CONFIG_DIR = "D:\portable\g0d-config"`.

Project instructions (first match): `AGENTS.md`, `G0D.md`, `.g0d/instructions.md`, `.g0d/AGENTS.md`, `CLAUDE.md`.

---

## Build from source

**Requirements:** Rust (stable), Windows.

```powershell
git clone https://github.com/x1-2023/G0D-cli.git
cd G0D-cli
powershell -ExecutionPolicy Bypass -File .\build.ps1 -Release -Test
powershell -ExecutionPolicy Bypass -File .\install.ps1 -SkipBuild
```

Binary: `cli\target\release\g0d.exe`.

### Maintainers — publish a release (feeds `irm | iex`)

```powershell
# needs: gh auth login, cargo
powershell -ExecutionPolicy Bypass -File .\scripts\publish-release.ps1 -Publish
```

Creates GitHub Release with asset **`g0d-windows-x64.exe`**. Users then run the install one-liner above.

---

## Safety

- Workspace-scoped paths only (no `..`, no absolute paths)
- Blocks writes under `.git` and to `.env*`
- Bounded steps, tool output, command timeout (120s)
- Approval gate is **policy**, not an OS sandbox — keep **ask** for untrusted prompts

---

## Roadmap

See [ROADMAP.md](./ROADMAP.md) for planned work (streaming agent, cancel mid-turn, full GODMODE race in TUI, Linux/macOS, etc.).

---

## License / notice

Includes vendored `grok-build` crates used for GODMODE-related features. See `LICENSE`, `SECURITY.md`, and `grok-build/` notices.
