mod support;

use rustok_core::MigrationSource;
use rustok_forum::ForumModule;
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use support::TestResult;
use support::read_model::exercise_bounded_cursor_read_models;

#[tokio::test]
async fn sqlite_forum_read_models_are_bounded_and_cursor_stable() -> TestResult<()> {
    let db = setup_sqlite().await?;
    exercise_bounded_cursor_read_models(&db).await
}

#[tokio::test]
async fn sqlite_forum_read_model_respects_category_visibility_for_public_and_system()
-> TestResult<()> {
    use rustok_core::{SecurityContext, UserRole};
    use rustok_forum::{
        ForumCategoryVisibility, ForumCategoryVisibilityPolicyService, ForumReadModelService,
        SetForumCategoryVisibilityPolicyInput, TopicCursorQuery,
    };
    use support::read_model::{seed_category, seed_topics};

    let db = setup_sqlite().await?;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let admin_security = SecurityContext::new(UserRole::Admin, Some(author_id));

    let category_id = seed_category(&db, tenant_id, 0).await?;

    // Mark category as authenticated-only
    let vis_svc = ForumCategoryVisibilityPolicyService::new(db.clone());
    vis_svc
        .set(
            tenant_id,
            category_id,
            admin_security.clone(),
            SetForumCategoryVisibilityPolicyInput {
                visibility: ForumCategoryVisibility::Authenticated,
            },
        )
        .await?;

    let topics = seed_topics(&db, tenant_id, category_id, 1).await?;
    let topic_id = topics[0];

    let read_service = ForumReadModelService::new(db.clone());

    // Public reader must NOT see the topic in the authenticated-only category
    let public_page = read_service
        .list_topics(
            tenant_id,
            SecurityContext::public_read(),
            TopicCursorQuery {
                locale: Some("en".into()),
                ..Default::default()
            },
        )
        .await?;
    assert!(
        !public_page.items.iter().any(|item| item.id == topic_id),
        "Public reader should not see topic in Authenticated category"
    );

    // System security context (user_id is None) MUST see the topic
    let system_page = read_service
        .list_topics(
            tenant_id,
            SecurityContext::system(),
            TopicCursorQuery {
                locale: Some("en".into()),
                ..Default::default()
            },
        )
        .await?;
    assert!(
        system_page.items.iter().any(|item| item.id == topic_id),
        "System security context should see topic in Authenticated category"
    );

    // Authenticated regular user MUST see the topic
    let user_security = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let user_page = read_service
        .list_topics(
            tenant_id,
            user_security,
            TopicCursorQuery {
                locale: Some("en".into()),
                ..Default::default()
            },
        )
        .await?;
    assert!(
        user_page.items.iter().any(|item| item.id == topic_id),
        "Authenticated user should see topic in Authenticated category"
    );

    Ok(())
}

async fn setup_sqlite() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_cursor_read_model_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    let manager = SchemaManager::new(&db);

    for migration in OutboxModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        );",
    )
    .await
    .expect("users table fixture should apply");
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }

    Ok(db)
}
