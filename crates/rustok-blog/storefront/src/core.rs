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

pub fn post_meta_pairs(
    slug_label: &str,
    slug: &str,
    locale_label: &str,
    effective_locale: &str,
    published_label: &str,
    published_at: &str,
) -> [String; 3] {
    [
        label_value_pair(slug_label, slug),
        label_value_pair(locale_label, effective_locale),
        label_value_pair(published_label, published_at),
    ]
}

pub fn meta_separator() -> &'static str {
    "·"
}

pub fn selected_post_meta_row(
    slug_label: &str,
    slug: &str,
    locale_label: &str,
    effective_locale: &str,
    published_label: &str,
    published_at: &str,
) -> (String, String, String, &'static str) {
    let [slug_meta, locale_meta, published_meta] = post_meta_pairs(
        slug_label,
        slug,
        locale_label,
        effective_locale,
        published_label,
        published_at,
    );
    (slug_meta, locale_meta, published_meta, meta_separator())
}

pub fn list_post_excerpt(post_excerpt: Option<String>, fallback: &str) -> String {
    fallback_excerpt(post_excerpt, fallback)
}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

pub fn module_href(base: &str, slug: &str) -> String {
    format!("{base}?slug={slug}")
}

pub fn post_link(base: &str, slug: &str, open_label: &str) -> (String, String) {
    (
        module_href(base, slug),
        open_link_label(open_label, slug),
    )
}

pub fn list_post_summary(
    slug: Option<String>,
    missing_slug_fallback: &str,
    excerpt: Option<String>,
    excerpt_fallback: &str,
    module_route_base: &str,
    open_label: &str,
) -> (String, String, String) {
    let resolved_slug = fallback_slug(slug, missing_slug_fallback);
    let resolved_excerpt = list_post_excerpt(excerpt, excerpt_fallback);
    let (href, resolved_open_label) = post_link(module_route_base, resolved_slug.as_str(), open_label);
    (resolved_excerpt, href, resolved_open_label)
}

pub fn list_post_locale_meta(locale_label: &str, effective_locale: &str) -> String {
    label_value_pair(locale_label, effective_locale)
}

pub fn list_post_card_fields(
    slug: Option<String>,
    missing_slug_fallback: &str,
    excerpt: Option<String>,
    excerpt_fallback: &str,
    module_route_base: &str,
    open_label: &str,
    locale_label: &str,
    effective_locale: &str,
) -> (String, String, String, String) {
    let (resolved_excerpt, href, resolved_open_label) = list_post_summary(
        slug,
        missing_slug_fallback,
        excerpt,
        excerpt_fallback,
        module_route_base,
        open_label,
    );
    let locale_meta = list_post_locale_meta(locale_label, effective_locale);
    (resolved_excerpt, href, resolved_open_label, locale_meta)
}

pub fn list_post_card_view(
    slug: Option<String>,
    missing_slug_fallback: &str,
    excerpt: Option<String>,
    excerpt_fallback: &str,
    module_route_base: &str,
    open_label: &str,
    locale_label: &str,
    effective_locale: &str,
    status: String,
) -> (String, String, String, String, String) {
    let (resolved_excerpt, href, resolved_open_label, locale_meta) = list_post_card_fields(
        slug,
        missing_slug_fallback,
        excerpt,
        excerpt_fallback,
        module_route_base,
        open_label,
        locale_label,
        effective_locale,
    );
    (status, resolved_excerpt, href, resolved_open_label, locale_meta)
}

pub fn fallback_slug(value: Option<String>, fallback: &str) -> String {
    fallback_text(value, fallback)
}

pub fn fallback_excerpt(value: Option<String>, fallback: &str) -> String {
    fallback_text(value, fallback)
}

pub fn selected_post_fallback_fields(
    slug: Option<String>,
    slug_fallback: &str,
    excerpt: Option<String>,
    excerpt_fallback: &str,
    published_at: Option<String>,
    published_at_fallback: &str,
) -> (String, String, String) {
    (
        fallback_slug(slug, slug_fallback),
        fallback_excerpt(excerpt, excerpt_fallback),
        fallback_text(published_at, published_at_fallback),
    )
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

pub fn summarized_body_or_fallback(
    body: Option<String>,
    body_format: &str,
    no_body_fallback: &str,
    raw_format_template: &str,
) -> String {
    body_or_fallback(
        body.map(|content| summarize_content(content.as_str(), body_format, raw_format_template)),
        no_body_fallback,
    )
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

pub fn has_items<T>(items: &[T]) -> bool {
    !items.is_empty()
}

pub fn status_presentation(status: &str, fallback: &str) -> (String, &'static str) {
    let label = status_label(status, fallback);
    let badge_css = status_badge_css(label.as_str());
    (label, badge_css)
}

pub fn status_badge_view(status: String, unknown_label: &str) -> (String, &'static str) {
    status_presentation(status.as_str(), unknown_label)
}

pub fn selected_post_tag_items(tags: Vec<String>) -> Option<Vec<String>> {
    if has_items(tags.as_slice()) {
        Some(tags)
    } else {
        None
    }
}

pub fn published_posts_or_empty_message<T>(
    items: Vec<T>,
    empty_message: String,
) -> Result<Vec<T>, String> {
    if has_items(items.as_slice()) {
        Ok(items)
    } else {
        Err(empty_message)
    }
}

pub fn published_posts_view_state<T>(
    items: Vec<T>,
    empty_message: String,
) -> (Option<Vec<T>>, Option<String>) {
    match published_posts_or_empty_message(items, empty_message) {
        Ok(items) => (Some(items), None),
        Err(message) => (None, Some(message)),
    }
}

pub fn published_posts_items_or_default<T>(items: Option<Vec<T>>) -> Vec<T> {
    items.unwrap_or_default()
}

pub fn published_posts_ready_items<T>(
    items: Vec<T>,
    empty_message: String,
) -> Result<Vec<T>, String> {
    let (items, empty_message_opt) = published_posts_view_state(items, empty_message);
    let ready_items = published_posts_items_or_default(items);
    if has_items(ready_items.as_slice()) {
        Ok(ready_items)
    } else {
        Err(empty_message_opt.unwrap_or_default())
    }
}

pub fn published_posts_empty_state_message(message: String) -> String {
    message
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
    fn summarized_body_or_fallback_handles_none_and_raw_payload() {
        assert_eq!(
            summarized_body_or_fallback(
                None,
                "markdown",
                "No body content yet.",
                "Stored in `{format}` format. Raw body length: {count} characters.",
            ),
            "No body content yet.".to_string()
        );
        assert_eq!(
            summarized_body_or_fallback(
                Some("raw payload".to_string()),
                "json",
                "No body content yet.",
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
        assert_eq!(
            post_link("/store/modules/blog", "hello-world", "Open"),
            (
                "/store/modules/blog?slug=hello-world".to_string(),
                "Open hello-world".to_string()
            )
        );
        assert_eq!(
            post_meta_pairs(
                "slug",
                "hello-world",
                "locale",
                "en",
                "published",
                "2026-01-01T00:00:00Z",
            ),
            [
                "slug: hello-world".to_string(),
                "locale: en".to_string(),
                "published: 2026-01-01T00:00:00Z".to_string(),
            ]
        );
        assert_eq!(meta_separator(), "·");
        assert_eq!(
            selected_post_meta_row(
                "slug",
                "hello-world",
                "locale",
                "en",
                "published",
                "2026-01-01T00:00:00Z",
            ),
            (
                "slug: hello-world".to_string(),
                "locale: en".to_string(),
                "published: 2026-01-01T00:00:00Z".to_string(),
                "·",
            )
        );
        assert_eq!(
            selected_post_fallback_fields(
                None,
                "missing-slug",
                None,
                "No excerpt yet.",
                None,
                "Unscheduled",
            ),
            (
                "missing-slug".to_string(),
                "No excerpt yet.".to_string(),
                "Unscheduled".to_string(),
            )
        );
        assert_eq!(
            list_post_summary(
                None,
                "missing-slug",
                None,
                "No excerpt yet.",
                "/store/modules/blog",
                "Open",
            ),
            (
                "No excerpt yet.".to_string(),
                "/store/modules/blog?slug=missing-slug".to_string(),
                "Open missing-slug".to_string(),
            )
        );
        assert_eq!(
            list_post_locale_meta("locale", "en"),
            "locale: en".to_string()
        );
        assert_eq!(
            list_post_card_fields(
                None,
                "missing-slug",
                None,
                "No excerpt yet.",
                "/store/modules/blog",
                "Open",
                "locale",
                "en",
            ),
            (
                "No excerpt yet.".to_string(),
                "/store/modules/blog?slug=missing-slug".to_string(),
                "Open missing-slug".to_string(),
                "locale: en".to_string(),
            )
        );
        assert_eq!(
            list_post_card_view(
                None,
                "missing-slug",
                None,
                "No excerpt yet.",
                "/store/modules/blog",
                "Open",
                "locale",
                "en",
                "published".to_string(),
            ),
            (
                "published".to_string(),
                "No excerpt yet.".to_string(),
                "/store/modules/blog?slug=missing-slug".to_string(),
                "Open missing-slug".to_string(),
                "locale: en".to_string(),
            )
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

    #[test]
    fn has_items_detects_non_empty_collection() {
        assert!(!has_items::<u8>(&[]));
        assert!(has_items(&[1_u8]));
    }

    #[test]
    fn status_presentation_returns_label_and_css() {
        let (label, css) = status_presentation("  published  ", "unknown");
        assert_eq!(label, "published".to_string());
        assert_eq!(
            css,
            "inline-flex rounded-full border border-emerald-300/50 bg-emerald-50 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-emerald-800 dark:border-emerald-700/40 dark:bg-emerald-900/25 dark:text-emerald-300"
        );
    }

    #[test]
    fn status_badge_view_maps_owned_status() {
        let (label, css) = status_badge_view(" archived ".to_string(), "unknown");
        assert_eq!(label, "archived".to_string());
        assert_eq!(
            css,
            "inline-flex rounded-full border border-border bg-muted px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground"
        );
    }

    #[test]
    fn selected_post_tag_items_filters_empty_vectors() {
        assert_eq!(selected_post_tag_items(vec![]), None);
        assert_eq!(
            selected_post_tag_items(vec!["news".to_string(), "release".to_string()]),
            Some(vec!["news".to_string(), "release".to_string()])
        );
    }

    #[test]
    fn published_posts_or_empty_message_maps_empty_and_non_empty() {
        assert_eq!(
            published_posts_or_empty_message(vec!["a".to_string()], "empty".to_string()),
            Ok(vec!["a".to_string()])
        );
        assert_eq!(
            published_posts_or_empty_message::<String>(vec![], "empty".to_string()),
            Err("empty".to_string())
        );
    }

    #[test]
    fn published_posts_view_state_maps_to_option_pair() {
        assert_eq!(
            published_posts_view_state(vec!["a".to_string()], "empty".to_string()),
            (Some(vec!["a".to_string()]), None)
        );
        assert_eq!(
            published_posts_view_state::<String>(vec![], "empty".to_string()),
            (None, Some("empty".to_string()))
        );
    }

    #[test]
    fn published_posts_items_or_default_returns_empty_for_none() {
        assert_eq!(
            published_posts_items_or_default::<String>(None),
            Vec::<String>::new()
        );
        assert_eq!(
            published_posts_items_or_default(Some(vec!["x".to_string()])),
            vec!["x".to_string()]
        );
    }

    #[test]
    fn published_posts_ready_items_maps_to_result() {
        assert_eq!(
            published_posts_ready_items(vec!["x".to_string()], "empty".to_string()),
            Ok(vec!["x".to_string()])
        );
        assert_eq!(
            published_posts_ready_items::<String>(vec![], "empty".to_string()),
            Err("empty".to_string())
        );
    }

    #[test]
    fn published_posts_empty_state_message_passthrough() {
        assert_eq!(
            published_posts_empty_state_message("No items".to_string()),
            "No items".to_string()
        );
    }
}
