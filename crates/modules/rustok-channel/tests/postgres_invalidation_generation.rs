use std::time::Duration;

use rustok_channel::read_resolution_invalidation_generation;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, TransactionTrait};
use sea_orm_migration::{MigrationTrait, SchemaManager};

const GENERATION_MIGRATION: &str = "m20260716_000009_create_channel_resolution_invalidation_state";
const CHANNEL_TABLES: &[&str] = &[
    "channels",
    "channel_targets",
    "channel_module_bindings",
    "channel_oauth_apps",
    "channel_resolution_policy_sets",
    "channel_resolution_policy_rules",
];

async fn connect_postgres(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options).await.unwrap()
}

async fn reset_fixture(db: &DatabaseConnection) {
    db.execute_unprepared(
        "DROP FUNCTION IF EXISTS channel_bump_resolution_invalidation_generation() CASCADE",
    )
    .await
    .unwrap();
    db.execute_unprepared("DROP TABLE IF EXISTS channel_resolution_invalidation_state CASCADE")
        .await
        .unwrap();
    for table in CHANNEL_TABLES.iter().rev() {
        db.execute_unprepared(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
            .await
            .unwrap();
    }
    for table in CHANNEL_TABLES {
        db.execute_unprepared(&format!(
            "CREATE TABLE {table} (id BIGSERIAL PRIMARY KEY, value TEXT)"
        ))
        .await
        .unwrap();
    }
}

fn generation_migration() -> Box<dyn MigrationTrait> {
    rustok_channel::migrations::migrations()
        .into_iter()
        .find(|migration| migration.name() == GENERATION_MIGRATION)
        .expect("channel generation migration must be registered")
}

#[tokio::test]
#[ignore = "requires RUSTOK_CHANNEL_TEST_POSTGRES_URL"]
async fn postgres_generation_is_transactional_concurrent_and_recoverable() {
    let url = std::env::var("RUSTOK_CHANNEL_TEST_POSTGRES_URL")
        .expect("RUSTOK_CHANNEL_TEST_POSTGRES_URL must be configured");
    let writer = connect_postgres(url.as_str()).await;
    let replica = connect_postgres(url.as_str()).await;
    reset_fixture(&writer).await;

    let migration = generation_migration();
    let manager = SchemaManager::new(&writer);
    migration.up(&manager).await.unwrap();
    assert_eq!(
        read_resolution_invalidation_generation(&writer)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        read_resolution_invalidation_generation(&replica)
            .await
            .unwrap(),
        0
    );

    let committed = writer.begin().await.unwrap();
    committed
        .execute_unprepared("INSERT INTO channels (value) VALUES ('committed')")
        .await
        .unwrap();
    committed.commit().await.unwrap();
    assert_eq!(
        read_resolution_invalidation_generation(&replica)
            .await
            .unwrap(),
        1
    );

    let rolled_back = writer.begin().await.unwrap();
    rolled_back
        .execute_unprepared("UPDATE channels SET value = 'rolled-back'")
        .await
        .unwrap();
    rolled_back.rollback().await.unwrap();
    assert_eq!(
        read_resolution_invalidation_generation(&replica)
            .await
            .unwrap(),
        1
    );

    let writer_a = writer.clone();
    let writer_b = writer.clone();
    let mutation_a = tokio::spawn(async move {
        let transaction = writer_a.begin().await.unwrap();
        transaction
            .execute_unprepared("INSERT INTO channel_targets (value) VALUES ('a')")
            .await
            .unwrap();
        transaction.commit().await.unwrap();
    });
    let mutation_b = tokio::spawn(async move {
        let transaction = writer_b.begin().await.unwrap();
        transaction
            .execute_unprepared("INSERT INTO channel_module_bindings (value) VALUES ('b')")
            .await
            .unwrap();
        transaction.commit().await.unwrap();
    });
    mutation_a.await.unwrap();
    mutation_b.await.unwrap();
    assert_eq!(
        read_resolution_invalidation_generation(&replica)
            .await
            .unwrap(),
        3
    );

    writer
        .execute_unprepared("DROP TABLE channel_resolution_invalidation_state")
        .await
        .unwrap();
    assert!(
        read_resolution_invalidation_generation(&writer)
            .await
            .is_err()
    );
    assert!(
        read_resolution_invalidation_generation(&replica)
            .await
            .is_err()
    );

    migration.up(&manager).await.unwrap();
    assert_eq!(
        read_resolution_invalidation_generation(&writer)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        read_resolution_invalidation_generation(&replica)
            .await
            .unwrap(),
        0
    );

    migration.down(&manager).await.unwrap();
    for table in CHANNEL_TABLES.iter().rev() {
        writer
            .execute_unprepared(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
            .await
            .unwrap();
    }
}
