use crate::i18n::t;

pub fn category_href(module_route_base: &str, category_id: &str) -> String {
    format!("{module_route_base}?category={category_id}")
}

pub fn topic_href(module_route_base: &str, category_id: Option<&str>, topic_id: &str) -> String {
    match category_id {
        Some(category_id) if !category_id.is_empty() => {
            format!("{module_route_base}?category={category_id}&topic={topic_id}")
        }
        _ => format!("{module_route_base}?topic={topic_id}"),
    }
}

pub fn summarize_rich_content(content: &str, format: &str, locale: Option<&str>) -> String {
    if format.eq_ignore_ascii_case("markdown") {
        return content.trim().to_string();
    }

    let template = t(
        locale,
        "forum.richContent.summary",
        "Stored in `{format}` format. Raw content length: {count} characters.",
    );
    template
        .replace("{format}", format)
        .replace("{count}", content.chars().count().to_string().as_str())
}

pub fn topic_status_class(status: &str) -> &'static str {
    match status.to_ascii_lowercase().as_str() {
        "published" | "active" | "open" | "approved" => "success",
        "draft" | "pending" => "warning",
        "archived" | "closed" | "hidden" => "muted",
        _ => "default",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_category_and_topic_hrefs_with_existing_query_keys() {
        assert_eq!(
            category_href("/forum", "category-1"),
            "/forum?category=category-1"
        );
        assert_eq!(
            topic_href("/forum", Some("category-1"), "topic-1"),
            "/forum?category=category-1&topic=topic-1"
        );
        assert_eq!(
            topic_href("/forum", None, "topic-1"),
            "/forum?topic=topic-1"
        );
    }

    #[test]
    fn summarizes_markdown_and_non_markdown_without_framework_state() {
        assert_eq!(
            summarize_rich_content("  hello  ", "markdown", None),
            "hello"
        );
        assert_eq!(
            summarize_rich_content("Здравствуйте", "rt-json", Some("en")),
            "Stored in `rt-json` format. Raw content length: 12 characters."
        );
    }

    #[test]
    fn maps_topic_status_to_stable_badge_class_keys() {
        assert_eq!(topic_status_class("PUBLISHED"), "success");
        assert_eq!(topic_status_class("pending"), "warning");
        assert_eq!(topic_status_class("closed"), "muted");
        assert_eq!(topic_status_class("unknown"), "default");
    }
}
