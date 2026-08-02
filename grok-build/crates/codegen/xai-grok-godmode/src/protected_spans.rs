pub fn is_protected_span(text: &str) -> bool {
    let markers = ["```", "`", "http://", "https://", "git@", "fn ", "class ", "struct "];
    markers.iter().any(|m| text.contains(m))
}

pub fn protect_structured_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut in_code_block = false;
    let mut code_start = 0;
    let mut i = 0;

    let chars: Vec<char> = text.chars().collect();
    while i < chars.len() {
        if i + 2 < chars.len() && chars[i] == '`' && chars[i+1] == '`' && chars[i+2] == '`' {
            if in_code_block {
                spans.push((code_start, i + 3));
                in_code_block = false;
            } else {
                code_start = i;
                in_code_block = true;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    spans
}
