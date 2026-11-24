fn post_edit_input(version: i32) -> UpdatePostInput {
    UpdatePostInput {
        locale: None,
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
        version,
        scheduled_at: Patch::Keep,
    }
}

#[tokio::test]
async fn stale_edit_preserves_owner_localized_rows_and_durable_events() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;
    let service = PostService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
    );
    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let post_id = service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".into(),
                title: "Original".into(),
                content: crate::richtext::article_document_from_plain_text("Body"),
                excerpt: Some("Original excerpt".into()),
                slug: Some("edit-cas".into()),
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
        .unwrap();
    let original = service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await
        .unwrap();
    service
        .update_post(
            tenant_id,
            post_id,
            admin.clone(),
            UpdatePostInput {
                locale: Some("en".into()),
                title: Some("First writer".into()),
                excerpt: Patch::Set("First writer excerpt".into()),
                ..post_edit_input(original.version)
            },
        )
        .await
        .unwrap();
    let before = post_edit_persisted_state(&db, post_id).await;
    let result = service
        .update_post(
            tenant_id,
            post_id,
            admin.clone(),
            UpdatePostInput {
                locale: Some("en".into()),
                title: Some("Stale writer".into()),
                excerpt: Patch::Clear,
                slug: Some("stale-route".into()),
                metadata: Some(serde_json::json!({"stale": true})),
                ..post_edit_input(original.version)
            },
        )
        .await;
    assert!(matches!(result, Err(BlogError::Conflict(_))));
    assert_eq!(post_edit_persisted_state(&db, post_id).await, before);
    let saved = service
        .get_post(tenant_id, admin, post_id, "en")
        .await
        .unwrap();
    assert_eq!(saved.title, "First writer");
    assert_eq!(saved.excerpt.as_deref(), Some("First writer excerpt"));
    assert_eq!(saved.slug, "edit-cas");
}

#[tokio::test]
async fn persisted_patches_and_new_locale_validation_preserve_write_provenance() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;
    let service = PostService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
    );
    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let post_id = service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".into(),
                title: "English only".into(),
                content: crate::richtext::article_document_from_plain_text("English body"),
                excerpt: None,
                slug: Some("patch-provenance".into()),
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
        .unwrap();
    let original = service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await
        .unwrap();
    service
        .update_post(
            tenant_id,
            post_id,
            admin.clone(),
            UpdatePostInput {
                locale: Some("en".into()),
                excerpt: Patch::Set("Summary".into()),
                seo_title: Patch::Set("SEO title".into()),
                seo_description: Patch::Set("SEO description".into()),
                featured_image_url: Patch::Set("/cover.png".into()),
                ..post_edit_input(original.version)
            },
        )
        .await
        .unwrap();
    let set = service
        .get_post(tenant_id, admin.clone(), post_id, "en")
        .await
        .unwrap();
    assert_eq!(set.excerpt.as_deref(), Some("Summary"));
    assert_eq!(set.seo_title.as_deref(), Some("SEO title"));
    assert_eq!(set.seo_description.as_deref(), Some("SEO description"));
    assert_eq!(set.featured_image_url.as_deref(), Some("/cover.png"));
    service
        .update_post(
            tenant_id,
            post_id,
            admin.clone(),
            UpdatePostInput {
                metadata: Some(serde_json::json!({"reviewed": true})),
                ..post_edit_input(set.version)
            },
        )
        .await
        .unwrap();
    let kept = service
        .get_post(tenant_id, admin.clone(), post_id, "fr")
        .await
        .unwrap();
    assert_eq!(kept.effective_locale, "en");
    assert_eq!(kept.available_locales, ["en"]);
    assert_eq!(kept.excerpt, set.excerpt);
    assert_eq!(kept.seo_title, set.seo_title);
    assert_eq!(kept.seo_description, set.seo_description);
    assert_eq!(kept.featured_image_url, set.featured_image_url);

    let before = post_edit_persisted_state(&db, post_id).await;
    let rejected = service
        .update_post(
            tenant_id,
            post_id,
            admin.clone(),
            UpdatePostInput {
                locale: Some("fr".into()),
                excerpt: Patch::Set("French summary without French title and body".into()),
                ..post_edit_input(kept.version)
            },
        )
        .await;
    assert!(matches!(rejected, Err(BlogError::Validation(_))));
    assert_eq!(post_edit_persisted_state(&db, post_id).await, before);

    service
        .update_post(
            tenant_id,
            post_id,
            admin.clone(),
            UpdatePostInput {
                locale: Some("en".into()),
                excerpt: Patch::Clear,
                seo_title: Patch::Clear,
                seo_description: Patch::Clear,
                featured_image_url: Patch::Clear,
                ..post_edit_input(kept.version)
            },
        )
        .await
        .unwrap();
    let cleared = service
        .get_post(tenant_id, admin, post_id, "en")
        .await
        .unwrap();
    assert_eq!(cleared.excerpt, None);
    assert_eq!(cleared.seo_title, None);
    assert_eq!(cleared.seo_description, None);
    assert_eq!(cleared.featured_image_url, None);
    assert_eq!(cleared.available_locales, ["en"]);
}

async fn post_edit_persisted_state(
    db: &DatabaseConnection,
    post_id: Uuid,
) -> (
    blog_post::Model,
    Vec<blog_post_translation::Model>,
    Vec<rustok_outbox::SysEvent>,
) {
    let post = blog_post::Entity::find_by_id(post_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let translations = blog_post_translation::Entity::find()
        .filter(blog_post_translation::Column::PostId.eq(post_id))
        .order_by_asc(blog_post_translation::Column::Locale)
        .all(db)
        .await
        .unwrap();
    let events = rustok_outbox::entity::Entity::find()
        .order_by_asc(rustok_outbox::entity::Column::Id)
        .all(db)
        .await
        .unwrap();
    (post, translations, events)
}

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
                command_id: uuid::Uuid::new_v4(),
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
                command_id: uuid::Uuid::new_v4(),
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
                command_id: uuid::Uuid::new_v4(),
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
                command_id: uuid::Uuid::new_v4(),
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
