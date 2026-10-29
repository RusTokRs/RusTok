#[test]
fn post_list_query_defaults() {
    let query = PostListQuery::default();
    assert!(query.after.is_none());
    assert_eq!(query.per_page(), 20);
    assert_eq!(
        query.sort(),
        (PostSortField::CreatedAt, PostSortOrder::Desc)
    );
}

#[test]
fn post_list_query_clamps_bounds() {
    let query = PostListQuery {
        per_page: Some(200),
        ..Default::default()
    };
    assert_eq!(query.per_page(), 100);
}

#[test]
fn admin_post_cursor_round_trips_with_and_without_sort_value() {
    let id = Uuid::new_v4();
    let value = chrono::DateTime::parse_from_rfc3339("2026-10-08T10:15:30.123456789+03:00")
        .expect("valid timestamp");
    for cursor_value in [Some(value), None] {
        let cursor = AdminPostCursor {
            sort_by: PostSortField::PublishedAt,
            sort_order: PostSortOrder::Asc,
            value: cursor_value,
            id,
        };
        let decoded = AdminPostCursor::decode(
            &cursor.encode(),
            PostSortField::PublishedAt,
            PostSortOrder::Asc,
        )
        .expect("cursor should decode for the same sort");
        assert_eq!(decoded, cursor);
    }
}

#[test]
fn admin_post_cursor_rejects_a_different_sort_and_garbage() {
    let cursor = AdminPostCursor {
        sort_by: PostSortField::CreatedAt,
        sort_order: PostSortOrder::Desc,
        value: None,
        id: Uuid::new_v4(),
    };
    let token = cursor.encode();
    assert!(
        AdminPostCursor::decode(&token, PostSortField::UpdatedAt, PostSortOrder::Desc).is_err()
    );
    assert!(AdminPostCursor::decode(&token, PostSortField::CreatedAt, PostSortOrder::Asc).is_err());
    assert!(
        AdminPostCursor::decode(
            "not-a-cursor",
            PostSortField::CreatedAt,
            PostSortOrder::Desc
        )
        .is_err()
    );
}

#[test]
fn channel_visibility_normalizes_and_filters_blog_channel_lists() {
    let channel_slugs =
        normalize_channel_slugs(&[" Web ".to_string(), "mobile".to_string(), "web".to_string()])
            .expect("channel slugs normalize");

    assert_eq!(channel_slugs, vec!["mobile".to_string(), "web".to_string()]);
    assert!(is_post_visible_for_channel(&channel_slugs, Some("web")));
    assert!(!is_post_visible_for_channel(
        &channel_slugs,
        Some("storefront")
    ));
    assert!(!is_post_visible_for_channel(&channel_slugs, None));
}

#[test]
fn slug_normalization_is_stable() {
    assert_eq!(normalize_slug("Hello, World!"), "hello-world");
    assert_eq!(normalize_slug("  many   spaces  "), "many-spaces");
}

#[test]
fn slug_normalization_transliterates_non_ascii_titles() {
    assert_eq!(normalize_slug("Привет, мир!"), "privet-mir");
    assert!(normalize_slug("!!!").is_empty());
}

#[test]
fn featured_image_url_accepts_only_http_https_or_root_relative_paths() {
    assert!(validate_featured_image_url("https://cdn.example.com/a.png").is_ok());
    assert!(validate_featured_image_url("/media/a.png").is_ok());
    assert!(validate_featured_image_url("javascript:alert(1)").is_err());
    assert!(validate_featured_image_url("data:image/png;base64,AAAA").is_err());
    assert!(validate_featured_image_url("//cdn.example.com/a.png").is_err());
    assert!(validate_featured_image_url("not a url").is_err());
    assert!(validate_featured_image_url(" https://cdn.example.com/a.png").is_err());
}

#[test]
fn unknown_persisted_post_status_is_an_invariant_failure() {
    let error =
        storage_to_status("corrupt-status").expect_err("unknown persisted status must fail closed");
    assert!(matches!(error, BlogError::Invariant(_)));
}

#[test]
fn post_title_limit_counts_unicode_characters_not_utf8_bytes() {
    let five_hundred_twelve_characters = "Ж".repeat(512);
    let five_hundred_thirteen_characters = "Ж".repeat(513);

    assert!(super::validate_title(&five_hundred_twelve_characters).is_ok());
    assert!(super::validate_title(&five_hundred_thirteen_characters).is_err());
}

/// A schedule is a draft-only intent on both write paths: `create_post` refuses it together
/// with `publish: true`, `update_post` may only set it while the row is a draft, and a publish
/// clears it. Clearing and keeping stay available on every status, so a stale schedule can
/// still be removed from a published post.

#[test]
fn schedule_patch_is_bounded_to_drafts_and_the_future() {
    let now = chrono::Utc::now();
    let future = now + chrono::Duration::hours(24);
    let past = now - chrono::Duration::hours(1);

    assert!(matches!(
        super::validate_schedule_for_update(&Patch::Set(future), BlogPostStatus::Draft, now),
        Ok(Patch::Set(_))
    ));
    for status in [BlogPostStatus::Published, BlogPostStatus::Archived] {
        assert!(
            matches!(
                super::validate_schedule_for_update(&Patch::Set(future), status, now),
                Err(BlogError::Validation(_))
            ),
            "a {status:?} post must not take a schedule"
        );
    }
    assert!(matches!(
        super::validate_schedule_for_update(&Patch::Set(past), BlogPostStatus::Draft, now),
        Err(BlogError::Validation(_))
    ));
    assert!(matches!(
        super::validate_schedule_for_update(&Patch::Clear, BlogPostStatus::Published, now),
        Ok(Patch::Clear)
    ));
    assert!(matches!(
        super::validate_schedule_for_update(&Patch::Keep, BlogPostStatus::Archived, now),
        Ok(Patch::Keep)
    ));
}

/// The wiring, not just the predicate: a caller that patches `scheduled_at` on a post it has
/// already published is refused, and publishing a scheduled draft clears the field.
