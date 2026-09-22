use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, ForumError, ForumModule, UpdateCategoryInput,
    services::{ForumCategoryRouteDisposition, ForumCategoryRouteService},
};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn setup() -> TestResult<DatabaseConnection> {
    let db_url = format!(
        "sqlite:file:forum_category_slug_alias_{}?mode=memory&cache=shared",
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
) -> Result<Uuid, ForumError> {
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

fn update_input(name: Option<&str>, slug: Option<&str>) -> UpdateCategoryInput {
    UpdateCategoryInput {
        locale: "en".to_string(),
        name: name.map(ToOwned::to_owned),
        slug: slug.map(ToOwned::to_owned),
        description: None,
        icon: None,
        color: None,
        position: None,
        moderated: None,
    }
}

async fn alias_count(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS alias_count FROM taxonomy_term_aliases WHERE tenant_id = ?",
            [tenant_id.into()],
        ))
        .await?
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "alias count row is missing")
        })?;
    Ok(row.try_get("", "alias_count")?)
}

#[tokio::test]
async fn explicit_and_name_derived_slug_changes_record_redirects_atomically() -> TestResult<()> {
    let db = setup().await?;
    let tenant_id = Uuid::new_v4();
    let security = admin();
    let category_id =
        create_category(&db, tenant_id, security.clone(), "en", "General", "general").await?;
    let service = CategoryService::new(db.clone());
    let routes = ForumCategoryRouteService::new(db.clone());

    let explicit = service
        .update(
            tenant_id,
            category_id,
            security.clone(),
            update_input(None, Some("community")),
        )
        .await?;
    assert_eq!(explicit.slug, "community");

    let old = routes.resolve(tenant_id, "en", "general", None).await?;
    assert_eq!(old.disposition, ForumCategoryRouteDisposition::Redirect);
    assert_eq!(old.canonical.category_id, category_id);
    assert_eq!(old.canonical.path, "/en/forum/c/community");
    assert!(old.alias_id.is_some());

    let current = routes.resolve(tenant_id, "en", "community", None).await?;
    assert_eq!(
        current.disposition,
        ForumCategoryRouteDisposition::Canonical
    );
    assert_eq!(current.alias_id, None);

    let derived = service
        .update(
            tenant_id,
            category_id,
            security.clone(),
            update_input(Some("Support Center"), None),
        )
        .await?;
    assert_eq!(derived.slug, "support-center");
    let second_old = routes.resolve(tenant_id, "en", "community", None).await?;
    assert_eq!(
        second_old.disposition,
        ForumCategoryRouteDisposition::Redirect
    );
    assert_eq!(second_old.canonical.path, "/en/forum/c/support-center");
    assert!(second_old.alias_id.is_some());
    assert_eq!(alias_count(&db, tenant_id).await?, 2);

    service
        .update(
            tenant_id,
            category_id,
            security,
            update_input(Some("Support Center"), None),
        )
        .await?;
    assert_eq!(alias_count(&db, tenant_id).await?, 2);

    Ok(())
}

#[tokio::test]
async fn historical_route_keys_cannot_be_reclaimed_inside_one_tenant() -> TestResult<()> {
    let db = setup().await?;
    let tenant_id = Uuid::new_v4();
    let security = admin();
    let category_id =
        create_category(&db, tenant_id, security.clone(), "en", "General", "general").await?;
    let service = CategoryService::new(db.clone());
    service
        .update(
            tenant_id,
            category_id,
            security.clone(),
            update_input(None, Some("community")),
        )
        .await?;

    assert!(matches!(
        create_category(
            &db,
            tenant_id,
            security.clone(),
            "en",
            "Replacement",
            "general"
        )
        .await,
        Err(ForumError::Validation(_) | ForumError::CategoryRouteResolutionConflict)
    ));
    assert!(matches!(
        service
            .update(
                tenant_id,
                category_id,
                security,
                update_input(None, Some("general")),
            )
            .await,
        Err(ForumError::Validation(_) | ForumError::CategoryRouteResolutionConflict)
    ));

    let other_tenant_id = Uuid::new_v4();
    let other_category_id = create_category(
        &db,
        other_tenant_id,
        admin(),
        "en",
        "Other tenant",
        "general",
    )
    .await?;
    assert_eq!(
        ForumCategoryRouteService::new(db)
            .resolve(other_tenant_id, "en", "general", None)
            .await?
            .canonical
            .category_id,
        other_category_id
    );

    Ok(())
}

#[tokio::test]
async fn archived_category_hides_current_and_historical_routes() -> TestResult<()> {
    let db = setup().await?;
    let tenant_id = Uuid::new_v4();
    let security = admin();
    let category_id =
        create_category(&db, tenant_id, security.clone(), "en", "General", "general").await?;
    let service = CategoryService::new(db.clone());
    service
        .update(
            tenant_id,
            category_id,
            security.clone(),
            update_input(None, Some("community")),
        )
        .await?;
    service.delete(tenant_id, category_id, security).await?;

    let routes = ForumCategoryRouteService::new(db);
    assert!(matches!(
        routes.resolve(tenant_id, "en", "general", None).await,
        Err(ForumError::CategoryRouteNotFound)
    ));
    assert!(matches!(
        routes.resolve(tenant_id, "en", "community", None).await,
        Err(ForumError::CategoryRouteNotFound)
    ));

    Ok(())
}

#[tokio::test]
async fn alias_rows_are_append_only_and_guard_direct_route_reuse() -> TestResult<()> {
    let db = setup().await?;
    let tenant_id = Uuid::new_v4();
    let security = admin();
    let category_id =
        create_category(&db, tenant_id, security.clone(), "en", "General", "general").await?;
    CategoryService::new(db.clone())
        .update(
            tenant_id,
            category_id,
            security,
            update_input(None, Some("community")),
        )
        .await?;

    assert!(
        db.execute_unprepared(
            "UPDATE forum_category_route_aliases SET reason = 'mutated' WHERE slug = 'general'"
        )
        .await
        .is_err()
    );
    assert!(
        db.execute_unprepared("DELETE FROM forum_category_route_aliases WHERE slug = 'general'")
            .await
            .is_err()
    );

    let bypass_category_id =
        create_category(&db, tenant_id, admin(), "fr", "Autre", "autre").await?;
    assert!(
        db.execute_unprepared(&format!(
            "INSERT INTO forum_category_translations (id, category_id, tenant_id, locale, name, slug, description) \
             VALUES ('{}', '{}', '{}', 'en', 'Bypass', 'general', NULL)",
            Uuid::new_v4(),
            bypass_category_id,
            tenant_id
        ))
        .await
        .is_err()
    );

    Ok(())
}
