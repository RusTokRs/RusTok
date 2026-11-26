use crate::core::*;
use crate::model::*;

fn sample_list_item(id: &str) -> BlogPostListItem {
    BlogPostListItem {
        id: id.to_string(),
        title: "Title".to_string(),
        effective_locale: "en".to_string(),
        slug: Some("title".to_string()),
        excerpt: Some("Excerpt".to_string()),
        status: "draft".to_string(),
        author_id: Some("00000000-0000-0000-0000-000000000001".to_string()),
        author_profile: None,
        created_at: "2026-06-14T00:00:00Z".to_string(),
        published_at: None,
        scheduled_at: None,
        is_pinned: false,
        pinned_at: None,
    }
}

#[test]
fn admin_posts_load_views_keep_adapter_policy_in_core() {
    let loaded = blog_post_admin_posts_load_view(
        Ok((
            vec![sample_list_item("post-1")],
            Some("cursor-2".to_string()),
        )),
        false,
        "Failed to load posts",
    );
    assert_eq!(
        loaded,
        BlogPostAdminPostsLoadViewModel::Loaded {
            items: vec![sample_list_item("post-1")],
            next_cursor: Some("cursor-2".to_string()),
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

    let error =
        blog_post_admin_posts_load_view(Err("network".to_string()), false, "Failed to load posts");
    assert_eq!(
        error,
        BlogPostAdminPostsLoadViewModel::Error {
            message: "Failed to load posts: network".to_string(),
        }
    );
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
            author_id: Some("00000000-0000-0000-0000-000000000002".to_string()),
            author_profile: Some(BlogAuthorProfileName {
                display_name: "Ada Author".to_string(),
            }),
            created_at: "2026-06-13T00:00:00Z".to_string(),
            published_at: Some("2026-06-13T00:00:00Z".to_string()),
            scheduled_at: None,
            is_pinned: false,
            pinned_at: None,
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
            preview_link: "Preview link",
        },
    );

    assert_eq!(row.post_id, "post-1");
    assert_eq!(row.slug, "draft");
    assert_eq!(row.excerpt, "No excerpt");
    assert_eq!(row.author, "Ada Author");
    assert_eq!(row.created_at, "2026-06-13");
    assert_eq!(row.published_at, "2026-06-13");
    assert!(row.is_editing);
    assert!(row.is_busy);
    assert!(row.is_published);
    assert!(!row.is_archived);
    assert!(!row.next_publish_state);
    assert!(row.show_publish_action);
    assert!(row.show_archive_action);
    assert!(!row.show_restore_action);
    // Preview links exist only while the post is a draft.
    assert!(!row.show_preview_action);
    assert_eq!(row.edit_label, "Editing");
    assert_eq!(row.publish_label, "Unpublish");
    assert_eq!(row.archive_label, "Archive");
    assert_eq!(row.delete_label, "Delete");
    assert_eq!(row.preview_label, "Preview link");
}

#[test]
fn preview_link_action_is_draft_only() {
    fn labels() -> BlogPostAdminTableRowLabels<'static> {
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
            preview_link: "Preview link",
        }
    }

    // Draft rows mint preview tokens; the token dies on the first lifecycle
    // transition away from draft, so published and archived rows must not offer it.
    let draft_row =
        blog_post_admin_table_row_view(sample_list_item("post-9"), None, None, labels());
    assert!(draft_row.show_preview_action);

    let mut archived = sample_list_item("post-9");
    archived.status = "archived".to_string();
    let archived_row = blog_post_admin_table_row_view(archived, None, None, labels());
    assert!(!archived_row.show_preview_action);
}

#[test]
fn busy_key_is_scoped_to_its_own_row() {
    fn labels() -> BlogPostAdminTableRowLabels<'static> {
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
            preview_link: "Preview link",
        }
    }

    // An in-flight action on one post must not freeze the action buttons of
    // every other row: only the row whose id matches the busy key is marked
    // busy. This guards the per-row scoping used by the list table.
    let other_busy = blog_post_admin_table_row_view(
        sample_list_item("post-9"),
        None,
        Some("save:post-1"),
        labels(),
    );
    assert!(!other_busy.is_busy);

    let own_busy = blog_post_admin_table_row_view(
        sample_list_item("post-9"),
        None,
        Some("save:post-9"),
        labels(),
    );
    assert!(own_busy.is_busy);

    // A create-scoped key (no post id) never marks a concrete row busy.
    let create_busy =
        blog_post_admin_table_row_view(sample_list_item("post-9"), None, Some("create"), labels());
    assert!(!create_busy.is_busy);
}
