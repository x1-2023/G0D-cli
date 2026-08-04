# G0D CLI — roadmap / plan

Living plan for post-v1.14 work. Priority can shift with user feedback.

## Done (recent)

- [x] Coding agent tools (files, git, shell, patch)
- [x] Sessions + resume + export
- [x] Grok-style TUI (header, chat, input, footer)
- [x] Slash menu on `/` + Tab completion
- [x] Shift+Tab → ask / always-approve
- [x] Message queue (Enter queue, Ctrl+Enter now)
- [x] Context meter + auto-compact + token tracking
- [x] Project instructions (`AGENTS.md` …)
- [x] Remote install script (`install-remote.ps1` + release publish)

## P0 — product polish

1. **Cancel mid-turn** — Esc / button kills agent worker (today queue only)
2. **True streaming** in agent loop (partial assistant text while tools pending)
3. **TUI GODMODE / Ultra race** — real multi-candidate race like CLI `-g` / `-u`, not just prompt tags
4. **Release CI** — GitHub Actions: `cargo test`, build `g0d-windows-x64.exe`, attach to tag
5. **Install script hardening** — checksum (SHA256 on release), optional arm64 later

## P1 — UX / parity with Grok shell

6. **Composer polish** — paste large text, better multi-line cursor, scrollback mouse
7. **Diff viewer** in TUI after edits (side panel or pager)
8. **Cost estimate** ($/turn) from provider pricing table
9. **Exact tokenizer** option (tiktoken / model-specific) vs char heuristic
10. **LLM-powered compact** (summarize via small model) when rule-based summary is weak
11. **Theme / density** — classic vs dense, NO_COLOR already supported

## P2 — agent power

12. **MCP client** — connect external tools (reuse `xai-grok-mcp` selectively)
13. **Web / docs fetch** tool (allowlisted hosts)
14. **Parallel read-only tools** in one step
15. **Test/runner presets** (`/test`, detect cargo/npm/pytest)
16. **Sandbox mode** (optional process isolation for `run_command`)

## P3 — distribution

17. **Linux + macOS** builds + install scripts (`install.sh`)
18. **winget / scoop** manifests
19. **Optional npm wrapper** (`npx @…/g0d` postinstall download)
20. **Signed releases** (Authenticode) for fewer SmartScreen blocks

## P4 — quality

21. **More E2E** with mock OpenAI (queue, approval modal, compact)
22. **Telemetry opt-in** (crash only) — off by default
23. **Docs site** — short Vietnamese + English getting-started

---

## Suggested next sprint (1–2 weeks)

| # | Item | Why |
| --- | --- | --- |
| 1 | GitHub Actions release pipeline | Makes `irm \| iex` reliable every tag |
| 2 | Cancel agent mid-turn | Closest Grok parity pain point |
| 3 | Streaming final answers | Feels much faster in TUI |
| 4 | SHA256 verify in install-remote | Trust for public install |

---

## Non-goals (for now)

- Full reimplementation of entire `grok-build` pager/shell monorepo inside `g0d`
- Cloud-hosted multi-user backend
- Replacing Cursor/VS Code as IDE
