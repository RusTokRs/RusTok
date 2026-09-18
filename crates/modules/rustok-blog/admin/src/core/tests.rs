use super::*;
use crate::model::*;
use rustok_api::{RichTextDocument, WritePathIssueKind};

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
    fn admin_posts_load_views_keep_adapter_policy_in_core() {
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
            tags: vec!["news".to_string(), "release".to_string()],
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
        });

        let issue =
            prepare_blog_post_save_command(None, draft, "Title and body are required".to_string())
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
        });

        let command = prepare_blog_post_save_command(None, draft, "required".to_string())
            .expect("valid create command");

        assert_eq!(command.operation, BlogPostSaveOperation::Create);
        assert_eq!(command.busy_key, "create");
        assert_eq!(command.draft.title, "Launch");
        assert_eq!(command.draft.content, content);
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
                restore: "Restore",
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
        assert!(row.show_publish_action);
        assert!(row.show_archive_action);
        assert!(!row.show_restore_action);
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
        assert_eq!(busy_key_for_restore("1"), "restore:1".to_string());
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
