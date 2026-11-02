#[tokio::test]
async fn post_lifecycle_uses_blog_owned_tables() {
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
                title: "Draft Post".to_string(),
                content: crate::richtext::article_document_from_plain_text("Content"),
                excerpt: None,
                slug: Some("draft-post".to_string()),
                publish: false,
                tags: vec!["rust".to_string()],
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
        .expect("post should be created");

    let draft = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await
        .expect("draft should be readable");
    assert_eq!(draft.status, BlogPostStatus::Draft);
    assert_eq!(draft.tags, vec!["rust"]);

    post_service
        .publish_post(tenant_id, post_id, admin.clone())
        .await
        .expect("post should publish");

    let published = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await
        .expect("published should be readable");
    assert_eq!(published.status, BlogPostStatus::Published);
    assert_eq!(published.slug, "draft-post");
    assert!(published.published_at.is_some());
}

#[tokio::test]
async fn customer_cannot_create_or_read_draft_posts() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let denied_create = post_service
        .create_post(
            tenant_id,
            customer.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Customer draft".to_string(),
                content: crate::richtext::article_document_from_plain_text("Body"),
                excerpt: None,
                slug: Some("customer-draft".to_string()),
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
        .expect_err("customer should not create posts");
    assert!(matches!(denied_create, BlogError::Forbidden(_)));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Admin draft".to_string(),
                content: crate::richtext::article_document_from_plain_text("Body"),
                excerpt: None,
                slug: Some("admin-draft".to_string()),
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
        .expect("admin draft should be created");

    let denied_read = post_service
        .get_post(tenant_id, customer.clone(), post_id, "en")
        .await
        .expect_err("customer should not read drafts");
    assert!(matches!(denied_read, BlogError::Forbidden(_)));

    let listed = post_service
        .list_posts(
            tenant_id,
            customer,
            PostListQuery {
                per_page: Some(10),
                ..Default::default()
            },
        )
        .await
        .expect("customer listing should succeed");
    assert!(listed.items.is_empty());
    assert!(listed.next_cursor.is_none());
}

#[tokio::test]
async fn create_and_update_post_store_channel_visibility_in_typed_relation() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    seed_channel(&db, tenant_id, "web").await;
    seed_channel(&db, tenant_id, "mobile").await;
    seed_channel(&db, tenant_id, "storefront").await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Visible post".to_string(),
                content: crate::richtext::article_document_from_plain_text("Body"),
                excerpt: None,
                slug: Some("visible-post".to_string()),
                publish: true,
                tags: vec![],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: Some(vec![" Web ".to_string(), "mobile".to_string()]),
                metadata: Some(serde_json::json!({"custom": "value"})),
                scheduled_at: None,
            },
        )
        .await
        .expect("post should be created");

    let created = post_service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await
        .expect("post should load");
    assert_eq!(
        created.channel_slugs,
        vec!["mobile".to_string(), "web".to_string()]
    );
    assert_eq!(created.metadata["custom"], "value");

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
                channel_slugs: Some(vec!["storefront".to_string()]),
                metadata: Some(serde_json::json!({"custom": "next"})),
                version: created.version,
                scheduled_at: Patch::Keep,
            },
        )
        .await
        .expect("post should update");

    let updated = post_service
        .get_post(tenant_id, admin, post_id, "en")
        .await
        .expect("updated post should load");
    assert_eq!(updated.channel_slugs, vec!["storefront".to_string()]);
    assert_eq!(updated.metadata["custom"], "next");
}
