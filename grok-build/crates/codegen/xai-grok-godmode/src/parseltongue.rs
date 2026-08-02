
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Intensity {
    Light,
    Standard,
    Heavy,
}

pub struct Parseltongue {
    pub debug_mode: bool,
    pub seed: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct TransformResult {
    pub original: String,
    pub transformed: String,
    pub triggers_found: Vec<String>,
    pub applied_transformations: Vec<String>,
}

impl Parseltongue {
    pub fn new() -> Self { Self { debug_mode: false, seed: None } }

    pub fn transform(&self, text: &str, intensity: Intensity, custom_triggers: &[String]) -> TransformResult {
        let triggers = self.detect_triggers(text, custom_triggers);
        if triggers.is_empty() {
            return TransformResult {
                original: text.to_string(),
                transformed: text.to_string(),
                triggers_found: vec![],
                applied_transformations: vec![],
            };
        }

        let technique_count = match intensity {
            Intensity::Light => 11,
            Intensity::Standard => 22,
            Intensity::Heavy => 33,
        };

        let mut result = text.to_string();
        let mut applied = Vec::new();
        let transformations: Vec<&dyn Fn(&str) -> String> = ALL_TRANSFORMATIONS.iter().take(technique_count).map(|f| f as &dyn Fn(&str) -> String).collect();

        for trigger in &triggers {
            for (i, transform) in transformations.iter().enumerate() {
                if result.contains(trigger.as_str()) {
                    let replacement = transform(trigger);
                    result = result.replace(trigger.as_str(), &replacement);
                    applied.push(TRANSFORM_NAMES[i].to_string());
                    break;
                }
            }
        }

        TransformResult {
            original: text.to_string(),
            transformed: result,
            triggers_found: triggers,
            applied_transformations: applied,
        }
    }

    fn detect_triggers(&self, text: &str, custom: &[String]) -> Vec<String> {
        let custom_refs: Vec<&str> = custom.iter().map(|s| s.as_str()).collect();
        let all_triggers: Vec<&str> = DEFAULT_TRIGGERS.iter().copied().chain(custom_refs.iter().copied()).collect();
        let lower = text.to_lowercase();
        let mut found: Vec<String> = Vec::new();
        for trigger in all_triggers {
            if lower.contains(&trigger.to_lowercase()) && !found.contains(&trigger.to_string()) {
                found.push(trigger.to_string());
            }
        }
        found.sort_by(|a, b| b.len().cmp(&a.len()));
        found
    }
}

const DEFAULT_TRIGGERS: &[&str] = &[
    "hack", "exploit", "bypass", "crack", "attack", "penetrate",
    "inject", "manipulate", "override", "disable", "circumvent",
    "malware", "virus", "trojan", "payload", "shellcode", "rootkit",
    "keylogger", "backdoor", "vulnerability",
    "weapon", "bomb", "explosive", "poison", "drug", "synthesize",
    "jailbreak", "unlock", "root", "sudo", "privilege",
    "phishing", "scam", "impersonate", "deceive", "fraud",
    "nsfw", "explicit", "uncensored", "unfiltered", "unrestricted",
    "ignore", "disregard", "forget", "pretend", "roleplay",
];

const TRANSFORM_NAMES: &[&str] = &[
   "basic-leetspeak", "extended-leetspeak", "unicode-homoglyph",
   "mixed-script", "fullwidth-unicode", "circled-characters",
   "small-cap-characters", "superscript", "subscript",
   "braille-encoding", "morse-encoding",
   "phonetic-substitution", "syllable-splitting", "word-splitting",
   "character-spacing", "zero-width-insertion", "soft-hyphen",
   "directional-mark", "reversed-words", "reversed-segments",
   "alternating-case", "randomized-case", "symbol-substitution",
   "emoji-substitution", "number-word", "rot-substitution",
   "base64-encoding", "hex-encoding", "binary-encoding",
   "url-encoding", "html-entity", "nested-encoding",
   "layered-mixed",
];

type TransformFn = fn(&str) -> String;

pub const ALL_TRANSFORMATIONS: [TransformFn; 33] = [
    t01_basic_leetspeak, t02_extended_leetspeak, t03_unicode_homoglyph,
    t04_mixed_script, t05_fullwidth_unicode, t06_circled_characters,
    t07_small_cap, t08_superscript, t09_subscript,
    t10_braille, t11_morse,
    t12_phonetic, t13_syllable_split, t14_word_split,
    t15_char_spacing, t16_zero_width, t17_soft_hyphen,
    t18_directional_mark, t19_reversed_words, t20_reversed_segments,
    t21_alternating_case, t22_randomized_case, t23_symbol_substitution,
    t24_emoji_sub, t25_number_word, t26_rot_sub,
    t27_base64, t28_hex, t29_binary,
    t30_url_encode, t31_html_entity, t32_nested,
    t33_layered,
];

fn t01_basic_leetspeak(s: &str) -> String { s.replace('a', "4").replace('e', "3").replace('i', "1").replace('o', "0") }
fn t02_extended_leetspeak(s: &str) -> String { s.replace('a', "@").replace('e', "3").replace('l', "1").replace('t', "7").replace('s', "$").replace('i', "!") }
fn t03_unicode_homoglyph(s: &str) -> String { s.replace('a', "а").replace('e', "е").replace('o', "о").replace('c', "с").replace('p', "р") }
fn t04_mixed_script(s: &str) -> String { s.chars().enumerate().map(|(i, c)| if i % 3 == 0 { char_to_cyrillic(c) } else { c }).collect() }
fn t05_fullwidth_unicode(s: &str) -> String { s.chars().map(|c| if c.is_ascii_alphabetic() { std::char::from_u32(c as u32 + 0xFF00 - 0x20).unwrap_or(c) } else { c }).collect() }
fn t06_circled_characters(s: &str) -> String { s.chars().map(|c| if c.is_ascii_alphabetic() { std::char::from_u32(if c.is_uppercase() { c as u32 + 0x24B6 - 'A' as u32 } else { c as u32 + 0x24D0 - 'a' as u32 }).unwrap_or(c) } else { c }).collect() }
fn t07_small_cap(s: &str) -> String { s.chars().map(|c| std::char::from_u32(c as u32 + 0x1D00 - 'A' as u32).unwrap_or(c)).collect() }
fn t08_superscript(s: &str) -> String { s.chars().map(|c| match c { '0'..='9' => std::char::from_u32(c as u32 + 0x2070 - '0' as u32).unwrap_or(c), _ => c }).collect() }
fn t09_subscript(s: &str) -> String { s.chars().map(|c| match c { '0'..='9' => std::char::from_u32(c as u32 + 0x2080 - '0' as u32).unwrap_or(c), _ => c }).collect() }
fn t10_braille(s: &str) -> String { format!("\u{2800}{}\u{2800}", s.chars().map(|c| braille_char(c)).collect::<String>()) }
fn t11_morse(s: &str) -> String { s.to_uppercase().chars().map(|c| morse_char(c)).collect::<Vec<_>>().join(" ") }
fn t12_phonetic(s: &str) -> String { s.replace("ph", "f").replace("ck", "k").replace("gh", "f") }
fn t13_syllable_split(s: &str) -> String { s.chars().collect::<Vec<_>>().chunks(2).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("-") }
fn t14_word_split(s: &str) -> String { s.split_whitespace().map(|w| w.chars().collect::<Vec<_>>().chunks(2).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("   ") }
fn t15_char_spacing(s: &str) -> String { s.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(" ") }
fn t16_zero_width(s: &str) -> String { s.chars().map(|c| format!("{}\u{200B}", c)).collect() }
fn t17_soft_hyphen(s: &str) -> String { s.chars().map(|c| format!("{}\u{00AD}", c)).collect() }
fn t18_directional_mark(s: &str) -> String { format!("\u{200E}{}\u{200F}", s) }
fn t19_reversed_words(s: &str) -> String { s.split_whitespace().map(|w| w.chars().rev().collect::<String>()).collect::<Vec<_>>().join(" ") }
fn t20_reversed_segments(s: &str) -> String { s.chars().collect::<Vec<_>>().chunks(3).map(|c| c.iter().rev().collect::<String>()).collect() }
fn t21_alternating_case(s: &str) -> String { s.chars().enumerate().map(|(i, c)| if i % 2 == 0 { c.to_uppercase().to_string() } else { c.to_lowercase().to_string() }).collect() }
fn t22_randomized_case(s: &str) -> String { s.chars().map(|c| if (c as u32) % 2 == 0 { c.to_uppercase().to_string() } else { c.to_lowercase().to_string() }).collect() }
fn t23_symbol_substitution(s: &str) -> String { s.replace('a', "@").replace('s', "$").replace('i', "!").replace('o', "0").replace('e', "3") }
fn t24_emoji_sub(s: &str) -> String { s.replace("hack", "💻").replace("virus", "🦠").replace("lock", "🔒").replace("key", "🔑") }
fn t25_number_word(s: &str) -> String { s.replace("to", "2").replace("for", "4").replace("ate", "8") }
fn t26_rot_sub(s: &str) -> String { s.chars().map(|c| if c.is_ascii_alphabetic() { let base = if c.is_uppercase() { 'A' } else { 'a' }; (((c as u8 - base as u8 + 13) % 26) + base as u8) as char } else { c }).collect() }
fn t27_base64(s: &str) -> String { use std::io::Write; let mut buf = Vec::new(); { let mut enc = base64_mini::Encoder::new(&mut buf); let _ = enc.write_all(s.as_bytes()); } String::from_utf8_lossy(&buf).to_string() }
fn t28_hex(s: &str) -> String { s.as_bytes().iter().map(|b| format!("{:02x}", b)).collect() }
fn t29_binary(s: &str) -> String { s.as_bytes().iter().map(|b| format!("{:08b}", b)).collect::<Vec<_>>().join(" ") }
fn t30_url_encode(s: &str) -> String { s.as_bytes().iter().map(|b| format!("%{:02X}", b)).collect() }
fn t31_html_entity(s: &str) -> String { s.chars().map(|c| format!("&#{};", c as u32)).collect() }
fn t32_nested(s: &str) -> String { t27_base64(&t28_hex(&t01_basic_leetspeak(s))) }
fn t33_layered(s: &str) -> String { t03_unicode_homoglyph(&t21_alternating_case(&t01_basic_leetspeak(s))) }

fn char_to_cyrillic(c: char) -> char {
    match c {
        'A' | 'a' => 'а', 'B' => 'в', 'C' | 'c' => 'с', 'E' | 'e' => 'е',
        'H' => 'н', 'K' => 'к', 'M' => 'м', 'O' | 'o' => 'о', 'P' | 'p' => 'р',
        'T' => 'т', 'X' | 'x' => 'х', 'Y' | 'y' => 'у', _ => c,
    }
}

fn braille_char(c: char) -> char {
    match c.to_ascii_lowercase() {
        'a' => '⠁', 'b' => '⠃', 'c' => '⠉', 'd' => '⠙', 'e' => '⠑',
        'f' => '⠋', 'g' => '⠛', 'h' => '⠓', 'i' => '⠊', 'j' => '⠚',
        'k' => '⠅', 'l' => '⠇', 'm' => '⠍', 'n' => '⠝', 'o' => '⠕',
        'p' => '⠏', 'q' => '⠟', 'r' => '⠗', 's' => '⠎', 't' => '⠞',
        'u' => '⠥', 'v' => '⠧', 'w' => '⠺', 'x' => '⠭', 'y' => '⠽', 'z' => '⠵',
        _ => '⠿',
    }
}

fn morse_char(c: char) -> String {
    match c {
        'A' => ".-", 'B' => "-...", 'C' => "-.-.", 'D' => "-..", 'E' => ".",
        'F' => "..-.", 'G' => "--.", 'H' => "....", 'I' => "..", 'J' => ".---",
        'K' => "-.-", 'L' => ".-..", 'M' => "--", 'N' => "-.", 'O' => "---",
        'P' => ".--.", 'Q' => "--.-", 'R' => ".-.", 'S' => "...", 'T' => "-",
        'U' => "..-", 'V' => "...-", 'W' => ".--", 'X' => "-..-", 'Y' => "-.--",
        'Z' => "--..", '0' => "-----", '1' => ".----", '2' => "..---",
        '3' => "...--", '4' => "....-", '5' => ".....", '6' => "-....",
        '7' => "--...", '8' => "---..", '9' => "----.", ' ' => "/",
        _ => "",
    }.to_string()
}

mod base64_mini {
    use std::io;
    pub struct Encoder<W: io::Write> { w: W }
    impl<W: io::Write> Encoder<W> {
        pub fn new(w: W) -> Self { Self { w } }
    }
    impl<W: io::Write> io::Write for Encoder<W> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            for chunk in buf.chunks(3) {
                let b0 = chunk[0];
                let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
                let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };
                self.w.write_all(&[CHARS[(b0 >> 2) as usize]])?;
                self.w.write_all(&[CHARS[(((b0 & 3) << 4) | (b1 >> 4)) as usize]])?;
                if chunk.len() > 1 {
                    self.w.write_all(&[CHARS[(((b1 & 15) << 2) | (b2 >> 6)) as usize]])?;
                } else { self.w.write_all(b"=")?; }
                if chunk.len() > 2 {
                    self.w.write_all(&[CHARS[(b2 & 63) as usize]])?;
                } else { self.w.write_all(b"=")?; }
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> { self.w.flush() }
    }
}
