use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, ForumModule, UpdateCategoryInput,
    services::{ForumCategoryRouteDisposition, ForumCategoryRouteService},
};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn category_slug_history_survives_without_forum_alias_storage() -> TestResult<()> {
    let db = setup().await?;
    let tenant_id = Uuid::new_v4();
    let service = CategoryService::new(db.clone());

    let category = service
        .create(
            tenant_id,
            admin(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Support".to_string(),
                slug: "support".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?;

    db.execute_unprepared("DROP TABLE IF EXISTS forum_category_route_aliases")
        .await?;

    service
        .update(
            tenant_id,
            category.id,
            admin(),
            UpdateCategoryInput {
                locale: "en".to_string(),
                name: Some("Help".to_string()),
                slug: Some("help".to_string()),
                description: None,
                icon: None,
                color: None,
                position: None,
                moderated: None,
            },
        )
        .await?;
    service
        .update(
            tenant_id,
            category.id,
            admin(),
            UpdateCategoryInput {
                locale: "en".to_string(),
                name: Some("Assistance".to_string()),
                slug: Some("assistance".to_string()),
                description: None,
                icon: None,
                color: None,
                position: None,
                moderated: None,
            },
        )
        .await?;

    let routes = ForumCategoryRouteService::new(db);

    let canonical = routes.resolve(tenant_id, "en", "assistance", None).await?;
    assert_eq!(canonical.canonical.category_id, category.id);
    assert_eq!(canonical.canonical.slug, "assistance");
    assert_eq!(
        canonical.disposition,
        ForumCategoryRouteDisposition::Canonical
    );
    assert_eq!(canonical.alias_id, None);

    let first_alias = routes.resolve(tenant_id, "en", "support", None).await?;
    assert_eq!(first_alias.canonical.category_id, category.id);
    assert_eq!(first_alias.canonical.slug, "assistance");
    assert_eq!(
        first_alias.disposition,
        ForumCategoryRouteDisposition::Redirect
    );
    assert!(first_alias.alias_id.is_some());

    let second_alias = routes.resolve(tenant_id, "en", "help", None).await?;
    assert_eq!(second_alias.canonical.category_id, category.id);
    assert_eq!(second_alias.canonical.slug, "assistance");
    assert_eq!(
        second_alias.disposition,
        ForumCategoryRouteDisposition::Redirect
    );
    assert!(second_alias.alias_id.is_some());
    assert_ne!(first_alias.alias_id, second_alias.alias_id);

    Ok(())
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn setup() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_category_taxonomy_owned_alias_history_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
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
    let manager = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}
