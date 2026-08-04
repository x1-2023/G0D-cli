use reedline::{Completer, Span, Suggestion};

#[derive(Clone, Debug)]
pub struct Command {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub desc: &'static str,
    pub usage: &'static str,
    pub args: &'static [ArgHint],
}

#[derive(Clone, Debug)]
pub struct ArgHint {
    pub desc: &'static str,
    pub completions: &'static [&'static str],
}

pub fn registry() -> Vec<Command> {
    vec![
        Command {
            name: "/help",
            aliases: &["/?", "/h"],
            desc: "Show commands and keybindings",
            usage: "/help",
            args: &[],
        },
        Command {
            name: "/status",
            aliases: &["/st"],
            desc: "Show session, provider, model, and config",
            usage: "/status",
            args: &[],
        },
        Command {
            name: "/key",
            aliases: &[],
            desc: "Save a key for the active provider",
            usage: "/key <api-key>",
            args: &[ArgHint {
                desc: "API key",
                completions: &[],
            }],
        },
        Command {
            name: "/model",
            aliases: &["/m"],
            desc: "Show or change the active model",
            usage: "/model [model-id]",
            args: &[ArgHint {
                desc: "model",
                completions: &[
                    "anthropic/claude-sonnet-4",
                    "openai/gpt-4.1",
                    "google/gemini-2.5-pro",
                    "deepseek/deepseek-chat",
                    "x-ai/grok-code-fast-1",
                ],
            }],
        },
        Command {
            name: "/provider",
            aliases: &["/prov"],
            desc: "Manage API providers",
            usage: "/provider <list|default|add|remove|key> ...",
            args: &[
                ArgHint {
                    desc: "action",
                    completions: &["list", "default", "add", "remove", "key"],
                },
                ArgHint {
                    desc: "provider",
                    completions: &["openrouter", "venice", "grok", "ollama", "lmstudio"],
                },
            ],
        },
        Command {
            name: "/providers",
            aliases: &["/ps"],
            desc: "List configured providers",
            usage: "/providers",
            args: &[],
        },
        Command {
            name: "/config",
            aliases: &["/cfg"],
            desc: "Show sanitized configuration or its path",
            usage: "/config [show|path]",
            args: &[ArgHint {
                desc: "action",
                completions: &["show", "path"],
            }],
        },
        Command {
            name: "/context",
            aliases: &["/ctx"],
            desc: "Show the project context sent to the model",
            usage: "/context",
            args: &[],
        },
        Command {
            name: "/language",
            aliases: &["/lang"],
            desc: "Set response language",
            usage: "/language <auto|vi|en>",
            args: &[ArgHint {
                desc: "language",
                completions: &["auto", "vi", "en"],
            }],
        },
        Command {
            name: "/chat",
            aliases: &[],
            desc: "Use normal chat mode",
            usage: "/chat",
            args: &[],
        },
        Command {
            name: "/godmode",
            aliases: &["/g"],
            desc: "Race five configured candidates",
            usage: "/godmode",
            args: &[],
        },
        Command {
            name: "/snake",
            aliases: &["/parseltongue"],
            desc: "Use Parseltongue mode",
            usage: "/snake",
            args: &[],
        },
        Command {
            name: "/ultra",
            aliases: &["/u"],
            desc: "Use ULTRAPLINIAN mode",
            usage: "/ultra [fast|standard|smart|power|ultra]",
            args: &[ArgHint {
                desc: "tier",
                completions: &["fast", "standard", "smart", "power", "ultra"],
            }],
        },
        Command {
            name: "/new",
            aliases: &[],
            desc: "Start a fresh conversation",
            usage: "/new",
            args: &[],
        },
        Command {
            name: "/approval",
            aliases: &["/approve"],
            desc: "Ask before commands and writes",
            usage: "/approval [on|off]",
            args: &[ArgHint {
                desc: "approval mode",
                completions: &["on", "off"],
            }],
        },
        Command {
            name: "/session",
            aliases: &[],
            desc: "Show the current resumable session",
            usage: "/session",
            args: &[],
        },
        Command {
            name: "/sessions",
            aliases: &[],
            desc: "List sessions for this workspace",
            usage: "/sessions",
            args: &[],
        },
        Command {
            name: "/resume",
            aliases: &[],
            desc: "Resume a saved session in this workspace",
            usage: "/resume <latest|session-id>",
            args: &[ArgHint {
                desc: "session",
                completions: &["latest"],
            }],
        },
        Command {
            name: "/history",
            aliases: &["/hist"],
            desc: "Print input history",
            usage: "/history",
            args: &[],
        },
        Command {
            name: "/clear",
            aliases: &["/cls"],
            desc: "Clear the terminal",
            usage: "/clear",
            args: &[],
        },
        Command {
            name: "/exit",
            aliases: &["/quit", "/q"],
            desc: "Exit g0d",
            usage: "/exit",
            args: &[],
        },
    ]
}

pub fn canonical_name(input: &str) -> Option<&'static str> {
    registry()
        .into_iter()
        .find(|command| command.name == input || command.aliases.contains(&input))
        .map(|command| command.name)
}

pub fn suggestions(input: &str, limit: usize) -> Vec<&'static str> {
    let mut candidates: Vec<(&str, usize)> = registry()
        .into_iter()
        .map(|command| {
            let distance = std::iter::once(command.name)
                .chain(command.aliases.iter().copied())
                .map(|name| levenshtein(input, name))
                .min()
                .unwrap_or(usize::MAX);
            (command.name, distance)
        })
        .filter(|(_, distance)| *distance <= 3)
        .collect();
    candidates.sort_by_key(|(_, distance)| *distance);
    candidates
        .into_iter()
        .take(limit)
        .map(|(name, _)| name)
        .collect()
}

fn levenshtein(left: &str, right: &str) -> usize {
    let right_chars: Vec<char> = right.chars().collect();
    let mut costs: Vec<usize> = (0..=right_chars.len()).collect();
    for (i, left_char) in left.chars().enumerate() {
        let mut diagonal = i;
        costs[0] = i + 1;
        for (j, right_char) in right_chars.iter().enumerate() {
            let above = costs[j + 1];
            costs[j + 1] = if left_char == *right_char {
                diagonal
            } else {
                1 + diagonal.min(above).min(costs[j])
            };
            diagonal = above;
        }
    }
    costs[right_chars.len()]
}

pub struct SlashCompleter {
    commands: Vec<Command>,
}

impl SlashCompleter {
    pub fn new() -> Self {
        Self {
            commands: registry(),
        }
    }

    fn find(&self, name: &str) -> Option<&Command> {
        self.commands
            .iter()
            .find(|command| command.name == name || command.aliases.contains(&name))
    }
}

impl Completer for SlashCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        if pos > line.len() || !line.is_char_boundary(pos) {
            return vec![];
        }
        let before_cursor = &line[..pos];
        if !before_cursor.starts_with('/') {
            return vec![];
        }

        let token_start = before_cursor.rfind(' ').map_or(0, |index| index + 1);
        let current_token = &before_cursor[token_start..];
        let parts: Vec<&str> = before_cursor.split_whitespace().collect();

        if parts.len() <= 1 && !before_cursor.ends_with(' ') {
            return self
                .commands
                .iter()
                .filter(|command| {
                    command.name.starts_with(current_token)
                        || command
                            .aliases
                            .iter()
                            .any(|alias| alias.starts_with(current_token))
                })
                .map(|command| suggestion(command.name, command.desc, 0, pos, true))
                .collect();
        }

        let Some(command_name) = parts.first() else {
            return vec![];
        };
        let Some(command) = self.find(command_name) else {
            return vec![];
        };
        let arg_index = if before_cursor.ends_with(' ') {
            parts.len() - 1
        } else {
            parts.len() - 2
        };
        let Some(arg) = command.args.get(arg_index) else {
            return vec![];
        };

        arg.completions
            .iter()
            .filter(|value| value.starts_with(current_token))
            .map(|value| suggestion(value, arg.desc, token_start, pos, true))
            .collect()
    }
}

fn suggestion(
    value: &str,
    description: &str,
    start: usize,
    end: usize,
    append_whitespace: bool,
) -> Suggestion {
    Suggestion {
        value: value.to_string(),
        description: Some(description.to_string()),
        extra: None,
        span: Span { start, end },
        append_whitespace,
        style: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_aliases() {
        assert_eq!(canonical_name("/m"), Some("/model"));
        assert_eq!(canonical_name("/quit"), Some("/exit"));
    }

    #[test]
    fn suggests_nearby_commands() {
        assert_eq!(suggestions("/modle", 3).first(), Some(&"/model"));
    }

    #[test]
    fn completes_command_arguments_without_replacing_the_command() {
        let mut completer = SlashCompleter::new();
        let suggestions = completer.complete("/language v", 11);
        assert_eq!(suggestions[0].value, "vi");
        assert_eq!(suggestions[0].span, Span { start: 10, end: 11 });
    }
}
