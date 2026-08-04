use std::io::IsTerminal;

#[derive(Clone, Copy, Debug)]
pub struct TerminalState {
    pub colors: bool,
    pub is_tty: bool,
}

impl TerminalState {
    pub fn detect(headless: bool, no_color_flag: bool) -> Self {
        let is_tty = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
        let no_color = no_color_flag
            || std::env::var_os("NO_COLOR").is_some()
            || std::env::var("TERM").as_deref() == Ok("dumb");

        #[cfg(windows)]
        let ansi_ok = windows_ansi();
        #[cfg(not(windows))]
        let ansi_ok = true;

        Self {
            colors: !headless && is_tty && !no_color && ansi_ok,
            is_tty,
        }
    }

    pub fn style(&self, text: &str, color: &str) -> String {
        if !self.colors {
            return text.to_string();
        }
        let code = match color {
            "cyan" => 36,
            "green" => 32,
            "yellow" => 33,
            "dim" => 90,
            "red" => 31,
            "bold" => 1,
            _ => 0,
        };
        if code == 0 {
            text.to_string()
        } else {
            format!("\x1b[{code}m{text}\x1b[0m")
        }
    }

    pub fn cyan(&self, text: &str) -> String {
        self.style(text, "cyan")
    }
    pub fn green(&self, text: &str) -> String {
        self.style(text, "green")
    }
    pub fn yellow(&self, text: &str) -> String {
        self.style(text, "yellow")
    }
    pub fn dim(&self, text: &str) -> String {
        self.style(text, "dim")
    }
    pub fn red(&self, text: &str) -> String {
        self.style(text, "red")
    }
    pub fn bold(&self, text: &str) -> String {
        self.style(text, "bold")
    }
}

#[cfg(windows)]
fn windows_ansi() -> bool {
    std::env::var_os("WT_SESSION").is_some()
        || std::env::var_os("TERM_PROGRAM").is_some()
        || std::env::var_os("ConEmuANSI").is_some()
        || std::env::var_os("ANSICON").is_some()
}
