/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 * You may not remove or alter this copyright notice or license header.
 */

//! Authentication artifact retention shared by the host worker and operator CLI.
//! Destructive expiry decisions need corroborated time, not signing authority.
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr, Statement, TransactionTrait};

use crate::application_clock::{application_clock, clock_unavailable, observe};
use crate::jwt_clock::ClockGuard;

pub const AUTH_ARTIFACT_RETENTION_DAYS: i64 = 30;
pub const OAUTH_TOKEN_RETENTION_DAYS: i64 = 120;
pub const OAUTH_AUTHORIZATION_CODE_RETENTION_DAYS: i64 = OAUTH_TOKEN_RETENTION_DAYS;

// Do not delete at the edge of the documented host/DB skew and sample
// uncertainty. A small delay is safer than losing replay evidence.
const RETENTION_CLOCK_MARGIN_MS: i64 = 2_000;

#[derive(Debug, Default, Clone, Copy)]
pub struct AuthRetentionPurge {
    pub sessions: u64,
    pub invite_consumptions: u64,
    pub invites: u64,
    pub email_change_requests: u64,
    pub oauth_tokens: u64,
    pub oauth_authorization_codes: u64,
}

fn parameter(backend: DbBackend) -> Result<&'static str, DbErr> {
    match backend {
        DbBackend::Postgres => Ok("$1"),
        DbBackend::Sqlite => Ok("?"),
        _ => Err(DbErr::Custom("Unsupported auth retention database".into())),
    }
}

async fn verified_now(tx: &DatabaseTransaction, clock: &ClockGuard) -> Result<DateTime<Utc>, DbErr> {
    let sample = observe(tx, clock).await?;
    conservative_now(sample.host_before, sample.database_now)
}

fn conservative_now(host_before: DateTime<Utc>, db_now: DateTime<Utc>) -> Result<DateTime<Utc>, DbErr> {
    host_before.min(db_now)
        .checked_sub_signed(Duration::milliseconds(RETENTION_CLOCK_MARGIN_MS))
        .ok_or_else(clock_unavailable)
}

fn cutoff(now: DateTime<Utc>, days: i64) -> Result<DateTime<Utc>, DbErr> {
    now.checked_sub_signed(Duration::days(days)).ok_or_else(clock_unavailable)
}

async fn delete_before(
    tx: &DatabaseTransaction,
    table: &'static str,
    now: DateTime<Utc>,
    inclusive: bool,
) -> Result<u64, DbErr> {
    let backend = tx.get_database_backend();
    let param = parameter(backend)?;
    let comparison = if inclusive { "<=" } else { "<" };
    // Table identifiers are fixed owner constants, never user input.
    Ok(tx.execute_raw(Statement::from_sql_and_values(
        backend,
        format!("DELETE FROM {table} WHERE expires_at {comparison} {param}"),
        [now.into()],
    )).await?.rows_affected())
}

async fn delete_artifacts(
    tx: &DatabaseTransaction, now: DateTime<Utc>, sessions_only: bool,
) -> Result<AuthRetentionPurge, DbErr> {
    let common_cutoff = cutoff(now, AUTH_ARTIFACT_RETENTION_DAYS)?;
    let mut report = AuthRetentionPurge {
        sessions: delete_before(tx, "sessions", common_cutoff, false).await?,
        ..Default::default()
    };
    if sessions_only {
        return Ok(report);
    }
    report.invite_consumptions = delete_before(tx, "auth_invite_consumptions", common_cutoff, false).await?;
    report.invites = delete_before(tx, "auth_invites", common_cutoff, false).await?;
    report.email_change_requests = delete_before(tx, "auth_email_change_requests", now, true).await?;

    // Retain the complete lineage until its latest member is past retention.
    // Descendants first for the SQLite immediate root guard.
    let backend = tx.get_database_backend();
    let param = parameter(backend)?;
    let oauth_cutoff = cutoff(now, OAUTH_TOKEN_RETENTION_DAYS)?;
    let descendants = tx.execute_raw(Statement::from_sql_and_values(
        backend,
        format!("DELETE FROM oauth_tokens WHERE id <> family_id AND family_id IN (SELECT family_id FROM oauth_tokens GROUP BY family_id HAVING MAX(expires_at) < {param})"),
        [oauth_cutoff.into()],
    )).await?;
    let roots = tx.execute_raw(Statement::from_sql_and_values(
        backend,
        format!("DELETE FROM oauth_tokens WHERE id = family_id AND family_id IN (SELECT family_id FROM oauth_tokens GROUP BY family_id HAVING MAX(expires_at) < {param}) AND NOT EXISTS (SELECT 1 FROM oauth_tokens member WHERE member.family_id = oauth_tokens.id AND member.id <> oauth_tokens.id)"),
        [oauth_cutoff.into()],
    )).await?;
    report.oauth_tokens = descendants.rows_affected() + roots.rows_affected();
    let code_cutoff = cutoff(now, OAUTH_AUTHORIZATION_CODE_RETENTION_DAYS)?;
    report.oauth_authorization_codes = tx.execute_raw(Statement::from_sql_and_values(
        backend,
        format!("DELETE FROM oauth_authorization_codes WHERE expires_at < {param} AND NOT EXISTS (SELECT 1 FROM oauth_tokens token WHERE token.authorization_code_id = oauth_authorization_codes.id)"),
        [code_cutoff.into()],
    )).await?.rows_affected();
    Ok(report)
}

async fn finish_purge(
    tx: DatabaseTransaction,
    result: Result<AuthRetentionPurge, DbErr>,
) -> Result<AuthRetentionPurge, DbErr> {
    match result {
        Ok(report) => {
            tx.commit().await?;
            Ok(report)
        }
        Err(error) => {
            tx.rollback().await?;
            Err(error)
        }
    }
}

async fn purge(db: &DatabaseConnection, clock: &ClockGuard, sessions_only: bool) -> Result<AuthRetentionPurge, DbErr> {
    let tx = db.begin().await?;
    let result = async {
        let now = verified_now(&tx, clock).await?;
        let report = delete_artifacts(&tx, now, sessions_only).await?;
        // Never advance deletion cutoffs during a sweep. Require another
        // bounded observation on the same transaction before committing.
        let final_now = verified_now(&tx, clock).await?;
        if final_now < now {
            clock.trip();
            return Err(clock_unavailable());
        }
        Ok(report)
    }.await;
    finish_purge(tx, result).await
}

/// Complete transactional worker sweep, with a process-lifetime clock latch.
pub async fn purge_expired_auth_artifacts(db: &DatabaseConnection) -> Result<AuthRetentionPurge, DbErr> {
    purge(db, application_clock(), false).await
}

/// Operator session-only cleanup. Uses the same replay-evidence retention
/// horizon and time checks as the worker; does not sweep unrelated tables.
pub async fn purge_expired_sessions(db: &DatabaseConnection) -> Result<u64, DbErr> {
    Ok(purge(db, application_clock(), true).await?.sessions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_uses_earliest_observation_with_margin() {
        let now = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
        let later = now + Duration::milliseconds(500);
        let expected = now - Duration::seconds(2);
        assert_eq!(conservative_now(now, later).unwrap(), expected);
        assert_eq!(conservative_now(later, now).unwrap(), expected);
    }

    #[tokio::test]
    async fn session_cleanup_preserves_recent_expiry_and_rolls_back_fault() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE sessions (id INTEGER PRIMARY KEY, expires_at TEXT NOT NULL)").await.unwrap();
        let now = Utc::now();
        for (id, expiry) in [
            (1_i32, now - Duration::days(AUTH_ARTIFACT_RETENTION_DAYS + 1)),
            (2_i32, now - Duration::days(1)),
            (3_i32, now + Duration::days(1)),
        ] {
            db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite, "INSERT INTO sessions (id, expires_at) VALUES (?, ?)",
                [id.into(), expiry.into()],
            )).await.unwrap();
        }
        let count = || async {
            db.query_one_raw(Statement::from_string(DbBackend::Sqlite, "SELECT COUNT(*) AS count FROM sessions".to_string()))
                .await.unwrap().unwrap().try_get::<i64>("", "count").unwrap()
        };
        let clock = ClockGuard::new();
        let tx = db.begin().await.unwrap();
        let verified = verified_now(&tx, &clock).await.unwrap();
        let report = delete_artifacts(&tx, verified, true).await.unwrap();
        assert_eq!(report.sessions, 1);
        clock.trip();
        let final_sample = verified_now(&tx, &clock).await.map(|_| report);
        assert!(finish_purge(tx, final_sample).await.is_err());
        assert_eq!(count().await, 3);
        assert!(purge(&db, &clock, true).await.is_err());
        assert_eq!(count().await, 3);
        assert_eq!(purge_expired_sessions(&db).await.unwrap(), 1);
        assert_eq!(count().await, 2);
        assert_eq!(purge_expired_sessions(&db).await.unwrap(), 0);
    }
}
