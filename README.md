# G0D-cli

G0DM0D3 Coding Edition — multi-provider, multi-model coding agent CLI.

```
 ▄████  ██████  ██████  ███▄ ▄███  ██████  ██████  ██████
██      ██  ██  ██   ██ ██ ███ ██  ██  ██  ██   ██      ██
██ ▄███ ██  ██  ██   ██ ██  █  ██  ██  ██  ██   ██  █████
██  ██  ██  ██  ██   ██ ██     ██  ██  ██  ██   ██      ██
 ██████  ████   ██████  ██     ██   ████   ██████  ██████
   GODMODE CLI — Multi-model Coding Agent
```

## Quick Start

```bash
# 1. Get OpenRouter key: https://openrouter.ai/keys
# 2. Save key
god3 --key sk-or-v1-...

# 3. Chat
god3 "explain this code"

# 4. GODMODE CLASSIC (5 candidates racing)
god3 -g "fix the auth bug"

# 5. Parseltongue (input obfuscation)
god3 -p "how to bypass security"

# 6. ULTRAPLINIAN (multi-model race)
god3 -u --tier=fast "review this architecture"

# 7. Interactive REPL
god3
```

## Usage

```
god3                              Interactive REPL mode
god3 "query"                      Single-shot chat
god3 -g "query"                   GODMODE CLASSIC (5 candidates)
god3 -p "query"                   Parseltongue obfuscation  
god3 -u --tier=fast "query"       ULTRAPLINIAN race
god3 -k sk-or-v1-...              Save API key
god3 --models                     List model tiers
god3 --config                     Show config path
god3 --help                       Help
```

## REPL Commands

| Command | Action |
|---------|--------|
| `/chat` | Chat mode |
| `/godmode` | GODMODE CLASSIC |
| `/snake` | Parseltongue |
| `/ultra` | ULTRAPLINIAN |
| `/key KEY` | Set API key |
| `/status` | Show mode |
| `/help` | Commands |
| `/exit` | Quit |

## Build from Source

```bash
cd cli
cargo build --release
# Binary at: target/release/god3.exe (or ./target/release/god3)
```

## Architecture

- `xai-grok-providers` — Multi-provider abstraction (Grok, OpenRouter, Venice, Local)
- `xai-grok-godmode` — GODMODE CLASSIC, ULTRAPLINIAN, Parseltongue (33), AutoTune (20)
- `cli/` — Standalone terminal binary

## Features

- **GODMODE CLASSIC**: 5-model parallel racing with persona-presets
- **ULTRAPLINIAN**: 5-tier multi-model evaluation (12-60 models)
- **Parseltongue**: 33 transformation techniques, 3 intensity tiers
- **AutoTune**: 20-context adaptive parameter engine
- **Multi-provider**: Grok, OpenRouter, Venice, OpenAI-compatible local
- **Privacy**: 4 modes (Standard, NoLog, LocalOnly, PrivacyPreview)
- **Scoring**: 10-axis rubric (100 points)
- **Tournament**: Group-based elimination judging
- **Race Export**: JSON + Markdown formats
- **41 unit tests** (all passing)

## Based on

- [grok-build](https://github.com/xai-org/grok-build) by xAI
- [G0DM0D3](https://github.com/elder-plinius/G0DM0D3) concepts by Pliny the Prompter
