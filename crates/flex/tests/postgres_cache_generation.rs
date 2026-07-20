use std::time::Duration;

use flex::cache_generation::{
    create_field_definition_cache_generation_table,
    create_field_definition_cache_generation_trigger,
    drop_field_definition_cache_generation_table,
    drop_field_definition_cache_generation_trigger, FIELD_DEFINITION_CACHE_GENERATION_TABLE,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement, TransactionTrait,
};
use sea_orm_migration::SchemaManager;

const OWNERS: [(&str, &str); 4] = [
    ("user_field_definitions", "flex_user_fd_cache_generation"),
    (
        "product_field_definitions",
        "flex_product_fd_cache_generation",
    ),
    ("order_field_definitions", "flex_order_fd_cache_generation"),
    ("topic_field_definitions", "flex_topic_fd_cache_generation"),
];

async fn connect_postgres(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("PostgreSQL Flex fixture should connect")
}

async fn read_generation(db: &DatabaseConnection) -> u64 {
    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            format!(
                "SELECT generation FROM {FIELD_DEFINITION_CACHE_GENERATION_TABLE} WHERE id = 1"
            ),
        ))
        .await
        .expect("Flex generation query should succeed")
        .expect("Flex generation singleton should exist");
    let generation: i64 = row
        .try_get("", "generation")
        .expect("Flex generation should decode");
    u64::try_from(generation).expect("Flex generation should remain non-negative")
}

async fn reset_fixture(db: &DatabaseConnection) {
    db.execute_unprepared(
        "DROP FUNCTION IF EXISTS rustok_bump_flex_field_definition_cache_generation() CASCADE",
    )
    .await
    .expect("old Flex generation function should drop");
    db.execute_unprepared(&format!(
        "DROP TABLE IF EXISTS {FIELD_DEFINITION_CACHE_GENERATION_TABLE} CASCADE"
    ))
    .await
    .expect("old Flex generation table should drop");
    for (table, _) in OWNERS.into_iter().rev() {
        db.execute_unprepared(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
            .await
            .expect("old owner table should drop");
    }
    for (table, _) in OWNERS {
        db.execute_unprepared(&format!(
            "CREATE TABLE {table} (id BIGINT PRIMARY KEY, position INTEGER NOT NULL DEFAULT 0, is_active BOOLEAN NOT NULL DEFAULT TRUE)"
        ))
        .await
        .expect("owner table should create");
    }
}

async fn install_generation_contract(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    create_field_definition_cache_generation_table(&manager)
        .await
        .expect("Flex generation table should create");
    for (table, trigger) in OWNERS {
        create_field_definition_cache_generation_trigger(&manager, table, trigger)
            .await
            .expect("Flex owner trigger should create");
    }
}

#[tokio::test]
#[ignore = "requires RUSTOK_FLEX_TEST_POSTGRES_URL"]
async fn postgres_flex_generation_is_transactional_concurrent_and_replay_safe() {
    let url = std::env::var("RUSTOK_FLEX_TEST_POSTGRES_URL")
        .expect("RUSTOK_FLEX_TEST_POSTGRES_URL must be configured");
    let writer = connect_postgres(url.as_str()).await;
    let replica = connect_postgres(url.as_str()).await;
    reset_fixture(&writer).await;
    install_generation_contract(&writer).await;

    assert_eq!(read_generation(&writer).await, 0);
    assert_eq!(read_generation(&replica).await, 0);

    for (index, (table, _)) in OWNERS.into_iter().enumerate() {
        writer
            .execute_unprepared(&format!(
                "INSERT INTO {table} (id, position, is_active) VALUES ({}, 0, TRUE)",
                index + 1
            ))
            .await
            .expect("owner insert should commit");
    }
    assert_eq!(read_generation(&replica).await, 4);

    let rolled_back = writer
        .begin()
        .await
        .expect("rollback transaction should begin");
    rolled_back
        .execute_unprepared("UPDATE user_field_definitions SET position = 99")
        .await
        .expect("rolled-back reorder should execute");
    rolled_back
        .rollback()
        .await
        .expect("reorder should roll back");
    assert_eq!(read_generation(&replica).await, 4);

    for (table, _) in OWNERS {
        writer
            .execute_unprepared(&format!(
                "UPDATE {table} SET position = position + 1"
            ))
            .await
            .expect("owner reorder should commit");
    }
    assert_eq!(read_generation(&replica).await, 8);

    for (table, _) in OWNERS {
        writer
            .execute_unprepared(&format!("UPDATE {table} SET is_active = FALSE"))
            .await
            .expect("owner soft delete should commit");
    }
    assert_eq!(read_generation(&replica).await, 12);

    let writer_a = connect_postgres(url.as_str()).await;
    let writer_b = connect_postgres(url.as_str()).await;
    let mutation_a = tokio::spawn(async move {
        let transaction = writer_a
            .begin()
            .await
            .expect("first concurrent transaction should begin");
        transaction
            .execute_unprepared(
                "INSERT INTO user_field_definitions (id, position, is_active) VALUES (101, 0, TRUE)",
            )
            .await
            .expect("first concurrent mutation should execute");
        transaction
            .commit()
            .await
            .expect("first concurrent mutation should commit");
    });
    let mutation_b = tokio::spawn(async move {
        let transaction = writer_b
            .begin()
            .await
            .expect("second concurrent transaction should begin");
        transaction
            .execute_unprepared(
                "INSERT INTO product_field_definitions (id, position, is_active) VALUES (102, 0, TRUE)",
            )
            .await
            .expect("second concurrent mutation should execute");
        transaction
            .commit()
            .await
            .expect("second concurrent mutation should commit");
    });
    mutation_a.await.expect("first mutation task should join");
    mutation_b.await.expect("second mutation task should join");
    assert_eq!(read_generation(&replica).await, 14);

    for (table, _) in OWNERS {
        writer
            .execute_unprepared(&format!("DELETE FROM {table}"))
            .await
            .expect("owner delete should commit");
    }
    assert_eq!(read_generation(&replica).await, 18);

    install_generation_contract(&writer).await;
    assert_eq!(read_generation(&writer).await, 18);
    assert_eq!(read_generation(&replica).await, 18);

    writer
        .execute_unprepared(&format!(
            "DROP TABLE {FIELD_DEFINITION_CACHE_GENERATION_TABLE}"
        ))
        .await
        .expect("generation table should drop for recovery evidence");
    assert!(replica
        .query_one(Statement::from_string(
            replica.get_database_backend(),
            format!(
                "SELECT generation FROM {FIELD_DEFINITION_CACHE_GENERATION_TABLE} WHERE id = 1"
            ),
        ))
        .await
        .is_err());

    install_generation_contract(&writer).await;
    assert_eq!(read_generation(&writer).await, 0);
    assert_eq!(read_generation(&replica).await, 0);

    let manager = SchemaManager::new(&writer);
    for (table, trigger) in OWNERS.into_iter().rev() {
        drop_field_definition_cache_generation_trigger(&manager, table, trigger)
            .await
            .expect("Flex owner trigger should drop");
    }
    drop_field_definition_cache_generation_table(&manager)
        .await
        .expect("Flex generation table should drop");
    for (table, _) in OWNERS.into_iter().rev() {
        writer
            .execute_unprepared(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
            .await
            .expect("owner table should drop");
    }
}
