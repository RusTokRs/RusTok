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

pub fn fallback_slug(value: Option<String>, fallback: &str) -> String {
    fallback_text(value, fallback)
}

pub fn fallback_excerpt(value: Option<String>, fallback: &str) -> String {
    fallback_text(value, fallback)
}

pub fn selected_slug_or_default(value: Option<String>, default_slug: &str) -> String {
    value.unwrap_or_else(|| default_slug.to_string())
}

pub fn route_segment_or_default(value: Option<String>, default_segment: &str) -> String {
    value.unwrap_or_else(|| default_segment.to_string())
}

pub fn body_or_fallback(value: Option<String>, fallback: &str) -> String {
    fallback_text(value, fallback)
}

pub fn summarize_content(content: &str, format: &str, fallback_template: &str) -> String {
    if format.eq_ignore_ascii_case("markdown") {
        return content.trim().to_string();
    }

    fallback_template
        .replace("{format}", format)
        .replace("{count}", &content.chars().count().to_string())
}

pub fn status_badge_css(status: &str) -> &'static str {
    let status = status.trim();

    if status.eq_ignore_ascii_case("published") {
        "inline-flex rounded-full border border-emerald-300/50 bg-emerald-50 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-emerald-800 dark:border-emerald-700/40 dark:bg-emerald-900/25 dark:text-emerald-300"
    } else if status.eq_ignore_ascii_case("archived") {
        "inline-flex rounded-full border border-border bg-muted px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground"
    } else {
        "inline-flex rounded-full border border-primary/30 bg-primary/10 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-primary"
    }
}

pub fn status_label(status: &str, fallback: &str) -> String {
    let normalized = status.trim();
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized.to_string()
    }
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
        assert_eq!(
            fallback_slug(None, "missing-slug"),
            "missing-slug".to_string()
        );
        assert_eq!(
            fallback_excerpt(None, "No excerpt yet."),
            "No excerpt yet.".to_string()
        );
        assert_eq!(
            selected_slug_or_default(None, "latest"),
            "latest".to_string()
        );
        assert_eq!(route_segment_or_default(None, "blog"), "blog".to_string());
        assert_eq!(
            body_or_fallback(None, "No body content yet."),
            "No body content yet.".to_string()
        );
    }

    #[test]
    fn status_badge_css_maps_known_statuses() {
        assert_eq!(
            status_badge_css("published"),
            "inline-flex rounded-full border border-emerald-300/50 bg-emerald-50 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-emerald-800 dark:border-emerald-700/40 dark:bg-emerald-900/25 dark:text-emerald-300"
        );
        assert_eq!(
            status_badge_css("archived"),
            "inline-flex rounded-full border border-border bg-muted px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground"
        );
        assert_eq!(
            status_badge_css("draft"),
            "inline-flex rounded-full border border-primary/30 bg-primary/10 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-primary"
        );
        assert_eq!(
            status_badge_css("  Published  "),
            "inline-flex rounded-full border border-emerald-300/50 bg-emerald-50 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-emerald-800 dark:border-emerald-700/40 dark:bg-emerald-900/25 dark:text-emerald-300"
        );
    }

    #[test]
    fn status_label_trims_and_falls_back() {
        assert_eq!(
            status_label("  published  ", "unknown"),
            "published".to_string()
        );
        assert_eq!(status_label("   ", "unknown"), "unknown".to_string());
    }
}
