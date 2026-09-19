use rustok_api::{RichTextDocument, RichTextNode, WritePathIssue, WritePathIssueKind};
pub use rustok_ui_core::normalize_ui_text as optional_text;
use rustok_ui_core::{
    AdminQueryKey, UiRouteQueryIntent, parse_ui_csv, ui_busy_key, ui_busy_key_last_segment_matches,
    ui_busy_key_matches_action, ui_busy_key_with_id,
};

use crate::model::{BlogPostDetail, BlogPostDraft, BlogPostList, BlogPostListItem};

pub type BlogPostAdminRouteQueryIntent = UiRouteQueryIntent;

pub fn blog_post_admin_open_post_query_intent(post_id: String) -> BlogPostAdminRouteQueryIntent {
    UiRouteQueryIntent::push(AdminQueryKey::PostId.as_str(), post_id)
}

pub fn blog_post_admin_saved_post_query_intent(post_id: String) -> BlogPostAdminRouteQueryIntent {
    UiRouteQueryIntent::replace(AdminQueryKey::PostId.as_str(), post_id)
}

pub fn blog_post_admin_clear_post_query_intent() -> BlogPostAdminRouteQueryIntent {
    UiRouteQueryIntent::clear(AdminQueryKey::PostId.as_str())
}

pub fn parse_tags(raw: &str) -> Vec<String> {
    parse_ui_csv(raw)
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
    ui_busy_key_with_id("edit", post_id)
}

pub fn busy_key_for_save(post_id: Option<&str>) -> String {
    post_id
        .map(|id| ui_busy_key_with_id("save", id))
        .unwrap_or_else(|| ui_busy_key("create"))
}

pub fn busy_key_for_publish(post_id: &str) -> String {
    ui_busy_key_with_id("publish", post_id)
}

pub fn busy_key_for_archive(post_id: &str) -> String {
    ui_busy_key_with_id("archive", post_id)
}

pub fn busy_key_for_restore(post_id: &str) -> String {
    ui_busy_key_with_id("restore", post_id)
}

pub fn busy_key_for_delete(post_id: &str) -> String {
    ui_busy_key_with_id("delete", post_id)
}

pub fn is_save_busy(busy_key: Option<&str>) -> bool {
    busy_key == Some("create") || ui_busy_key_matches_action(busy_key, "save")
}

pub fn label_with_id(template: &str, id: &str) -> String {
    template.replace("{id}", id)
}

pub fn label_with_optional_id(template: &str, id: Option<&str>) -> String {
    id.map(|value| label_with_id(template, value))
        .unwrap_or_default()
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminStatusBadgeViewModel {
    pub status: String,
    pub class: String,
}

pub fn blog_post_admin_status_badge_view(status: &str) -> BlogPostAdminStatusBadgeViewModel {
    BlogPostAdminStatusBadgeViewModel {
        status: status.to_string(),
        class: status_badge_css(status),
    }
}

pub fn has_non_empty_text(value: &str) -> bool {
    !value.trim().is_empty()
}

pub fn should_autofill_slug(current_slug: &str) -> bool {
    !has_non_empty_text(current_slug)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminTitleInputViewModel {
    pub title: String,
    pub slug_update: Option<String>,
}

pub fn blog_post_admin_title_input_view(
    title: String,
    current_slug: &str,
) -> BlogPostAdminTitleInputViewModel {
    BlogPostAdminTitleInputViewModel {
        slug_update: should_autofill_slug(current_slug).then(|| slugify(title.as_str())),
        title,
    }
}

pub fn loadable_post_id(post_id: Option<&str>) -> Option<String> {
    post_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn selected_post_request(
    post_id: Option<&str>,
    requested_locale: &str,
) -> Option<(String, String)> {
    loadable_post_id(post_id).map(|id| (id, requested_locale.to_string()))
}

pub fn trimmed_text(value: &str) -> String {
    optional_text(value).unwrap_or_default()
}

pub fn fallback_post_slug(value: Option<String>, fallback: &str) -> String {
    value.unwrap_or_else(|| fallback.to_string())
}

pub fn fallback_post_excerpt(value: Option<String>, fallback: &str) -> String {
    value.unwrap_or_else(|| fallback.to_string())
}

pub fn optional_text_or_default(value: Option<String>) -> String {
    value.unwrap_or_default()
}

pub fn tags_input_value(tags: &[String]) -> String {
    tags.join(", ")
}

pub fn row_is_busy_for_post(busy_key: Option<&str>, post_id: &str) -> bool {
    ui_busy_key_last_segment_matches(busy_key, post_id)
}

pub fn is_editing_post(editing_post_id: Option<&str>, post_id: &str) -> bool {
    editing_post_id == Some(post_id)
}

pub fn should_reset_form_after_delete(
    editing_post_id: Option<&str>,
    deleted_post_id: &str,
) -> bool {
    is_editing_post(editing_post_id, deleted_post_id)
}

pub fn is_editing_mode(editing_post_id: Option<&str>) -> bool {
    editing_post_id.is_some()
}

pub fn editing_post_id_if_editing_mode(editing_post_id: Option<String>) -> Option<String> {
    if is_editing_mode(editing_post_id.as_deref()) {
        editing_post_id
    } else {
        None
    }
}

pub fn edit_action_label(is_editing: bool, editing_label: String, edit_label: String) -> String {
    if is_editing {
        editing_label
    } else {
        edit_label
    }
}

pub fn publish_action_label(
    is_published: bool,
    unpublish_label: String,
    publish_label: String,
) -> String {
    if is_published {
        unpublish_label
    } else {
        publish_label
    }
}

pub fn should_show_archive_action(is_published: bool) -> bool {
    is_published
}

pub fn should_show_publish_action(is_archived: bool) -> bool {
    !is_archived
}

pub fn should_show_restore_action(is_archived: bool) -> bool {
    is_archived
}

pub fn next_publish_state(is_published: bool) -> bool {
    !is_published
}

pub fn should_publish_now(publish: bool) -> bool {
    publish
}

pub fn locale_arg(locale: &str) -> Option<String> {
    Some(locale.to_string())
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlogPostFormInput<'a> {
    pub locale: &'a str,
    pub version: Option<i32>,
    pub title: &'a str,
    pub slug: &'a str,
    pub excerpt: &'a str,
    pub content: &'a RichTextDocument,
    pub publish: bool,
    pub tags: &'a str,
}

pub fn build_blog_post_draft(input: BlogPostFormInput<'_>) -> BlogPostDraft {
    BlogPostDraft {
        locale: trimmed_text(input.locale),
        version: input.version,
        title: trimmed_text(input.title),
        slug: trimmed_text(input.slug),
        excerpt: trimmed_text(input.excerpt),
        content: input.content.clone(),
        publish: input.publish,
        tags: parse_tags(input.tags),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlogPostEditorFormState {
    pub editing_post_id: Option<String>,
    pub version: Option<i32>,
    pub title: String,
    pub slug: String,
    pub excerpt: String,
    pub content: RichTextDocument,
    pub locale: String,
    pub tags_input: String,
    pub publish_now: bool,
}

impl BlogPostEditorFormState {
    pub fn empty(default_locale: &str) -> Self {
        Self {
            editing_post_id: None,
            version: None,
            title: String::new(),
            slug: String::new(),
            excerpt: String::new(),
            content: RichTextDocument::empty(),
            locale: default_locale.to_string(),
            tags_input: String::new(),
            publish_now: false,
        }
    }

    pub fn from_post(post: &BlogPostDetail) -> Self {
        Self {
            editing_post_id: Some(post.id.clone()),
            version: Some(post.version),
            title: post.title.clone(),
            slug: optional_text_or_default(post.slug.clone()),
            excerpt: optional_text_or_default(post.excerpt.clone()),
            content: post
                .content
                .as_ref()
                .map(|view| view.document.clone())
                .unwrap_or_default(),
            locale: post.requested_locale.clone(),
            tags_input: tags_input_value(post.tags.as_slice()),
            publish_now: is_published_status(post.status.as_str()),
        }
    }
}

pub fn has_required_draft_fields(title: &str, content: &RichTextDocument) -> bool {
    !title.is_empty() && document_has_text(content)
}

fn document_has_text(document: &RichTextDocument) -> bool {
    fn node_has_text(node: &RichTextNode) -> bool {
        node.text
            .as_deref()
            .is_some_and(|text| !text.trim().is_empty())
            || node.content.iter().any(node_has_text)
    }

    document.content.iter().any(node_has_text)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlogPostSaveOperation {
    Create,
    Update { post_id: String },
}

#[derive(Debug, Clone)]
pub struct BlogPostSaveCommand {
    pub operation: BlogPostSaveOperation,
    pub draft: BlogPostDraft,
    pub busy_key: String,
}

pub fn prepare_blog_post_save_command(
    editing_post_id: Option<String>,
    draft: BlogPostDraft,
    required_fields_message: String,
) -> Result<BlogPostSaveCommand, WritePathIssue> {
    if !has_required_draft_fields(draft.title.as_str(), &draft.content) {
        return Err(WritePathIssue::new(required_fields_message));
    }

    let busy_key = busy_key_for_save(editing_post_id.as_deref());
    let operation = match editing_post_id_if_editing_mode(editing_post_id) {
        Some(post_id) => {
            if draft.version.is_none() {
                return Err(WritePathIssue::new(
                    "The Blog post revision is missing; reload before saving",
                ));
            }
            BlogPostSaveOperation::Update { post_id }
        }
        None => BlogPostSaveOperation::Create,
    };

    Ok(BlogPostSaveCommand {
        operation,
        draft,
        busy_key,
    })
}
