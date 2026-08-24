use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, ForumModule, UpdateCategoryInput,
    entities::{forum_category, forum_category_lifecycle, forum_category_translation},
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
async fn sqlite_category_writes_are_atomic_with_translations() -> TestResult<()> {
    let db = setup_sqlite().await?;
    let service = CategoryService::new(db.clone());
    let tenant_id = Uuid::new_v4();

    execute(
        &db,
        r#"
CREATE TRIGGER forum_test_reject_category_translation_insert
BEFORE INSERT ON forum_category_translations
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'forced category translation insert failure');
END
"#,
    )
    .await?;

    let failed_create = service
        .create(
            tenant_id,
            admin_security(),
            create_input("Atomic create", "atomic-create", 1),
        )
        .await;
    assert!(
        failed_create.is_err(),
        "forced translation failure must make category creation fail"
    );
    assert!(
        forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .all(&db)
            .await?
            .is_empty(),
        "category row must roll back when its initial translation fails"
    );

    execute(
        &db,
        "DROP TRIGGER forum_test_reject_category_translation_insert",
    )
    .await?;

    let category = service
        .create(
            tenant_id,
            admin_security(),
            create_input("Original category", "original-category", 3),
        )
        .await?;

    execute(
        &db,
        r#"
CREATE TRIGGER forum_test_reject_category_translation_update
BEFORE UPDATE ON forum_category_translations
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'forced category translation update failure');
END
"#,
    )
    .await?;

    let failed_update = service
        .update(
            tenant_id,
            category.id,
            admin_security(),
            UpdateCategoryInput {
                locale: "en".to_string(),
                name: Some("Changed category".to_string()),
                slug: Some("changed-category".to_string()),
                description: Some("changed description".to_string()),
                icon: None,
                color: None,
                position: None,
                moderated: Some(true),
            },
        )
        .await;
    assert!(
        failed_update.is_err(),
        "forced translation update failure must make category update fail"
    );
    let persisted = load_category(&db, tenant_id, category.id).await?;
    assert_eq!(persisted.position, 3);
    assert!(!persisted.moderated);
    assert!(
        forum_category_translation::Entity::find()
            .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_category_translation::Column::CategoryId.eq(category.id))
            .filter(forum_category_translation::Column::Name.eq("Changed category"))
            .one(&db)
            .await?
            .is_none(),
        "failed update must not leak changed localized copy"
    );

    execute(
        &db,
        "DROP TRIGGER forum_test_reject_category_translation_update",
    )
    .await?;

    execute(
        &db,
        r#"
CREATE TRIGGER forum_test_reject_new_category_locale
BEFORE INSERT ON forum_category_translations
FOR EACH ROW
WHEN NEW.locale = 'fr'
BEGIN
    SELECT RAISE(ABORT, 'forced new category locale failure');
END
"#,
    )
    .await?;

    let failed_locale_insert = service
        .update(
            tenant_id,
            category.id,
            admin_security(),
            UpdateCategoryInput {
                locale: "fr".to_string(),
                name: Some("Catégorie modifiée".to_string()),
                slug: Some("categorie-modifiee".to_string()),
                description: None,
                icon: None,
                color: None,
                position: None,
                moderated: Some(true),
            },
        )
        .await;
    assert!(
        failed_locale_insert.is_err(),
        "forced new-locale failure must make category update fail"
    );
    let persisted = load_category(&db, tenant_id, category.id).await?;
    assert_eq!(persisted.position, 3);
    assert!(!persisted.moderated);
    assert!(
        forum_category_translation::Entity::find()
            .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_category_translation::Column::CategoryId.eq(category.id))
            .filter(forum_category_translation::Column::Locale.eq("fr"))
            .one(&db)
            .await?
            .is_none(),
        "failed locale insert must not leak a translation"
    );

    execute(&db, "DROP TRIGGER forum_test_reject_new_category_locale").await?;

    execute(
        &db,
        r#"
CREATE TRIGGER forum_test_reject_category_archive
BEFORE INSERT ON forum_category_lifecycle
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'forced category archive failure');
END
"#,
    )
    .await?;

    let failed_archive = service
        .delete(tenant_id, category.id, admin_security())
        .await;
    assert!(
        failed_archive.is_err(),
        "forced lifecycle failure must make category deletion/archive fail"
    );
    load_category(&db, tenant_id, category.id).await?;
    assert!(
        forum_category_translation::Entity::find()
            .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_category_translation::Column::CategoryId.eq(category.id))
            .one(&db)
            .await?
            .is_some(),
        "normal category deletion must preserve translations"
    );
    assert!(
        forum_category_lifecycle::Entity::find()
            .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
            .filter(forum_category_lifecycle::Column::CategoryId.eq(category.id))
            .one(&db)
            .await?
            .is_none(),
        "failed category archive must not leak lifecycle state"
    );

    execute(&db, "DROP TRIGGER forum_test_reject_category_archive").await?;

    service
        .delete(tenant_id, category.id, admin_security())
        .await?;
    assert!(
        forum_category_lifecycle::Entity::find()
            .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
            .filter(forum_category_lifecycle::Column::CategoryId.eq(category.id))
            .one(&db)
            .await?
            .is_some(),
        "successful category deletion must archive the category"
    );
    load_category(&db, tenant_id, category.id).await?;

    Ok(())
}

fn create_input(name: &str, slug: &str, position: i32) -> CreateCategoryInput {
    CreateCategoryInput {
        locale: "en".to_string(),
        name: name.to_string(),
        slug: slug.to_string(),
        description: None,
        icon: None,
        color: None,
        parent_id: None,
        position: Some(position),
        moderated: false,
    }
}

fn admin_security() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn load_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> TestResult<forum_category::Model> {
    forum_category::Entity::find_by_id(category_id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other(format!("category {category_id} is missing")))
        .map_err(Into::into)
}

async fn setup_sqlite() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_category_atomicity_{}?mode=memory&cache=shared",
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

async fn execute(db: &DatabaseConnection, sql: impl Into<String>) -> TestResult<()> {
    db.execute_unprepared(&sql.into()).await?;
    Ok(())
}
