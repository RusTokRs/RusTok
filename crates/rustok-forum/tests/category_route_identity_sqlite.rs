use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, ForumError, ForumModule, UpdateCategoryInput,
    services::{
        ForumCategoryRouteDisposition, ForumCategoryRouteService,
    },
};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn setup() -> TestResult<DatabaseConnection> {
    let db_url = format!(
        "sqlite:file:forum_category_route_identity_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    db.execute_unprepared(
        "CREATE TABLE users (\
            id TEXT NOT NULL PRIMARY KEY, \
            tenant_id TEXT NOT NULL, \
            UNIQUE (tenant_id, id)\
        )",
    )
    .await?;
    let schema = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration.up(&schema).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&schema).await?;
    }
    for migration in ForumModule.migrations() {
        migration.up(&schema).await?;
    }
    Ok(db)
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn create_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    security: SecurityContext,
    locale: &str,
    name: &str,
    slug: &str,
) -> TestResult<Uuid> {
    Ok(CategoryService::new(db.clone())
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: locale.to_string(),
                name: name.to_string(),
                slug: slug.to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?
        .id)
}

async fn add_translation(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    security: SecurityContext,
    locale: &str,
    name: &str,
    slug: &str,
) -> TestResult<()> {
    CategoryService::new(db.clone())
        .update(
            tenant_id,
            category_id,
            security,
            UpdateCategoryInput {
                locale: locale.to_string(),
                name: Some(name.to_string()),
                slug: Some(slug.to_string()),
                description: None,
                icon: None,
                color: None,
                position: None,
                moderated: None,
            },
        )
        .await?;
    Ok(())
}

#[tokio::test]
async fn localized_routes_follow_exact_and_shared_fallback_precedence() -> TestResult<()> {
    let db = setup().await?;
    let tenant_id = Uuid::new_v4();
    let security = admin();
    let category_id = create_category(
        &db,
        tenant_id,
        security.clone(),
        "en",
        "General",
        "general",
    )
    .await?;
    add_translation(
        &db,
        tenant_id,
        category_id,
        security.clone(),
        "fr",
        "Discussions",
        "discussions",
    )
    .await?;

    let routes = ForumCategoryRouteService::new(db.clone());
    let canonical_fr = routes
        .canonical_descriptor(tenant_id, category_id, " FR ", None)
        .await?;
    assert_eq!(canonical_fr.locale, "fr");
    assert_eq!(canonical_fr.slug, "discussions");
    assert_eq!(canonical_fr.path, "/fr/forum/c/discussions");

    let exact = routes
        .resolve(tenant_id, "fr", "discussions", None)
        .await?;
    assert_eq!(exact.disposition, ForumCategoryRouteDisposition::Canonical);
    assert_eq!(exact.canonical, canonical_fr);

    let fallback_slug = routes.resolve(tenant_id, "fr", "general", None).await?;
    assert_eq!(
        fallback_slug.disposition,
        ForumCategoryRouteDisposition::Redirect
    );
    assert_eq!(fallback_slug.canonical.path, "/fr/forum/c/discussions");

    let missing_locale = routes.resolve(tenant_id, "de", "general", None).await?;
    assert_eq!(
        missing_locale.disposition,
        ForumCategoryRouteDisposition::Redirect
    );
    assert_eq!(missing_locale.canonical.path, "/en/forum/c/general");

    Ok(())
}

#[tokio::test]
async fn exact_archived_route_does_not_fall_through_to_another_locale() -> TestResult<()> {
    let db = setup().await?;
    let tenant_id = Uuid::new_v4();
    let security = admin();
    let fallback_category_id = create_category(
        &db,
        tenant_id,
        security.clone(),
        "en",
        "Fallback",
        "general",
    )
    .await?;
    let exact_category_id = create_category(
        &db,
        tenant_id,
        security.clone(),
        "fr",
        "Exact",
        "general",
    )
    .await?;
    let routes = ForumCategoryRouteService::new(db.clone());

    let exact = routes.resolve(tenant_id, "fr", "general", None).await?;
    assert_eq!(exact.canonical.category_id, exact_category_id);
    assert_ne!(exact.canonical.category_id, fallback_category_id);

    CategoryService::new(db.clone())
        .delete(tenant_id, exact_category_id, security)
        .await?;
    assert!(matches!(
        routes.resolve(tenant_id, "fr", "general", None).await,
        Err(ForumError::CategoryRouteNotFound)
    ));
    assert!(matches!(
        routes
            .canonical_descriptor(tenant_id, exact_category_id, "fr", None)
            .await,
        Err(ForumError::CategoryRouteNotFound)
    ));

    Ok(())
}

#[tokio::test]
async fn first_available_reverse_lookup_fails_closed_across_category_identities() -> TestResult<()> {
    let db = setup().await?;
    let tenant_id = Uuid::new_v4();
    let security = admin();
    create_category(
        &db,
        tenant_id,
        security.clone(),
        "de",
        "Deutsch",
        "shared",
    )
    .await?;
    create_category(
        &db,
        tenant_id,
        security,
        "it",
        "Italiano",
        "shared",
    )
    .await?;

    let routes = ForumCategoryRouteService::new(db);
    assert!(matches!(
        routes.resolve(tenant_id, "fr", "shared", None).await,
        Err(ForumError::CategoryRouteResolutionConflict)
    ));
    assert!(matches!(
        routes.resolve(tenant_id, "fr", "missing", None).await,
        Err(ForumError::CategoryRouteNotFound)
    ));

    Ok(())
}
