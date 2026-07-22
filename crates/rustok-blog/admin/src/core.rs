use rustok_api::{WritePathIssue, WritePathIssueKind};
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

#[allow(dead_code)]
pub fn has_issue(issue: Option<WritePathIssueKind>) -> bool {
    issue.is_some()
}

#[allow(dead_code)]
pub fn issue_kind(issue: Option<&WritePathIssue>) -> Option<WritePathIssueKind> {
    issue.map(|value| value.kind)
}

#[allow(dead_code)]
pub fn has_items<T>(items: &[T]) -> bool {
    !items.is_empty()
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

pub fn should_show_archive_action(is_archived: bool) -> bool {
    !is_archived
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlogPostFormInput<'a> {
    pub locale: &'a str,
    pub title: &'a str,
    pub slug: &'a str,
    pub excerpt: &'a str,
    pub body: &'a str,
    pub body_format: &'a str,
    pub publish: bool,
    pub tags: &'a str,
}

pub fn build_blog_post_draft(input: BlogPostFormInput<'_>) -> BlogPostDraft {
    BlogPostDraft {
        locale: trimmed_text(input.locale),
        title: trimmed_text(input.title),
        slug: trimmed_text(input.slug),
        excerpt: trimmed_text(input.excerpt),
        body: trimmed_text(input.body),
        body_format: input.body_format.to_string(),
        publish: input.publish,
        tags: parse_tags(input.tags),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostEditorFormState {
    pub editing_post_id: Option<String>,
    pub title: String,
    pub slug: String,
    pub excerpt: String,
    pub body: String,
    pub locale: String,
    pub body_format: String,
    pub tags_input: String,
    pub publish_now: bool,
}

impl BlogPostEditorFormState {
    pub fn empty(default_locale: &str) -> Self {
        Self {
            editing_post_id: None,
            title: String::new(),
            slug: String::new(),
            excerpt: String::new(),
            body: String::new(),
            locale: default_locale.to_string(),
            body_format: "markdown".to_string(),
            tags_input: String::new(),
            publish_now: false,
        }
    }

    pub fn from_post(post: &BlogPostDetail) -> Self {
        Self {
            editing_post_id: Some(post.id.clone()),
            title: post.title.clone(),
            slug: optional_text_or_default(post.slug.clone()),
            excerpt: optional_text_or_default(post.excerpt.clone()),
            body: optional_text_or_default(post.body.clone()),
            locale: post.requested_locale.clone(),
            body_format: post.body_format.clone(),
            tags_input: tags_input_value(post.tags.as_slice()),
            publish_now: is_published_status(post.status.as_str()),
        }
    }
}

pub fn has_required_draft_fields(title: &str, body: &str) -> bool {
    !title.is_empty() && !body.is_empty()
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
    if !has_required_draft_fields(draft.title.as_str(), draft.body.as_str()) {
        return Err(WritePathIssue::new(required_fields_message));
    }

    let busy_key = busy_key_for_save(editing_post_id.as_deref());
    let operation = match editing_post_id_if_editing_mode(editing_post_id) {
        Some(post_id) => BlogPostSaveOperation::Update { post_id },
        None => BlogPostSaveOperation::Create,
    };

    Ok(BlogPostSaveCommand {
        operation,
        draft,
        busy_key,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminTableRowViewModel {
    pub post_id: String,
    pub title: String,
    pub slug: String,
    pub excerpt: String,
    pub status: String,
    pub locale: String,
    pub is_editing: bool,
    pub is_busy: bool,
    pub is_published: bool,
    pub is_archived: bool,
    pub next_publish_state: bool,
    pub show_archive_action: bool,
    pub edit_label: String,
    pub publish_label: String,
    pub archive_label: String,
    pub delete_label: String,
}

#[derive(Clone, Copy)]
pub struct BlogPostAdminTableRowLabels<'a> {
    pub draft_slug: &'a str,
    pub no_excerpt: &'a str,
    pub editing: &'a str,
    pub edit: &'a str,
    pub unpublish: &'a str,
    pub publish: &'a str,
    pub archive: &'a str,
    pub delete: &'a str,
}

pub fn blog_post_admin_table_row_view(
    post: BlogPostListItem,
    editing_post_id: Option<&str>,
    busy_key: Option<&str>,
    labels: BlogPostAdminTableRowLabels<'_>,
) -> BlogPostAdminTableRowViewModel {
    let post_id = post.id;
    let is_editing = is_editing_post(editing_post_id, post_id.as_str());
    let is_busy = row_is_busy_for_post(busy_key, post_id.as_str());
    let is_published = is_published_status(post.status.as_str());
    let is_archived = is_archived_status(post.status.as_str());
    let show_archive_action = should_show_archive_action(is_archived);

    BlogPostAdminTableRowViewModel {
        post_id,
        title: post.title,
        slug: fallback_post_slug(post.slug, labels.draft_slug),
        excerpt: fallback_post_excerpt(post.excerpt, labels.no_excerpt),
        status: post.status,
        locale: post.effective_locale,
        is_editing,
        is_busy,
        is_published,
        is_archived,
        next_publish_state: next_publish_state(is_published),
        show_archive_action,
        edit_label: edit_action_label(
            is_editing,
            labels.editing.to_string(),
            labels.edit.to_string(),
        ),
        publish_label: publish_action_label(
            is_published,
            labels.unpublish.to_string(),
            labels.publish.to_string(),
        ),
        archive_label: labels.archive.to_string(),
        delete_label: labels.delete.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminTableLabels {
    pub empty_message: String,
    pub total_label: String,
    pub title_header: String,
    pub slug_header: String,
    pub status_header: String,
    pub locale_header: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminTableViewModel {
    pub is_empty: bool,
    pub total_label: String,
    pub empty_message: String,
    pub title_header: String,
    pub slug_header: String,
    pub status_header: String,
    pub locale_header: String,
}

pub fn blog_post_admin_table_view(
    item_count: usize,
    total: u64,
    labels: BlogPostAdminTableLabels,
) -> BlogPostAdminTableViewModel {
    BlogPostAdminTableViewModel {
        is_empty: item_count == 0,
        total_label: count_label(labels.total_label.as_str(), total),
        empty_message: labels.empty_message,
        title_header: labels.title_header,
        slug_header: labels.slug_header,
        status_header: labels.status_header,
        locale_header: labels.locale_header,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminFormLabels {
    pub edit_title: String,
    pub create_title: String,
    pub saving: String,
    pub update: String,
    pub create: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminFormViewModel {
    pub title: String,
    pub submit_label: String,
    pub submit_disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminPostsTableViewModel {
    pub is_empty: bool,
    pub total_label: String,
    pub empty_message: String,
    pub title_header: String,
    pub slug_header: String,
    pub status_header: String,
    pub locale_header: String,
    pub rows: Vec<BlogPostAdminTableRowViewModel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminPostsTableLabels {
    pub empty_message: String,
    pub total_label: String,
    pub title_header: String,
    pub slug_header: String,
    pub status_header: String,
    pub locale_header: String,
    pub draft_slug: String,
    pub no_excerpt: String,
    pub editing: String,
    pub edit: String,
    pub unpublish: String,
    pub publish: String,
    pub archive: String,
    pub delete: String,
}

pub fn blog_post_admin_posts_table_view_from_items(
    items: Vec<BlogPostListItem>,
    total: u64,
    editing_post_id: Option<&str>,
    busy_key: Option<&str>,
    labels: BlogPostAdminPostsTableLabels,
) -> BlogPostAdminPostsTableViewModel {
    let table = blog_post_admin_table_view(
        items.len(),
        total,
        BlogPostAdminTableLabels {
            empty_message: labels.empty_message,
            total_label: labels.total_label,
            title_header: labels.title_header,
            slug_header: labels.slug_header,
            status_header: labels.status_header,
            locale_header: labels.locale_header,
        },
    );
    let row_labels = BlogPostAdminTableRowLabels {
        draft_slug: labels.draft_slug.as_str(),
        no_excerpt: labels.no_excerpt.as_str(),
        editing: labels.editing.as_str(),
        edit: labels.edit.as_str(),
        unpublish: labels.unpublish.as_str(),
        publish: labels.publish.as_str(),
        archive: labels.archive.as_str(),
        delete: labels.delete.as_str(),
    };
    let rows = items
        .into_iter()
        .map(|post| blog_post_admin_table_row_view(post, editing_post_id, busy_key, row_labels))
        .collect();

    BlogPostAdminPostsTableViewModel {
        is_empty: table.is_empty,
        total_label: table.total_label,
        empty_message: table.empty_message,
        title_header: table.title_header,
        slug_header: table.slug_header,
        status_header: table.status_header,
        locale_header: table.locale_header,
        rows,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminEditorFormCopyViewModel {
    pub subtitle: String,
    pub title_label: String,
    pub slug_label: String,
    pub locale_label: String,
    pub body_format_label: String,
    pub excerpt_label: String,
    pub body_label: String,
    pub tags_label: String,
    pub tags_placeholder: String,
    pub publish_now_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminEditorFormCopyLabels {
    pub subtitle: String,
    pub title_label: String,
    pub slug_label: String,
    pub locale_label: String,
    pub body_format_label: String,
    pub excerpt_label: String,
    pub body_label: String,
    pub tags_label: String,
    pub tags_placeholder: String,
    pub publish_now_label: String,
}

pub fn blog_post_admin_editor_form_copy_view(
    labels: BlogPostAdminEditorFormCopyLabels,
) -> BlogPostAdminEditorFormCopyViewModel {
    BlogPostAdminEditorFormCopyViewModel {
        subtitle: labels.subtitle,
        title_label: labels.title_label,
        slug_label: labels.slug_label,
        locale_label: labels.locale_label,
        body_format_label: labels.body_format_label,
        excerpt_label: labels.excerpt_label,
        body_label: labels.body_label,
        tags_label: labels.tags_label,
        tags_placeholder: labels.tags_placeholder,
        publish_now_label: labels.publish_now_label,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlogPostAdminEditorFieldClassesViewModel {
    pub text_input: &'static str,
    pub textarea_short: &'static str,
    pub textarea_long: &'static str,
    pub label_text: &'static str,
    pub checkbox_label: &'static str,
    pub submit_button: &'static str,
}

pub fn blog_post_admin_editor_field_classes_view() -> BlogPostAdminEditorFieldClassesViewModel {
    BlogPostAdminEditorFieldClassesViewModel {
        text_input: "w-full rounded-lg border border-input bg-background px-3 py-2 text-sm",
        textarea_short: "min-h-24 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm",
        textarea_long: "min-h-48 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm",
        label_text: "text-sm font-medium text-card-foreground",
        checkbox_label: "flex items-center gap-2 text-sm text-card-foreground",
        submit_button: "inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlogPostAdminTableClassesViewModel {
    pub empty_state: &'static str,
    pub total_label: &'static str,
    pub table_container: &'static str,
    pub table: &'static str,
    pub table_head: &'static str,
    pub header_cell: &'static str,
    pub actions_header_cell: &'static str,
    pub table_body: &'static str,
    pub row: &'static str,
    pub title_cell: &'static str,
    pub title_text: &'static str,
    pub excerpt_text: &'static str,
    pub muted_cell: &'static str,
    pub actions_cell: &'static str,
    pub actions_group: &'static str,
    pub primary_action_button: &'static str,
    pub destructive_action_button: &'static str,
}

pub fn blog_post_admin_table_classes_view() -> BlogPostAdminTableClassesViewModel {
    BlogPostAdminTableClassesViewModel {
        empty_state: "rounded-xl border border-dashed border-border p-12 text-center",
        total_label: "text-sm text-muted-foreground",
        table_container: "overflow-hidden rounded-xl border border-border",
        table: "w-full text-sm",
        table_head: "border-b border-border bg-muted/50",
        header_cell: "px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground",
        actions_header_cell: "px-4 py-3",
        table_body: "divide-y divide-border",
        row: "transition-colors hover:bg-muted/30",
        title_cell: "px-4 py-3 align-top",
        title_text: "font-medium text-foreground",
        excerpt_text: "mt-1 text-xs text-muted-foreground",
        muted_cell: "px-4 py-3 align-top text-xs text-muted-foreground",
        actions_cell: "px-4 py-3 align-top text-right",
        actions_group: "flex flex-wrap justify-end gap-2",
        primary_action_button: "text-xs font-medium text-primary hover:underline",
        destructive_action_button: "text-xs font-medium text-destructive hover:underline",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlogPostAdminShellClassesViewModel {
    pub page: &'static str,
    pub header: &'static str,
    pub header_content: &'static str,
    pub badge: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub layout: &'static str,
    pub list_card: &'static str,
    pub list_header: &'static str,
    pub list_header_copy: &'static str,
    pub list_title: &'static str,
    pub list_subtitle: &'static str,
    pub locale_filter_label: &'static str,
    pub locale_filter_text: &'static str,
    pub locale_filter_input: &'static str,
    pub sidebar: &'static str,
    pub form_card: &'static str,
    pub form_header: &'static str,
    pub form_title: &'static str,
    pub form_subtitle: &'static str,
    pub load_error: &'static str,
    pub skeleton_stack: &'static str,
    pub skeleton_row: &'static str,
}

pub fn blog_post_admin_shell_classes_view() -> BlogPostAdminShellClassesViewModel {
    BlogPostAdminShellClassesViewModel {
        page: "space-y-6",
        header: "flex flex-col gap-4 rounded-2xl border border-border bg-card p-6 shadow-sm lg:flex-row lg:items-start lg:justify-between",
        header_content: "space-y-2",
        badge: "inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground",
        title: "text-2xl font-semibold text-card-foreground",
        subtitle: "max-w-2xl text-sm text-muted-foreground",
        layout: "grid gap-6 xl:grid-cols-[minmax(0,1fr)_28rem]",
        list_card: "rounded-2xl border border-border bg-card p-6 shadow-sm",
        list_header: "mb-4 flex items-end justify-between gap-4",
        list_header_copy: "",
        list_title: "text-lg font-semibold text-card-foreground",
        list_subtitle: "text-sm text-muted-foreground",
        locale_filter_label: "block space-y-2",
        locale_filter_text: "text-xs font-semibold uppercase tracking-wider text-muted-foreground",
        locale_filter_input: "rounded-lg border border-input bg-background px-3 py-2 text-sm",
        sidebar: "space-y-6",
        form_card: "rounded-2xl border border-border bg-card p-6 shadow-sm",
        form_header: "space-y-1",
        form_title: "text-lg font-semibold text-card-foreground",
        form_subtitle: "text-sm text-muted-foreground",
        load_error: "rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive",
        skeleton_stack: "space-y-2",
        skeleton_row: "h-14 animate-pulse rounded-xl bg-muted",
    }
}

pub fn blog_post_admin_form_view(
    editing_post_id: Option<&str>,
    busy_key: Option<&str>,
    labels: BlogPostAdminFormLabels,
) -> BlogPostAdminFormViewModel {
    let save_busy = is_save_busy(busy_key);
    BlogPostAdminFormViewModel {
        title: edit_action_label(
            is_editing_mode(editing_post_id),
            labels.edit_title,
            labels.create_title,
        ),
        submit_label: submit_action_label(
            submit_button_state(save_busy, is_editing_mode(editing_post_id)),
            labels.saving,
            labels.update,
            labels.create,
        ),
        submit_disabled: save_busy,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminEditBannerViewModel {
    pub visible: bool,
    pub class: &'static str,
    pub banner_text: String,
    pub create_new_label: String,
}

pub fn edit_banner_class(visible: bool) -> &'static str {
    if visible {
        "mt-4 flex items-center justify-between gap-3 rounded-xl border border-border bg-muted/30 px-4 py-3"
    } else {
        "hidden"
    }
}

pub fn blog_post_admin_edit_banner_view(
    editing_post_id: Option<&str>,
    editing_template: &str,
    create_new_label: String,
) -> BlogPostAdminEditBannerViewModel {
    let visible = is_editing_mode(editing_post_id);
    BlogPostAdminEditBannerViewModel {
        visible,
        class: edit_banner_class(visible),
        banner_text: label_with_optional_id(editing_template, editing_post_id),
        create_new_label,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminRawBodyWarningViewModel {
    pub visible: bool,
    pub class: &'static str,
    pub message: String,
}

pub fn raw_body_warning_class(visible: bool) -> &'static str {
    if visible {
        "rounded-xl border border-amber-300/60 bg-amber-50 px-4 py-3 text-sm text-amber-900"
    } else {
        "hidden"
    }
}

pub fn blog_post_admin_raw_body_warning_view(
    body_format: &str,
    warning_message: String,
) -> BlogPostAdminRawBodyWarningViewModel {
    let visible = should_show_raw_body_warning(body_format);
    BlogPostAdminRawBodyWarningViewModel {
        visible,
        class: raw_body_warning_class(visible),
        message: warning_message,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlogPostAdminPostsLoadViewModel {
    Loaded {
        items: Vec<BlogPostListItem>,
        total: u64,
    },
    EmptyContractUnavailable,
    Error {
        message: String,
    },
}

pub fn blog_post_admin_posts_load_view(
    result: Result<(Vec<BlogPostListItem>, u64), String>,
    contract_unavailable: bool,
    error_context: &str,
) -> BlogPostAdminPostsLoadViewModel {
    match result {
        Ok((items, total)) => BlogPostAdminPostsLoadViewModel::Loaded { items, total },
        Err(_) if contract_unavailable => BlogPostAdminPostsLoadViewModel::EmptyContractUnavailable,
        Err(error) => BlogPostAdminPostsLoadViewModel::Error {
            message: error_with_context(error_context, error.as_str()),
        },
    }
}

pub fn blog_post_admin_posts_load_view_from_list(
    result: Result<BlogPostList, String>,
    contract_unavailable: bool,
    error_context: &str,
) -> BlogPostAdminPostsLoadViewModel {
    blog_post_admin_posts_load_view(
        result.map(|post_list| (post_list.items, post_list.total)),
        contract_unavailable,
        error_context,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminBodyFormatOptionViewModel {
    pub value: String,
    pub label: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminBodyFormatSelectViewModel {
    pub options: Vec<BlogPostAdminBodyFormatOptionViewModel>,
}

pub fn supported_blog_post_body_formats() -> &'static [&'static str] {
    &["markdown", "rt_json_v1"]
}

pub fn normalize_blog_post_body_format(value: &str) -> String {
    supported_blog_post_body_formats()
        .iter()
        .copied()
        .find(|format| value.trim().eq_ignore_ascii_case(format))
        .unwrap_or("markdown")
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminBodyFormatChangeViewModel {
    pub body_format: String,
}

pub fn blog_post_admin_body_format_change_view(
    selected_format: String,
) -> BlogPostAdminBodyFormatChangeViewModel {
    BlogPostAdminBodyFormatChangeViewModel {
        body_format: normalize_blog_post_body_format(selected_format.as_str()),
    }
}

pub fn blog_post_admin_body_format_select_view(
    selected_format: &str,
) -> BlogPostAdminBodyFormatSelectViewModel {
    let selected_format = normalize_blog_post_body_format(selected_format);
    BlogPostAdminBodyFormatSelectViewModel {
        options: supported_blog_post_body_formats()
            .iter()
            .copied()
            .map(|format| BlogPostAdminBodyFormatOptionViewModel {
                value: format.to_string(),
                label: format.to_string(),
                selected: selected_format.eq(format),
            })
            .collect(),
    }
}

pub fn is_markdown_format(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("markdown")
}

pub fn should_show_raw_body_warning(body_format: &str) -> bool {
    !is_markdown_format(body_format)
}

pub fn issue_banner_class(kind: WritePathIssueKind) -> &'static str {
    match kind {
        WritePathIssueKind::Validation => {
            "rounded-xl border border-amber-300/60 bg-amber-50 px-4 py-3 text-sm text-amber-900"
        }
        WritePathIssueKind::Sanitization => {
            "rounded-xl border border-blue-300/60 bg-blue-50 px-4 py-3 text-sm text-blue-900"
        }
        WritePathIssueKind::Runtime => {
            "rounded-xl border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm text-destructive"
        }
    }
}

pub fn issue_banner_class_or_hidden(kind: Option<WritePathIssueKind>) -> &'static str {
    kind.map(issue_banner_class).unwrap_or("hidden")
}

pub fn issue_kind_label(kind: WritePathIssueKind) -> &'static str {
    match kind {
        WritePathIssueKind::Validation => "Validation",
        WritePathIssueKind::Sanitization => "Sanitize",
        WritePathIssueKind::Runtime => "Runtime",
    }
}

pub fn issue_label_for(issue: &WritePathIssue) -> &'static str {
    issue_kind_label(issue.kind)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostAdminIssueBannerViewModel {
    pub visible: bool,
    pub class: &'static str,
    pub label: &'static str,
    pub message: String,
}

pub fn blog_post_admin_issue_banner_view(
    issue: Option<&WritePathIssue>,
) -> BlogPostAdminIssueBannerViewModel {
    match issue {
        Some(issue) => BlogPostAdminIssueBannerViewModel {
            visible: true,
            class: issue_banner_class(issue.kind),
            label: issue_label_for(issue),
            message: issue.message.clone(),
        },
        None => BlogPostAdminIssueBannerViewModel {
            visible: false,
            class: issue_banner_class_or_hidden(None),
            label: "",
            message: String::new(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlogPostStatusOperation {
    Publish,
    Unpublish,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostStatusCommand {
    pub post_id: String,
    pub operation: BlogPostStatusOperation,
    pub locale: Option<String>,
    pub busy_key: String,
}

pub fn prepare_blog_post_status_command(
    post_id: String,
    publish: bool,
    post_locale: &str,
) -> BlogPostStatusCommand {
    BlogPostStatusCommand {
        busy_key: busy_key_for_publish(post_id.as_str()),
        post_id,
        operation: if should_publish_now(publish) {
            BlogPostStatusOperation::Publish
        } else {
            BlogPostStatusOperation::Unpublish
        },
        locale: locale_arg(post_locale),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostArchiveCommand {
    pub post_id: String,
    pub locale: Option<String>,
    pub busy_key: String,
}

pub fn prepare_blog_post_archive_command(
    post_id: String,
    post_locale: &str,
) -> BlogPostArchiveCommand {
    BlogPostArchiveCommand {
        busy_key: busy_key_for_archive(post_id.as_str()),
        post_id,
        locale: locale_arg(post_locale),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostLoadResultViewModel {
    pub apply_returned_post_to_form: bool,
    pub reset_form: bool,
}

pub fn blog_post_load_result_view(
    found: bool,
    post_not_found_message: String,
) -> Result<BlogPostLoadResultViewModel, WritePathIssue> {
    if !found {
        return Err(WritePathIssue::new(post_not_found_message));
    }

    Ok(BlogPostLoadResultViewModel {
        apply_returned_post_to_form: true,
        reset_form: false,
    })
}

pub fn blog_post_transport_failure_issue(context: &str, error: &str) -> WritePathIssue {
    WritePathIssue::with_context(context, error)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostSaveResultViewModel {
    pub refresh_posts: bool,
    pub apply_returned_post_to_form: bool,
    pub selected_post_query_intent: Option<BlogPostAdminRouteQueryIntent>,
}

pub fn blog_post_save_result_view(returned_post_id: &str) -> BlogPostSaveResultViewModel {
    BlogPostSaveResultViewModel {
        refresh_posts: true,
        apply_returned_post_to_form: true,
        selected_post_query_intent: Some(blog_post_admin_saved_post_query_intent(
            returned_post_id.to_string(),
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostMutationResultViewModel {
    pub refresh_posts: bool,
    pub apply_returned_post_to_form: bool,
}

pub fn blog_post_mutation_result_view(
    editing_post_id: Option<&str>,
    returned_post_id: &str,
) -> BlogPostMutationResultViewModel {
    BlogPostMutationResultViewModel {
        refresh_posts: true,
        apply_returned_post_to_form: is_editing_post(editing_post_id, returned_post_id),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostDeleteCommand {
    pub post_id: String,
    pub busy_key: String,
}

pub fn prepare_blog_post_delete_command(post_id: String) -> BlogPostDeleteCommand {
    BlogPostDeleteCommand {
        busy_key: busy_key_for_delete(post_id.as_str()),
        post_id,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogPostDeleteResultViewModel {
    pub refresh_posts: bool,
    pub reset_form: bool,
    pub selected_post_query_intent: Option<BlogPostAdminRouteQueryIntent>,
}

pub fn blog_post_delete_result_view(
    deleted: bool,
    editing_post_id: Option<&str>,
    deleted_post_id: &str,
    delete_returned_false_message: String,
) -> Result<BlogPostDeleteResultViewModel, WritePathIssue> {
    if !deleted {
        return Err(WritePathIssue::new(delete_returned_false_message));
    }

    let reset_form = should_reset_form_after_delete(editing_post_id, deleted_post_id);

    Ok(BlogPostDeleteResultViewModel {
        refresh_posts: true,
        reset_form,
        selected_post_query_intent: reset_form.then(blog_post_admin_clear_post_query_intent),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitButtonState {
    Saving,
    Editing,
    Creating,
}

pub fn submit_button_state(is_save_busy: bool, is_editing_mode: bool) -> SubmitButtonState {
    if is_save_busy {
        SubmitButtonState::Saving
    } else if is_editing_mode {
        SubmitButtonState::Editing
    } else {
        SubmitButtonState::Creating
    }
}

pub fn submit_action_label(
    state: SubmitButtonState,
    saving_label: String,
    update_label: String,
    create_label: String,
) -> String {
    match state {
        SubmitButtonState::Saving => saving_label,
        SubmitButtonState::Editing => update_label,
        SubmitButtonState::Creating => create_label,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_list_item(id: &str) -> BlogPostListItem {
        BlogPostListItem {
            id: id.to_string(),
            title: "Title".to_string(),
            effective_locale: "en".to_string(),
            slug: Some("title".to_string()),
            excerpt: Some("Excerpt".to_string()),
            status: "draft".to_string(),
            created_at: "2026-06-14T00:00:00Z".to_string(),
            published_at: None,
        }
    }

    #[test]
    fn admin_warning_and_posts_load_views_keep_adapter_policy_in_core() {
        let markdown = blog_post_admin_raw_body_warning_view("markdown", "warn".to_string());
        assert!(!markdown.visible);
        assert_eq!(markdown.class, "hidden");
        assert_eq!(markdown.message, "warn");

        let raw = blog_post_admin_raw_body_warning_view("rt_json_v1", "warn".to_string());
        assert!(raw.visible);
        assert_eq!(
            raw.class,
            "rounded-xl border border-amber-300/60 bg-amber-50 px-4 py-3 text-sm text-amber-900"
        );

        let loaded = blog_post_admin_posts_load_view(
            Ok((vec![sample_list_item("post-1")], 1)),
            false,
            "Failed to load posts",
        );
        assert_eq!(
            loaded,
            BlogPostAdminPostsLoadViewModel::Loaded {
                items: vec![sample_list_item("post-1")],
                total: 1,
            }
        );

        let unavailable = blog_post_admin_posts_load_view(
            Err("contract unavailable".to_string()),
            true,
            "Failed to load posts",
        );
        assert_eq!(
            unavailable,
            BlogPostAdminPostsLoadViewModel::EmptyContractUnavailable
        );

        let error = blog_post_admin_posts_load_view(
            Err("network".to_string()),
            false,
            "Failed to load posts",
        );
        assert_eq!(
            error,
            BlogPostAdminPostsLoadViewModel::Error {
                message: "Failed to load posts: network".to_string(),
            }
        );
    }

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
            vec![
                "news".to_string(),
                "launch".to_string(),
                "release".to_string()
            ]
        );
    }

    #[test]
    fn blog_post_draft_builder_normalizes_form_state_without_ui_runtime() {
        let draft = build_blog_post_draft(BlogPostFormInput {
            locale: " ru ",
            title: "  Launch Notes  ",
            slug: " launch-notes ",
            excerpt: "  short summary  ",
            body: "  body text  ",
            body_format: "markdown",
            publish: true,
            tags: " news, launch ,, release ",
        });

        assert_eq!(draft.locale, "ru");
        assert_eq!(draft.title, "Launch Notes");
        assert_eq!(draft.slug, "launch-notes");
        assert_eq!(draft.excerpt, "short summary");
        assert_eq!(draft.body, "body text");
        assert_eq!(draft.body_format, "markdown");
        assert!(draft.publish);
        assert_eq!(
            draft.tags,
            vec![
                "news".to_string(),
                "launch".to_string(),
                "release".to_string()
            ]
        );
    }

    #[test]
    fn editor_form_state_maps_empty_and_loaded_post_without_ui_runtime() {
        let empty = BlogPostEditorFormState::empty("ru");

        assert_eq!(empty.editing_post_id, None);
        assert_eq!(empty.locale, "ru");
        assert_eq!(empty.body_format, "markdown");
        assert!(!empty.publish_now);

        let post = BlogPostDetail {
            id: "post-1".to_string(),
            requested_locale: "en".to_string(),
            effective_locale: "en".to_string(),
            available_locales: vec!["en".to_string()],
            title: "Launch".to_string(),
            slug: Some("launch".to_string()),
            excerpt: None,
            body: Some("Body".to_string()),
            body_format: "markdown".to_string(),
            content_json: None,
            status: "published".to_string(),
            created_at: "2026-06-13T00:00:00Z".to_string(),
            updated_at: "2026-06-13T00:00:00Z".to_string(),
            published_at: Some("2026-06-13T00:00:00Z".to_string()),
            tags: vec!["news".to_string(), "release".to_string()],
            featured_image_url: None,
            seo_title: None,
            seo_description: None,
        };

        let state = BlogPostEditorFormState::from_post(&post);

        assert_eq!(state.editing_post_id, Some("post-1".to_string()));
        assert_eq!(state.slug, "launch");
        assert_eq!(state.excerpt, "");
        assert_eq!(state.body, "Body");
        assert_eq!(state.tags_input, "news, release");
        assert!(state.publish_now);
    }

    #[test]
    fn prepare_save_command_rejects_missing_required_fields() {
        let draft = build_blog_post_draft(BlogPostFormInput {
            locale: "en",
            title: "   ",
            slug: "draft",
            excerpt: "summary",
            body: "body",
            body_format: "markdown",
            publish: false,
            tags: "",
        });

        let issue =
            prepare_blog_post_save_command(None, draft, "Title and body are required".to_string())
                .expect_err("missing title must fail before transport selection");

        assert_eq!(issue.message, "Title and body are required");
    }

    #[test]
    fn prepare_save_command_selects_create_operation() {
        let draft = build_blog_post_draft(BlogPostFormInput {
            locale: "en",
            title: "Launch",
            slug: "launch",
            excerpt: "summary",
            body: "body",
            body_format: "markdown",
            publish: true,
            tags: "news",
        });

        let command = prepare_blog_post_save_command(None, draft, "required".to_string())
            .expect("valid create command");

        assert_eq!(command.operation, BlogPostSaveOperation::Create);
        assert_eq!(command.busy_key, "create");
        assert_eq!(command.draft.title, "Launch");
    }

    #[test]
    fn prepare_save_command_selects_update_operation() {
        let draft = build_blog_post_draft(BlogPostFormInput {
            locale: "en",
            title: "Launch",
            slug: "launch",
            excerpt: "summary",
            body: "body",
            body_format: "markdown",
            publish: false,
            tags: "news",
        });

        let command = prepare_blog_post_save_command(
            Some("post-1".to_string()),
            draft,
            "required".to_string(),
        )
        .expect("valid update command");

        assert_eq!(
            command.operation,
            BlogPostSaveOperation::Update {
                post_id: "post-1".to_string()
            }
        );
        assert_eq!(command.busy_key, "save:post-1");
    }

    #[test]
    fn action_commands_prepare_status_archive_and_delete_without_ui_runtime() {
        let publish = prepare_blog_post_status_command("post-1".to_string(), true, "en");
        assert_eq!(publish.post_id, "post-1");
        assert_eq!(publish.operation, BlogPostStatusOperation::Publish);
        assert_eq!(publish.locale, Some("en".to_string()));
        assert_eq!(publish.busy_key, "publish:post-1");

        let unpublish = prepare_blog_post_status_command("post-2".to_string(), false, "ru");
        assert_eq!(unpublish.operation, BlogPostStatusOperation::Unpublish);
        assert_eq!(unpublish.locale, Some("ru".to_string()));
        assert_eq!(unpublish.busy_key, "publish:post-2");

        let archive = prepare_blog_post_archive_command("post-3".to_string(), "de");
        assert_eq!(archive.post_id, "post-3");
        assert_eq!(archive.locale, Some("de".to_string()));
        assert_eq!(archive.busy_key, "archive:post-3");

        let delete = prepare_blog_post_delete_command("post-4".to_string());
        assert_eq!(delete.post_id, "post-4");
        assert_eq!(delete.busy_key, "delete:post-4");
    }

    #[test]
    fn admin_route_query_intents_keep_post_selection_policy_in_core() {
        assert_eq!(
            blog_post_admin_open_post_query_intent("post-1".to_string()),
            BlogPostAdminRouteQueryIntent::Push {
                key: AdminQueryKey::PostId.as_str(),
                value: "post-1".to_string(),
            }
        );
        assert_eq!(
            blog_post_admin_saved_post_query_intent("post-2".to_string()),
            BlogPostAdminRouteQueryIntent::Replace {
                key: AdminQueryKey::PostId.as_str(),
                value: "post-2".to_string(),
            }
        );
        assert_eq!(
            blog_post_admin_clear_post_query_intent(),
            BlogPostAdminRouteQueryIntent::Clear {
                key: AdminQueryKey::PostId.as_str(),
            }
        );
    }

    #[test]
    fn save_result_view_model_maps_apply_refresh_and_query_policy() {
        let view = blog_post_save_result_view("post-1");

        assert!(view.refresh_posts);
        assert!(view.apply_returned_post_to_form);
        assert_eq!(
            view.selected_post_query_intent,
            Some(BlogPostAdminRouteQueryIntent::Replace {
                key: AdminQueryKey::PostId.as_str(),
                value: "post-1".to_string(),
            })
        );
    }

    #[test]
    fn mutation_result_view_model_maps_apply_and_refresh_policy() {
        let matching = blog_post_mutation_result_view(Some("post-1"), "post-1");

        assert!(matching.refresh_posts);
        assert!(matching.apply_returned_post_to_form);

        let different = blog_post_mutation_result_view(Some("post-2"), "post-1");

        assert!(different.refresh_posts);
        assert!(!different.apply_returned_post_to_form);

        let not_editing = blog_post_mutation_result_view(None, "post-1");

        assert!(not_editing.refresh_posts);
        assert!(!not_editing.apply_returned_post_to_form);
    }

    #[test]
    fn delete_result_view_model_maps_reset_and_false_outcomes() {
        let reset = blog_post_delete_result_view(
            true,
            Some("post-1"),
            "post-1",
            "Delete post returned false".to_string(),
        )
        .expect("successful delete should produce apply instructions");

        assert!(reset.refresh_posts);
        assert!(reset.reset_form);
        assert_eq!(
            reset.selected_post_query_intent,
            Some(BlogPostAdminRouteQueryIntent::Clear {
                key: AdminQueryKey::PostId.as_str(),
            })
        );

        let keep_form = blog_post_delete_result_view(
            true,
            Some("post-2"),
            "post-1",
            "Delete post returned false".to_string(),
        )
        .expect("deleting a non-edited row should not reset the current form");

        assert!(keep_form.refresh_posts);
        assert!(!keep_form.reset_form);
        assert_eq!(keep_form.selected_post_query_intent, None);

        let issue = blog_post_delete_result_view(
            false,
            Some("post-1"),
            "post-1",
            "Delete post returned false".to_string(),
        )
        .expect_err("false delete result must become a typed write-path issue");

        assert_eq!(issue.message, "Delete post returned false");
    }

    #[test]
    fn table_row_view_model_composes_row_policy_without_ui_runtime() {
        let row = blog_post_admin_table_row_view(
            BlogPostListItem {
                id: "post-1".to_string(),
                title: "Launch".to_string(),
                effective_locale: "en".to_string(),
                slug: None,
                excerpt: None,
                status: "published".to_string(),
                created_at: "2026-06-13T00:00:00Z".to_string(),
                published_at: Some("2026-06-13T00:00:00Z".to_string()),
            },
            Some("post-1"),
            Some("publish:post-1"),
            BlogPostAdminTableRowLabels {
                draft_slug: "draft",
                no_excerpt: "No excerpt",
                editing: "Editing",
                edit: "Edit",
                unpublish: "Unpublish",
                publish: "Publish",
                archive: "Archive",
                delete: "Delete",
            },
        );

        assert_eq!(row.post_id, "post-1");
        assert_eq!(row.slug, "draft");
        assert_eq!(row.excerpt, "No excerpt");
        assert!(row.is_editing);
        assert!(row.is_busy);
        assert!(row.is_published);
        assert!(!row.is_archived);
        assert!(!row.next_publish_state);
        assert!(row.show_archive_action);
        assert_eq!(row.edit_label, "Editing");
        assert_eq!(row.publish_label, "Unpublish");
        assert_eq!(row.archive_label, "Archive");
        assert_eq!(row.delete_label, "Delete");
    }

    #[test]
    fn title_input_view_model_keeps_autoslug_policy_without_ui_runtime() {
        let generated = blog_post_admin_title_input_view("Hello RusTok".to_string(), "   ");
        assert_eq!(generated.title, "Hello RusTok");
        assert_eq!(generated.slug_update, Some("hello-rustok".to_string()));

        let preserved =
            blog_post_admin_title_input_view("Changed title".to_string(), "custom-slug");
        assert_eq!(preserved.title, "Changed title");
        assert_eq!(preserved.slug_update, None);
    }

    #[test]
    fn body_format_select_view_model_keeps_options_without_ui_runtime() {
        let selected = blog_post_admin_body_format_select_view(" RT_JSON_V1 ");

        assert_eq!(selected.options.len(), 2);
        assert_eq!(selected.options[0].value, "markdown");
        assert_eq!(selected.options[0].label, "markdown");
        assert!(!selected.options[0].selected);
        assert_eq!(selected.options[1].value, "rt_json_v1");
        assert_eq!(selected.options[1].label, "rt_json_v1");
        assert!(selected.options[1].selected);

        let fallback = blog_post_admin_body_format_select_view("unknown");
        assert!(fallback.options[0].selected);

        let changed = blog_post_admin_body_format_change_view(" RT_JSON_V1 ".to_string());
        assert_eq!(changed.body_format, "rt_json_v1");

        let unknown = blog_post_admin_body_format_change_view("unknown".to_string());
        assert_eq!(unknown.body_format, "markdown");
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
        assert_eq!(
            label_with_optional_id("Editing post {id}", Some("42")),
            "Editing post 42"
        );
        assert_eq!(label_with_optional_id("Editing post {id}", None), "");
        assert_eq!(count_label("{count} total", 7), "7 total");
        assert!(is_published_status("published"));
        assert!(is_archived_status("archived"));
        assert!(!is_archived_status("draft"));
        assert_eq!(
            status_badge_css("published"),
            "inline-flex rounded-full px-2.5 py-0.5 text-xs font-semibold bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
        );
        let published_badge = blog_post_admin_status_badge_view("published");
        assert_eq!(published_badge.status, "published");
        assert_eq!(
            published_badge.class,
            "inline-flex rounded-full px-2.5 py-0.5 text-xs font-semibold bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
        );
        assert!(has_non_empty_text(" x "));
        assert!(!has_non_empty_text("   "));
        assert!(should_autofill_slug("   "));
        assert!(!should_autofill_slug("existing-slug"));
        assert_eq!(
            loadable_post_id(Some(" post-1 ")),
            Some("post-1".to_string())
        );
        assert_eq!(loadable_post_id(Some("   ")), None);
        assert_eq!(loadable_post_id(None), None);
        assert_eq!(
            selected_post_request(Some(" post-1 "), "en"),
            Some(("post-1".to_string(), "en".to_string()))
        );
        assert_eq!(selected_post_request(Some("   "), "en"), None);
        assert_eq!(trimmed_text(" abc "), "abc".to_string());
        assert_eq!(
            fallback_post_slug(None, "missing-slug"),
            "missing-slug".to_string()
        );
        assert_eq!(
            fallback_post_excerpt(None, "No excerpt"),
            "No excerpt".to_string()
        );
        assert_eq!(
            optional_text_or_default(Some("hello".to_string())),
            "hello".to_string()
        );
        assert_eq!(optional_text_or_default(None), "".to_string());
        assert_eq!(
            tags_input_value(&["news".to_string(), "launch".to_string()]),
            "news, launch".to_string()
        );
        assert!(row_is_busy_for_post(Some("edit:42"), "42"));
        assert!(!row_is_busy_for_post(Some("edit:41"), "42"));
        assert!(is_editing_post(Some("42"), "42"));
        assert!(!is_editing_post(Some("41"), "42"));
        assert!(!is_editing_post(None, "42"));
        assert!(should_reset_form_after_delete(Some("42"), "42"));
        assert!(!should_reset_form_after_delete(Some("41"), "42"));
        assert!(!should_reset_form_after_delete(None, "42"));
        assert!(is_editing_mode(Some("42")));
        assert!(!is_editing_mode(None));
        assert_eq!(
            editing_post_id_if_editing_mode(Some("42".to_string())),
            Some("42".to_string())
        );
        assert_eq!(editing_post_id_if_editing_mode(None), None);
        assert!(has_issue(Some(WritePathIssueKind::Runtime)));
        assert!(!has_issue(None));
        assert_eq!(
            issue_kind(Some(&WritePathIssue::new("runtime issue"))),
            Some(WritePathIssueKind::Runtime)
        );
        assert_eq!(issue_kind(None), None);
        assert!(has_items(&[1, 2, 3]));
        assert!(!has_items::<u8>(&[]));
        assert_eq!(
            edit_action_label(true, "Editing".to_string(), "Edit".to_string()),
            "Editing".to_string()
        );
        assert_eq!(
            edit_action_label(false, "Editing".to_string(), "Edit".to_string()),
            "Edit".to_string()
        );
        assert_eq!(
            publish_action_label(true, "Unpublish".to_string(), "Publish".to_string()),
            "Unpublish".to_string()
        );
        assert_eq!(
            publish_action_label(false, "Unpublish".to_string(), "Publish".to_string()),
            "Publish".to_string()
        );
        assert!(should_show_archive_action(false));
        assert!(!should_show_archive_action(true));
        assert!(!next_publish_state(true));
        assert!(next_publish_state(false));
        assert!(should_publish_now(true));
        assert!(!should_publish_now(false));
        assert_eq!(locale_arg("en"), Some("en".to_string()));
        assert!(has_required_draft_fields("Title", "Body"));
        assert!(!has_required_draft_fields("", "Body"));
        assert!(!has_required_draft_fields("Title", ""));
        assert!(is_markdown_format("markdown"));
        assert!(is_markdown_format(" Markdown "));
        assert!(!is_markdown_format("rt_json_v1"));
        assert!(should_show_raw_body_warning("rt_json_v1"));
        assert!(!should_show_raw_body_warning("markdown"));
        assert_eq!(
            issue_banner_class(WritePathIssueKind::Validation),
            "rounded-xl border border-amber-300/60 bg-amber-50 px-4 py-3 text-sm text-amber-900"
        );
        assert_eq!(
            issue_banner_class_or_hidden(Some(WritePathIssueKind::Runtime)),
            "rounded-xl border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm text-destructive"
        );
        assert_eq!(issue_banner_class_or_hidden(None), "hidden");
        assert_eq!(issue_kind_label(WritePathIssueKind::Runtime), "Runtime");
        assert_eq!(
            issue_label_for(&WritePathIssue::new("runtime issue")),
            "Runtime"
        );
        let issue_banner =
            blog_post_admin_issue_banner_view(Some(&WritePathIssue::new("runtime issue")));
        assert!(issue_banner.visible);
        assert_eq!(issue_banner.label, "Runtime");
        assert_eq!(issue_banner.message, "runtime issue");
        assert_eq!(
            issue_banner.class,
            "rounded-xl border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm text-destructive"
        );
        let hidden_issue_banner = blog_post_admin_issue_banner_view(None);
        assert!(!hidden_issue_banner.visible);
        assert_eq!(hidden_issue_banner.class, "hidden");
        assert_eq!(hidden_issue_banner.label, "");
        assert_eq!(hidden_issue_banner.message, "");
        assert_eq!(
            submit_action_label(
                SubmitButtonState::Saving,
                "Saving...".to_string(),
                "Update post".to_string(),
                "Create post".to_string()
            ),
            "Saving...".to_string()
        );
        assert_eq!(
            submit_action_label(
                SubmitButtonState::Editing,
                "Saving...".to_string(),
                "Update post".to_string(),
                "Create post".to_string()
            ),
            "Update post".to_string()
        );
        assert_eq!(
            submit_action_label(
                SubmitButtonState::Creating,
                "Saving...".to_string(),
                "Update post".to_string(),
                "Create post".to_string()
            ),
            "Create post".to_string()
        );

        let table = blog_post_admin_table_view(
            3,
            42,
            BlogPostAdminTableLabels {
                empty_message: "No posts".to_string(),
                total_label: "{count} post(s)".to_string(),
                title_header: "Title".to_string(),
                slug_header: "Slug".to_string(),
                status_header: "Status".to_string(),
                locale_header: "Locale".to_string(),
            },
        );
        assert!(!table.is_empty);
        assert_eq!(table.total_label, "42 post(s)");
        assert_eq!(table.title_header, "Title");

        let empty_table = blog_post_admin_table_view(
            0,
            0,
            BlogPostAdminTableLabels {
                empty_message: "No posts".to_string(),
                total_label: "{count} post(s)".to_string(),
                title_header: "Title".to_string(),
                slug_header: "Slug".to_string(),
                status_header: "Status".to_string(),
                locale_header: "Locale".to_string(),
            },
        );
        assert!(empty_table.is_empty);
        assert_eq!(empty_table.empty_message, "No posts");

        let form = blog_post_admin_form_view(
            Some("post-1"),
            Some("save:post-1"),
            BlogPostAdminFormLabels {
                edit_title: "Edit post".to_string(),
                create_title: "Create post".to_string(),
                saving: "Saving...".to_string(),
                update: "Update post".to_string(),
                create: "Create post".to_string(),
            },
        );
        assert_eq!(form.title, "Edit post");
        assert_eq!(form.submit_label, "Saving...");
        assert!(form.submit_disabled);

        let edit_banner = blog_post_admin_edit_banner_view(
            Some("post-1"),
            "Editing post {id}",
            "Create new instead".to_string(),
        );
        assert!(edit_banner.visible);
        assert_eq!(
            edit_banner.class,
            "mt-4 flex items-center justify-between gap-3 rounded-xl border border-border bg-muted/30 px-4 py-3"
        );
        assert_eq!(edit_banner.banner_text, "Editing post post-1");
        assert_eq!(edit_banner.create_new_label, "Create new instead");

        let hidden_edit_banner = blog_post_admin_edit_banner_view(
            None,
            "Editing post {id}",
            "Create new instead".to_string(),
        );
        assert!(!hidden_edit_banner.visible);
        assert_eq!(hidden_edit_banner.class, "hidden");
        assert_eq!(hidden_edit_banner.banner_text, "");

        let form_copy = blog_post_admin_editor_form_copy_view(BlogPostAdminEditorFormCopyLabels {
            subtitle: "Form subtitle".to_string(),
            title_label: "Title".to_string(),
            slug_label: "Slug".to_string(),
            locale_label: "Locale".to_string(),
            body_format_label: "Body format".to_string(),
            excerpt_label: "Excerpt".to_string(),
            body_label: "Body".to_string(),
            tags_label: "Tags".to_string(),
            tags_placeholder: "news, launch".to_string(),
            publish_now_label: "Publish immediately".to_string(),
        });
        assert_eq!(form_copy.subtitle, "Form subtitle");
        assert_eq!(form_copy.title_label, "Title");
        assert_eq!(form_copy.tags_placeholder, "news, launch");
        assert_eq!(form_copy.publish_now_label, "Publish immediately");

        let table_classes = blog_post_admin_table_classes_view();
        assert_eq!(
            table_classes.primary_action_button,
            "text-xs font-medium text-primary hover:underline"
        );
        assert_eq!(
            table_classes.destructive_action_button,
            "text-xs font-medium text-destructive hover:underline"
        );
        assert_eq!(table_classes.table, "w-full text-sm");

        let create_form = blog_post_admin_form_view(
            None,
            None,
            BlogPostAdminFormLabels {
                edit_title: "Edit post".to_string(),
                create_title: "Create post".to_string(),
                saving: "Saving...".to_string(),
                update: "Update post".to_string(),
                create: "Create post".to_string(),
            },
        );
        assert_eq!(create_form.title, "Create post");
        assert_eq!(create_form.submit_label, "Create post");
        assert!(!create_form.submit_disabled);
    }
}
