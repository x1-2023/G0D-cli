#[derive(Debug, Clone)]
pub enum Credential {
    BearerToken(String),
    XApiKey(String),
    None,
}

impl Credential {
    pub fn header_value(&self) -> Option<String> {
        match self {
            Credential::BearerToken(token) => Some(format!("Bearer {}", token)),
            Credential::XApiKey(key) => Some(key.clone()),
            Credential::None => None,
        }
    }

    pub fn redacted(&self) -> String {
        match self {
            Credential::BearerToken(t) => format!("Bearer {}...{}", &t[..t.len().min(4)], &t[t.len().saturating_sub(4)..]),
            Credential::XApiKey(k) => format!("xai-...{}", &k[k.len().saturating_sub(4)..]),
            Credential::None => "none".into(),
        }
    }

    pub fn is_set(&self) -> bool {
        !matches!(self, Credential::None)
    }
}

pub fn redact_auth_header(value: &str) -> String {
    if value.len() <= 8 {
        return "***".into();
    }
    let prefix = &value[..value.len().min(7)];
    if value.len() > 16 {
        format!("{}...***...{}", prefix, &value[value.len().saturating_sub(4)..])
    } else {
        format!("{}...***", prefix)
    }
}

pub fn redact_key(key: &str) -> String {
    if key.len() <= 8 { return "***".into(); }
    format!("{}...{}", &key[..4], &key[key.len().saturating_sub(4)..])
}
