use super::fixtures::editor_post;
use crate::core::*;
use crate::model::*;
use rustok_api::RichTextDocument;
use rustok_ui_core::AdminQueryKey;

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
    let content = RichTextDocument::empty();
    let draft = build_blog_post_draft(BlogPostFormInput {
        version: None,
        locale: " ru ",
        title: "  Launch Notes  ",
        slug: " launch-notes ",
        excerpt: "  short summary  ",
        content: &content,
        publish: true,
        tags: " news, launch ,, release ",
        category_id: "",
        channel_slugs: "",
        featured_image_url: "",
        seo_title: "",
        seo_description: "",
        scheduled_at: "",
        schedule_editable: true,
    });

    assert_eq!(draft.locale, "ru");
    assert_eq!(draft.title, "Launch Notes");
    assert_eq!(draft.slug, "launch-notes");
    assert_eq!(draft.excerpt, "short summary");
    assert_eq!(draft.content, content);
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
    assert_eq!(empty.content, RichTextDocument::empty());
    assert!(!empty.publish_now);

    let post = BlogPostDetail {
        id: "post-1".to_string(),
        requested_locale: "en".to_string(),
        effective_locale: "en".to_string(),
        available_locales: vec!["en".to_string()],
        title: "Launch".to_string(),
        slug: Some("launch".to_string()),
        excerpt: None,
        content: None,
        content_plain_text: None,
        status: "published".to_string(),
        created_at: "2026-06-13T00:00:00Z".to_string(),
        updated_at: "2026-06-13T00:00:00Z".to_string(),
        published_at: Some("2026-06-13T00:00:00Z".to_string()),
        scheduled_at: None,
        is_pinned: false,
        pinned_at: None,
        tags: vec!["news".to_string(), "release".to_string()],
        category_id: None,
        channel_slugs: Vec::new(),
        featured_image_url: None,
        seo_title: None,
        seo_description: None,
        version: 9,
    };

    let state = BlogPostEditorFormState::from_post(&post);

    assert_eq!(state.editing_post_id, Some("post-1".to_string()));
    assert_eq!(state.version, Some(9));
    assert_eq!(state.slug, "launch");
    assert_eq!(state.excerpt, "");
    assert_eq!(state.content, RichTextDocument::empty());
    assert_eq!(state.tags_input, "news, release");
    assert!(state.publish_now);
}

#[test]
fn prepare_save_command_rejects_missing_required_fields() {
    let content = RichTextDocument::empty();
    let draft = build_blog_post_draft(BlogPostFormInput {
        version: None,
        locale: "en",
        title: "   ",
        slug: "draft",
        excerpt: "summary",
        content: &content,
        publish: false,
        tags: "",
        category_id: "",
        channel_slugs: "",
        featured_image_url: "",
        seo_title: "",
        seo_description: "",
        scheduled_at: "",
        schedule_editable: true,
    });

    let issue = prepare_blog_post_save_command(
        None,
        None,
        draft,
        "Title and body are required".to_string(),
        "slug required".into(),
    )
    .expect_err("missing title must fail before transport selection");

    assert_eq!(issue.message, "Title and body are required");
}

#[test]
fn prepare_save_command_selects_create_operation() {
    let content = RichTextDocument::single_paragraph("Hello world");
    let draft = build_blog_post_draft(BlogPostFormInput {
        version: None,
        locale: "en",
        title: "Launch",
        slug: "launch",
        excerpt: "summary",
        content: &content,
        publish: true,
        tags: "news",
        category_id: "",
        channel_slugs: "",
        featured_image_url: "",
        seo_title: "",
        seo_description: "",
        scheduled_at: "",
        schedule_editable: true,
    });

    let command = prepare_blog_post_save_command(
        None,
        None,
        draft,
        "required".to_string(),
        "slug required".into(),
    )
    .expect("valid create command");

    assert_eq!(command.busy_key, "create");
    match command.operation {
        BlogPostSaveOperation::Create(draft) => {
            assert_eq!(draft.title, "Launch");
            assert_eq!(draft.content, content);
        }
        _ => panic!("expected create"),
    }
}

#[test]
fn prepare_save_command_selects_update_operation() {
    let content = RichTextDocument::single_paragraph("Hello world");
    let draft = build_blog_post_draft(BlogPostFormInput {
        version: Some(7),
        locale: "en",
        title: "Launch",
        slug: "launch",
        excerpt: "summary",
        content: &content,
        publish: false,
        tags: "news",
        category_id: "",
        channel_slugs: "",
        featured_image_url: "",
        seo_title: "",
        seo_description: "",
        scheduled_at: "",
        schedule_editable: true,
    });

    let mut post = editor_post();
    post.id = "post-1".into();
    post.version = 7;
    let command = prepare_blog_post_save_command(
        Some("post-1".into()),
        Some(&post),
        draft,
        "required".into(),
        "slug required".into(),
    )
    .expect("valid update command");
    match command.operation {
        BlogPostSaveOperation::Update { post_id, update } => {
            assert_eq!(post_id, "post-1");
            assert_eq!(update.version, 7);
        }
        _ => panic!("expected update"),
    }
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

    let restore = prepare_blog_post_restore_command("post-5".to_string(), "fr");
    assert_eq!(restore.post_id, "post-5");
    assert_eq!(restore.locale, Some("fr".to_string()));
    assert_eq!(restore.busy_key, "restore:post-5");

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
    assert!(matching.reload_selected_post);

    let different = blog_post_mutation_result_view(Some("post-2"), "post-1");

    assert!(different.refresh_posts);
    assert!(!different.reload_selected_post);

    let not_editing = blog_post_mutation_result_view(None, "post-1");

    assert!(not_editing.refresh_posts);
    assert!(!not_editing.reload_selected_post);
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
fn title_input_does_not_invent_slug_policy() {
    let english = blog_post_admin_title_input_view("Hello RusTok".to_string());
    assert_eq!(english.title, "Hello RusTok");

    // The Blog owner applies the same canonical Taxonomy transliteration in both UIs.
    let cyrillic = blog_post_admin_title_input_view("Привет мир".to_string());
    assert_eq!(cyrillic.title, "Привет мир");
}

#[test]
fn parse_tags_deduplicates_while_preserving_order() {
    assert_eq!(parse_tags("rust, search"), ["rust", "search"]);
    assert_eq!(parse_tags("rust, rust, search"), ["rust", "search"]);
    assert_eq!(parse_tags("rust, search, rust"), ["rust", "search"]);
    assert_eq!(parse_tags("rust, , ,search"), ["rust", "search"]);
    assert_eq!(parse_tags(""), [] as [String; 0]);
}
