//! SQL time with a durable monotonic floor. Host time is only an independent
//! sanity check, never a substitute for database time.
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement, TransactionTrait};

const MAX_CLOCK_SKEW_MS: i64 = 2_000;
const MAX_HOST_STEP_MS: i64 = 100;
const MAX_OBSERVATION: Duration = Duration::from_secs(2);

fn unavailable(reason: &str) -> DbErr {
    DbErr::Custom(format!("outbox temporal authority unavailable: {reason}"))
}

fn validate_sample(before: i64, after: i64, elapsed: Duration, observed: i64, floor: i64) -> Result<(), DbErr> {
    if elapsed > MAX_OBSERVATION
        || (i128::from(after) - i128::from(before) - elapsed.as_millis() as i128).abs()
            > i128::from(MAX_HOST_STEP_MS)
    {
        return Err(unavailable("host clock discontinuity or slow observation"));
    }
    if floor < 0 || observed < floor {
        return Err(unavailable("database clock is behind its durable floor"));
    }
    if observed < before.saturating_sub(MAX_CLOCK_SKEW_MS)
        || observed > after.saturating_add(MAX_CLOCK_SKEW_MS)
    {
        return Err(unavailable("database clock exceeds host sanity bound"));
    }
    Ok(())
}

/// Read SQL time and the durable floor in one statement. This is read-only:
/// an owner transaction never acquires the shared temporal writer lock.
/// A stale repeatable-read snapshot may observe an older committed floor.
pub async fn database_now(connection: &impl ConnectionTrait) -> Result<DateTime<Utc>, DbErr> {
    let backend = connection.get_database_backend();
    let expression = match backend {
        DbBackend::Postgres => "CAST(FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000) AS BIGINT)",
        DbBackend::Sqlite => "CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)",
        _ => return Err(unavailable("unsupported database backend")),
    };
    let started = Instant::now();
    let before = Utc::now().timestamp_millis();
    let row = connection
        .query_one_raw(Statement::from_string(
            backend,
            format!("SELECT {expression} AS now_ms, high_water_ms FROM outbox_temporal_clock WHERE id = 1"),
        ))
        .await?
        .ok_or_else(|| unavailable("missing clock floor"))?;
    let after = Utc::now().timestamp_millis();
    let elapsed = started.elapsed();
    let millis: i64 = row.try_get("", "now_ms")?;
    let floor: i64 = row.try_get("", "high_water_ms")?;
    validate_sample(before, after, elapsed, millis, floor)?;
    DateTime::from_timestamp_millis(millis).ok_or_else(|| unavailable("timestamp out of range"))
}

/// Advance the durable high-water mark in its own short transaction. Call
/// only at an independent admission boundary, before owner/event locks;
/// never from a transaction holding domain locks or broker I/O.
pub async fn observe_database_clock(db: &DatabaseConnection) -> Result<DateTime<Utc>, DbErr> {
    let txn = db.begin().await?;
    let locked = txn.execute_unprepared(
        "UPDATE outbox_temporal_clock SET high_water_ms = high_water_ms WHERE id = 1",
    ).await?;
    if locked.rows_affected() != 1 {
        txn.rollback().await?;
        return Err(unavailable("missing clock floor"));
    }
    let now = match database_now(&txn).await {
        Ok(now) => now,
        Err(error) => {
            txn.rollback().await?;
            return Err(error);
        }
    };
    let backend = txn.get_database_backend();
    let sql = match backend {
        DbBackend::Postgres => "UPDATE outbox_temporal_clock SET high_water_ms = $1 WHERE id = 1",
        DbBackend::Sqlite => "UPDATE outbox_temporal_clock SET high_water_ms = ? WHERE id = 1",
        _ => return Err(unavailable("unsupported database backend")),
    };
    let advanced = txn.execute_raw(Statement::from_sql_and_values(
        backend, sql, [now.timestamp_millis().into()],
    )).await?;
    if advanced.rows_affected() != 1 {
        txn.rollback().await?;
        return Err(unavailable("clock floor disappeared"));
    }
    txn.commit().await?;
    Ok(now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    #[test]
    fn observation_rejects_backward_floor_and_forward_skew() {
        let elapsed = Duration::from_millis(10);
        assert!(validate_sample(10_000, 10_010, elapsed, 10_005, 10_000).is_ok());
        assert!(validate_sample(10_000, 10_010, elapsed, 10_005, 10_006).is_err());
        assert!(validate_sample(10_000, 10_010, elapsed, 13_000, 0).is_err());
        assert!(validate_sample(10_000, 11_000, elapsed, 10_005, 0).is_err());
    }

    #[tokio::test]
    async fn durable_floor_is_monotonic_and_a_future_floor_fails_closed() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        assert!(database_now(&db).await.is_err());
        crate::SysEventsMigration.up(&SchemaManager::new(&db)).await.unwrap();
        let first = observe_database_clock(&db).await.unwrap();
        let txn = db.begin().await.unwrap();
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert!(database_now(&txn).await.unwrap() > first);
        txn.rollback().await.unwrap();
        let second = observe_database_clock(&db).await.unwrap();
        assert!(second >= first);
        for sql in [
            "DELETE FROM outbox_temporal_clock",
            "INSERT OR REPLACE INTO outbox_temporal_clock VALUES (1, 0)",
            "UPDATE outbox_temporal_clock SET high_water_ms = 0",
            "UPDATE outbox_temporal_clock SET high_water_ms = high_water_ms + 60000",
            "UPDATE outbox_temporal_clock SET high_water_ms = high_water_ms + 0.5",
            "UPDATE outbox_temporal_clock SET id = 2",
        ] {
            assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
        }
        crate::migration::create_outbox_temporal_clock(&SchemaManager::new(&db)).await.unwrap();
        assert!(observe_database_clock(&db).await.unwrap() >= second);
    }

    #[tokio::test]
    async fn legacy_future_floor_is_preserved_and_fails_closed() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let manager = SchemaManager::new(&db);
        crate::migration::create_outbox_temporal_clock(&manager).await.unwrap();
        let future = Utc::now().timestamp_millis() + 60000;
        db.execute_unprepared(&format!("UPDATE outbox_temporal_clock SET high_water_ms = {future}")).await.unwrap();
        crate::migration::create_outbox_temporal_clock_integrity(&manager).await.unwrap();
        assert!(database_now(&db).await.is_err());
        assert!(observe_database_clock(&db).await.is_err());
        assert!(db.execute_unprepared("UPDATE outbox_temporal_clock SET high_water_ms = 0").await.is_err());
        crate::migration::create_outbox_temporal_clock(&manager).await.unwrap();
        assert!(database_now(&db).await.is_err());
    }

    #[tokio::test]
    async fn missing_legacy_singleton_is_not_reseeded() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let manager = SchemaManager::new(&db);
        crate::migration::create_outbox_temporal_clock(&manager).await.unwrap();
        db.execute_unprepared("DELETE FROM outbox_temporal_clock").await.unwrap();
        crate::migration::create_outbox_temporal_clock(&manager).await.unwrap();
        assert!(crate::migration::create_outbox_temporal_clock_integrity(&manager).await.is_err());
        assert!(database_now(&db).await.is_err());
    }
}
