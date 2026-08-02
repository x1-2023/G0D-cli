use std::io::IsTerminal;

pub enum TerminalMode { FullTui, EnhancedLine, Plain, Headless }
pub struct TerminalState { pub mode: TerminalMode, pub colors: bool, pub unicode: bool, pub is_tty: bool }

impl TerminalState {
    pub fn detect(headless: bool, _explicit_ui: Option<&str>) -> Self {
        let is_tty = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
        if headless || !is_tty { return Self { mode: TerminalMode::EnhancedLine, colors: false, unicode: true, is_tty }; }
        let no_color = std::env::var("NO_COLOR").is_ok() || std::env::var("TERM").as_deref() == Ok("dumb");
        #[cfg(windows)]
        let ansi_ok = win_ansi();
        #[cfg(not(windows))]
        let ansi_ok = true;
        Self { mode: TerminalMode::EnhancedLine, colors: !no_color && ansi_ok, unicode: true, is_tty }
    }

    pub fn s(&self, text: &str, color: &str) -> String {
        if !self.colors { return text.to_string(); }
        let code = match color { "cyan"=>36,"green"=>32,"yellow"=>33,"magenta"=>35,"dim"=>90,"red"=>31,"bold"=>1, _=>0 };
        if code == 0 { text.to_string() } else { format!("\x1b[{}m{}\x1b[0m", code, text) }
    }
    pub fn c(&self, t: &str) -> String { self.s(t, "cyan") }
    pub fn g(&self, t: &str) -> String { self.s(t, "green") }
    pub fn y(&self, t: &str) -> String { self.s(t, "yellow") }
    pub fn d(&self, t: &str) -> String { self.s(t, "dim") }
    pub fn b(&self, t: &str) -> String { self.s(t, "bold") }
}

#[cfg(windows)]
fn win_ansi() -> bool {
    std::env::var("WT_SESSION").is_ok() || std::env::var("TERM_PROGRAM").is_ok() || std::env::var("ConEmuANSI").is_ok() || std::env::var("ANSICON").is_ok()
}
