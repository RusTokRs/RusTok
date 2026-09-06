#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelTargetType {
    WebDomain,
    MobileApp,
    ApiClient,
    Embedded,
    External,
}

impl ChannelTargetType {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "web_domain" => Some(Self::WebDomain),
            "mobile_app" => Some(Self::MobileApp),
            "api_client" => Some(Self::ApiClient),
            "embedded" => Some(Self::Embedded),
            "external" => Some(Self::External),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WebDomain => "web_domain",
            Self::MobileApp => "mobile_app",
            Self::ApiClient => "api_client",
            Self::Embedded => "embedded",
            Self::External => "external",
        }
    }

    pub fn supports_host_resolution(&self) -> bool {
        matches!(self, Self::WebDomain)
    }

    pub fn normalize_value(&self, raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        match self {
            Self::WebDomain => normalize_web_domain(trimmed),
            _ => Some(trimmed.to_string()),
        }
    }
}

fn normalize_web_domain(raw: &str) -> Option<String> {
    let without_scheme = raw
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(raw)
        .trim();
    let authority = without_scheme.split(['/', '?', '#']).next()?.trim();
    if authority.is_empty() || authority.contains('@') {
        return None;
    }

    let host = strip_optional_port(authority)?;
    let normalized = host.trim_end_matches('.').to_ascii_lowercase();
    if normalized.is_empty() || !is_valid_web_domain(&normalized) {
        return None;
    }

    Some(normalized)
}

fn strip_optional_port(authority: &str) -> Option<&str> {
    if authority.starts_with('[') {
        return None;
    }

    match authority.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && port.chars().all(|ch| ch.is_ascii_digit()) => {
            Some(host)
        }
        _ => Some(authority),
    }
}

fn is_valid_web_domain(value: &str) -> bool {
    if value.len() > 253 {
        return false;
    }

    value.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    })
}

#[cfg(test)]
mod tests {
    use super::ChannelTargetType;

    #[test]
    fn normalizes_web_domain_from_url_like_input() {
        assert_eq!(
            ChannelTargetType::WebDomain.normalize_value(" https://Store.Example.TEST:443/ "),
            Some("store.example.test".to_string())
        );
        assert_eq!(
            ChannelTargetType::WebDomain.normalize_value("Example.test."),
            Some("example.test".to_string())
        );
    }

    #[test]
    fn rejects_invalid_web_domain_values() {
        assert_eq!(
            ChannelTargetType::WebDomain.normalize_value("bad host"),
            None
        );
        assert_eq!(
            ChannelTargetType::WebDomain.normalize_value("https://user@example.test"),
            None
        );
        assert_eq!(
            ChannelTargetType::WebDomain.normalize_value("-example.test"),
            None
        );
    }
}
