/// The wiring, not just the predicate: a caller that patches `scheduled_at` on a post it has
/// already published is refused, and publishing a scheduled draft clears the field.

#[tokio::test]
async fn published_posts_reject_a_schedule_patch() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Scheduled later".to_string(),
                content: crate::richtext::article_document_from_plain_text("Body"),
                excerpt: None,
                slug: Some("scheduled-later".to_string()),
                publish: false,
                tags: vec![],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
                scheduled_at: None,
            },
        )
        .await
        .expect("draft should be created");

    let draft = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await
        .expect("draft should load");

    post_service
        .update_post(
            tenant_id,
            post_id,
            admin.clone(),
            UpdatePostInput {
                locale: Some("en".to_string()),
                title: None,
                content: None,
                excerpt: Patch::Keep,
                slug: None,
                tags: None,
                category_id: Patch::Keep,
                featured_image_url: Patch::Keep,
                seo_title: Patch::Keep,
                seo_description: Patch::Keep,
                channel_slugs: None,
                metadata: None,
                version: draft.version,
                scheduled_at: Patch::Set(chrono::Utc::now() + chrono::Duration::hours(2)),
            },
        )
        .await
        .expect("a draft may be scheduled");

    let scheduled = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await
        .expect("scheduled draft should load");
    assert_eq!(scheduled.status, BlogPostStatus::Draft);
    assert!(
        scheduled.scheduled_at.is_some(),
        "the schedule of a draft is stored"
    );

    post_service
        .publish_post(tenant_id, post_id, admin.clone())
        .await
        .expect("draft should publish");

    let published = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await
        .expect("published post should load");
    assert_eq!(published.status, BlogPostStatus::Published);
    assert!(
        published.scheduled_at.is_none(),
        "publishing clears the schedule it consumed"
    );

    let refused = post_service
        .update_post(
            tenant_id,
            post_id,
            admin.clone(),
            UpdatePostInput {
                locale: Some("en".to_string()),
                title: None,
                content: None,
                excerpt: Patch::Keep,
                slug: None,
                tags: None,
                category_id: Patch::Keep,
                featured_image_url: Patch::Keep,
                seo_title: Patch::Keep,
                seo_description: Patch::Keep,
                channel_slugs: None,
                metadata: None,
                version: published.version,
                scheduled_at: Patch::Set(chrono::Utc::now() + chrono::Duration::hours(3)),
            },
        )
        .await
        .expect_err("a published post must not take a schedule");
    assert!(matches!(refused, BlogError::Validation(_)));
}
