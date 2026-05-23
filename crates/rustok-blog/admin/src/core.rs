pub fn optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn parse_tags(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn slugify(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn status_badge_class(status: &str) -> &'static str {
    if status.eq_ignore_ascii_case("published") {
        "bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
    } else if status.eq_ignore_ascii_case("archived") {
        "bg-muted text-muted-foreground"
    } else {
        "bg-primary/10 text-primary"
    }
}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

pub fn busy_key_for_edit(post_id: &str) -> String {
    format!("edit:{post_id}")
}

pub fn busy_key_for_save(post_id: Option<&str>) -> String {
    match post_id {
        Some(id) => format!("save:{id}"),
        None => "create".to_string(),
    }
}

pub fn busy_key_for_publish(post_id: &str) -> String {
    format!("publish:{post_id}")
}

pub fn busy_key_for_archive(post_id: &str) -> String {
    format!("archive:{post_id}")
}

pub fn busy_key_for_delete(post_id: &str) -> String {
    format!("delete:{post_id}")
}

pub fn is_save_busy(busy_key: Option<&str>) -> bool {
    busy_key == Some("create") || busy_key.map(|key| key.starts_with("save:")).unwrap_or(false)
}

pub fn label_with_id(template: &str, id: &str) -> String {
    template.replace("{id}", id)
}

pub fn count_label(template: &str, total: u64) -> String {
    template.replace("{count}", &total.to_string())
}

pub fn is_published_status(status: &str) -> bool {
    status.eq_ignore_ascii_case("published")
}

pub fn is_archived_status(status: &str) -> bool {
    status.eq_ignore_ascii_case("archived")
}

pub fn status_badge_css(status: &str) -> String {
    format!(
        "inline-flex rounded-full px-2.5 py-0.5 text-xs font-semibold {}",
        status_badge_class(status)
    )
}

pub fn has_non_empty_text(value: &str) -> bool {
    !value.trim().is_empty()
}

pub fn trimmed_text(value: &str) -> String {
    value.trim().to_string()
}

pub fn fallback_post_slug(value: Option<String>, fallback: &str) -> String {
    value.unwrap_or_else(|| fallback.to_string())
}

pub fn fallback_post_excerpt(value: Option<String>, fallback: &str) -> String {
    value.unwrap_or_else(|| fallback.to_string())
}

pub fn row_is_busy_for_post(busy_key: Option<&str>, post_id: &str) -> bool {
    busy_key
        .map(|key| key.contains(post_id))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_text_returns_none_for_blank() {
        assert_eq!(optional_text("   "), None);
    }

    #[test]
    fn optional_text_returns_trimmed_value() {
        assert_eq!(optional_text("  slug  "), Some("slug".to_string()));
    }

    #[test]
    fn parse_tags_trims_and_skips_empty() {
        assert_eq!(
            parse_tags("news, launch, , release"),
            vec!["news".to_string(), "launch".to_string(), "release".to_string()]
        );
    }

    #[test]
    fn slugify_normalizes_text() {
        assert_eq!(slugify("Hello, Rustok UI!"), "hello-rustok-ui");
    }

    #[test]
    fn status_badge_class_handles_known_statuses() {
        assert_eq!(
            status_badge_class("published"),
            "bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
        );
        assert_eq!(
            status_badge_class("archived"),
            "bg-muted text-muted-foreground"
        );
        assert_eq!(status_badge_class("draft"), "bg-primary/10 text-primary");
    }

    #[test]
    fn busy_key_helpers_and_save_busy_are_consistent() {
        assert_eq!(busy_key_for_edit("1"), "edit:1".to_string());
        assert_eq!(busy_key_for_save(Some("1")), "save:1".to_string());
        assert_eq!(busy_key_for_save(None), "create".to_string());
        assert_eq!(busy_key_for_publish("1"), "publish:1".to_string());
        assert_eq!(busy_key_for_archive("1"), "archive:1".to_string());
        assert_eq!(busy_key_for_delete("1"), "delete:1".to_string());
        assert!(is_save_busy(Some("create")));
        assert!(is_save_busy(Some("save:1")));
        assert!(!is_save_busy(Some("publish:1")));
        assert!(!is_save_busy(None));
    }

    #[test]
    fn error_with_context_formats_as_expected() {
        assert_eq!(
            error_with_context("Failed to save post", "timeout"),
            "Failed to save post: timeout".to_string()
        );
    }

    #[test]
    fn label_count_and_status_helpers_work() {
        assert_eq!(label_with_id("Editing post {id}", "42"), "Editing post 42");
        assert_eq!(count_label("{count} total", 7), "7 total");
        assert!(is_published_status("published"));
        assert!(is_archived_status("archived"));
        assert!(!is_archived_status("draft"));
        assert_eq!(
            status_badge_css("published"),
            "inline-flex rounded-full px-2.5 py-0.5 text-xs font-semibold bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
        );
        assert!(has_non_empty_text(" x "));
        assert!(!has_non_empty_text("   "));
        assert_eq!(trimmed_text(" abc "), "abc".to_string());
        assert_eq!(
            fallback_post_slug(None, "missing-slug"),
            "missing-slug".to_string()
        );
        assert_eq!(
            fallback_post_excerpt(None, "No excerpt"),
            "No excerpt".to_string()
        );
        assert!(row_is_busy_for_post(Some("edit:42"), "42"));
        assert!(!row_is_busy_for_post(Some("edit:41"), "42"));
    }
}
