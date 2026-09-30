use chrono::Utc;
use rustok_fulfillment::migrations::migrations;
use rustok_test_utils::db::setup_test_db;
use sea_orm::ConnectionTrait;
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

mod support;

#[tokio::test]
async fn translation_change_journal_rollback_is_blocked_when_evidence_exists() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;

    let migration = migrations()
        .into_iter()
        .nth(13)
        .expect("translation change journal migration should exist");
    let manager = SchemaManager::new(&db);
    migration
        .up(&manager)
        .await
        .expect("translation change journal migration should install");

    db.execute_unprepared(&format!(
        "INSERT INTO shipping_option_translation_change_journal          (operation_id, tenant_id, shipping_option_id, resource_revision, lifecycle)          VALUES ('{}', '{}', '{}', 'sha256:test', 'active')",
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4()
    ))
    .await
    .expect("change evidence should be insertable");

    let rollback = migration.down(&manager).await;

    assert!(
        rollback.is_err(),
        "rollback must not silently delete durable translation change evidence"
    );

    let count = db
        .query_one_raw(
            "SELECT COUNT(*) AS count FROM shipping_option_translation_change_journal",
            []
        )
        .await
        .expect("change journal should remain queryable")
        .expect("count row should exist");
    let count: i64 = count.try_get_by_index(0).expect("count should be an integer");
    assert_eq!(count, 1, "blocked rollback must not delete change evidence");
}

#[tokio::test]
async fn translation_change_journal_rollback_succeeds_when_empty() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;

    let migration = migrations()
        .into_iter()
        .nth(13)
        .expect("translation change journal migration should exist");
    let manager = SchemaManager::new(&db);
    migration
        .up(&manager)
        .await
        .expect("translation change journal migration should install");

    migration
        .down(&manager)
        .await
        .expect("empty translation change journal may be rolled back");
}
