# G0D CLI implementation logbook

This file is the durable handoff for future agents and sessions. Update it after every meaningful checkpoint. Do not record API keys or other secrets.

## Target

Turn `g0d` into an installable Codex/Claude-style coding agent:

- Running `g0d` from a terminal uses that terminal's current directory as the workspace.
- Agent tools can inspect/edit files, execute commands/tests, inspect and mutate Git through explicit tools, and apply unified patches.
- An approval mode can require `[y/N]` before every potentially mutating action.
- Sessions persist on disk and can be resumed in another terminal/session.
- A Windows installer makes `g0d` available through the user `PATH`.

## Safety invariants

- Workspace paths must remain below the canonical current working directory.
- Block absolute paths, `..` traversal, writes under `.git`, and writes to `.env*`.
- Bound agent steps, tool output, file reads, recursive listings, searches, and command runtime.
- Surface tool errors to the model as observations; never report an unverified success.
- Approval mode defaults to on. Headless writes/mutating commands are denied while approval is on.

## Checkpoints

### 2026-08-02 - Baseline v1.8.0

Status: completed before this expansion.

- Rust CLI builds and runs as `g0d 1.8.0`.
- Existing agent loop has five curated tools: `list_files`, `read_file`, `search_files`, `replace_in_file`, `create_file`.
- File writes require approval; path traversal and sensitive targets are blocked.
- ReAct loop is limited to 10 steps.
- Baseline verification: 14 unit tests passed; mock tool-calling E2E passed; headless file creation was denied.

### 2026-08-02 - Expansion started

Status: in progress.

Planned next:

1. Add configurable approval policy.
2. Add bounded command/test and Git tools plus unified patch application.
3. Persist and resume sessions.
4. Add Windows install/uninstall packaging and perform a real user-level installation.
5. Run safety/E2E tests, release build, smoke test from a separate project directory, and record exact results here.

### 2026-08-02 - Agent execution and resumable state implemented

Status: implementation compiles; full tests pending.

- Config now has persistent `approval_mode`, defaulting to `on`; CLI supports `--approval on|off` and REPL supports `/approval`.
- Agent tool set expanded to 10 curated tools. New tools: bounded `run_command`, read-only `git_status`, `git_diff`, `git_log`, and validated `apply_patch`.
- Commands run only below the canonical workspace, time out after at most 120 seconds, and return exit code/stdout/stderr as tool evidence.
- Unified patches are checked with `git apply --check` before approval/application and reject absolute/traversal, `.git`, and `.env*` targets.
- Sessions now persist as workspace-scoped JSON under the G0D config directory. Added `--resume[=ID]`, `/session`, `/sessions`, `/resume`, and durable `/new`.
- `cargo check` passes after this checkpoint. Full unit/E2E/release verification remains pending.

## Known limitations at start

- No command execution or build/test verification tool.
- No Git or unified-patch tool.
- Conversation state is memory-only.
- Root binary exists, but there is no durable global installer or user `PATH` setup.

### 2026-08-02 - v1.9.0 packaged, installed, and verified

Status: completed for the requested scope.

- Version bumped to 1.9.0 and optimized Windows release built successfully.
- Added idempotent `install.ps1` and `uninstall.ps1`. Installed binary at `C:\Users\0xQ\AppData\Local\Programs\g0d\g0d.exe` and added that directory to user `PATH`.
- New CMD smoke test from `E:\LastWar-Multibox` resolved `g0d` and printed `g0d 1.9.0`.
- Unit verification against the synced E: repo: 17 passed, 0 failed.
- Mock-provider E2E verified: agent executed a safe PowerShell command with approval off, captured exit code/stdout, saved a session, and resumed it in a second process.
- Approval safety E2E verified: headless approval-on mode denied `create_file`; the requested file did not exist afterward.
- README now documents installation, approval modes, tools, and resume workflow.

## Remaining limitations after v1.9.0

- Approval mode is a policy gate, not an OS/container sandbox. Keep it `on` for untrusted prompts; `off` intentionally permits shell commands with the current user's privileges.
- Git status/diff/log are dedicated read-only tools. Mutating Git actions (add/commit/checkout) use the general command tool and therefore always prompt while approval is on.
- Unified patch paths with Git's quoted filename syntax are rejected; ordinary UTF-8 paths without quoted headers work.
- Sessions store bounded conversation messages, not a full replay of every intermediate tool call.

### 2026-08-02 - CMD discovery compatibility

- A user reported that an already-open CMD did not see the newly appended install directory in PATH.
- Added `g0d.cmd` under the conventional `%APPDATA%\npm` command directory and updated install/uninstall scripts to manage it.
- This keeps the dedicated installed executable while making discovery work through a directory commonly present in existing Windows terminal PATH snapshots.

### 2026-08-02 - v1.9.1 mixed-shell loop fix

- Root cause from a real run: the model emitted cmd.exe operators such as `||` while `run_command` executed Windows PowerShell 5.1, causing repeated discovery failures and exhausting the 10-step bound.
- `run_command` now exposes an explicit `shell` choice (`powershell`, `cmd`, or `sh`) and rejects unsupported host/shell combinations.
- Windows agent guidance now forbids mixed syntax, recommends `Get-Command` for PowerShell discovery, and stops repeated probing after two failures.
- The bounded loop remains finite but is raised from 10 to 20 model/tool iterations for real build-and-test tasks.
- Windows held the active 1.9.0 executable open during upgrade. Installed `g0d-1.9.1.exe` side-by-side and switched the global command shim to it; verified the shim reports 1.9.1.

### 2026-08-02 - v1.9.2 resumable safety-budget checkpoint

- Real run showed a vague `tiếp tục công việc` in a new empty session triggering a broad workspace scan, then losing all progress when the 20-step bound was reached.
- Empty sessions are no longer saved at startup or by `/new`; `resume latest` ignores legacy empty sessions.
- A vague continuation in an empty session now asks for `/resume latest` or a concrete objective without calling tools.
- When the bounded loop is exhausted, G0D performs one no-tool finalization request, prints and persists a selective checkpoint, and returns normally instead of discarding context with an error.
- Verification: 20 unit tests passed and the optimized 1.9.2 release built successfully.
- Mock-provider E2E forced all 20 iterations, verified a no-tool checkpoint response was printed, process exited successfully, and the checkpoint was present in the saved session JSON.

### 2026-08-04 - v1.14.x ship: TUI queue, Shift+Tab approval, README + remote install

Status: shipping to github.com/x1-2023/G0D-cli

- TUI: message queue, Shift+Tab ask/always-approve, `/` slash menu, multi-line composer
- `install-remote.ps1` + `scripts/publish-release.ps1` for `irm | iex`
- README rewritten for install/update; ROADMAP.md for next work
- Binaries no longer intended in git (`g0d.exe` via Releases)

### 2026-08-04 - v1.13.0 Grok-style fullscreen TUI

Status: completed and verified.

UI (matches Grok Build shell chrome):

- Default interactive mode is a ratatui fullscreen TUI:
  - Header: `≡ branch path` · `TOKs / BUDGET | steps ✓`
  - Chat: user `›` lines, `◆` tool activity, assistant text, thinking timer
  - Rounded input with model · approval label
  - Footer key hints (Esc/Ctrl-C/Enter)
- Approval modal for mutating tools while the agent runs on a worker thread
- Agent output abstracted via `EventSink` (`ConsoleSink` + `ChannelSink`)
- `--classic` / `G0D_CLASSIC` restores the reedline REPL

Verification: 27 unit tests passed; release `g0d 1.13.0`.

### 2026-08-04 - v1.12.0 context meter, tokens, auto-compact

Status: completed and verified.

UI / context:

- Prompt right side shows live meter: `messages · ~est tokens · %`.
- `/context` and `/status` show a bar, estimated tokens vs budget, last + lifetime API usage, compact count.
- Spinner labels show `step i/n`.

Token tracking:

- Session stores last and lifetime prompt/completion tokens when the provider returns `usage`.
- Post-turn footer prints turn usage + context estimate.

Auto-compact:

- New `meter` module: token estimate heuristic, bar render, rule-based compaction (summary of older turns + keep recent).
- Config: `auto_compact`, `context_token_budget`, `keep_recent_messages`.
- Runs automatically before agent queries when message cap or ~85% token budget is hit.
- Manual `/compact` (force by default, `/compact auto` respects thresholds only).

Verification: 27 unit tests passed; release `g0d 1.12.0`.

### 2026-08-04 - v1.11.0 project memory + git + export

Status: completed and verified.

Improvements:

- Auto-load project instructions (`AGENTS.md` / `G0D.md` / `.g0d/*` / `CLAUDE.md`) into agent context; `/instructions` to inspect.
- New tools: `rename_file`, `git_add`, `git_commit` (no push/amend/skip-hooks; message via stdin `-F -`).
- Tool registry now 16 curated tools.
- Stop after 3 identical consecutive tool `ERROR:` observations and emit a checkpoint.
- `/export [path]` writes a Markdown session transcript; default path is workspace-safe.
- `/approval session on|off|clear` and `/steps [n|clear]` for process-local overrides; CLI `--steps N`.
- Banner/status show whether project instructions were loaded.

Verification:

- `cargo test --bins`: 24 passed, 0 failed.
- Release build reports `g0d 1.11.0`.

### 2026-08-04 - v1.10.0 coding-agent completeness pass

Status: completed and verified.

Improvements:

- Tool set expanded from 10 → 13: `glob_files`, `write_file` (create/overwrite), `delete_file`, plus `replace_all` on `replace_in_file`.
- Sessions gain optional titles from the first user turn; `/session` and `/sessions` display them.
- Crash-safe progress snapshots after each tool batch; final answer still replaces the provisional snapshot.
- Token usage summary when providers return `usage` fields.
- Configurable `max_agent_steps` (default 20, range 1–50) in config.toml.
- Root `build.ps1` for release/test/install from one entrypoint.
- Agent tool logs show short argument previews (`→ write_file (src/main.rs)`).
- System prompt updated for the fuller tool surface.

Verification:

- `cargo test --bins`: 21 passed, 0 failed.
- `build.ps1 -Release`: produced `cli\target\release\g0d.exe` and root `g0d.exe` reporting `g0d 1.10.0`.

Remaining limitations:

- Approval is still a policy gate, not an OS sandbox.
- Glob matcher is intentional and lightweight (`*`, `**`, `?`); not full gitignore semantics.
- Progress snapshots store summaries of tool observations, not full multi-turn tool transcripts.
- Streaming is still used only for non-agent chat/parseltongue paths; the agent loop is non-streamed tool-calling.
