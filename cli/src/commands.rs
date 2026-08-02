use reedline::Completer;

#[derive(Clone)]
pub struct Command {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub desc: &'static str,
    pub desc_vi: &'static str,
    pub usage: &'static str,
    pub args: &'static [ArgHint],
}

#[derive(Clone)]
pub struct ArgHint {
    pub name: &'static str,
    pub desc: &'static str,
    pub completions: &'static [&'static str],
}

pub fn registry() -> Vec<Command> {
    vec![
        Command { name: "/key", aliases: &[], desc: "Set API key", desc_vi: "Nhập API key", usage: "/key <api-key>", args: &[ArgHint { name: "key", desc: "API key", completions: &[] }] },
        Command { name: "/model", aliases: &["/m"], desc: "Change model", desc_vi: "Đổi model", usage: "/model <provider:model/id>", args: &[ArgHint { name: "model", desc: "provider:model", completions: &["anthropic/claude-sonnet-4.6", "openai/gpt-5.6", "google/gemini-2.5-pro", "deepseek/deepseek-chat", "x-ai/grok-code-fast"] }] },
        Command { name: "/provider", aliases: &["/prov"], desc: "Manage providers", desc_vi: "Quản lý provider", usage: "/provider <key|add|default|rm>", args: &[ArgHint { name: "action", desc: "key|add|default|rm", completions: &["key", "add", "default", "rm"] }, ArgHint { name: "id", desc: "provider-id", completions: &["openrouter", "venice", "grok", "ollama", "lmstudio"] }] },
        Command { name: "/providers", aliases: &["/ps"], desc: "List providers", desc_vi: "Danh sách provider", usage: "/providers", args: &[] },
        Command { name: "/status", aliases: &["/st"], desc: "Show status", desc_vi: "Xem trạng thái", usage: "/status", args: &[] },
        Command { name: "/chat", aliases: &[], desc: "Chat mode", desc_vi: "Chế độ chat", usage: "/chat", args: &[] },
        Command { name: "/godmode", aliases: &["/g"], desc: "GODMODE CLASSIC — 5 candidates", desc_vi: "GODMODE — 5 ứng viên", usage: "/godmode [on|off|classic|profile|compare|export]", args: &[ArgHint { name: "mode", desc: "on|off|classic|profile", completions: &["on", "off", "classic", "profile", "compare", "export"] }] },
        Command { name: "/snake", aliases: &["/parseltongue"], desc: "Parseltongue obfuscation", desc_vi: "Parseltongue — làm rối văn bản", usage: "/snake", args: &[] },
        Command { name: "/ultra", aliases: &["/u"], desc: "ULTRAPLINIAN race", desc_vi: "ULTRAPLINIAN — đua model", usage: "/ultra <fast|standard|smart|power|ultra>", args: &[ArgHint { name: "tier", desc: "fast|standard|smart|power|ultra", completions: &["fast", "standard", "smart", "power", "ultra"] }] },
        Command { name: "/language", aliases: &["/lang"], desc: "Set language", desc_vi: "Đổi ngôn ngữ", usage: "/language <auto|vi|en>", args: &[ArgHint { name: "lang", desc: "auto|vi|en", completions: &["auto", "vi", "en"] }] },
        Command { name: "/autotune", aliases: &[], desc: "AutoTune settings", desc_vi: "AutoTune — tự động chọn tham số", usage: "/autotune [on|off|status]", args: &[ArgHint { name: "action", desc: "on|off|status", completions: &["on", "off", "status"] }] },
        Command { name: "/privacy", aliases: &[], desc: "Privacy mode", desc_vi: "Chế độ riêng tư", usage: "/privacy [standard|no-log|local-only|preview]", args: &[ArgHint { name: "mode", desc: "standard|no-log|local-only|preview", completions: &["standard", "no-log", "local-only", "preview"] }] },
        Command { name: "/help", aliases: &["/?", "/h"], desc: "Show help", desc_vi: "Xem trợ giúp", usage: "/help", args: &[] },
        Command { name: "/exit", aliases: &["/quit", "/q"], desc: "Exit", desc_vi: "Thoát", usage: "/exit", args: &[] },
    ]
}

pub struct SlashCompleter {
    commands: Vec<Command>,
}

impl SlashCompleter {
    pub fn new() -> Self { Self { commands: registry() } }

    pub fn find(&self, name: &str) -> Option<&Command> {
        self.commands.iter().find(|c| c.name == name || c.aliases.contains(&name))
    }
}

impl Completer for SlashCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<reedline::Suggestion> {
        let before_cursor = &line[..pos];
        if !before_cursor.starts_with('/') && !before_cursor.is_empty() && !before_cursor.starts_with(' ') {
            return vec![];
        }

        let input = before_cursor.trim();

        // Empty: show all commands
        if input.is_empty() || input == "/" {
            return self.commands.iter().map(|c| reedline::Suggestion {
                value: c.name.to_string(),
                description: Some(format!("{} — {}", c.desc, c.usage)),
                extra: None, span: reedline::Span { start: 0, end: pos },
                append_whitespace: true, style: None,
            }).collect();
        }

        // Partial command name
        if !input.contains(' ') {
            return self.commands.iter().filter(|c| {
                c.name.starts_with(input) || c.aliases.iter().any(|a| a.starts_with(input))
            }).map(|c| reedline::Suggestion {
                value: c.name.to_string(),
                description: Some(format!("{} — {}", c.desc, c.usage)),
                extra: None, span: reedline::Span { start: 0, end: pos },
                append_whitespace: true, style: None,
            }).collect();
        }

        // Command with args
        let parts: Vec<&str> = input.splitn(3, ' ').collect();
        if let Some(cmd) = self.find(parts[0]) {
            if let Some(arg) = cmd.args.first() {
                let partial = parts.get(1).unwrap_or(&"");
                return arg.completions.iter().filter(|c| c.starts_with(partial)).map(|c| reedline::Suggestion {
                    value: format!("{} {}", parts[0], c),
                    description: Some(arg.desc.to_string()),
                    extra: None, span: reedline::Span { start: 0, end: pos },
                    append_whitespace: true, style: None,
                }).collect();
            }
        }

        vec![]
    }
}
