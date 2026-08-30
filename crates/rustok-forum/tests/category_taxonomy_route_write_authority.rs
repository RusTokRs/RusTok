use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, ForumModule, UpdateCategoryInput,
    services::{ForumCategoryRouteDisposition, ForumCategoryRouteService},
};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::{TaxonomyModule, entities::taxonomy_term_translation};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    EntityTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn taxonomy_route_registry_rejects_writes_when_legacy_route_state_is_stale() -> TestResult<()> {
    let db = setup().await?;
    let tenant_id = Uuid::new_v4();
    let service = CategoryService::new(db.clone());

    let owner = create_category(&service, tenant_id, None, "Support", "support").await?;
    service
        .update(
            tenant_id,
            owner,
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

    let challenger = create_category(&service, tenant_id, None, "General", "general").await?;

    let canonical_collision = service
        .update(
            tenant_id,
            challenger,
            admin(),
            UpdateCategoryInput {
                locale: "en".to_string(),
                name: Some("Help Challenger".to_string()),
                slug: Some("help".to_string()),
                description: None,
                icon: None,
                color: None,
                position: None,
                moderated: None,
            },
        )
        .await;
    assert!(
        canonical_collision.is_err(),
        "Taxonomy canonical route ownership must reject a collision"
    );
    assert_eq!(persisted_slug(&db, tenant_id, challenger).await?, "general");

    let alias_collision = service
        .create(
            tenant_id,
            admin(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Nested Support".to_string(),
                slug: "support".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: Some(challenger),
                position: Some(0),
                moderated: false,
            },
        )
        .await;
    assert!(
        alias_collision.is_err(),
        "Taxonomy alias ownership must reject a collision"
    );

    let routes = ForumCategoryRouteService::new(db);
    let canonical = routes.resolve(tenant_id, "en", "help", None).await?;
    assert_eq!(canonical.canonical.category_id, owner);
    assert_eq!(canonical.disposition, ForumCategoryRouteDisposition::Canonical);

    let alias = routes.resolve(tenant_id, "en", "support", None).await?;
    assert_eq!(alias.canonical.category_id, owner);
    assert_eq!(alias.disposition, ForumCategoryRouteDisposition::Redirect);
    assert!(alias.alias_id.is_some());

    Ok(())
}

async fn create_category(
    service: &CategoryService,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
    name: &str,
    slug: &str,
) -> TestResult<Uuid> {
    Ok(service
        .create(
            tenant_id,
            admin(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: name.to_string(),
                slug: slug.to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id,
                position: Some(0),
                moderated: false,
            },
        )
        .await?
        .id)
}

async fn persisted_slug(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> TestResult<String> {
    Ok(taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_translation::Column::TermId.eq(category_id))
        .filter(taxonomy_term_translation::Column::Locale.eq("en"))
        .one(db)
        .await?
        .expect("translation must remain after rollback")
        .slug)
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn setup() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_category_taxonomy_route_write_authority_{}?mode=memory&cache=shared",
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
