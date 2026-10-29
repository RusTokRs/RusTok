/// The customer-facing list is a public surface, so it applies the same corpus
/// rule as the public keyset list, the SEO scan and the comment target gate.
///
/// A published row whose `published_at` was lost is damaged state: an editor with
/// tenant-wide read authority still sees it (that is how it gets repaired), a
/// customer does not — the list used to filter on `status` only and showed it.

#[tokio::test]
async fn public_visible_listing_filters_by_typed_channel_relation() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    seed_channel(&db, tenant_id, "web").await;
    seed_channel(&db, tenant_id, "mobile").await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    for (slug, title, channel_slugs) in [
        ("web-visible", "Web Visible", Some(vec!["web".to_string()])),
        (
            "mobile-only",
            "Mobile Only",
            Some(vec!["mobile".to_string()]),
        ),
        ("global", "Global", None),
    ] {
        post_service
            .create_post(
                tenant_id,
                admin.clone(),
                CreatePostInput {
                    locale: "en".to_string(),
                    title: title.to_string(),
                    content: crate::richtext::article_document_from_plain_text("Body"),
                    excerpt: None,
                    slug: Some(slug.to_string()),
                    publish: true,
                    tags: vec![],
                    category_id: None,
                    featured_image_url: None,
                    seo_title: None,
                    seo_description: None,
                    channel_slugs,
                    metadata: None,
                    scheduled_at: None,
                },
            )
            .await
            .expect("post should be created");
    }

    let visible = post_service
        .list_public_visible_keyset(
            tenant_id,
            PublicPostsPageQuery {
                locale: Some("en".to_string()),
                per_page: Some(10),
                ..Default::default()
            },
            Some("en"),
            Some("web"),
        )
        .await
        .expect("public visible list should succeed");

    assert!(visible.next_cursor.is_none());
    let slugs = visible
        .items
        .into_iter()
        .map(|item| item.slug)
        .collect::<Vec<_>>();
    assert!(slugs.contains(&"web-visible".to_string()));
    assert!(slugs.contains(&"global".to_string()));
    assert!(!slugs.contains(&"mobile-only".to_string()));
}

#[tokio::test]
async fn public_visible_keyset_keeps_pinned_post_via_cursor_rank() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    seed_channel(&db, tenant_id, "web").await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let mut created = Vec::new();
    for index in 0..5 {
        let post = post_service
            .create_post(
                tenant_id,
                admin.clone(),
                CreatePostInput {
                    locale: "en".to_string(),
                    title: format!("Paged {index}"),
                    content: crate::richtext::article_document_from_plain_text("Body"),
                    excerpt: None,
                    slug: Some(format!("paged-{index}")),
                    publish: true,
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
            .expect("post should be created");
        created.push(post);
    }

    // The oldest post is pinned after publication. A cursor that only carried
    // `(published_at, id)` could not describe this rank, so paging would either
    // repeat or skip the remaining posts.
    let pinned = created[0];
    post_service
        .pin_post(tenant_id, pinned, admin.clone())
        .await
        .expect("published post should be pinnable");

    let mut seen: Vec<Uuid> = Vec::new();
    let mut after: Option<PublishedPostCursor> = None;
    let mut pages = 0;
    loop {
        pages += 1;
        assert!(pages <= 5, "keyset pagination must terminate");
        let page = post_service
            .list_public_visible_keyset(
                tenant_id,
                PublicPostsPageQuery {
                    locale: Some("en".to_string()),
                    per_page: Some(2),
                    after,
                    ..Default::default()
                },
                Some("en"),
                Some("web"),
            )
            .await
            .expect("public keyset page should succeed");
        seen.extend(page.items.iter().map(|item| item.id));

        match page.next_cursor {
            Some(cursor) => {
                let decoded = PublishedPostCursor::decode(&cursor.encode())
                    .expect("cursor must survive the client boundary");
                assert_eq!(decoded, cursor);
                after = Some(cursor);
            }
            None => break,
        }
    }

    assert_eq!(
        seen.first(),
        Some(&pinned),
        "pinned post leads the first page"
    );
    assert_eq!(seen.len(), created.len(), "no post may be repeated");
    let mut expected = created.clone();
    expected.sort();
    let mut actual = seen.clone();
    actual.sort();
    assert_eq!(
        actual, expected,
        "every published post must be reached once"
    );
}

#[tokio::test]
async fn customer_list_hides_a_published_post_without_a_publication_timestamp() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let healthy = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Healthy".to_string(),
                content: crate::richtext::article_document_from_plain_text("Body"),
                excerpt: None,
                slug: Some("healthy".to_string()),
                publish: true,
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
        .expect("published post should be created");
    let damaged = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Damaged".to_string(),
                content: crate::richtext::article_document_from_plain_text("Body"),
                excerpt: None,
                slug: Some("damaged".to_string()),
                publish: true,
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
        .expect("published post should be created");

    crate::tests::hardening::allow_legacy_invalid_post_fixture(&db).await;
    let row = blog_post::Entity::find_by_id(damaged)
        .one(&db)
        .await
        .expect("post read should not fail")
        .expect("post should exist");
    let mut active: blog_post::ActiveModel = row.into();
    active.published_at = sea_orm::Set(None);
    active.update(&db).await.expect("damage should persist");

    let listed_for_customer = post_service
        .list_posts(
            tenant_id,
            customer.clone(),
            PostListQuery {
                per_page: Some(10),
                ..Default::default()
            },
        )
        .await
        .expect("customer listing should succeed");
    let visible = listed_for_customer
        .items
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    assert_eq!(
        visible,
        vec![healthy],
        "only the intact published post belongs in the customer list"
    );

    let denied_detail = post_service
        .get_post(tenant_id, customer.clone(), damaged, "en")
        .await
        .expect_err("unstamped published detail is not public");
    assert!(matches!(denied_detail, BlogError::Forbidden(_)));
    assert!(
        post_service
            .get_post_by_slug(tenant_id, customer.clone(), "en", "damaged")
            .await
            .expect("slug lookup should succeed")
            .is_none(),
        "unstamped published post must not be exposed through the slug lookup"
    );
    assert_eq!(
        post_service
            .get_post(tenant_id, admin.clone(), damaged, "en")
            .await
            .expect("an editor may repair the broken post")
            .id,
        damaged
    );

    // The explicit `status: Published` request takes the other branch and must
    // apply the same rule.
    let explicit = post_service
        .list_posts(
            tenant_id,
            customer,
            PostListQuery {
                status: Some(BlogPostStatus::Published),
                per_page: Some(10),
                ..Default::default()
            },
        )
        .await
        .expect("explicit-status listing should succeed");
    assert_eq!(explicit.items.len(), 1);

    let listed_for_admin = post_service
        .list_posts(
            tenant_id,
            admin,
            PostListQuery {
                per_page: Some(10),
                ..Default::default()
            },
        )
        .await
        .expect("admin listing should succeed");
    assert_eq!(
        listed_for_admin.items.len(),
        2,
        "an authorized editor still sees the damaged row they have to repair"
    );
}

/// The public cursor must be calculated over renderable posts, not a page
/// containing posts whose translations have been lost.

#[tokio::test]
async fn public_keyset_skips_untranslated_pinned_post_before_limit() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;
    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);
    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let mut broken_input = crate::tests::hardening::post_input("missing-translation");
    broken_input.publish = true;
    let broken = post_service
        .create_post(tenant_id, admin.clone(), broken_input)
        .await
        .expect("published post should be created");
    let mut healthy_input = crate::tests::hardening::post_input("healthy-translation");
    healthy_input.publish = true;
    let healthy = post_service
        .create_post(tenant_id, admin.clone(), healthy_input)
        .await
        .expect("second published post should be created");
    post_service
        .pin_post(tenant_id, broken, admin)
        .await
        .expect("the future damaged post is pinnable");

    blog_post_translation::Entity::delete_many()
        .filter(blog_post_translation::Column::PostId.eq(broken))
        .exec(&db)
        .await
        .expect("remove the damaged post's translation fixture");

    let page = post_service
        .list_public_visible_keyset(
            tenant_id,
            PublicPostsPageQuery {
                locale: Some("en".to_string()),
                per_page: Some(1),
                ..Default::default()
            },
            Some("en"),
            None,
        )
        .await
        .expect("public listing should succeed");

    assert_eq!(
        page.items.iter().map(|post| post.id).collect::<Vec<_>>(),
        vec![healthy],
        "damaged pinned post cannot consume the only slot of the first page"
    );
    assert!(
        page.next_cursor.is_none(),
        "the sole renderable post must not produce a phantom next page"
    );
}

/// The authenticated post endpoints used to expose channel-restricted posts to
/// non-editor readers, even though the public Blog list hid them.

#[tokio::test]
async fn customer_channel_scope_applies_before_post_list_keyset() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PostService::new(db.clone(), event_bus);
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    seed_channel(&db, tenant_id, "web").await;
    seed_channel(&db, tenant_id, "mobile").await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let mut restricted = crate::tests::hardening::post_input("mobile-secret");
    restricted.publish = true;
    restricted.channel_slugs = Some(vec!["mobile".to_string()]);
    let mobile_id = service
        .create_post(tenant_id, admin.clone(), restricted)
        .await
        .expect("mobile published post should be created");

    let mut global = crate::tests::hardening::post_input("public-global");
    global.publish = true;
    let global_id = service
        .create_post(tenant_id, admin.clone(), global)
        .await
        .expect("global published post should be created");

    assert!(
        service
            .get_post_with_locale_fallback_on_channel(
                tenant_id,
                customer.clone(),
                mobile_id,
                "en",
                Some("en"),
                Some("web"),
            )
            .await
            .is_err(),
        "a web reader must not read a mobile-only post by id"
    );
    assert!(
        service
            .get_post_with_locale_fallback(tenant_id, customer.clone(), mobile_id, "en", Some("en"))
            .await
            .is_err(),
        "an unscoped public reader may only see globally visible posts"
    );
    assert_eq!(
        service
            .get_post_with_locale_fallback_on_channel(
                tenant_id,
                admin.clone(),
                mobile_id,
                "en",
                Some("en"),
                Some("web"),
            )
            .await
            .expect("editor can inspect any channel")
            .id,
        mobile_id
    );

    let first = service
        .list_posts_with_locale_fallback_on_channel(
            tenant_id,
            customer.clone(),
            PostListQuery {
                locale: Some("en".to_string()),
                per_page: Some(1),
                ..Default::default()
            },
            Some("en"),
            Some("web"),
        )
        .await
        .expect("customer list should succeed");
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].id, global_id);
    assert!(
        first.next_cursor.is_none(),
        "hidden rows cannot create a phantom next page"
    );

    let mobile = service
        .list_posts_with_locale_fallback_on_channel(
            tenant_id,
            customer,
            PostListQuery {
                locale: Some("en".to_string()),
                per_page: Some(10),
                ..Default::default()
            },
            Some("en"),
            Some("mobile"),
        )
        .await
        .expect("mobile scoped list should succeed");
    assert_eq!(mobile.items.len(), 2);

    let all_for_admin = service
        .list_posts_with_locale_fallback_on_channel(
            tenant_id,
            admin,
            PostListQuery::default(),
            Some("en"),
            Some("web"),
        )
        .await
        .expect("editor list should succeed");
    assert_eq!(all_for_admin.items.len(), 2);
}

/// Comments and post listings must agree on a canonical channel slug. A route
/// carrying an uppercase/space-padded slug must not turn a visible post into a
/// false 404 or mask a Channel owner failure as missing content.

#[tokio::test]
async fn public_comment_target_uses_same_normalized_channel_as_post_listing() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;
    let events = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let posts = PostService::new(db.clone(), events);
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    seed_channel(&db, tenant_id, "web").await;
    let mut input = crate::tests::hardening::post_input("channel-comments");
    input.publish = true;
    input.channel_slugs = Some(vec!["web".to_string()]);
    let post_id = posts
        .create_post(
            tenant_id,
            SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4())),
            input,
        )
        .await
        .expect("channel restricted post should publish");

    let comments = crate::CommentService::from_runtime_capabilities(db, None, None);
    comments
        .ensure_public_post_visible(tenant_id, post_id, Some(" WEB "))
        .await
        .expect("normalized channel permits the published post");
    assert!(
        matches!(
            comments
                .ensure_public_post_visible(tenant_id, post_id, Some("other"))
                .await,
            Err(BlogError::PostNotFound(_))
        ),
        "unmatched channel must fail closed"
    );
    assert!(
        matches!(
            comments
                .ensure_public_post_visible(tenant_id, post_id, Some("  "))
                .await,
            Err(BlogError::PostNotFound(_))
        ),
        "empty channel does not inherit another channel's authorization"
    );
}
