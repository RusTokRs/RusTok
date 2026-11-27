//! Shared local persistence for the platform Outbox. Event delivery belongs to the relay.

pub mod consumers;
pub mod entity;
pub mod idempotency;
pub mod migration;
pub mod transactional;
pub mod transport;

pub use consumers::OutboxConsumer;
pub use entity::{Entity as SysEvents, Model as SysEvent};
pub use migration::SysEventsMigration;
pub use transactional::{OutboxRootEventWriter, TransactionalEventBus};
pub use transport::{ContractEventWriteOnceError, OutboxTransport};

/// Samples durable wall time from the supplied database connection.
/// PostgreSQL uses actual wall time rather than transaction-start time.
/// There is no process-clock fallback.
pub async fn database_now(
    connection: &impl sea_orm::ConnectionTrait,
) -> Result<chrono::DateTime<chrono::Utc>, sea_orm::DbErr> {
    use sea_orm::{DbBackend, DbErr, Statement};
    let backend = connection.get_database_backend();
    let expression = match backend {
        DbBackend::Postgres => "CAST(FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000) AS BIGINT)",
        DbBackend::Sqlite => "CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)",
        _ => return Err(DbErr::Custom("unsupported outbox database clock".to_string())),
    };
    let row = connection
        .query_one_raw(Statement::from_string(backend, format!("SELECT {expression} AS now_ms")))
        .await?
        .ok_or_else(|| DbErr::Custom("outbox database clock returned no row".to_string()))?;
    let millis: i64 = row.try_get("", "now_ms")?;
    chrono::DateTime::from_timestamp_millis(millis)
        .ok_or_else(|| DbErr::Custom("outbox database clock is out of range".to_string()))
}

#[cfg(test)]
mod clock_tests {
    use super::database_now;
    use sea_orm::{Database, TransactionTrait};

    #[tokio::test]
    async fn database_clock_advances_inside_transaction() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let txn = db.begin().await.unwrap();
        let first = database_now(&txn).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        let second = database_now(&txn).await.unwrap();
        assert!(second > first);
        txn.rollback().await.unwrap();
    }
}
