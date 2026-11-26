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
use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement};

use crate::jwt_clock::{ClockGuard, ClockSample};

// All application DB consumers in one process share the same latch. Production
// assumes a single trusted primary; this is not a durable or distributed clock.
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

/// Corroborate host time against a fresh observation on the supplied connection
/// or transaction. Do not substitute SQL transaction-start time (now()).
/// Credential timestamps remain on the host UTC timeline; DB time is evidence,
/// not an alternate source for extending a credential deadline.
pub async fn verified_now(db: &impl ConnectionTrait) -> Result<DateTime<Utc>, DbErr> {
    Ok(observe(db, application_clock()).await?.host_after)
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
