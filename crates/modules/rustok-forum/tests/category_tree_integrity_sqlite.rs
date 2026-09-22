use rustok_core::MigrationSource;
use rustok_forum::ForumModule;
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn sqlite_rejects_self_parent_and_category_cycles() -> TestResult<()> {
    let db = setup_sqlite().await?;
    let tenant_id = Uuid::new_v4();
    let service = rustok_forum::CategoryService::new(db.clone());
    let security = rustok_core::SecurityContext::system();

    let root = service
        .create(
            tenant_id,
            security.clone(),
            rustok_forum::CreateCategoryInput {
                name: "Root".to_string(),
                slug: "root".to_string(),
                locale: "en".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?;

    let child = service
        .create(
            tenant_id,
            security.clone(),
            rustok_forum::CreateCategoryInput {
                name: "Child".to_string(),
                slug: "child".to_string(),
                locale: "en".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: Some(root.id),
                position: Some(0),
                moderated: false,
            },
        )
        .await?;

    let grandchild = service
        .create(
            tenant_id,
            security.clone(),
            rustok_forum::CreateCategoryInput {
                name: "Grandchild".to_string(),
                slug: "grandchild".to_string(),
                locale: "en".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: Some(child.id),
                position: Some(0),
                moderated: false,
            },
        )
        .await?;

    let self_parent_err = service
        .move_category(
            tenant_id,
            root.id,
            security.clone(),
            rustok_forum::MoveCategoryInput {
                parent_id: Some(root.id),
                position: 0,
            },
        )
        .await;
    assert!(
        self_parent_err.is_err(),
        "self-parent category must be rejected"
    );

    let cycle_err = service
        .move_category(
            tenant_id,
            root.id,
            security.clone(),
            rustok_forum::MoveCategoryInput {
                parent_id: Some(grandchild.id),
                position: 0,
            },
        )
        .await;
    assert!(
        cycle_err.is_err(),
        "three-level category cycle must be rejected"
    );

    service
        .move_category(
            tenant_id,
            grandchild.id,
            security.clone(),
            rustok_forum::MoveCategoryInput {
                parent_id: Some(root.id),
                position: 1,
            },
        )
        .await?;

    service
        .move_category(
            tenant_id,
            grandchild.id,
            security.clone(),
            rustok_forum::MoveCategoryInput {
                parent_id: None,
                position: 1,
            },
        )
        .await?;

    service
        .archive_subtree(tenant_id, root.id, security.clone())
        .await?;

    let move_under_archived_err = service
        .move_category(
            tenant_id,
            grandchild.id,
            security.clone(),
            rustok_forum::MoveCategoryInput {
                parent_id: Some(root.id),
                position: 0,
            },
        )
        .await;
    assert!(
        move_under_archived_err.is_err(),
        "moving active category under archived parent must be rejected"
    );

    Ok(())
}

async fn setup_sqlite() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_category_tree_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute_unprepared(
        r#"
CREATE TABLE users (
    id TEXT NOT NULL PRIMARY KEY,
    tenant_id TEXT NOT NULL
)
"#,
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

