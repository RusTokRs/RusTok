// Admin text filters (`title`, `slug`) on the authenticated post list.
//
// These tests pin the contract the admin tables rely on: a case-insensitive
// substring match that treats `LIKE` wildcard characters literally, resolves
// the title through the same locale chain the projection displays, and
// normalizes whitespace/length instead of silently dropping a broken needle.

fn filter_post_input(locale: &str, title: &str, slug: &str) -> CreatePostInput {
    CreatePostInput {
        command_id: uuid::Uuid::new_v4(),
        locale: locale.into(),
        title: title.into(),
        content: crate::richtext::article_document_from_plain_text("Body"),
        excerpt: None,
        slug: Some(slug.into()),
        publish: true,
        tags: vec![],
        category_id: None,
        featured_image_url: None,
        seo_title: None,
        seo_description: None,
        channel_slugs: None,
        metadata: None,
        scheduled_at: None,
    }
}

fn filter_translation_input(locale: &str, title: &str, version: i32) -> UpdatePostInput {
    UpdatePostInput {
        locale: Some(locale.into()),
        title: Some(title.into()),
        content: Some(crate::richtext::article_document_from_plain_text(
            "Translated body",
        )),
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
async fn title_filter_uses_the_displayed_translation_before_keyset_pagination() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;
    assert_displayed_title_filter_pages(db).await;
}

async fn assert_displayed_title_filter_pages(db: DatabaseConnection) {
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PostService::new(db.clone(), event_bus);
    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let mut ids = Vec::new();
    for (slug, en, ru) in [
        ("hidden-match", "Orange", "Apple"),
        ("first-match", "Apple one", "Other one"),
        ("second-match", "Apple two", "Other two"),
    ] {
        let id = service
            .create_post(tenant_id, admin.clone(), filter_post_input("en", en, slug))
            .await
            .unwrap();
        let post = service
            .get_post(tenant_id, admin.clone(), id, "en")
            .await
            .unwrap();
        service
            .update_post(
                tenant_id,
                id,
                admin.clone(),
                filter_translation_input("ru", ru, post.version),
            )
            .await
            .unwrap();
        ids.push(id);
    }
    let query = PostListQuery {
        locale: Some("en".into()),
        title: Some("apple".into()),
        per_page: Some(1),
        sort_by: Some(PostSortField::CreatedAt),
        sort_order: Some(PostSortOrder::Asc),
        ..Default::default()
    };
    let first = service
        .list_posts_with_locale_fallback(tenant_id, admin.clone(), query.clone(), Some("ru"))
        .await
        .unwrap();
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].id, ids[1]);
    assert_eq!(first.items[0].title, "Apple one");
    assert!(first.next_cursor.is_some());
    let second = service
        .list_posts_with_locale_fallback(
            tenant_id,
            admin.clone(),
            PostListQuery {
                after: first.next_cursor,
                ..query.clone()
            },
            Some("ru"),
        )
        .await
        .unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].id, ids[2]);
    assert!(second.next_cursor.is_none());

    let reversed = service
        .list_posts_with_locale_fallback(
            tenant_id,
            admin,
            PostListQuery {
                locale: Some("ru".into()),
                ..query
            },
            Some("en"),
        )
        .await
        .unwrap();
    assert_eq!(reversed.items.len(), 1);
    assert_eq!(reversed.items[0].id, ids[0]);
    assert_eq!(reversed.items[0].effective_locale, "ru");
    assert!(reversed.next_cursor.is_none());
}

#[tokio::test]
async fn title_filter_uses_the_same_final_locale_fallback_as_the_projection() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;
    assert_final_title_filter_locale(db).await;
}

async fn assert_final_title_filter_locale(db: DatabaseConnection) {
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PostService::new(db.clone(), event_bus);
    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let id = service
        .create_post(
            tenant_id,
            admin.clone(),
            filter_post_input("ent", "Hidden fallback", "last-fallback"),
        )
        .await
        .unwrap();
    let post = service
        .get_post(tenant_id, admin.clone(), id, "ent")
        .await
        .unwrap();
    // Rust orders en-US before ent. The database's language collation must not
    // change which remaining translation the projection and filter choose.
    service
        .update_post(
            tenant_id,
            id,
            admin.clone(),
            filter_translation_input("en-US", "Displayed fallback", post.version),
        )
        .await
        .unwrap();
    for (needle, matches) in [("Displayed", true), ("Hidden", false)] {
        let result = service
            .list_posts_with_locale_fallback(
                tenant_id,
                admin.clone(),
                PostListQuery {
                    locale: Some("it".into()),
                    title: Some(needle.into()),
                    ..Default::default()
                },
                Some("es"),
            )
            .await
            .unwrap();
        assert_eq!(result.items.len(), usize::from(matches));
        if matches {
            assert_eq!(result.items[0].id, id);
            assert_eq!(result.items[0].effective_locale, "en-US");
            assert_eq!(result.items[0].title, "Displayed fallback");
        }
    }
}

#[tokio::test]
#[ignore = "requires RUSTOK_BLOG_TEST_DATABASE_URL and permission to create a disposable database"]
async fn postgres_title_filter_and_locale_storage_contract() {
    use rustok_test_utils::db::{
        assert_postgres_url, connect_postgres, create_postgres_database,
        drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
    };
    let url = std::env::var("RUSTOK_BLOG_TEST_DATABASE_URL")
        .expect("set RUSTOK_BLOG_TEST_DATABASE_URL to a PostgreSQL test instance");
    assert_postgres_url(&url);
    let control = connect_postgres(&url).await.unwrap();
    let name = unique_postgres_database_name("blog_title_filter");
    create_postgres_database(&control, &name).await.unwrap();
    let url = postgres_database_url(&url, &name);
    let result = tokio::spawn(async move {
        let db = connect_postgres(&url).await.unwrap();
        schema::apply_blog_schema(&db, crate::migrations::migrations())
            .await
            .unwrap();
        assert_displayed_title_filter_pages(db.clone()).await;
        assert_final_title_filter_locale(db.clone()).await;
        let service = PostService::new(
            db.clone(),
            TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
        );
        let tenant_id = Uuid::new_v4();
        let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
        let locale = "sl-Latn-SI-rozaj-biske";
        assert!(locale.len() > 16);
        let id = service
            .create_post(
                tenant_id,
                admin.clone(),
                filter_post_input(locale, "Long locale", "long-locale"),
            )
            .await
            .unwrap();
        crate::migrations::m20261030_000001_widen_blog_translation_columns::Migration
            .down(&SchemaManager::new(&db))
            .await
            .unwrap();
        let column = db
            .query_one_raw(Statement::from_string(
                db.get_database_backend(),
                "SELECT character_maximum_length FROM information_schema.columns
             WHERE table_schema = current_schema() AND table_name = 'blog_post_translations'
               AND column_name = 'locale'"
                    .to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            column
                .try_get::<i32>("", "character_maximum_length")
                .unwrap(),
            32
        );
        // Width DDL changes PostgreSQL result descriptors. Reconnect as a
        // restarted application would, rather than reusing pre-DDL plans.
        drop(service);
        db.close().await.unwrap();
        let db = connect_postgres(&url).await.unwrap();
        let service = PostService::new(
            db.clone(),
            TransactionalEventBus::new(Arc::new(OutboxTransport::new(db))),
        );
        let post = service
            .get_post(tenant_id, admin, id, locale)
            .await
            .unwrap();
        assert_eq!(post.effective_locale, "sl-Latn-SI-biske-rozaj");
        assert_eq!(post.available_locales, ["sl-Latn-SI-biske-rozaj"]);
        assert_eq!(post.title, "Long locale");
    })
    .await;
    drop_postgres_database_if_exists(&control, &name)
        .await
        .unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error.into_panic());
    }
}

#[tokio::test]
async fn admin_list_title_filter_matches_case_insensitively() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    for (slug, title) in [
        ("alpha-launch", "Alpha Launch"),
        ("beta-release", "Beta Release"),
    ] {
        post_service
            .create_post(
                tenant_id,
                admin.clone(),
                CreatePostInput {
                    command_id: uuid::Uuid::new_v4(),
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
                    channel_slugs: None,
                    metadata: None,
                    scheduled_at: None,
                },
            )
            .await
            .expect("post should be created");
    }

    let matched = post_service
        .list_posts(
            tenant_id,
            admin.clone(),
            PostListQuery {
                locale: Some("en".to_string()),
                title: Some("alpha LAUNCH".to_string()),
                per_page: Some(10),
                ..Default::default()
            },
        )
        .await
        .expect("title-filtered list should succeed");

    assert_eq!(
        matched.items.len(),
        1,
        "the title filter must match exactly one post"
    );
    assert_eq!(matched.items[0].slug, "alpha-launch");
}

#[tokio::test]
async fn admin_list_title_filter_escapes_like_wildcards() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    for (slug, title) in [
        ("percent-post", "100% Rust"),
        ("underscore-post", "Wild_Card"),
        ("plain-post", "Plain Title"),
    ] {
        post_service
            .create_post(
                tenant_id,
                admin.clone(),
                CreatePostInput {
                    command_id: uuid::Uuid::new_v4(),
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
                    channel_slugs: None,
                    metadata: None,
                    scheduled_at: None,
                },
            )
            .await
            .expect("post should be created");
    }

    // A literal `%`/`_` needle must match only the post whose title contains
    // those characters, never every row.
    for (needle, expected_slug) in [
        ("100% Rust", "percent-post"),
        ("Wild_Card", "underscore-post"),
    ] {
        let matched = post_service
            .list_posts(
                tenant_id,
                admin.clone(),
                PostListQuery {
                    locale: Some("en".to_string()),
                    title: Some(needle.to_string()),
                    per_page: Some(10),
                    ..Default::default()
                },
            )
            .await
            .expect("title-filtered list should succeed");
        assert_eq!(
            matched.items.len(),
            1,
            "needle {needle:?} must match exactly one post"
        );
        assert_eq!(matched.items[0].slug, expected_slug);
    }
}

#[tokio::test]
async fn admin_list_title_filter_resolves_the_fallback_locale_translation() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    // The only translation is `en`; the list is requested in `ru` with the
    // tenant's `en` fallback, so the displayed (and matched) title is English.
    post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".to_string(),
                title: "Fallback Locale Title".to_string(),
                content: crate::richtext::article_document_from_plain_text("Body"),
                excerpt: None,
                slug: Some("fallback-locale".to_string()),
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

    let matched = post_service
        .list_posts_with_locale_fallback(
            tenant_id,
            admin.clone(),
            PostListQuery {
                locale: Some("ru".to_string()),
                title: Some("fallback locale".to_string()),
                per_page: Some(10),
                ..Default::default()
            },
            Some("en"),
        )
        .await
        .expect("title-filtered fallback list should succeed");

    assert_eq!(
        matched.items.len(),
        1,
        "the title filter must resolve the fallback translation"
    );
    assert_eq!(matched.items[0].effective_locale, "en");
}

#[tokio::test]
async fn admin_list_slug_filter_matches_substring_case_insensitively() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    for (slug, title) in [
        ("news-october", "October News"),
        ("deep-dive-rust", "Rust Deep Dive"),
    ] {
        post_service
            .create_post(
                tenant_id,
                admin.clone(),
                CreatePostInput {
                    command_id: uuid::Uuid::new_v4(),
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
                    channel_slugs: None,
                    metadata: None,
                    scheduled_at: None,
                },
            )
            .await
            .expect("post should be created");
    }

    let matched = post_service
        .list_posts(
            tenant_id,
            admin.clone(),
            PostListQuery {
                locale: Some("en".to_string()),
                slug: Some("OCTOBER".to_string()),
                per_page: Some(10),
                ..Default::default()
            },
        )
        .await
        .expect("slug-filtered list should succeed");

    assert_eq!(
        matched.items.len(),
        1,
        "the slug filter must match exactly one post"
    );
    assert_eq!(matched.items[0].slug, "news-october");
}

#[tokio::test]
async fn admin_list_text_filters_normalize_blank_and_reject_oversized_needles() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;

    let transport = OutboxTransport::new(db.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    let post_service = PostService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".to_string(),
                title: "Only Post".to_string(),
                content: crate::richtext::article_document_from_plain_text("Body"),
                excerpt: None,
                slug: Some("only-post".to_string()),
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

    // A blank needle means "no filter": every post is returned.
    let blank = post_service
        .list_posts(
            tenant_id,
            admin.clone(),
            PostListQuery {
                locale: Some("en".to_string()),
                title: Some("   ".to_string()),
                slug: Some("\t".to_string()),
                per_page: Some(10),
                ..Default::default()
            },
        )
        .await
        .expect("blank-filtered list should succeed");
    assert_eq!(blank.items.len(), 1, "a blank needle must not filter");

    // An oversized needle is a validation error, never a silently dropped filter.
    let oversized = "x".repeat(256);
    let rejected = post_service
        .list_posts(
            tenant_id,
            admin.clone(),
            PostListQuery {
                locale: Some("en".to_string()),
                title: Some(oversized),
                per_page: Some(10),
                ..Default::default()
            },
        )
        .await;
    assert!(
        rejected.is_err(),
        "an oversized title needle must be rejected"
    );
}
