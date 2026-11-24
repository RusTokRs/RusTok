mod support;

use std::{future::Future, sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{Patch, RichTextDocument};
use rustok_blog::{
    BlogCategoryDeleteCleanupPort, BlogError, CategoryService, CreateCategoryInput, CreateTagInput,
    NoopBlogCategoryDeleteCleanupPort, PostService, TagService, UpdateCategoryInput,
    dto::{CreatePostInput, ListTagsFilter, PublicPostsPageQuery, UpdatePostInput, UpdateTagInput},
};
use rustok_content_orchestration::in_process_blog_taxonomy_port;
use rustok_core::{SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::{TaxonomyService, TaxonomyTermKind};
use rustok_test_utils::db::{
    connect_postgres, create_postgres_database, drop_postgres_database_if_exists,
    postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend,
    Statement, TransactionTrait,
    sea_query::{Alias, Expr, Query},
};
use uuid::Uuid;

// The identical owner scenario runs on both supported profiles. PostgreSQL's
// trigger/lock enforcement still has its separate contention tests; a portable
// scenario must never imply that SQLite proves those engine-specific effects.
async fn channel_publication_and_locale_contract(db: DatabaseConnection) {
    support::apply_blog_schema(&db).await.unwrap();
    let tenant = insert_tenant(&db).await;
    let foreign_tenant = insert_tenant(&db).await;
    for slug in ["web", "mobile"] {
        rustok_channel::ChannelService::new(db.clone())
            .create_channel(rustok_channel::CreateChannelInput {
                tenant_id: tenant,
                slug: slug.into(),
                name: slug.into(),
                settings: None,
            })
            .await
            .unwrap();
    }
    let posts = PostService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
    );
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let mut expected = Vec::new();
    for (owner, slug, published, channels, visible) in [
        (tenant, "web-a", true, Some(vec!["web".into()]), true),
        (tenant, "web-b", true, None, true),
        (
            tenant,
            "mobile-only",
            true,
            Some(vec!["mobile".into()]),
            false,
        ),
        (tenant, "draft", false, None, false),
        (foreign_tenant, "foreign", true, None, false),
    ] {
        let id = posts
            .create_post(
                owner,
                admin.clone(),
                CreatePostInput {
                    command_id: uuid::Uuid::new_v4(),
                    locale: "en".into(),
                    title: slug.into(),
                    content: RichTextDocument::single_paragraph("Fixture body"),
                    excerpt: None,
                    slug: Some(slug.into()),
                    publish: published,
                    tags: vec![],
                    category_id: None,
                    featured_image_url: None,
                    seo_title: None,
                    seo_description: None,
                    channel_slugs: channels,
                    metadata: None,
                    scheduled_at: None,
                },
            )
            .await
            .unwrap();
        if visible {
            expected.push(id);
        }
    }
    let mut cursor = None;
    let mut actual = Vec::new();
    loop {
        let page = posts
            .list_public_visible_keyset(
                tenant,
                PublicPostsPageQuery {
                    locale: Some("fr".into()),
                    per_page: Some(1),
                    after: cursor,
                    ..Default::default()
                },
                Some("en"),
                Some("web"),
            )
            .await
            .unwrap();
        assert_eq!(
            page.items.len(),
            1,
            "hidden rows must be filtered before page limits"
        );
        let post = &page.items[0];
        assert_eq!(post.locale, "fr");
        assert_eq!(post.effective_locale, "en");
        assert_eq!(post.available_locales, ["en"]);
        actual.push(post.id);
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
        assert!(
            actual.len() < 3,
            "keyset traversal must not loop or repeat rows"
        );
    }
    actual.sort();
    expected.sort();
    assert_eq!(
        actual, expected,
        "one page per visible owner row, no draft/channel/tenant leaks"
    );
}

async fn insert_tenant(db: &DatabaseConnection) -> Uuid {
    let id = Uuid::new_v4();
    let mut insert = Query::insert();
    insert
        .into_table(Alias::new("tenants"))
        .columns([Alias::new("id"), Alias::new("name"), Alias::new("slug")])
        .values_panic([
            Expr::value(id),
            Expr::value("Fixture"),
            Expr::value(id.to_string()),
        ]);
    db.execute_raw(db.get_database_backend().build(&insert))
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn sqlite_shipped_migrations_preserve_channel_publication_and_locale_contract() {
    channel_publication_and_locale_contract(rustok_test_utils::setup_test_db().await).await;
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL and disposable-database permission"]
async fn postgres_shipped_migrations_preserve_channel_publication_and_locale_contract() {
    with_postgres_database("blog_profiles", |db, _| {
        channel_publication_and_locale_contract(db)
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL and disposable-database permission"]
async fn postgres_embedded_blog_taxonomy_lifecycle() {
    with_postgres_database("blog_taxonomy", |db, _| embedded_taxonomy_lifecycle(db)).await;
}

async fn with_postgres_database<F, Fut>(prefix: &str, scenario: F)
where
    F: FnOnce(DatabaseConnection, String) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let url = support::postgres_database_url().unwrap();
    let control = connect_postgres(&url).await.unwrap();
    let name = unique_postgres_database_name(prefix);
    create_postgres_database(&control, &name).await.unwrap();
    let url = postgres_database_url(&url, &name);
    let result = tokio::spawn(async move {
        scenario(connect_postgres(&url).await.unwrap(), url).await;
    })
    .await;
    drop_postgres_database_if_exists(&control, &name)
        .await
        .unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error.into_panic());
    }
}

struct RejectAttachedCleanup;

#[async_trait]
impl BlogCategoryDeleteCleanupPort for RejectAttachedCleanup {
    async fn cleanup_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _category_id: Uuid,
    ) -> Result<(), BlogError> {
        Err(BlogError::conflict("fixture rejected attached cleanup"))
    }
}

async fn embedded_taxonomy_lifecycle(db: DatabaseConnection) {
    support::apply_blog_schema(&db).await.unwrap();
    let tenant = insert_tenant(&db).await;
    let foreign_tenant = insert_tenant(&db).await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let port = in_process_blog_taxonomy_port(db.clone());
    let bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let categories = CategoryService::new(db.clone(), bus.clone())
        .with_taxonomy_port(port.clone())
        .with_category_delete_cleanup(Arc::new(RejectAttachedCleanup));
    let posts = PostService::new(db.clone(), bus).with_taxonomy_port(port.clone());
    let tags = TagService::new(db.clone()).with_taxonomy_port(port);
    let taxonomy = TaxonomyService::new(db);

    let category_id = categories
        .create(
            tenant,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "Field notes".into(),
                slug: Some("field-notes".into()),
                description: None,
                parent_id: None,
                position: Some(0),
                settings: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    let mut draft = draft_post("An expedition");
    draft.tags = vec!["Expedition".into()];
    draft.category_id = Some(category_id);
    let post_id = posts
        .create_post(tenant, admin.clone(), draft)
        .await
        .unwrap();
    let category = categories
        .update(
            tenant,
            category_id,
            admin.clone(),
            UpdateCategoryInput {
                locale: "en".into(),
                name: Some("Dispatches".into()),
                slug: Some("dispatches".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(category.slug, "dispatches");
    let old_route = taxonomy
        .resolve_term_route_for_module(
            tenant,
            TaxonomyTermKind::Category,
            "blog",
            "en",
            None,
            "field-notes",
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old_route.term_id, category_id);
    assert!(old_route.alias_id.is_some());

    let (items, total) = tags
        .list_tags(tenant, admin.clone(), ListTagsFilter::default())
        .await
        .unwrap();
    assert_eq!(total, 1);
    let tag_id = items[0].id;
    assert!(matches!(
        tags.get_tag(foreign_tenant, admin.clone(), tag_id, "en")
            .await,
        Err(BlogError::TagNotFound(_))
    ));
    tags.update_tag(
        tenant,
        tag_id,
        admin.clone(),
        UpdateTagInput {
            locale: "en".into(),
            name: Some("Journey".into()),
            slug: None,
        },
    )
    .await
    .unwrap();
    let before = posts
        .get_post(tenant, admin.clone(), post_id, "en")
        .await
        .unwrap();
    assert_eq!(before.category_name.as_deref(), Some("Dispatches"));
    assert_eq!(before.tags, ["Journey"]);

    assert!(matches!(
        categories.delete(tenant, category_id, admin.clone()).await,
        Err(BlogError::Conflict(message)) if message == "fixture rejected attached cleanup"
    ));
    let rolled_back = posts
        .get_post(tenant, admin.clone(), post_id, "en")
        .await
        .unwrap();
    assert_eq!(rolled_back.category_id, Some(category_id));
    assert_eq!(rolled_back.version, before.version);

    let categories =
        categories.with_category_delete_cleanup(Arc::new(NoopBlogCategoryDeleteCleanupPort));
    categories
        .delete(tenant, category_id, admin.clone())
        .await
        .unwrap();
    tags.delete_tag(tenant, tag_id, admin.clone())
        .await
        .unwrap();
    let detached = posts.get_post(tenant, admin, post_id, "en").await.unwrap();
    assert_eq!(detached.category_id, None);
    assert!(detached.tags.is_empty());
    assert_eq!(detached.version, before.version + 2);
}

fn draft_post(title: &str) -> CreatePostInput {
    CreatePostInput {
        command_id: Uuid::new_v4(),
        locale: "en".into(),
        title: title.into(),
        content: RichTextDocument::single_paragraph("Fixture body"),
        excerpt: None,
        slug: None,
        publish: false,
        tags: Vec::new(),
        category_id: None,
        featured_image_url: None,
        seo_title: None,
        seo_description: None,
        channel_slugs: None,
        metadata: None,
        scheduled_at: None,
    }
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL and disposable-database permission"]
async fn postgres_concurrent_tag_removals_keep_the_usage_projection_current() {
    with_postgres_database("blog_tag_usage", concurrent_tag_removals).await;
}

async fn concurrent_tag_removals(db: DatabaseConnection, url: String) {
    support::apply_blog_schema(&db).await.unwrap();
    let tenant = insert_tenant(&db).await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let tags =
        TagService::new(db.clone()).with_taxonomy_port(in_process_blog_taxonomy_port(db.clone()));
    let tag_id = tags
        .create_tag(
            tenant,
            admin.clone(),
            CreateTagInput {
                locale: "en".into(),
                name: "Shared".into(),
                slug: None,
            },
        )
        .await
        .unwrap();
    let posts = tag_aware_posts(db.clone());
    let mut post_ids = Vec::new();
    for title in ["First draft", "Second draft"] {
        let mut draft = draft_post(title);
        draft.tags = vec!["Shared".into()];
        post_ids.push(
            posts
                .create_post(tenant, admin.clone(), draft)
                .await
                .unwrap(),
        );
    }

    let hold = db.begin().await.unwrap();
    hold.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT tag_id FROM blog_tag_usage WHERE tenant_id = $1 AND tag_id = $2 FOR UPDATE",
        [tenant.into(), tag_id.into()],
    ))
    .await
    .unwrap()
    .expect("attached tag has a usage projection row");

    let mut writers = Vec::new();
    for (post, name) in post_ids
        .into_iter()
        .zip(["blog_tag_writer_a", "blog_tag_writer_b"])
    {
        let mut options = ConnectOptions::new(url.clone());
        options
            .max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);
        let worker = Database::connect(options).await.unwrap();
        worker
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT set_config('application_name', $1, false)",
                [name.into()],
            ))
            .await
            .unwrap();
        let service = tag_aware_posts(worker);
        let actor = admin.clone();
        writers.push(tokio::spawn(async move {
            service
                .update_post(
                    tenant,
                    post,
                    actor,
                    UpdatePostInput {
                        locale: Some("en".into()),
                        title: None,
                        content: None,
                        excerpt: Patch::Keep,
                        slug: None,
                        tags: Some(Vec::new()),
                        category_id: Patch::Keep,
                        featured_image_url: Patch::Keep,
                        seo_title: Patch::Keep,
                        seo_description: Patch::Keep,
                        channel_slugs: None,
                        metadata: None,
                        version: 1,
                        scheduled_at: Patch::Keep,
                    },
                )
                .await
        }));
    }
    let wait = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let row = db.query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT COUNT(*) AS waiters FROM pg_stat_activity WHERE datname = current_database() \
                 AND application_name IN ('blog_tag_writer_a', 'blog_tag_writer_b') AND wait_event_type = 'Lock'",
            )).await.unwrap().unwrap();
            if row.try_get::<i64>("", "waiters").unwrap() == 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await;
    hold.commit().await.unwrap();
    for writer in writers {
        writer.await.unwrap().unwrap();
    }
    wait.expect("both writers should reach contention while the usage row is held");
    let tag = tags.get_tag(tenant, admin, tag_id, "en").await.unwrap();
    assert_eq!(tag.use_count, 0);
}

fn tag_aware_posts(db: DatabaseConnection) -> PostService {
    let taxonomy = in_process_blog_taxonomy_port(db.clone());
    PostService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db))),
    )
    .with_taxonomy_port(taxonomy)
}
