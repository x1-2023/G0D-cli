# G0D CLI

`g0d` is a Rust-based, multi-provider coding agent with a Codex-style interactive terminal experience.

## Highlights

- Agentic chat with an OpenAI-compatible function-calling loop
- Workspace-scoped file inspection/editing, bounded command/test execution, Git inspection, and unified patch tools
- Approval mode defaults to `on`: every command or write asks `[y/N]`; `--approval off` enables automatic execution
- Path traversal, absolute paths, `.git`, and `.env` writes are blocked
- Persistent input history and `Ctrl-R` search
- Slash-command and argument completion with `Tab`
- Command aliases and typo suggestions
- Workspace-scoped sessions persist across terminals; use `--resume`, `/resume`, or `/new`
- Project context (working directory, project type, Git branch/status)
- Provider/model configuration with OpenRouter, Venice, xAI, Ollama, and LM Studio defaults
- Animated request status in a TTY and clean status messages in headless mode
- GODMODE, Parseltongue, and ULTRAPLINIAN modes from the existing `grok-build` crates
- TTY-safe colors plus `NO_COLOR`, `--no-color`, and `--headless`

## Build

```powershell
cd E:\LastWar-Multibox\G0D-cli\cli
cargo build --release
```

Local end-to-end smoke test (no real API key): start `python .\tests\mock_openai.py`, then run the release binary with `--provider lmstudio --model mock "ping"` from another terminal.

The binary is written to `cli\target\release\g0d.exe`.

## Install on Windows

```powershell
cd E:\LastWar-Multibox\G0D-cli
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

Open a new terminal after installation. From any project directory, run `g0d`. To remove the user-level installation, run `.\uninstall.ps1`.

## Configure

Environment variables are recommended so secrets do not need to be stored in the config file:

```powershell
$env:OPENROUTER_API_KEY = "sk-or-v1-..."
g0d --provider openrouter --model anthropic/claude-sonnet-4
```

To persist a key for the current provider:

```powershell
g0d --key sk-or-v1-...
```

Use `g0d --config` to print the config path. Local OpenAI-compatible endpoints are available as `ollama` (`127.0.0.1:11434/v1`) and `lmstudio` (`127.0.0.1:1234/v1`).

For portable installs and CI, set `G0D_CONFIG_DIR` to override both the config and history directory.

## Usage

```text
g0d [OPTIONS] [QUERY]

g0d                              Interactive REPL
g0d "inspect and fix this bug"   One-shot coding-agent task
g0d -g "review this design"      GODMODE candidate race
g0d -p "rewrite this prompt"     Parseltongue mode
g0d -u --tier fast "compare"     ULTRAPLINIAN race
g0d --models                     List model tiers
g0d --approval on               Ask before every command/write (default)
g0d --approval off              Allow automatic command/write execution
g0d --resume                    Resume latest session for this directory
g0d --resume=<session-id>       Resume a specific session
g0d --help                       Full CLI help
```

Piped input is supported:

```powershell
Get-Content .\error.log | g0d --headless
```

## Interactive commands

| Command | Purpose |
| --- | --- |
| `/help` | Show commands and keybindings |
| `/status` | Show mode, provider, model, key source, config, and history |
| `/key <key>` | Save a key for the active provider |
| `/model [id]` | Show or change the model |
| `/provider <action>` | List/add/remove/select providers or set a provider key |
| `/config [show\|path]` | Show sanitized config or its path |
| `/context` | Show the project context sent to the model |
| `/language <auto\|vi\|en>` | Select response language |
| `/chat`, `/godmode`, `/snake`, `/ultra [tier]` | Change mode |
| `/approval [on\|off]` | Show or change command/write approval |
| `/session`, `/sessions`, `/resume [id]` | Inspect and resume workspace sessions |
| `/new` | Start and persist a fresh session |
| `/history` | Print persistent input history |
| `/clear` | Clear the terminal |
| `/exit` | Exit |

Aliases such as `/m`, `/g`, `/u`, `/ps`, `/cfg`, and `/quit` are supported.

## Keybindings

- `Tab`: open/advance completion
- `Shift-Tab`: previous completion
- `Ctrl-R`: searchable history menu
- `Up` / `Down`: navigate history
- `Ctrl-L`: clear screen
- `Alt-Enter`: insert a newline
- `Ctrl-C`: cancel the current input
- `Ctrl-D`: exit

Slash commands are intentionally excluded from the history file so keys entered with `/key` are not persisted there.

## Coding-agent behavior

Normal chat mode is agentic. The model can inspect the current working directory, edit files, apply unified patches, inspect Git, and execute commands/tests. Read-only tools run automatically. With approval mode on, commands and writes ask `[y/N]`; a non-interactive/headless process denies them. The loop stops after 10 model/tool steps, command runtime is capped at 120 seconds, and tool output is bounded.

Run `g0d` from the repository you want it to work on:

```powershell
cd E:\path\to\your-project
g0d "inspect the auth flow, fix the validation, run tests, and show the diff"
```
