use crate::model::{
    CategoryDetail, CategoryDraft, CategoryListItem, ReplyListItem, TopicDetail, TopicDraft,
    TopicListItem,
};

const DEFAULT_CATEGORY_ACCENT_STYLE: &str =
    "background:linear-gradient(180deg,#0ea5e9 0%,#f59e0b 100%);";

#[derive(Clone, Debug)]
pub struct ForumAdminCategoryRenderLabels {
    pub no_description: String,
    pub topics_count_template: String,
    pub replies_count_template: String,
    pub icon_template: String,
    pub editing: String,
    pub edit: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumAdminCategoryCardViewModel {
    pub id: String,
    pub effective_locale: String,
    pub name: String,
    pub slug_badge: String,
    pub description: String,
    pub accent_style: String,
    pub topics_count_label: String,
    pub replies_count_label: String,
    pub icon_label: Option<String>,
    pub action_label: String,
    pub is_busy: bool,
}

#[derive(Clone, Debug)]
pub struct ForumAdminTopicRenderLabels {
    pub thread_path_template: String,
    pub opened: String,
    pub open_thread: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumAdminTopicCardViewModel {
    pub id: String,
    pub status: String,
    pub status_class: &'static str,
    pub effective_locale: String,
    pub pinned: bool,
    pub locked: bool,
    pub title: String,
    pub thread_path: String,
    pub reply_count: i32,
    pub action_label: String,
    pub is_busy: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumAdminCategorySidebarViewModel {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub topic_count: i32,
    pub is_active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumAdminReplyCardViewModel {
    pub status: String,
    pub status_class: &'static str,
    pub effective_locale: String,
    pub content_preview: String,
}

pub fn category_sidebar_view_model(
    item: &CategoryListItem,
    active_category_id: &str,
) -> ForumAdminCategorySidebarViewModel {
    ForumAdminCategorySidebarViewModel {
        id: item.id.clone(),
        name: item.name.clone(),
        slug: item.slug.clone(),
        topic_count: item.topic_count,
        is_active: active_category_id == item.id,
    }
}

pub fn category_sidebar_total_count(items: &[CategoryListItem]) -> usize {
    items.len()
}

pub fn reply_card_view_model(item: &ReplyListItem) -> ForumAdminReplyCardViewModel {
    ForumAdminReplyCardViewModel {
        status: item.status.clone(),
        status_class: topic_status_class(item.status.as_str()),
        effective_locale: item.effective_locale.clone(),
        content_preview: item.content_preview.clone(),
    }
}

pub fn category_card_view_model(
    item: &CategoryListItem,
    editing_id: Option<&str>,
    busy_key: Option<&str>,
    labels: &ForumAdminCategoryRenderLabels,
) -> ForumAdminCategoryCardViewModel {
    let is_editing = editing_id == Some(item.id.as_str());
    ForumAdminCategoryCardViewModel {
        id: item.id.clone(),
        effective_locale: item.effective_locale.clone(),
        name: item.name.clone(),
        slug_badge: format!("#{}", item.slug),
        description: item
            .description
            .clone()
            .unwrap_or_else(|| labels.no_description.clone()),
        accent_style: item
            .color
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!("background:{};", value))
            .unwrap_or_else(|| DEFAULT_CATEGORY_ACCENT_STYLE.to_string()),
        topics_count_label: render_count_label(&labels.topics_count_template, item.topic_count),
        replies_count_label: render_count_label(&labels.replies_count_template, item.reply_count),
        icon_label: item
            .icon
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| labels.icon_template.replace("{value}", value)),
        action_label: if is_editing {
            labels.editing.clone()
        } else {
            labels.edit.clone()
        },
        is_busy: item_busy(busy_key, item.id.as_str()),
    }
}

pub fn topic_card_view_model(
    item: &TopicListItem,
    editing_id: Option<&str>,
    busy_key: Option<&str>,
    labels: &ForumAdminTopicRenderLabels,
) -> ForumAdminTopicCardViewModel {
    let is_editing = editing_id == Some(item.id.as_str());
    ForumAdminTopicCardViewModel {
        id: item.id.clone(),
        status: item.status.clone(),
        status_class: topic_status_class(item.status.as_str()),
        effective_locale: item.effective_locale.clone(),
        pinned: item.is_pinned,
        locked: item.is_locked,
        title: item.title.clone(),
        thread_path: labels
            .thread_path_template
            .replace("{category}", item.category_id.as_str())
            .replace("{slug}", item.slug.as_str()),
        reply_count: item.reply_count,
        action_label: if is_editing {
            labels.opened.clone()
        } else {
            labels.open_thread.clone()
        },
        is_busy: item_busy(busy_key, item.id.as_str()),
    }
}

fn render_count_label(template: &str, value: i32) -> String {
    template.replace("{count}", value.to_string().as_str())
}

fn item_busy(busy_key: Option<&str>, item_id: &str) -> bool {
    busy_key
        .and_then(|value| value.rsplit(':').next())
        .map(|busy_item_id| busy_item_id == item_id)
        .unwrap_or(false)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForumAdminFormError {
    CategoryRequired,
    TopicRequired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryFormSnapshot {
    pub editing_id: Option<String>,
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub position: i32,
    pub moderated: bool,
}

impl CategoryFormSnapshot {
    pub fn blank(default_locale: impl Into<String>) -> Self {
        Self {
            editing_id: None,
            locale: default_locale.into(),
            name: String::new(),
            slug: String::new(),
            description: String::new(),
            icon: String::new(),
            color: String::new(),
            position: 0,
            moderated: false,
        }
    }

    pub fn from_detail(category: &CategoryDetail) -> Self {
        Self {
            editing_id: Some(category.id.clone()),
            locale: category.locale.clone(),
            name: category.name.clone(),
            slug: category.slug.clone(),
            description: category.description.clone().unwrap_or_default(),
            icon: category.icon.clone().unwrap_or_default(),
            color: category.color.clone().unwrap_or_default(),
            position: category.position,
            moderated: category.moderated,
        }
    }

    pub fn to_draft(&self) -> Result<CategoryDraft, ForumAdminFormError> {
        let draft = CategoryDraft {
            locale: self.locale.clone(),
            name: self.name.trim().to_string(),
            slug: self.slug.trim().to_string(),
            description: self.description.trim().to_string(),
            icon: self.icon.trim().to_string(),
            color: self.color.trim().to_string(),
            position: self.position,
            moderated: self.moderated,
        };
        if draft.name.is_empty() || draft.slug.is_empty() {
            return Err(ForumAdminFormError::CategoryRequired);
        }
        Ok(draft)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopicFormSnapshot {
    pub editing_id: Option<String>,
    pub locale: String,
    pub category_id: String,
    pub title: String,
    pub slug: String,
    pub body: String,
    pub body_format: String,
    pub tags_raw: String,
}

impl TopicFormSnapshot {
    pub fn blank(default_locale: impl Into<String>) -> Self {
        Self {
            editing_id: None,
            locale: default_locale.into(),
            category_id: String::new(),
            title: String::new(),
            slug: String::new(),
            body: String::new(),
            body_format: "markdown".to_string(),
            tags_raw: String::new(),
        }
    }

    pub fn from_detail(topic: &TopicDetail) -> Self {
        Self {
            editing_id: Some(topic.id.clone()),
            locale: topic.locale.clone(),
            category_id: topic.category_id.clone(),
            title: topic.title.clone(),
            slug: topic.slug.clone(),
            body: topic.body.clone(),
            body_format: topic.body_format.clone(),
            tags_raw: topic.tags.join(", "),
        }
    }

    pub fn to_draft(&self) -> Result<TopicDraft, ForumAdminFormError> {
        let draft = TopicDraft {
            locale: self.locale.clone(),
            category_id: self.category_id.trim().to_string(),
            title: self.title.trim().to_string(),
            slug: self.slug.trim().to_string(),
            body: self.body.trim().to_string(),
            body_format: self.body_format.trim().to_string(),
            tags: parse_tags(self.tags_raw.as_str()),
        };
        if draft.category_id.is_empty() || draft.title.is_empty() || draft.body.is_empty() {
            return Err(ForumAdminFormError::TopicRequired);
        }
        Ok(draft)
    }
}

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

    #[test]
    fn builds_category_form_snapshot_and_trimmed_draft() {
        let snapshot = CategoryFormSnapshot {
            editing_id: Some("category-1".to_string()),
            locale: "ru".to_string(),
            name: "  Общение  ".to_string(),
            slug: "  community  ".to_string(),
            description: "  Описание  ".to_string(),
            icon: "  chat  ".to_string(),
            color: "  #fff  ".to_string(),
            position: 3,
            moderated: true,
        };
        let draft = snapshot.to_draft().expect("valid category draft");
        assert_eq!(draft.name, "Общение");
        assert_eq!(draft.slug, "community");
        assert_eq!(draft.description, "Описание");
        assert_eq!(draft.icon, "chat");
        assert_eq!(draft.color, "#fff");
        assert_eq!(draft.position, 3);
        assert!(draft.moderated);
    }

    #[test]
    fn rejects_category_snapshot_without_required_fields() {
        let snapshot = CategoryFormSnapshot {
            name: " ".to_string(),
            slug: "category".to_string(),
            ..CategoryFormSnapshot::blank("en")
        };
        assert_eq!(
            snapshot.to_draft().unwrap_err(),
            ForumAdminFormError::CategoryRequired
        );
    }

    #[test]
    fn builds_topic_form_snapshot_and_trimmed_draft() {
        let snapshot = TopicFormSnapshot {
            editing_id: None,
            locale: "en".to_string(),
            category_id: "  cat-1  ".to_string(),
            title: "  Welcome  ".to_string(),
            slug: "  welcome  ".to_string(),
            body: "  Body  ".to_string(),
            body_format: "  markdown  ".to_string(),
            tags_raw: " rust, forum ,, ffa ".to_string(),
        };
        let draft = snapshot.to_draft().expect("valid topic draft");
        assert_eq!(draft.category_id, "cat-1");
        assert_eq!(draft.title, "Welcome");
        assert_eq!(draft.slug, "welcome");
        assert_eq!(draft.body, "Body");
        assert_eq!(draft.body_format, "markdown");
        assert_eq!(draft.tags, vec!["rust", "forum", "ffa"]);
    }

    #[test]
    fn rejects_topic_snapshot_without_required_fields() {
        let snapshot = TopicFormSnapshot {
            category_id: "cat-1".to_string(),
            title: " ".to_string(),
            body: "Body".to_string(),
            ..TopicFormSnapshot::blank("en")
        };
        assert_eq!(
            snapshot.to_draft().unwrap_err(),
            ForumAdminFormError::TopicRequired
        );
    }

    #[test]
    fn builds_category_card_view_model_with_labels_and_fallbacks() {
        let item = CategoryListItem {
            id: "category-1".to_string(),
            locale: "ru".to_string(),
            effective_locale: "ru".to_string(),
            name: "Community".to_string(),
            slug: "community".to_string(),
            description: None,
            icon: Some(" chat ".to_string()),
            color: Some(" ".to_string()),
            topic_count: 7,
            reply_count: 12,
        };
        let labels = ForumAdminCategoryRenderLabels {
            no_description: "No description".to_string(),
            topics_count_template: "topics: {count}".to_string(),
            replies_count_template: "replies: {count}".to_string(),
            icon_template: "icon: {value}".to_string(),
            editing: "Editing".to_string(),
            edit: "Edit".to_string(),
        };
        let vm = category_card_view_model(
            &item,
            Some("category-1"),
            Some("category:edit:category-1"),
            &labels,
        );
        assert_eq!(vm.slug_badge, "#community");
        assert_eq!(vm.description, "No description");
        assert_eq!(vm.topics_count_label, "topics: 7");
        assert_eq!(vm.replies_count_label, "replies: 12");
        assert_eq!(vm.icon_label.as_deref(), Some("icon:  chat "));
        assert_eq!(vm.action_label, "Editing");
        assert!(vm.is_busy);
        assert_eq!(vm.accent_style, DEFAULT_CATEGORY_ACCENT_STYLE);

        let other_item = CategoryListItem {
            id: "category-10".to_string(),
            ..item
        };
        let other_vm = category_card_view_model(
            &other_item,
            Some("category-10"),
            Some("category:edit:category-1"),
            &labels,
        );
        assert!(!other_vm.is_busy);
    }

    #[test]
    fn builds_topic_card_view_model_with_status_and_thread_path() {
        let item = TopicListItem {
            id: "topic-1".to_string(),
            locale: "en".to_string(),
            effective_locale: "en".to_string(),
            category_id: "category-1".to_string(),
            author_id: None,
            title: "Welcome".to_string(),
            slug: "welcome".to_string(),
            status: "PUBLISHED".to_string(),
            is_pinned: true,
            is_locked: false,
            reply_count: 4,
            created_at: "2026-06-08T00:00:00Z".to_string(),
        };
        let labels = ForumAdminTopicRenderLabels {
            thread_path_template: "thread/{category}/{slug}".to_string(),
            opened: "Opened".to_string(),
            open_thread: "Open thread".to_string(),
        };
        let vm = topic_card_view_model(&item, None, Some("topic:delete:topic-2"), &labels);
        assert_eq!(vm.status_class, "success");
        assert_eq!(vm.thread_path, "thread/category-1/welcome");
        assert_eq!(vm.action_label, "Open thread");
        assert!(!vm.is_busy);
        assert!(vm.pinned);
        assert!(!vm.locked);
    }

    #[test]
    fn builds_sidebar_category_view_model_and_total_count() {
        let items = vec![
            CategoryListItem {
                id: "category-1".to_string(),
                locale: "en".to_string(),
                effective_locale: "en".to_string(),
                name: "General".to_string(),
                slug: "general".to_string(),
                description: None,
                icon: None,
                color: None,
                topic_count: 5,
                reply_count: 9,
            },
            CategoryListItem {
                id: "category-2".to_string(),
                locale: "en".to_string(),
                effective_locale: "en".to_string(),
                name: "Support".to_string(),
                slug: "support".to_string(),
                description: None,
                icon: None,
                color: None,
                topic_count: 2,
                reply_count: 4,
            },
        ];
        let vm = category_sidebar_view_model(&items[1], "category-2");
        assert_eq!(category_sidebar_total_count(&items), 2);
        assert_eq!(vm.id, "category-2");
        assert_eq!(vm.name, "Support");
        assert_eq!(vm.slug, "support");
        assert_eq!(vm.topic_count, 2);
        assert!(vm.is_active);
    }

    #[test]
    fn builds_reply_card_view_model_with_status_class() {
        let item = ReplyListItem {
            id: "reply-1".to_string(),
            locale: "en".to_string(),
            effective_locale: "en".to_string(),
            topic_id: "topic-1".to_string(),
            author_id: None,
            content_preview: "Thanks for the update".to_string(),
            status: "pending".to_string(),
            parent_reply_id: None,
            created_at: "2026-06-08T00:00:00Z".to_string(),
        };
        let vm = reply_card_view_model(&item);
        assert_eq!(vm.status, "pending");
        assert_eq!(vm.status_class, "warning");
        assert_eq!(vm.effective_locale, "en");
        assert_eq!(vm.content_preview, "Thanks for the update");
    }
}
