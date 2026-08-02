use std::io::IsTerminal;

pub enum TerminalMode { FullTui, EnhancedLine, Plain, Headless }

pub struct TerminalState {
    pub mode: TerminalMode,
    pub colors: bool,
    pub unicode: bool,
    pub is_tty: bool,
}

impl TerminalState {
    pub fn detect(headless: bool, explicit_ui: Option<&str>) -> Self {
        let is_tty = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
        if headless || !is_tty {
            return Self { mode: TerminalMode::Headless, colors: false, unicode: true, is_tty };
        }
        let no_color = std::env::var("NO_COLOR").is_ok() || std::env::var("TERM").as_deref() == Ok("dumb");
        let mode = match explicit_ui {
            Some("plain") | Some("line") => TerminalMode::EnhancedLine,
            _ => TerminalMode::EnhancedLine,
        };
        Self { mode, colors: !no_color, unicode: true, is_tty }
    }

    pub fn style(&self, color: Option<&str>, text: &str) -> String {
        if !self.colors { return text.to_string(); }
        match color {
            Some("cyan") => format!("\x1b[36m{}\x1b[0m", text),
            Some("green") => format!("\x1b[32m{}\x1b[0m", text),
            Some("yellow") => format!("\x1b[33m{}\x1b[0m", text),
            Some("magenta") => format!("\x1b[35m{}\x1b[0m", text),
            Some("dim") => format!("\x1b[90m{}\x1b[0m", text),
            Some("red") => format!("\x1b[31m{}\x1b[0m", text),
            Some("bold") => format!("\x1b[1m{}\x1b[0m", text),
            _ => text.to_string(),
        }
    }

    pub fn c(&self, text: &str) -> String { self.style(Some("cyan"), text) }
    pub fn g(&self, text: &str) -> String { self.style(Some("green"), text) }
    pub fn y(&self, text: &str) -> String { self.style(Some("yellow"), text) }
    pub fn m(&self, text: &str) -> String { self.style(Some("magenta"), text) }
    pub fn d(&self, text: &str) -> String { self.style(Some("dim"), text) }
    pub fn r(&self, text: &str) -> String { self.style(Some("red"), text) }
    pub fn b(&self, text: &str) -> String { self.style(Some("bold"), text) }

    pub fn dim_line(&self) -> String {
        if self.colors { "\x1b[90m──────────────────────────────────────────────────\x1b[0m".into() }
        else { "--------------------------------------------------".into() }
    }

    pub fn render(&self, colored: &str) -> String {
        if !self.colors { return strip_ansi(colored); }
        colored.to_string()
    }
}

fn strip_ansi(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '\x1b' && chars[i+1] == '[' {
            i += 2;
            while i < chars.len() && !(chars[i] >= '@' && chars[i] <= '~') { i += 1; }
            if i < chars.len() { i += 1; }
        } else { result.push(chars[i]); i += 1; }
    }
    result
}

impl Default for TerminalState {
    fn default() -> Self { Self::detect(false, None) }
}
