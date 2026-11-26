/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 * You may not remove or alter this copyright notice or license header.
 */

//! Bounded observations of the application database clock, separate from the
//! independent JWT signing authority. A detected discontinuity fences this
//! process until operator remediation and restart; a slow read is inconclusive.
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement, TransactionTrait};

use crate::jwt_clock::{ClockGuard, ClockSample};

// All application DB consumers in one process share the same latch. Production
// assumes a single trusted primary. Admission evidence is persisted there.
static APPLICATION_CLOCK: OnceLock<ClockGuard> = OnceLock::new();

pub(crate) fn application_clock() -> &'static ClockGuard {
    APPLICATION_CLOCK.get_or_init(ClockGuard::new)
}

pub(crate) fn clock_unavailable() -> DbErr {
    DbErr::Custom("Auth application clock unavailable".into())
}

pub(crate) struct ClockObservation {
    pub(crate) host_before: DateTime<Utc>,
    pub(crate) host_after: DateTime<Utc>,
    pub(crate) database_now: DateTime<Utc>,
}

pub(crate) async fn observe(
    db: &impl ConnectionTrait,
    clock: &ClockGuard,
) -> Result<ClockObservation, DbErr> {
    let backend = db.get_database_backend();
    let expression = match backend {
        DbBackend::Postgres => "CAST(EXTRACT(EPOCH FROM clock_timestamp()) * 1000 AS BIGINT)",
        DbBackend::Sqlite => "CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)",
        _ => return Err(clock_unavailable()),
    };
    let before = clock.credential_sample(ClockSample::now).ok_or_else(clock_unavailable)?;
    let row = db.query_one_raw(Statement::from_string(
        backend, format!("SELECT {expression} AS now_ms"),
    )).await.map_err(|_| clock_unavailable())?;
    let after = clock.credential_sample(ClockSample::now).ok_or_else(clock_unavailable)?;
    let db_ms: i64 = row.ok_or_else(clock_unavailable)?
        .try_get("", "now_ms").map_err(|_| clock_unavailable())?;
    if !clock.validate_db(before, after, db_ms) {
        return Err(clock_unavailable());
    }
    Ok(ClockObservation {
        host_before: before.utc().ok_or_else(clock_unavailable)?,
        host_after: after.utc().ok_or_else(clock_unavailable)?,
        database_now: DateTime::<Utc>::from_timestamp_millis(db_ms).ok_or_else(clock_unavailable)?,
    })
}

#[derive(Clone, Copy)]
struct ClockFloor {
    host_ms: i64,
    database_ms: i64,
}

async fn read_floor(db: &impl ConnectionTrait, lock: bool) -> Result<ClockFloor, DbErr> {
    let backend = db.get_database_backend();
    let suffix = match (backend, lock) {
        (DbBackend::Postgres, true) => " FOR UPDATE",
        (DbBackend::Postgres | DbBackend::Sqlite, false) | (DbBackend::Sqlite, true) => "",
        _ => return Err(clock_unavailable()),
    };
    if lock && backend == DbBackend::Sqlite {
        let result = db.execute_raw(Statement::from_string(backend,
            "UPDATE auth_application_clock SET host_ms = host_ms WHERE id = 1".to_string(),
        )).await?;
        if result.rows_affected() != 1 {
            return Err(clock_unavailable());
        }
    }
    let row = db.query_one_raw(Statement::from_string(backend,
        format!("SELECT host_ms, database_ms FROM auth_application_clock WHERE id = 1{suffix}"),
    )).await?.ok_or_else(clock_unavailable)?;
    let floor = ClockFloor {
        host_ms: row.try_get("", "host_ms")?,
        database_ms: row.try_get("", "database_ms")?,
    };
    if floor.host_ms < 0 || floor.database_ms < 0 {
        return Err(clock_unavailable());
    }
    Ok(floor)
}

fn check_floor(floor: ClockFloor, sample: &ClockObservation, clock: &ClockGuard) -> Result<(), DbErr> {
    if sample.database_now.timestamp_millis() < floor.database_ms {
        clock.trip();
        return Err(clock_unavailable());
    }
    // A synchronized but slower replica may be behind another replica.
    // Reject the operation without permanently poisoning its local guard.
    if sample.host_after.timestamp_millis() < floor.host_ms {
        return Err(clock_unavailable());
    }
    Ok(())
}

/// Commit the admission sample independently of domain mutations. Call on
/// a pool before acquiring any domain transaction or row lock.
pub async fn admit_now(db: &DatabaseConnection) -> Result<DateTime<Utc>, DbErr> {
    admit_with_clock(db, application_clock()).await
}

pub(crate) async fn admit_with_clock(db: &DatabaseConnection, clock: &ClockGuard) -> Result<DateTime<Utc>, DbErr> {
    let tx = db.begin().await?;
    if tx.get_database_backend() == DbBackend::Postgres {
        tx.execute_unprepared("SET LOCAL synchronous_commit = 'on'").await?;
    }
    // Acquire the serialization fence before sampling to avoid stale writes.
    let floor = read_floor(&tx, true).await?;
    let sample = observe(&tx, clock).await?;
    check_floor(floor, &sample, clock)?;
    let backend = tx.get_database_backend();
    let sql = match backend {
        DbBackend::Postgres => "UPDATE auth_application_clock SET host_ms = $1, database_ms = $2 WHERE id = 1 AND host_ms <= $1 AND database_ms <= $2",
        DbBackend::Sqlite => "UPDATE auth_application_clock SET host_ms = ?1, database_ms = ?2 WHERE id = 1 AND host_ms <= ?1 AND database_ms <= ?2",
        _ => return Err(clock_unavailable()),
    };
    let changed = tx.execute_raw(Statement::from_sql_and_values(backend, sql, [
        sample.host_after.timestamp_millis().into(), sample.database_now.timestamp_millis().into(),
    ])).await?;
    if changed.rows_affected() != 1 {
        return Err(clock_unavailable());
    }
    tx.commit().await?;
    clock.credential_sample(ClockSample::now).ok_or_else(clock_unavailable)?;
    Ok(sample.host_after)
}

pub(crate) async fn observe_with_floor(
    db: &impl ConnectionTrait, clock: &ClockGuard,
) -> Result<ClockObservation, DbErr> {
    // Read before sampling; never acquire the global clock row lock inside
    // a business transaction (which may already hold unrelated locks).
    let floor = read_floor(db, false).await?;
    let sample = observe(db, clock).await?;
    check_floor(floor, &sample, clock)?;
    Ok(sample)
}

/// Corroborate host time against a fresh observation on the supplied connection
/// or transaction. Do not substitute SQL transaction-start time (now()).
/// Credential timestamps remain on the host UTC timeline; DB time is evidence,
/// not an alternate source for extending a credential deadline.
pub async fn verified_now(db: &impl ConnectionTrait) -> Result<DateTime<Utc>, DbErr> {
    Ok(observe_with_floor(db, application_clock()).await?.host_after)
}

/// Recheck the local latch after a commit or an asynchronous operation without
/// adding another fallible DB round trip after a one-shot credential commits.
pub fn check_local() -> Result<(), DbErr> {
    application_clock().credential_sample(ClockSample::now)
        .map(|_| ()).ok_or_else(clock_unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn admission_survives_business_rollback_and_new_guard() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE auth_application_clock (id INTEGER PRIMARY KEY, host_ms BIGINT NOT NULL, database_ms BIGINT NOT NULL)").await.unwrap();
        db.execute_unprepared("INSERT INTO auth_application_clock VALUES (1, 0, 0)").await.unwrap();
        let admitted = admit_with_clock(&db, &ClockGuard::new()).await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.rollback().await.unwrap();
        let floor = read_floor(&db, false).await.unwrap();
        assert_eq!(floor.host_ms, admitted.timestamp_millis());
        assert!(floor.database_ms > 0);
        admit_with_clock(&db, &ClockGuard::new()).await.unwrap();
        db.execute_unprepared("UPDATE auth_application_clock SET database_ms = database_ms + 60000").await.unwrap();
        let restarted = ClockGuard::new();
        assert!(admit_with_clock(&db, &restarted).await.is_err());
        assert!(restarted.faulted());
        db.execute_unprepared("DELETE FROM auth_application_clock").await.unwrap();
        assert!(admit_with_clock(&db, &ClockGuard::new()).await.is_err());
    }

    #[tokio::test]
    async fn observation_uses_transaction_clock_and_refuses_a_latched_guard() {
        use sea_orm::TransactionTrait;
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let tx = db.begin().await.unwrap();
        let clock = ClockGuard::new();
        let sample = observe(&tx, &clock).await.unwrap();
        assert!(sample.host_before <= sample.host_after);
        assert!(sample.database_now >= sample.host_before - chrono::Duration::milliseconds(500));
        assert!(sample.database_now <= sample.host_after + chrono::Duration::milliseconds(500));
        clock.trip();
        assert!(observe(&tx, &clock).await.is_err());
        tx.rollback().await.unwrap();
    }
}
