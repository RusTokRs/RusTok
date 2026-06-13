use rustok_api::{normalize_ui_text, parse_ui_csv};

use crate::model::BlogPostDraft;

pub fn optional_text(value: &str) -> Option<String> {
    normalize_ui_text(value)
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
    busy_key == Some("create")
        || busy_key
            .map(|key| key.starts_with("save:"))
            .unwrap_or(false)
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

pub fn has_non_empty_text(value: &str) -> bool {
    !value.trim().is_empty()
}

pub fn should_autofill_slug(current_slug: &str) -> bool {
    !has_non_empty_text(current_slug)
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
    normalize_ui_text(value).unwrap_or_default()
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
    busy_key.map(|key| key.contains(post_id)).unwrap_or(false)
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

pub fn has_issue(issue: Option<WritePathIssueKind>) -> bool {
    issue.is_some()
}

pub fn issue_kind(issue: Option<&WritePathIssue>) -> Option<WritePathIssueKind> {
    issue.map(|value| value.kind)
}

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
    }
}
use rustok_api::{WritePathIssue, WritePathIssueKind};
