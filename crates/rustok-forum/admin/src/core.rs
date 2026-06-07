use crate::model::ReplyListItem;

pub fn topic_category_filter(category_id: String) -> Option<String> {
    let trimmed = category_id.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub fn parse_tags(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn format_count(value: usize) -> String {
    value.to_string()
}

pub fn topic_status_class(status: &str) -> &'static str {
    match status.to_ascii_lowercase().as_str() {
        "published" | "active" | "open" => "success",
        "draft" | "pending" => "warning",
        "archived" | "closed" => "muted",
        _ => "default",
    }
}

pub fn reply_count_label(replies: Option<Result<Vec<ReplyListItem>, String>>) -> usize {
    match replies {
        Some(Ok(items)) => items.len(),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_topic_category_filter() {
        assert_eq!(
            topic_category_filter("  category-1  ".to_string()),
            Some("category-1".to_string())
        );
        assert_eq!(topic_category_filter("   ".to_string()), None);
    }

    #[test]
    fn parses_comma_separated_tags_without_empty_values() {
        assert_eq!(
            parse_tags(" rust, forum ,, ffa "),
            vec!["rust", "forum", "ffa"]
        );
    }

    #[test]
    fn maps_topic_status_to_stable_class_keys() {
        assert_eq!(topic_status_class("PUBLISHED"), "success");
        assert_eq!(topic_status_class("pending"), "warning");
        assert_eq!(topic_status_class("closed"), "muted");
        assert_eq!(topic_status_class("other"), "default");
    }
}
