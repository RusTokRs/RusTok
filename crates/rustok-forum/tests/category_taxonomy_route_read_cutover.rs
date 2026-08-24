use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, ForumModule, UpdateCategoryInput,
    entities::{forum_category_taxonomy_binding, forum_category_translation},
    services::{ForumCategoryRouteDisposition, ForumCategoryRouteService},
};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn category_routes_read_taxonomy_after_legacy_route_copy_is_removed() -> TestResult<()> {
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
                description: Some("Support description".to_string()),
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
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
                name: Some("Help".to_string()),
                slug: Some("help".to_string()),
                description: Some("Help description".to_string()),
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
                locale: "fr".to_string(),
                name: Some("Assistance".to_string()),
                slug: Some("aide".to_string()),
                description: Some("Assistance description".to_string()),
                icon: None,
                color: None,
                position: None,
                moderated: None,
            },
        )
        .await?;

    delete_legacy_route_copy(&db, tenant_id, category.id).await?;

    let routes = ForumCategoryRouteService::new(db.clone());
    let canonical_en = routes
        .canonical_descriptor(tenant_id, category.id, "en", None)
        .await?;
    assert_eq!(canonical_en.locale, "en");
    assert_eq!(canonical_en.slug, "help");
    assert_eq!(canonical_en.path, "/en/forum/c/help");

    let canonical_fr = routes
        .canonical_descriptor(tenant_id, category.id, "fr-CA", Some("fr"))
        .await?;
    assert_eq!(canonical_fr.locale, "fr");
    assert_eq!(canonical_fr.slug, "aide");
    assert_eq!(canonical_fr.path, "/fr/forum/c/aide");

    let exact = routes.resolve(tenant_id, "fr", "aide", Some("en")).await?;
    assert_eq!(exact.disposition, ForumCategoryRouteDisposition::Canonical);
    assert_eq!(exact.canonical.path, "/fr/forum/c/aide");
    assert_eq!(exact.alias_id, None);

    let alias = routes.resolve(tenant_id, "en", "support", None).await?;
    assert_eq!(alias.disposition, ForumCategoryRouteDisposition::Redirect);
    assert_eq!(alias.canonical.path, "/en/forum/c/help");
    assert!(alias.alias_id.is_some(), "Taxonomy alias identity must survive cutover");

    let fallback_match = routes
        .resolve(tenant_id, "fr", "help", Some("en"))
        .await?;
    assert_eq!(
        fallback_match.disposition,
        ForumCategoryRouteDisposition::Redirect
    );
    assert_eq!(fallback_match.canonical.path, "/fr/forum/c/aide");

    forum_category_taxonomy_binding::Entity::delete_by_id((tenant_id, category.id))
        .exec(&db)
        .await?;
    assert!(
        routes.resolve(tenant_id, "en", "help", None).await.is_err(),
        "Taxonomy route lookup must fail closed when the Forum binding is missing"
    );
    assert!(
        routes
            .canonical_descriptor(tenant_id, category.id, "en", None)
            .await
            .is_err(),
        "canonical route reads must not fall back to legacy Forum copy"
    );

    Ok(())
}

async fn delete_legacy_route_copy(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> TestResult<()> {
    forum_category_translation::Entity::delete_many()
        .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
        .filter(forum_category_translation::Column::CategoryId.eq(category_id))
        .exec(db)
        .await?;
    Ok(())
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn setup() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_category_taxonomy_route_read_cutover_{}?mode=memory&cache=shared",
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
