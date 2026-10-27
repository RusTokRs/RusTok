use super::*;
use std::sync::Arc;

use rustok_core::MigrationSource;
use rustok_core::{SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, SysEventsMigration};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};

async fn setup_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:blog_service_post_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);

    Database::connect(opts)
        .await
        .expect("failed to connect blog test sqlite database")
}

async fn ensure_blog_schema(db: &DatabaseConnection) {
    db.execute_raw(Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE IF NOT EXISTS tenants (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            domain TEXT NULL UNIQUE,
            settings TEXT NOT NULL DEFAULT '{}',
            default_locale TEXT NOT NULL DEFAULT 'en',
            is_active BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    ))
    .await
    .expect("tenants table should exist for channel foreign keys");

    let manager = SchemaManager::new(db);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in rustok_channel::migrations::migrations() {
        if db.get_database_backend() == sea_orm::DatabaseBackend::Sqlite
            && (migration.name() == "m20260730_000010_add_channel_index_revision"
                || migration.name() == "m20260731_000011_add_channel_index_tombstones"
                || migration.name() == "m20260807_000012_add_channel_index_identity_generation")
        {
            continue;
        }
        migration
            .up(&manager)
            .await
            .expect("channel migration should apply");
    }
    for migration in crate::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("blog migration should apply");
    }
}

async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT OR IGNORE INTO tenants (id, name, slug, settings, default_locale, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        [
            tenant_id.into(),
            format!("Tenant {tenant_id}").into(),
            format!("tenant-{tenant_id}").into(),
            "{}".to_string().into(),
            "en".to_string().into(),
            true.into(),
        ],
    ))
    .await
    .expect("tenant should be seeded");
}

async fn seed_channel(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) {
    let service = rustok_channel::ChannelService::new(db.clone());
    service
        .create_channel(rustok_channel::CreateChannelInput {
            tenant_id,
            slug: slug.to_string(),
            name: format!("Channel {slug}"),
            settings: None,
        })
        .await
        .expect("channel should be created");
}

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

/// The customer-facing list is a public surface, so it applies the same corpus
/// rule as the public keyset list, the SEO scan and the comment target gate.
///
/// A published row whose `published_at` was lost is damaged state: an editor with
/// tenant-wide read authority still sees it (that is how it gets repaired), a
/// customer does not — the list used to filter on `status` only and showed it.
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
