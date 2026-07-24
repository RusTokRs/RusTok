use crate::i18n::t;
use rustok_ui_core::css_hex_accent_class;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumStorefrontCategoryRailLabels {
    pub no_description: String,
    pub total_template: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumStorefrontCategoryCardViewModel {
    pub href: String,
    pub is_active: bool,
    pub container_class: &'static str,
    pub accent_class: &'static str,
    pub name: String,
    pub slug_badge: String,
    pub topic_count: i32,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumStorefrontTopicCardViewModel {
    pub href: String,
    pub is_active: bool,
    pub container_class: &'static str,
    pub status_class: &'static str,
    pub status_badge_class: &'static str,
    pub unread_badge_class: &'static str,
    pub status: String,
    pub effective_locale: String,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub is_unread: bool,
    pub unread_count: i64,
    pub has_unread_topic_revision: bool,
    pub title: String,
    pub slug_label: String,
    pub reply_count: i32,
}

pub fn forum_storefront_count_label(template: &str, count: impl ToString) -> String {
    template.replace("{count}", count.to_string().as_str())
}

pub fn forum_storefront_slug_label(template: &str, slug: &str) -> String {
    template.replace("{slug}", slug)
}

pub fn forum_storefront_category_card_class(is_active: bool) -> &'static str {
    if is_active {
        "border-primary/40 bg-primary/5 shadow-sm"
    } else {
        "border-border bg-background hover:border-primary/20 hover:bg-muted/40"
    }
}

pub fn forum_storefront_topic_card_class(is_active: bool, is_unread: bool) -> &'static str {
    if is_active {
        "border-primary/40 bg-primary/5 shadow-sm"
    } else if is_unread {
        "border-sky-500/40 bg-sky-500/5 hover:border-sky-500/60 hover:shadow-sm"
    } else {
        "border-border bg-background hover:border-primary/25 hover:shadow-sm"
    }
}

pub fn forum_storefront_accent_class(color: Option<&str>) -> &'static str {
    css_hex_accent_class(color)
}

pub fn forum_storefront_status_badge_class(status_class: &'static str) -> &'static str {
    match status_class {
        "success" => {
            "rounded-full bg-emerald-500/15 px-2.5 py-1 text-[11px] font-medium text-emerald-700 dark:text-emerald-300"
        }
        "warning" => {
            "rounded-full bg-amber-500/15 px-2.5 py-1 text-[11px] font-medium text-amber-700 dark:text-amber-300"
        }
        "muted" => {
            "rounded-full bg-muted px-2.5 py-1 text-[11px] font-medium text-muted-foreground"
        }
        _ => {
            "rounded-full border border-border px-2.5 py-1 text-[11px] font-medium text-muted-foreground"
        }
    }
}

pub fn forum_storefront_unread_badge_class() -> &'static str {
    "rounded-full bg-sky-500/15 px-2.5 py-1 text-[11px] font-semibold text-sky-700 dark:text-sky-300"
}

pub fn forum_storefront_category_card_view_model(
    module_route_base: &str,
    item: &crate::model::ForumCategoryListItem,
    selected_category_id: Option<&str>,
    labels: &ForumStorefrontCategoryRailLabels,
) -> ForumStorefrontCategoryCardViewModel {
    let is_active = selected_category_id == Some(item.id.as_str());
    ForumStorefrontCategoryCardViewModel {
        href: category_href(module_route_base, item.id.as_str()),
        is_active,
        container_class: forum_storefront_category_card_class(is_active),
        accent_class: forum_storefront_accent_class(item.color.as_deref()),
        name: item.name.clone(),
        slug_badge: forum_storefront_slug_label("#{slug}", item.slug.as_str()),
        topic_count: item.topic_count,
        description: item
            .description
            .clone()
            .unwrap_or_else(|| labels.no_description.clone()),
    }
}

pub fn forum_storefront_topic_card_view_model(
    module_route_base: &str,
    item: &crate::model::ForumTopicListItem,
    selected_category_id: Option<&str>,
    selected_topic_id: Option<&str>,
    slug_template: &str,
) -> ForumStorefrontTopicCardViewModel {
    let is_active = selected_topic_id == Some(item.id.as_str());
    let is_unread = item.is_unread.unwrap_or(false);
    let status_class = topic_status_class(item.status.as_str());
    ForumStorefrontTopicCardViewModel {
        href: topic_href(module_route_base, selected_category_id, item.id.as_str()),
        is_active,
        container_class: forum_storefront_topic_card_class(is_active, is_unread),
        status_class,
        status_badge_class: forum_storefront_status_badge_class(status_class),
        unread_badge_class: forum_storefront_unread_badge_class(),
        status: item.status.clone(),
        effective_locale: item.effective_locale.clone(),
        is_pinned: item.is_pinned,
        is_locked: item.is_locked,
        is_unread,
        unread_count: item.unread_count.unwrap_or(0),
        has_unread_topic_revision: item.has_unread_topic_revision.unwrap_or(false),
        title: item.title.clone(),
        slug_label: forum_storefront_slug_label(slug_template, item.slug.as_str()),
        reply_count: item.reply_count,
    }
}

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

    #[test]
    fn builds_storefront_copy_and_class_policies_without_framework_state() {
        assert_eq!(
            forum_storefront_count_label("{count} threads", 12_u64),
            "12 threads"
        );
        assert_eq!(
            forum_storefront_slug_label("slug: {slug}", "welcome"),
            "slug: welcome"
        );
        assert!(forum_storefront_category_card_class(true).contains("border-primary/40"));
        assert!(forum_storefront_topic_card_class(false, false).contains("hover:shadow-sm"));
        assert!(forum_storefront_topic_card_class(false, true).contains("border-sky-500/40"));
        assert_eq!(forum_storefront_accent_class(Some("#0ea5e9")), "bg-sky-500");
        assert!(forum_storefront_accent_class(Some(" ")).contains("from-sky-500"));
        assert!(forum_storefront_status_badge_class("success").contains("emerald"));
        assert!(forum_storefront_status_badge_class("warning").contains("amber"));
        assert!(forum_storefront_status_badge_class("muted").contains("bg-muted"));
        assert!(forum_storefront_status_badge_class("default").contains("border-border"));
        assert!(forum_storefront_unread_badge_class().contains("sky"));
    }

    #[test]
    fn rejects_persisted_css_declaration_injection() {
        let class =
            forum_storefront_accent_class(Some("#fff;background:url(https://attacker.invalid/x)"));
        assert!(class.contains("from-sky-500"));
    }
}
