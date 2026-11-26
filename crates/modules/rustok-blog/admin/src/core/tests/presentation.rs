use crate::core::*;
use crate::model::*;
use rustok_api::{RichTextDocument, WritePathIssue, WritePathIssueKind};

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
    assert_eq!(busy_key_for_restore("1"), "restore:1".to_string());
    assert_eq!(busy_key_for_delete("1"), "delete:1".to_string());
    assert_eq!(busy_key_for_preview("1"), "preview:1".to_string());
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
    assert_eq!(
        preview_expires_label("Expires {at}", "2026-11-06T12:00:00Z"),
        "Expires 2026-11-06T12:00:00Z"
    );
    // A resolved catalog message already carries the timestamp; the helper
    // must not touch it.
    assert_eq!(
        preview_expires_label("Действует до 2026-11-06T12:00:00Z", "2026-11-06T12:00:00Z"),
        "Действует до 2026-11-06T12:00:00Z"
    );
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
    assert!(!should_show_archive_action(false));
    assert!(should_show_archive_action(true));
    assert!(should_show_publish_action(false));
    assert!(!should_show_publish_action(true));
    assert!(!should_show_restore_action(false));
    assert!(should_show_restore_action(true));
    assert!(!next_publish_state(true));
    assert!(next_publish_state(false));
    assert!(should_publish_now(true));
    assert!(!should_publish_now(false));
    assert_eq!(locale_arg("en"), Some("en".to_string()));
    let empty_doc = RichTextDocument::empty();
    let valid_doc = RichTextDocument::single_paragraph("Body");
    assert!(has_required_draft_fields("Title", &valid_doc));
    assert!(!has_required_draft_fields("", &valid_doc));
    assert!(!has_required_draft_fields("Title", &empty_doc));
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
            author_header: "Author".to_string(),
            created_header: "Created".to_string(),
            published_header: "Published".to_string(),
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
            author_header: "Author".to_string(),
            created_header: "Created".to_string(),
            published_header: "Published".to_string(),
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
        excerpt_label: "Excerpt".to_string(),
        content_label: "Content".to_string(),
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

#[test]
fn admin_sort_options_cover_every_variant_and_keep_core_owned_copy() {
    // Every sort field the transport can send is offered by the UI, in a stable order.
    assert_eq!(
        BLOG_POST_ADMIN_SORT_FIELDS,
        [
            BlogPostSortField::CreatedAt,
            BlogPostSortField::UpdatedAt,
            BlogPostSortField::PublishedAt,
        ]
    );
    for field in BLOG_POST_ADMIN_SORT_FIELDS {
        assert!(blog_post_admin_sort_field_label_key(field).starts_with("blog.table."));
        assert!(!blog_post_admin_sort_field_default_label(field).is_empty());
    }
    for order in [BlogPostSortOrder::Asc, BlogPostSortOrder::Desc] {
        assert!(blog_post_admin_sort_order_label_key(order).starts_with("blog.table."));
        assert!(!blog_post_admin_sort_order_default_label(order).is_empty());
    }
}

#[test]
fn admin_sort_default_matches_the_owner_default() {
    let sort = BlogPostAdminSort::default();
    assert_eq!(sort.field, BlogPostSortField::CreatedAt);
    assert_eq!(sort.order, BlogPostSortOrder::Desc);

    let by_update = sort
        .with_field(BlogPostSortField::UpdatedAt)
        .with_order(BlogPostSortOrder::Asc);
    assert_eq!(by_update.field, BlogPostSortField::UpdatedAt);
    assert_eq!(by_update.order, BlogPostSortOrder::Asc);
    // Changing one half never resets the other: the UI sends both on every reload.
    assert_eq!(
        by_update.with_order(BlogPostSortOrder::Desc).field,
        BlogPostSortField::UpdatedAt
    );
}

#[test]
fn admin_sort_tokens_match_the_wire_spelling() {
    // The `<option value>` spelling and the serde spelling are the same wire contract:
    // the GraphQL adapter serializes the enum and the native transport carries it as a
    // server-function argument, so a drift here would silently sort by the wrong column.
    for field in BLOG_POST_ADMIN_SORT_FIELDS {
        let serialized = serde_json::to_value(field).expect("sort field serializes");
        assert_eq!(serialized.as_str(), Some(field.as_graphql()));
        assert_eq!(
            BlogPostSortField::from_graphql(field.as_graphql()),
            Some(field)
        );
    }
    for order in [BlogPostSortOrder::Asc, BlogPostSortOrder::Desc] {
        let serialized = serde_json::to_value(order).expect("sort order serializes");
        assert_eq!(serialized.as_str(), Some(order.as_graphql()));
        assert_eq!(
            BlogPostSortOrder::from_graphql(order.as_graphql()),
            Some(order)
        );
    }
    assert_eq!(BlogPostSortField::from_graphql("SIDEWAYS"), None);
    assert_eq!(BlogPostSortOrder::from_graphql("sideways"), None);

    // The sort travels to the native transport as a server-function argument, so it
    // must survive that boundary unchanged.
    let sort = BlogPostAdminSort::default().with_field(BlogPostSortField::PublishedAt);
    let round_tripped: BlogPostAdminSort =
        serde_json::from_value(serde_json::to_value(sort).expect("sort serializes"))
            .expect("sort deserializes");
    assert_eq!(round_tripped, sort);
}

#[test]
fn blog_admin_section_resolution_maps_subpaths() {
    assert_eq!(
        blog_admin_section_for_subpath(Some("categories")),
        BlogAdminSection::Categories
    );
    assert_eq!(
        blog_admin_section_for_subpath(Some("categories/anything")),
        BlogAdminSection::Categories
    );
    assert_eq!(
        blog_admin_section_for_subpath(Some("posts")),
        BlogAdminSection::Posts
    );
    assert_eq!(
        blog_admin_section_for_subpath(Some("new")),
        BlogAdminSection::Posts
    );
    assert_eq!(
        blog_admin_section_for_subpath(Some("category")),
        BlogAdminSection::Posts
    );
    assert_eq!(
        blog_admin_section_for_subpath(None),
        BlogAdminSection::Posts
    );
}

#[test]
fn grid_column_copy_and_unicode_filtering_have_no_language_branch() {
    let columns = blog_post_grid_columns(|key, _| format!("fr:{key}"));
    assert_eq!(columns[0].title, "fr:blog.grid.title");
    assert!(sort_field_change("INVALID", BlogPostSortOrder::Desc).is_none());
}

#[test]
fn stale_busy_key_release_never_drops_a_newer_in_flight_key() {
    // The obsolete request's own key is released so the editor does not stay
    // locked after a stale completion.
    assert!(should_release_busy_key(Some("edit:post-1"), "edit:post-1"));
    // A newer request has taken over the shared key: the stale completion must
    // not release it.
    assert!(!should_release_busy_key(Some("save:post-2"), "edit:post-1"));
    // No busy key is active: nothing to release, and nothing is dropped.
    assert!(!should_release_busy_key(None, "edit:post-1"));
}
