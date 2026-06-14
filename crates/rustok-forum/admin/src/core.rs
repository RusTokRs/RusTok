use rustok_api::AdminQueryKey;

use crate::model::{
    CategoryDetail, CategoryDraft, CategoryListItem, ReplyListItem, TopicDetail, TopicDraft,
    TopicListItem,
};

const DEFAULT_CATEGORY_ACCENT_STYLE: &str =
    "background:linear-gradient(180deg,#0ea5e9 0%,#f59e0b 100%);";

#[derive(Clone, Debug)]
pub struct ForumAdminHeaderLabels {
    pub badge: String,
    pub categories_title: String,
    pub topics_title: String,
    pub categories_body: String,
    pub topics_body: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumAdminHeaderViewModel {
    pub badge: String,
    pub title: String,
    pub body: String,
}

pub fn forum_admin_header_view_model(
    is_categories_page: bool,
    labels: &ForumAdminHeaderLabels,
) -> ForumAdminHeaderViewModel {
    ForumAdminHeaderViewModel {
        badge: labels.badge.clone(),
        title: if is_categories_page {
            labels.categories_title.clone()
        } else {
            labels.topics_title.clone()
        },
        body: if is_categories_page {
            labels.categories_body.clone()
        } else {
            labels.topics_body.clone()
        },
    }
}

#[derive(Clone, Debug)]
pub struct ForumAdminTitleEnvelopeLabels {
    pub edit_title: String,
    pub create_title: String,
    pub active_badge: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumAdminTitleEnvelopeViewModel {
    pub title: String,
    pub active_badge: Option<String>,
}

pub fn forum_admin_title_envelope_view_model(
    is_editing: bool,
    labels: &ForumAdminTitleEnvelopeLabels,
) -> ForumAdminTitleEnvelopeViewModel {
    ForumAdminTitleEnvelopeViewModel {
        title: if is_editing {
            labels.edit_title.clone()
        } else {
            labels.create_title.clone()
        },
        active_badge: is_editing.then(|| labels.active_badge.clone()),
    }
}

#[derive(Clone, Debug)]
pub struct ForumAdminPlaceholderPolicy {
    pub locale: String,
    pub category_name: String,
    pub category_slug: String,
    pub category_description: String,
    pub category_icon: String,
    pub category_color: String,
    pub category_position: String,
    pub topic_title: String,
    pub topic_slug: String,
    pub topic_body_format: String,
    pub topic_tags: String,
    pub topic_body: String,
}

pub fn forum_admin_placeholder_policy(default_locale: &str) -> ForumAdminPlaceholderPolicy {
    ForumAdminPlaceholderPolicy {
        locale: if default_locale.trim().is_empty() {
            "en"
        } else {
            default_locale.trim()
        }
        .to_string(),
        category_name: "General discussion".to_string(),
        category_slug: "general-discussion".to_string(),
        category_description: "Space for announcements, introductions, and open questions."
            .to_string(),
        category_icon: "chat".to_string(),
        category_color: "#f59e0b".to_string(),
        category_position: "0".to_string(),
        topic_title: "How should we structure weekly updates?".to_string(),
        topic_slug: "weekly-updates-structure".to_string(),
        topic_body_format: "markdown".to_string(),
        topic_tags: "release, roadmap, updates".to_string(),
        topic_body: "Write the first post here...".to_string(),
    }
}

#[derive(Clone, Debug)]
pub struct ForumAdminSeoCopyLabels {
    pub title: String,
    pub subtitle: String,
    pub empty_message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForumAdminSeoSurface {
    Category,
    Topic,
}

pub fn forum_admin_seo_copy_labels(
    surface: ForumAdminSeoSurface,
    title: String,
    subtitle: String,
    empty_message: String,
) -> ForumAdminSeoCopyLabels {
    match surface {
        ForumAdminSeoSurface::Category | ForumAdminSeoSurface::Topic => ForumAdminSeoCopyLabels {
            title,
            subtitle,
            empty_message,
        },
    }
}

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumAdminCategorySelectOption {
    pub value: String,
    pub label: String,
    pub is_selected: bool,
}

pub fn category_select_options(
    items: &[CategoryListItem],
    selected_category_id: &str,
) -> Vec<ForumAdminCategorySelectOption> {
    items
        .iter()
        .map(|item| ForumAdminCategorySelectOption {
            value: item.id.clone(),
            label: item.name.clone(),
            is_selected: item.id == selected_category_id,
        })
        .collect()
}

pub fn forum_admin_topic_tag_count_label(tags_raw: &str, ready_template: &str) -> String {
    render_count_label(ready_template, parse_tags(tags_raw).len() as i32)
}

pub fn forum_admin_editing_thread_label(
    editing_topic_id: Option<&str>,
    open_inspector_label: &str,
    nothing_selected_label: &str,
) -> String {
    if editing_topic_id.is_some() {
        open_inspector_label.to_string()
    } else {
        nothing_selected_label.to_string()
    }
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

pub fn selected_category_filter_label(
    categories: Option<Result<Vec<CategoryListItem>, String>>,
    selected_id: &str,
    all_categories_label: &str,
    filtered_category_label: &str,
) -> String {
    if selected_id.trim().is_empty() {
        return all_categories_label.to_string();
    }

    match categories {
        Some(Ok(items)) => items
            .into_iter()
            .find(|item| item.id == selected_id)
            .map(|item| item.name)
            .unwrap_or_else(|| filtered_category_label.to_string()),
        _ => all_categories_label.to_string(),
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForumAdminBusyAction {
    Edit,
    Save,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForumAdminBusySurface {
    Category,
    Topic,
}

impl ForumAdminBusySurface {
    fn as_str(self) -> &'static str {
        match self {
            Self::Category => "category",
            Self::Topic => "topic",
        }
    }
}

impl ForumAdminBusyAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Edit => "edit",
            Self::Save => "save",
            Self::Delete => "delete",
        }
    }
}

pub fn forum_admin_busy_key(
    surface: ForumAdminBusySurface,
    action: ForumAdminBusyAction,
    item_id: Option<&str>,
) -> String {
    match item_id {
        Some(item_id) if !item_id.trim().is_empty() => {
            format!(
                "{}:{}:{}",
                surface.as_str(),
                action.as_str(),
                item_id.trim()
            )
        }
        _ => format!("{}:{}", surface.as_str(), action.as_str()),
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

#[derive(Clone, Debug)]
pub struct ForumAdminFormErrorLabels {
    pub category_required: String,
    pub topic_required: String,
}

pub fn forum_admin_form_error_message(
    error: ForumAdminFormError,
    labels: &ForumAdminFormErrorLabels,
) -> String {
    match error {
        ForumAdminFormError::CategoryRequired => labels.category_required.clone(),
        ForumAdminFormError::TopicRequired => labels.topic_required.clone(),
    }
}

pub fn forum_admin_transport_error_message(base: &str, err: impl std::fmt::Display) -> String {
    format!("{}: {err}", base.trim_end_matches(':'))
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForumAdminQuerySurface {
    Category,
    Topic,
}

impl ForumAdminQuerySurface {
    pub fn query_key(self) -> &'static str {
        match self {
            Self::Category => AdminQueryKey::CategoryId.as_str(),
            Self::Topic => AdminQueryKey::TopicId.as_str(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForumAdminRouteQueryOperation {
    Push,
    Replace,
    Clear,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumAdminRouteQueryIntent {
    pub operation: ForumAdminRouteQueryOperation,
    pub key: &'static str,
    pub value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumAdminDeleteOutcome {
    pub should_clear_form: bool,
    pub should_refresh: bool,
    pub route_query_intent: Option<ForumAdminRouteQueryIntent>,
}

pub fn forum_admin_open_query_intent(
    surface: ForumAdminQuerySurface,
    id: impl Into<String>,
) -> ForumAdminRouteQueryIntent {
    ForumAdminRouteQueryIntent {
        operation: ForumAdminRouteQueryOperation::Push,
        key: surface.query_key(),
        value: Some(id.into()),
    }
}

pub fn forum_admin_saved_query_intent(
    surface: ForumAdminQuerySurface,
    id: impl Into<String>,
) -> ForumAdminRouteQueryIntent {
    ForumAdminRouteQueryIntent {
        operation: ForumAdminRouteQueryOperation::Replace,
        key: surface.query_key(),
        value: Some(id.into()),
    }
}

pub fn forum_admin_reset_query_intent(
    surface: ForumAdminQuerySurface,
) -> ForumAdminRouteQueryIntent {
    ForumAdminRouteQueryIntent {
        operation: ForumAdminRouteQueryOperation::Clear,
        key: surface.query_key(),
        value: None,
    }
}

pub fn selected_query_id(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

pub fn deleted_selection_matches(current_id: Option<&str>, deleted_id: &str) -> bool {
    current_id == Some(deleted_id)
}

pub fn forum_admin_delete_outcome(
    surface: ForumAdminQuerySurface,
    current_id: Option<&str>,
    deleted_id: &str,
) -> ForumAdminDeleteOutcome {
    let should_clear_form = deleted_selection_matches(current_id, deleted_id);
    ForumAdminDeleteOutcome {
        should_clear_form,
        should_refresh: true,
        route_query_intent: should_clear_form.then(|| forum_admin_reset_query_intent(surface)),
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

pub fn forum_admin_tag_chips(raw: &str) -> Vec<String> {
    parse_tags(raw)
}

pub fn forum_admin_position_value(raw: &str) -> i32 {
    raw.trim().parse::<i32>().unwrap_or(0)
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

pub fn forum_admin_sidebar_category_class(is_active: bool) -> &'static str {
    if is_active {
        "flex w-full items-center justify-between gap-3 rounded-2xl border border-primary/30 bg-primary/10 px-3 py-3 text-left"
    } else {
        "flex w-full items-center justify-between gap-3 rounded-2xl border border-border bg-card px-3 py-3 text-left transition hover:bg-muted"
    }
}

pub fn forum_admin_status_badge_class(status_class: &'static str) -> &'static str {
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

#[derive(Clone, Debug)]
pub enum ForumAdminCollectionState<T> {
    Empty,
    Ready(Vec<T>),
    Error(String),
}

pub fn forum_admin_collection_state<T>(
    result: Result<Vec<T>, String>,
) -> ForumAdminCollectionState<T> {
    match result {
        Ok(items) if items.is_empty() => ForumAdminCollectionState::Empty,
        Ok(items) => ForumAdminCollectionState::Ready(items),
        Err(err) => ForumAdminCollectionState::Error(err),
    }
}

pub fn result_item_count<T>(result: Option<Result<Vec<T>, String>>) -> usize {
    match result {
        Some(Ok(items)) => items.len(),
        _ => 0,
    }
}

pub fn reply_count_label(replies: Option<Result<Vec<ReplyListItem>, String>>) -> usize {
    result_item_count(replies)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn category_item(id: &str, name: &str) -> CategoryListItem {
        CategoryListItem {
            id: id.to_string(),
            locale: "en".to_string(),
            effective_locale: "en".to_string(),
            name: name.to_string(),
            slug: name.to_ascii_lowercase(),
            description: None,
            icon: None,
            color: None,
            topic_count: 1,
            reply_count: 2,
        }
    }

    fn two_category_items() -> Vec<CategoryListItem> {
        vec![
            category_item("category-1", "General"),
            category_item("category-2", "Support"),
        ]
    }

    fn some_category_items() -> Result<Vec<CategoryListItem>, String> {
        Ok(two_category_items())
    }

    #[test]
    fn trims_topic_category_filter() {
        assert_eq!(
            topic_category_filter("  category-1  ".to_string()),
            Some("category-1".to_string())
        );
        assert_eq!(topic_category_filter("   ".to_string()), None);
    }

    #[test]
    fn builds_route_query_intents_for_admin_surfaces() {
        assert_eq!(
            forum_admin_open_query_intent(ForumAdminQuerySurface::Category, "category-1"),
            ForumAdminRouteQueryIntent {
                operation: ForumAdminRouteQueryOperation::Push,
                key: AdminQueryKey::CategoryId.as_str(),
                value: Some("category-1".to_string()),
            }
        );
        assert_eq!(
            forum_admin_saved_query_intent(ForumAdminQuerySurface::Topic, "topic-1"),
            ForumAdminRouteQueryIntent {
                operation: ForumAdminRouteQueryOperation::Replace,
                key: AdminQueryKey::TopicId.as_str(),
                value: Some("topic-1".to_string()),
            }
        );
        assert_eq!(
            forum_admin_reset_query_intent(ForumAdminQuerySurface::Category),
            ForumAdminRouteQueryIntent {
                operation: ForumAdminRouteQueryOperation::Clear,
                key: AdminQueryKey::CategoryId.as_str(),
                value: None,
            }
        );
    }

    #[test]
    fn normalizes_selected_query_ids_and_deleted_selection_match() {
        assert_eq!(
            selected_query_id(Some("  topic-1  ".to_string())),
            Some("topic-1".to_string())
        );
        assert_eq!(selected_query_id(Some("   ".to_string())), None);
        assert_eq!(selected_query_id(None), None);
        assert!(deleted_selection_matches(Some("topic-1"), "topic-1"));
        assert!(!deleted_selection_matches(Some("topic-10"), "topic-1"));
        assert!(!deleted_selection_matches(None, "topic-1"));
    }

    #[test]
    fn builds_delete_outcomes_for_selected_and_non_selected_items() {
        assert_eq!(
            forum_admin_delete_outcome(ForumAdminQuerySurface::Topic, Some("topic-1"), "topic-1"),
            ForumAdminDeleteOutcome {
                should_clear_form: true,
                should_refresh: true,
                route_query_intent: Some(forum_admin_reset_query_intent(
                    ForumAdminQuerySurface::Topic
                )),
            }
        );
        assert_eq!(
            forum_admin_delete_outcome(
                ForumAdminQuerySurface::Category,
                Some("category-2"),
                "category-1"
            ),
            ForumAdminDeleteOutcome {
                should_clear_form: false,
                should_refresh: true,
                route_query_intent: None,
            }
        );
    }

    #[test]
    fn parses_comma_separated_tags_without_empty_values() {
        assert_eq!(
            parse_tags(" rust, forum ,, ffa "),
            vec!["rust", "forum", "ffa"]
        );
        assert_eq!(
            forum_admin_tag_chips(" rust, forum ,, ffa "),
            vec!["rust", "forum", "ffa"]
        );
        assert_eq!(forum_admin_position_value(" 42 "), 42);
        assert_eq!(forum_admin_position_value("not-a-number"), 0);
    }

    #[test]
    fn maps_topic_status_to_stable_class_keys() {
        assert_eq!(topic_status_class("PUBLISHED"), "success");
        assert_eq!(topic_status_class("pending"), "warning");
        assert_eq!(topic_status_class("closed"), "muted");
        assert_eq!(topic_status_class("other"), "default");
    }

    #[test]
    fn maps_status_and_sidebar_classes_without_leptos() {
        assert!(forum_admin_sidebar_category_class(true).contains("border-primary/30"));
        assert!(forum_admin_sidebar_category_class(false).contains("hover:bg-muted"));
        assert!(forum_admin_status_badge_class("success").contains("emerald"));
        assert!(forum_admin_status_badge_class("warning").contains("amber"));
        assert!(forum_admin_status_badge_class("muted").contains("bg-muted"));
        assert!(forum_admin_status_badge_class("default").contains("border-border"));
    }

    #[test]
    fn builds_typed_busy_keys_for_admin_surfaces() {
        assert_eq!(
            forum_admin_busy_key(
                ForumAdminBusySurface::Category,
                ForumAdminBusyAction::Edit,
                Some(" category-1 "),
            ),
            "category:edit:category-1"
        );
        assert_eq!(
            forum_admin_busy_key(
                ForumAdminBusySurface::Topic,
                ForumAdminBusyAction::Delete,
                Some("topic-1"),
            ),
            "topic:delete:topic-1"
        );
        assert_eq!(
            forum_admin_busy_key(
                ForumAdminBusySurface::Category,
                ForumAdminBusyAction::Save,
                None,
            ),
            "category:save"
        );
    }

    #[test]
    fn maps_form_and_transport_error_messages() {
        let labels = ForumAdminFormErrorLabels {
            category_required: "Category required".to_string(),
            topic_required: "Topic required".to_string(),
        };

        assert_eq!(
            forum_admin_form_error_message(ForumAdminFormError::CategoryRequired, &labels),
            "Category required"
        );
        assert_eq!(
            forum_admin_form_error_message(ForumAdminFormError::TopicRequired, &labels),
            "Topic required"
        );
        assert_eq!(
            forum_admin_transport_error_message("Failed to save category", "boom"),
            "Failed to save category: boom"
        );
        assert_eq!(
            forum_admin_transport_error_message("Failed to save category:", "boom"),
            "Failed to save category: boom"
        );
    }

    #[test]
    fn builds_title_envelopes_placeholders_and_seo_copy() {
        let labels = ForumAdminTitleEnvelopeLabels {
            edit_title: "Edit category".to_string(),
            create_title: "Create category".to_string(),
            active_badge: "Live edit".to_string(),
        };

        assert_eq!(
            forum_admin_title_envelope_view_model(true, &labels),
            ForumAdminTitleEnvelopeViewModel {
                title: "Edit category".to_string(),
                active_badge: Some("Live edit".to_string()),
            }
        );
        assert_eq!(
            forum_admin_title_envelope_view_model(false, &labels),
            ForumAdminTitleEnvelopeViewModel {
                title: "Create category".to_string(),
                active_badge: None,
            }
        );

        let placeholders = forum_admin_placeholder_policy(" ru ");
        assert_eq!(placeholders.locale, "ru");
        assert_eq!(placeholders.category_slug, "general-discussion");
        assert_eq!(placeholders.topic_body_format, "markdown");

        let default_placeholders = forum_admin_placeholder_policy("   ");
        assert_eq!(default_placeholders.locale, "en");

        let seo_copy = forum_admin_seo_copy_labels(
            ForumAdminSeoSurface::Topic,
            "Topic SEO".to_string(),
            "Diagnostics".to_string(),
            "Open a topic first".to_string(),
        );
        assert_eq!(seo_copy.title, "Topic SEO");
        assert_eq!(seo_copy.subtitle, "Diagnostics");
        assert_eq!(seo_copy.empty_message, "Open a topic first");
    }

    #[test]
    fn selects_header_copy_for_categories_and_topics() {
        let labels = ForumAdminHeaderLabels {
            badge: "forum control room".to_string(),
            categories_title: "Category architecture".to_string(),
            topics_title: "Moderation workspace".to_string(),
            categories_body: "Shape navigation clusters".to_string(),
            topics_body: "Review topic flow".to_string(),
        };

        let categories = forum_admin_header_view_model(true, &labels);
        assert_eq!(categories.badge, "forum control room");
        assert_eq!(categories.title, "Category architecture");
        assert_eq!(categories.body, "Shape navigation clusters");

        let topics = forum_admin_header_view_model(false, &labels);
        assert_eq!(topics.title, "Moderation workspace");
        assert_eq!(topics.body, "Review topic flow");
    }

    #[test]
    fn resolves_selected_category_filter_label() {
        assert_eq!(
            selected_category_filter_label(
                some_category_items(),
                "category-2",
                "All categories",
                "Filtered category",
            ),
            "Support"
        );
        assert_eq!(
            selected_category_filter_label(
                some_category_items(),
                "missing",
                "All categories",
                "Filtered category",
            ),
            "Filtered category"
        );
        assert_eq!(
            selected_category_filter_label(
                some_category_items(),
                "  ",
                "All categories",
                "Filtered category",
            ),
            "All categories"
        );
        assert_eq!(
            selected_category_filter_label(
                Some(Err("boom".to_string())),
                "category-2",
                "All categories",
                "Filtered category",
            ),
            "All categories"
        );
    }

    #[test]
    fn classifies_collection_state_for_empty_ready_and_error() {
        assert!(matches!(
            forum_admin_collection_state::<CategoryListItem>(Ok(Vec::new())),
            ForumAdminCollectionState::Empty
        ));
        match forum_admin_collection_state(some_category_items()) {
            ForumAdminCollectionState::Ready(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].id, "category-1");
                assert_eq!(items[1].id, "category-2");
            }
            _ => panic!("expected ready collection state"),
        }
        match forum_admin_collection_state::<CategoryListItem>(Err("boom".to_string())) {
            ForumAdminCollectionState::Error(err) => assert_eq!(err, "boom"),
            _ => panic!("expected error collection state"),
        }
    }

    #[test]
    fn counts_loaded_result_items_only() {
        assert_eq!(result_item_count::<CategoryListItem>(None), 0);
        assert_eq!(
            result_item_count::<CategoryListItem>(Some(Err("boom".to_string()))),
            0
        );
        assert_eq!(
            result_item_count(Some(Ok(vec![
                CategoryListItem {
                    id: "category-1".to_string(),
                    locale: "en".to_string(),
                    effective_locale: "en".to_string(),
                    name: "General".to_string(),
                    slug: "general".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    topic_count: 1,
                    reply_count: 2,
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
                    topic_count: 3,
                    reply_count: 4,
                },
            ]))),
            2
        );
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
    fn builds_category_select_options_and_topic_sidebar_labels() {
        let options = category_select_options(&two_category_items(), "category-2");
        assert_eq!(
            options,
            vec![
                ForumAdminCategorySelectOption {
                    value: "category-1".to_string(),
                    label: "General".to_string(),
                    is_selected: false,
                },
                ForumAdminCategorySelectOption {
                    value: "category-2".to_string(),
                    label: "Support".to_string(),
                    is_selected: true,
                },
            ]
        );
        assert_eq!(
            forum_admin_topic_tag_count_label(" rust, forum ,, ffa ", "{count} ready"),
            "3 ready"
        );
        assert_eq!(
            forum_admin_editing_thread_label(
                Some("topic-1"),
                "Open in inspector",
                "Nothing selected",
            ),
            "Open in inspector"
        );
        assert_eq!(
            forum_admin_editing_thread_label(None, "Open in inspector", "Nothing selected"),
            "Nothing selected"
        );
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
