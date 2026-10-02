use std::error::Error;
use std::sync::Arc;

use chrono::Utc;
use rustok_blog::{BlogPostStatus, PostService};
use rustok_channel::ChannelModule;
use rustok_content::entities::node::ContentStatus;
use rustok_core::{MigrationSource, SecurityContext};
use rustok_forum::TopicService;
use rustok_forum::dto::ListTopicsFilter;
use rustok_navigation::services::MenuBindingService;
use rustok_navigation::{MenuLocation, NavigationModule};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_pages::PagesModule;
use rustok_pages::services::PageService;
use rustok_starter::{default_starter, StarterEngine};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

async fn setup_db(tenant_id: Uuid) -> TestResult<(DatabaseConnection, Uuid)> {
    let database_url = format!(
        "sqlite:file:starter_integration_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    // 1. Core tenant tables expected by platform checks
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)".to_string(),
    ))
    .await?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?)",
        [tenant_id.into()],
    ))
    .await?;

    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE users (id TEXT NOT NULL, tenant_id TEXT NOT NULL, PRIMARY KEY (id), UNIQUE (tenant_id, id))".to_string(),
    ))
    .await?;

    let now = Utc::now();
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenant_modules (\
            id TEXT PRIMARY KEY NOT NULL, \
            tenant_id TEXT NOT NULL, \
            module_slug TEXT NOT NULL, \
            enabled INTEGER NOT NULL, \
            settings TEXT NOT NULL, \
            created_at TEXT NOT NULL, \
            updated_at TEXT NOT NULL\
        )"
        .to_string(),
    ))
    .await?;

    for slug in ["pages", "blog", "forum", "navigation"] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            [
                Uuid::new_v4().into(),
                tenant_id.into(),
                slug.into(),
                1i32.into(),
                "{\"builder\":{\"publish\":{\"enabled\":true}}}".into(),
                now.to_rfc3339().into(),
                now.to_rfc3339().into(),
            ],
        ))
        .await?;
    }

    // 2. Run migrations for all affected domain modules
    let manager = SchemaManager::new(&db);
    SysEventsMigration.up(&manager).await?;

    for migration in ChannelModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in PagesModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in rustok_blog::BlogModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in rustok_forum::ForumModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in NavigationModule.migrations() {
        migration.up(&manager).await?;
    }

    // 3. Create default web channel
    let channel_id = Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO channels (id, tenant_id, slug, name, is_active, is_default, status, settings, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        [
            channel_id.into(),
            tenant_id.into(),
            "web".into(),
            "Default Web Channel".into(),
            1i32.into(),
            1i32.into(),
            "active".into(),
            "{}".into(),
            now.to_rfc3339().into(),
            now.to_rfc3339().into(),
        ],
    ))
    .await?;

    Ok((db, channel_id))
}

#[tokio::test]
async fn test_starter_engine_import_default_blueprint_and_verify_storefront_visibility() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let (db, channel_id) = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let engine = StarterEngine::new(db.clone(), event_bus.clone());
    let blueprint = default_starter();

    // Import using system security context (author_id is None, tests bootstrap fallback)
    let report = engine
        .import_blueprint(tenant_id, &SecurityContext::system(), &blueprint)
        .await?;

    assert_eq!(report.tenant_id, tenant_id);
    assert_eq!(report.blueprint_id, "default-starter");
    assert_eq!(report.pages_created, 1);
    assert_eq!(report.blog_categories_created, 3);
    assert_eq!(report.blog_posts_created, 5);
    assert_eq!(report.forum_categories_created, 4);
    assert_eq!(report.forum_topics_created, 4);
    assert_eq!(report.forum_replies_created, 2);
    assert_eq!(report.menus_created, 1);
    assert_eq!(report.skipped_existing, 0);

    // 1. Verify Page is published and queryable by slug with storefront locale fallback
    let page_service = PageService::new(db.clone(), event_bus.clone());
    let page = page_service
        .get_by_slug_with_locale_fallback(tenant_id, SecurityContext::system(), "ru", "home", None)
        .await?
        .expect("Home page should be found and published");

    assert_eq!(page.status, ContentStatus::Published);
    assert!(page.published_at.is_some());

    // 2. Verify Blog Posts are published and accessible
    let post_service = PostService::new(db.clone(), event_bus.clone());
    let post = post_service
        .get_post_by_slug(tenant_id, SecurityContext::system(), "ru", "welcome-to-rustok")
        .await?
        .expect("Blog post should be found and published");
    assert_eq!(post.slug, "welcome-to-rustok");
    assert_eq!(post.status, BlogPostStatus::Published);

    // 3. Verify Forum Topics are listed
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let (topics, total) = topic_service
        .list(
            tenant_id,
            SecurityContext::system(),
            ListTopicsFilter {
                per_page: 20,
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(total, 4);
    assert_eq!(topics.len(), 4);

    // 4. Verify Navigation Menu is bound to default channel and active
    let binding_service = MenuBindingService::new(db.clone());
    let active_header = binding_service
        .get_active(tenant_id, SecurityContext::system(), channel_id, MenuLocation::Header, "ru")
        .await?
        .expect("Header menu should be bound and active for default channel");

    assert_eq!(active_header.items.len(), 3);
    assert_eq!(active_header.items[0].url, "/");
    assert_eq!(active_header.items[1].url, "/modules/blog");
    assert_eq!(active_header.items[2].url, "/modules/forum");

    // 5. Verify Idempotency: Re-running import should not fail and should skip existing
    let report_second_run = engine
        .import_blueprint(tenant_id, &SecurityContext::system(), &blueprint)
        .await?;

    assert_eq!(report_second_run.pages_created, 0);
    assert_eq!(report_second_run.blog_categories_created, 0);
    assert_eq!(report_second_run.blog_posts_created, 0);
    assert_eq!(report_second_run.forum_categories_created, 0);
    assert_eq!(report_second_run.forum_topics_created, 0);
    assert_eq!(report_second_run.menus_created, 0);
    assert!(report_second_run.skipped_existing > 0);

    Ok(())
}
