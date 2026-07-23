use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{CategoryService, CreateCategoryInput, ForumModule, UpdateCategoryInput};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
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
    assert_eq!(
        scalar_i64(
            &db,
            format!(
                "SELECT COUNT(*) AS value FROM forum_categories WHERE tenant_id = '{tenant_id}'"
            ),
        )
        .await?,
        0,
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
                position: Some(99),
                moderated: Some(true),
            },
        )
        .await;
    assert!(
        failed_update.is_err(),
        "forced translation update failure must make category update fail"
    );
    assert_eq!(
        scalar_i64(
            &db,
            format!(
                "SELECT position AS value FROM forum_categories WHERE id = '{}'",
                category.id
            ),
        )
        .await?,
        3
    );
    assert_eq!(
        scalar_i64(
            &db,
            format!(
                "SELECT moderated AS value FROM forum_categories WHERE id = '{}'",
                category.id
            ),
        )
        .await?,
        0
    );
    assert_eq!(
        scalar_i64(
            &db,
            format!(
                "SELECT COUNT(*) AS value
                 FROM forum_category_translations
                 WHERE category_id = '{}' AND name = 'Changed category'",
                category.id
            ),
        )
        .await?,
        0
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
                position: Some(77),
                moderated: None,
            },
        )
        .await;
    assert!(
        failed_locale_insert.is_err(),
        "forced new-locale failure must make category update fail"
    );
    assert_eq!(
        scalar_i64(
            &db,
            format!(
                "SELECT position AS value FROM forum_categories WHERE id = '{}'",
                category.id
            ),
        )
        .await?,
        3
    );
    assert_eq!(
        scalar_i64(
            &db,
            format!(
                "SELECT COUNT(*) AS value
                 FROM forum_category_translations
                 WHERE category_id = '{}' AND locale = 'fr'",
                category.id
            ),
        )
        .await?,
        0
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
    assert_eq!(
        scalar_i64(
            &db,
            format!(
                "SELECT COUNT(*) AS value FROM forum_categories WHERE id = '{}'",
                category.id
            ),
        )
        .await?,
        1,
        "normal category deletion must preserve the category row"
    );
    assert_eq!(
        scalar_i64(
            &db,
            format!(
                "SELECT COUNT(*) AS value
                 FROM forum_category_translations
                 WHERE category_id = '{}'",
                category.id
            ),
        )
        .await?,
        1,
        "normal category deletion must preserve translations"
    );
    assert_eq!(
        scalar_i64(
            &db,
            format!(
                "SELECT COUNT(*) AS value
                 FROM forum_category_lifecycle
                 WHERE tenant_id = '{tenant_id}' AND category_id = '{}'",
                category.id
            ),
        )
        .await?,
        0,
        "failed category archive must not leak lifecycle state"
    );

    execute(&db, "DROP TRIGGER forum_test_reject_category_archive").await?;

    service
        .delete(tenant_id, category.id, admin_security())
        .await?;
    assert_eq!(
        scalar_i64(
            &db,
            format!(
                "SELECT COUNT(*) AS value
                 FROM forum_category_lifecycle
                 WHERE tenant_id = '{tenant_id}' AND category_id = '{}'",
                category.id
            ),
        )
        .await?,
        1,
        "successful category deletion must archive the category"
    );
    assert_eq!(
        scalar_i64(
            &db,
            format!(
                "SELECT COUNT(*) AS value FROM forum_categories WHERE id = '{}'",
                category.id
            ),
        )
        .await?,
        1
    );

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

    execute(
        &db,
        r#"
CREATE TABLE taxonomy_terms (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    scope_type TEXT NOT NULL,
    scope_value TEXT NOT NULL DEFAULT '',
    canonical_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
)
"#,
    )
    .await?;

    let manager = SchemaManager::new(&db);
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }

    Ok(db)
}

async fn execute(db: &DatabaseConnection, sql: impl Into<String>) -> TestResult<()> {
    db.execute_unprepared(&sql.into()).await?;
    Ok(())
}

async fn scalar_i64(db: &DatabaseConnection, sql: impl Into<String>) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_string(DatabaseBackend::Sqlite, sql.into()))
        .await?
        .ok_or_else(|| std::io::Error::other("scalar query returned no row"))?;
    Ok(row.try_get("", "value")?)
}
