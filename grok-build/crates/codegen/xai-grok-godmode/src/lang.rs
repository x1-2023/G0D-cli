use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponseLanguage {
    Auto,
    English,
    Vietnamese,
}

impl ResponseLanguage {
    pub fn label(&self) -> &str {
        match self {
            ResponseLanguage::Auto => "auto",
            ResponseLanguage::English => "en",
            ResponseLanguage::Vietnamese => "vi",
        }
    }
}

impl Default for ResponseLanguage {
    fn default() -> Self { Self::Auto }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SupportedLanguage {
    English,
    Vietnamese,
}

impl SupportedLanguage {
    pub fn label(&self) -> &str {
        match self {
            SupportedLanguage::English => "en",
            SupportedLanguage::Vietnamese => "vi",
        }
    }

    pub fn display(&self) -> &str {
        match self {
            SupportedLanguage::English => "English",
            SupportedLanguage::Vietnamese => "Tiếng Việt",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedLanguage {
    English,
    Vietnamese,
}

#[derive(Debug, Clone)]
pub struct LanguageContext {
    pub configured: ResponseLanguage,
    pub detected: Option<DetectedLanguage>,
    pub effective: SupportedLanguage,
    pub confidence: f32,
}

impl LanguageContext {
    pub fn new(configured: ResponseLanguage) -> Self {
        Self {
            configured,
            detected: None,
            effective: SupportedLanguage::English,
            confidence: 0.0,
        }
    }

    pub fn resolve(&mut self, text: &str) {
        if self.configured == ResponseLanguage::Vietnamese {
            self.effective = SupportedLanguage::Vietnamese;
            self.confidence = 1.0;
            self.detected = Some(DetectedLanguage::Vietnamese);
            return;
        }
        if self.configured == ResponseLanguage::English {
            self.effective = SupportedLanguage::English;
            self.confidence = 1.0;
            self.detected = Some(DetectedLanguage::English);
            return;
        }

        let (detected, confidence) = detect_language(text);
        self.detected = Some(detected);
        self.confidence = confidence;
        self.effective = match detected {
            DetectedLanguage::Vietnamese => SupportedLanguage::Vietnamese,
            DetectedLanguage::English => SupportedLanguage::English,
        };
    }

    pub fn is_vietnamese(&self) -> bool {
        self.effective == SupportedLanguage::Vietnamese
    }

    pub fn candidate_instruction(&self) -> &str {
        if self.is_vietnamese() {
            VIETNAMESE_CANDIDATE_INSTRUCTION
        } else {
            ""
        }
    }

    pub fn judge_language_instruction(&self) -> &str {
        if self.is_vietnamese() {
            VIETNAMESE_JUDGE_INSTRUCTION
        } else {
            ""
        }
    }
}

impl Default for LanguageContext {
    fn default() -> Self {
        Self::new(ResponseLanguage::Auto)
    }
}

pub const VIETNAMESE_CANDIDATE_INSTRUCTION: &str = "\
Trả lời bằng tiếng Việt vì người dùng đang sử dụng tiếng Việt.

Sử dụng tiếng Việt tự nhiên, chính xác về mặt kỹ thuật cho các phần giải thích, \
tóm tắt, câu hỏi, cảnh báo, kế hoạch và đề xuất.

Giữ nguyên toàn bộ mã nguồn, định danh, đường dẫn file, mã model, mã provider, \
lệnh, khóa cấu hình, tên API, log, stack trace và thông báo lỗi gốc.

Không dịch mã nguồn hoặc tạo ra phiên bản tiếng Việt của các định danh kỹ thuật.";

pub const VIETNAMESE_JUDGE_INSTRUCTION: &str = "\
Trả lời bằng tiếng Việt. Đánh giá chất lượng tiếng Việt của ứng viên: \
độ trôi chảy, nhất quán ngôn ngữ, bảo toàn thuật ngữ kỹ thuật. \
Trừ điểm nếu ứng viên trả lời sai ngôn ngữ. \
Không trừ điểm cho mã nguồn, lệnh, định danh, log hoặc thông báo lỗi gốc.";

// ── Language Detection ─────────────────────────────────────────

pub fn detect_language(text: &str) -> (DetectedLanguage, f32) {
    let normalized = normalize_nfc(text);
    let clean = strip_technical_content(&normalized);
    if clean.trim().is_empty() {
        return (DetectedLanguage::English, 0.5);
    }

    let vn_score = vietnamese_score(&clean);

    if vn_score >= 0.15 {
        (DetectedLanguage::Vietnamese, (vn_score * 3.0).min(0.95))
    } else {
        (DetectedLanguage::English, (1.0 - vn_score).max(0.3))
    }
}

fn normalize_nfc(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    text.nfc().collect::<String>()
}

fn strip_technical_content(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut in_code = false;
    let mut in_json = false;
    let mut brace_depth = 0u32;

    while i < chars.len() {
        if i + 2 < chars.len() && chars[i] == '`' && chars[i + 1] == '`' && chars[i + 2] == '`' {
            in_code = !in_code;
            i += 3;
            continue;
        }
        if i + 1 < chars.len() && chars[i] == '{' {
            if !in_code { brace_depth += 1; in_json = true; }
        }
        if i + 1 < chars.len() && chars[i] == '}' {
            if !in_code && brace_depth > 0 { brace_depth -= 1; }
            if brace_depth == 0 { in_json = false; }
        }
        if i + 6 < chars.len() && chars[i..i + 7].iter().collect::<String>() == "http://" {
            while i < chars.len() && !chars[i].is_whitespace() { i += 1; }
            continue;
        }
        if i + 7 < chars.len() && chars[i..i + 8].iter().collect::<String>() == "https://" {
            while i < chars.len() && !chars[i].is_whitespace() { i += 1; }
            continue;
        }
        if !in_code && !in_json {
            result.push(chars[i]);
        }
        i += 1;
    }
    result
}

fn vietnamese_score(text: &str) -> f32 {
    let mut raw_score = 0.0f32;
    let mut vn_diacritic_count = 0u32;

    for c in text.chars() {
        if is_vietnamese_diacritic(c) {
            raw_score += 1.0;
            vn_diacritic_count += 1;
        }
    }

    if vn_diacritic_count == 0 {
        // Check for VN function words even without diacritics
        let lower = text.to_lowercase();
        let vn_words = [
            "của", "và", "một", "cho", "để", "với", "trong", "được", "không", "có",
            "là", "tôi", "tao", "này", "kia", "đó", "như", "nhưng", "nếu", "thì",
            "đã", "đang", "sẽ", "phải", "cần", "nên", "bị", "bởi", "vào",
            "ra", "lên", "xuống", "qua", "lại", "về", "tới", "đến", "từ", "ở",
            "làm", "chạy", "gọi", "xem", "hỏi", "nói", "viết", "đọc", "sửa", "xóa",
            "thêm", "bớt", "dùng", "thử", "kiểm", "tra", "tìm", "giúp",
            "lỗi", "sai", "đúng", "hỏng",
            "giải", "thích", "hướng", "dẫn", "cách", "sao",
        ];
        let mut fn_hits = 0u32;
        for word in &vn_words {
            if lower.contains(word) { fn_hits += 1; }
        }
        if fn_hits >= 2 { return 0.3; } // Strong word-level signal
        return 0.0;
    }

    // 2+ diacritics = strong Vietnamese signal
    if vn_diacritic_count >= 2 { raw_score += 4.0; }

    // Function words boost
    let lower = text.to_lowercase();
    let vn_fn = ["của", "và", "một", "cho", "để", "với", "trong", "được", "không",
        "là", "tôi", "tao", "này", "kia", "đó", "như", "nhưng", "nếu", "thì",
        "đã", "đang", "sẽ", "phải", "cần", "nên", "bị", "bởi",
        "lỗi", "sai", "đúng", "hỏng", "giúp", "giải", "thích",
    ];
    let mut fn_hits = 0u32;
    for word in &vn_fn {
        if lower.contains(word) { fn_hits += 1; }
    }
    raw_score += (fn_hits as f32).min(3.0);

    (raw_score / 8.0).min(1.0)
}

fn is_vietnamese_diacritic(c: char) -> bool {
    matches!(c,
        'ă' | 'â' | 'đ' | 'ê' | 'ô' | 'ơ' | 'ư' |
        'Ă' | 'Â' | 'Đ' | 'Ê' | 'Ô' | 'Ơ' | 'Ư' |
        'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' | 'ấ' | 'ầ' | 'ẩ' | 'ẫ' | 'ậ' |
        'Ắ' | 'Ằ' | 'Ẳ' | 'Ẵ' | 'Ặ' | 'Ấ' | 'Ầ' | 'Ẩ' | 'Ẫ' | 'Ậ' |
        'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' | 'Ế' | 'Ề' | 'Ể' | 'Ễ' | 'Ệ' |
        'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' | 'Ố' | 'Ồ' | 'Ổ' | 'Ỗ' | 'Ộ' |
        'ớ' | 'ờ' | 'ở' | 'ỡ' | 'ợ' | 'Ớ' | 'Ờ' | 'Ở' | 'Ỡ' | 'Ợ' |
        'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' | 'Ứ' | 'Ừ' | 'Ử' | 'Ữ' | 'Ự' |
        'á' | 'à' | 'ả' | 'ã' | 'ạ' | 'Á' | 'À' | 'Ả' | 'Ã' | 'Ạ' |
        'é' | 'è' | 'ẻ' | 'ẽ' | 'ẹ' | 'É' | 'È' | 'Ẻ' | 'Ẽ' | 'Ẹ' |
        'í' | 'ì' | 'ỉ' | 'ĩ' | 'ị' | 'Í' | 'Ì' | 'Ỉ' | 'Ĩ' | 'Ị' |
        'ó' | 'ò' | 'ỏ' | 'õ' | 'ọ' | 'Ó' | 'Ò' | 'Ỏ' | 'Õ' | 'Ọ' |
        'ú' | 'ù' | 'ủ' | 'ũ' | 'ụ' | 'Ú' | 'Ù' | 'Ủ' | 'Ũ' | 'Ụ' |
        'ý' | 'ỳ' | 'ỷ' | 'ỹ' | 'ỵ' | 'Ý' | 'Ỳ' | 'Ỷ' | 'Ỹ' | 'Ỵ'
    )
}

// ── Unicode Utilities ──────────────────────────────────────────

pub fn safe_truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars { return text.to_string(); }
    text.chars().take(max_chars).collect()
}

pub fn display_width(text: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(text)
}

pub fn safe_truncate_by_width(text: &str, max_width: usize) -> &str {
    let mut width = 0;
    for (i, c) in text.char_indices() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if width + cw > max_width { return &text[..i]; }
        width += cw;
    }
    text
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_plain_vietnamese() {
        let (lang, conf) = detect_language("tôi muốn sửa lỗi đăng nhập này");
        assert_eq!(lang, DetectedLanguage::Vietnamese);
        assert!(conf > 0.5);
    }

    #[test]
    fn test_detect_vietnamese_with_english_terms() {
        let (lang, _) = detect_language("refactor auth middleware cho dễ đọc");
        assert_eq!(lang, DetectedLanguage::Vietnamese);
    }

    #[test]
    fn test_detect_vietnamese_code_mixed() {
        let (lang, _) = detect_language("cargo test đang lỗi ở auth.rs, kiểm tra giúp tao");
        assert_eq!(lang, DetectedLanguage::Vietnamese);
    }

    #[test]
    fn test_detect_english() {
        let (lang, _) = detect_language("explain this function");
        assert_eq!(lang, DetectedLanguage::English);
    }

    #[test]
    fn test_detect_code_only() {
        let (lang, _) = detect_language("fn main() { println!(\"hello\"); }");
        assert_eq!(lang, DetectedLanguage::English);
    }

    #[test]
    fn test_detect_json_only() {
        let (lang, _) = detect_language(r#"{"key": "value", "num": 42}"#);
        assert_eq!(lang, DetectedLanguage::English);
    }

    #[test]
    fn test_detect_emoji_mixed_vietnamese() {
        let (lang, _) = detect_language("🔥 sửa lỗi này gấp 🚀");
        assert_eq!(lang, DetectedLanguage::Vietnamese);
    }

    #[test]
    fn test_language_context_override_vi() {
        let mut ctx = LanguageContext::new(ResponseLanguage::Vietnamese);
        ctx.resolve("hello world");
        assert_eq!(ctx.effective, SupportedLanguage::Vietnamese);
        assert_eq!(ctx.confidence, 1.0);
    }

    #[test]
    fn test_language_context_override_en() {
        let mut ctx = LanguageContext::new(ResponseLanguage::English);
        ctx.resolve("xin chào");
        assert_eq!(ctx.effective, SupportedLanguage::English);
        assert_eq!(ctx.confidence, 1.0);
    }

    #[test]
    fn test_nfc_normalization() {
        let nfd = "ti\u{1ebf}ng Vi\u{1ec7}t";
        let nfc = normalize_nfc(nfd);
        assert!(nfc.contains("tiếng"));
        assert!(nfc.contains("Việt"));
    }

    #[test]
    fn test_safe_truncate_vietnamese() {
        let text = "tiếng Việt";
        let truncated = safe_truncate(text, 5);
        assert_eq!(truncated.chars().count(), 5);
    }

    #[test]
    fn test_display_width_vietnamese() {
        let w = display_width("tiếng");
        assert!(w > 0);
    }

    #[test]
    fn test_strip_technical_content_code_blocks() {
        let input = "sửa ```fn main() {}``` trong file này";
        let clean = strip_technical_content(input);
        assert!(!clean.contains("fn main"));
        assert!(clean.contains("sửa"));
        assert!(clean.contains("trong file này"));
    }
}
