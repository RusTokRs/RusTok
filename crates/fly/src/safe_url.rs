pub(crate) fn validate_safe_url(value: &str, label: &str) -> Result<(), String> {
    normalize_safe_url(value, label)?;
    Ok(())
}

pub(crate) fn normalize_safe_url(value: &str, label: &str) -> Result<String, String> {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    if value.is_empty()
        || value.chars().any(char::is_control)
        || value.chars().any(char::is_whitespace)
        || value.contains('\\')
        || value.starts_with("//")
        || lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("vbscript:")
    {
        return Err(format!("{label} `{value}` is unsafe"));
    }

    if value.starts_with('/') || value.starts_with('#') || value.starts_with('?') {
        return Ok(value.to_string());
    }
    if lower.starts_with("https://") {
        validate_authority(value, "https://".len(), label)?;
        return Ok(value.to_string());
    }
    if lower.starts_with("http://") {
        validate_authority(value, "http://".len(), label)?;
        return Ok(value.to_string());
    }
    if lower.starts_with("mailto:") {
        validate_non_empty_target(value, "mailto:".len(), label)?;
        return Ok(value.to_string());
    }
    if lower.starts_with("tel:") {
        validate_non_empty_target(value, "tel:".len(), label)?;
        return Ok(value.to_string());
    }

    Err(format!("{label} `{value}` uses an unsupported URL form"))
}

fn validate_authority(value: &str, prefix_len: usize, label: &str) -> Result<(), String> {
    let authority = value[prefix_len..]
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if authority.is_empty() || authority.starts_with(':') {
        Err(format!("{label} `{value}` has no valid authority"))
    } else {
        Ok(())
    }
}

fn validate_non_empty_target(value: &str, prefix_len: usize, label: &str) -> Result<(), String> {
    if value[prefix_len..].is_empty() {
        Err(format!("{label} `{value}` has no target"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_local_and_absolute_urls() {
        for value in [
            "/pricing",
            "#contact",
            "?source=hero",
            "https://example.com/path?q=1#section",
            "http://localhost:3000/form",
            "mailto:sales@example.com",
            "tel:+12025550123",
        ] {
            assert_eq!(normalize_safe_url(value, "URL").unwrap(), value);
        }
    }

    #[test]
    fn rejects_network_paths_backslashes_controls_and_unsafe_schemes() {
        for value in [
            "//attacker.example/path",
            "/\\attacker.example/path",
            "javascript:alert(1)",
            "data:text/html,unsafe",
            "vbscript:msgbox(1)",
            "https://example.com/has space",
            "https://example.com/line\nbreak",
        ] {
            assert!(normalize_safe_url(value, "URL").is_err(), "{value}");
        }
    }

    #[test]
    fn rejects_absolute_urls_without_authority_or_scheme_targets() {
        for value in ["https://", "http:///path", "mailto:", "tel:"] {
            assert!(normalize_safe_url(value, "URL").is_err(), "{value}");
        }
    }
}
