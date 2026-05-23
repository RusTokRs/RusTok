pub fn fallback_text(value: Option<String>, fallback: &str) -> String {
    value.unwrap_or_else(|| fallback.to_string())
}

pub fn count_label(total: u64, suffix: &str) -> String {
    format!("{total} {suffix}")
}

pub fn open_link_label(label: &str, slug: &str) -> String {
    format!("{label} {slug}")
}

pub fn label_value_pair(label: &str, value: &str) -> String {
    format!("{label}: {value}")
}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

pub fn module_href(base: &str, slug: &str) -> String {
    format!("{base}?slug={slug}")
}

pub fn summarize_content(content: &str, format: &str, fallback_template: &str) -> String {
    if format.eq_ignore_ascii_case("markdown") {
        return content.trim().to_string();
    }

    fallback_template
        .replace("{format}", format)
        .replace("{count}", &content.chars().count().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_text_returns_fallback_for_none() {
        assert_eq!(fallback_text(None, "fallback"), "fallback".to_string());
    }

    #[test]
    fn summarize_content_handles_markdown_and_raw() {
        assert_eq!(summarize_content("  hello  ", "markdown", "x"), "hello");
        assert_eq!(
            summarize_content(
                "raw payload",
                "json",
                "Stored in `{format}` format. Raw body length: {count} characters.",
            ),
            "Stored in `json` format. Raw body length: 11 characters.".to_string()
        );
    }

    #[test]
    fn error_and_href_helpers_format_expected_values() {
        assert_eq!(
            error_with_context("Failed to load", "timeout"),
            "Failed to load: timeout"
        );
        assert_eq!(
            module_href("/store/modules/blog", "hello-world"),
            "/store/modules/blog?slug=hello-world"
        );
    }
}
