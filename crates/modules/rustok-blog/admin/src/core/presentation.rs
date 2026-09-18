use super::commands::*;
use crate::model::*;
use rustok_api::{WritePathIssue, WritePathIssueKind};

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
    pub show_publish_action: bool,
    pub show_archive_action: bool,
    pub show_restore_action: bool,
    pub edit_label: String,
    pub publish_label: String,
    pub archive_label: String,
    pub restore_label: String,
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
    pub restore: &'a str,
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
    let show_publish_action = should_show_publish_action(is_archived);
    let show_archive_action = should_show_archive_action(is_published);
    let show_restore_action = should_show_restore_action(is_archived);

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
        show_publish_action,
        show_archive_action,
        show_restore_action,
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
        restore_label: labels.restore.to_string(),
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
    pub restore: String,
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
        restore: labels.restore.as_str(),
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
    pub excerpt_label: String,
    pub content_label: String,
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
    pub excerpt_label: String,
    pub content_label: String,
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
        excerpt_label: labels.excerpt_label,
        content_label: labels.content_label,
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

pub fn prepare_blog_post_restore_command(
    post_id: String,
    post_locale: &str,
) -> BlogPostArchiveCommand {
    BlogPostArchiveCommand {
        busy_key: busy_key_for_restore(post_id.as_str()),
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
