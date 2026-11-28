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
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr, Statement,
    IsolationLevel, TransactionTrait,
};

use crate::application_clock::{
    admit_with_clock, application_clock, clock_unavailable, observe_with_floor,
};
use crate::jwt_clock::ClockGuard;
use uuid::Uuid;

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

async fn verified_now(
    tx: &DatabaseTransaction,
    clock: &ClockGuard,
) -> Result<DateTime<Utc>, DbErr> {
    let sample = observe_with_floor(tx, clock).await?;
    conservative_now(sample.host_before, sample.database_now)
}

fn conservative_now(
    host_before: DateTime<Utc>,
    db_now: DateTime<Utc>,
) -> Result<DateTime<Utc>, DbErr> {
    host_before
        .min(db_now)
        .checked_sub_signed(Duration::milliseconds(RETENTION_CLOCK_MARGIN_MS))
        .ok_or_else(clock_unavailable)
}

fn cutoff(now: DateTime<Utc>, days: i64) -> Result<DateTime<Utc>, DbErr> {
    now.checked_sub_signed(Duration::days(days))
        .ok_or_else(clock_unavailable)
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
    // Row NOWAIT does not cover the implicit DELETE relation lock.
    if backend == DbBackend::Postgres {
        tx.execute_unprepared(&format!("LOCK TABLE {table} IN ROW EXCLUSIVE MODE NOWAIT")).await?;
    }
    // Table identifiers are fixed owner constants, never user input.
    // Do not block on an earlier owner's child row while retaining other
    // expired artifacts from this sweep. A busy candidate aborts the sweep.
    let sql = match backend {
        DbBackend::Postgres => format!(
            "WITH candidates AS MATERIALIZED (
                 SELECT id FROM {table} WHERE expires_at {comparison} {param}
                 ORDER BY id FOR UPDATE NOWAIT
             )
             DELETE FROM {table} target USING candidates
             WHERE target.id = candidates.id"
        ),
        _ => format!("DELETE FROM {table} WHERE expires_at {comparison} {param}"),
    };
    Ok(tx
        .execute_raw(Statement::from_sql_and_values(
            backend,
            sql,
            [now.into()],
        ))
        .await?
        .rows_affected())
}


/// Legacy receipts can have a different expiry from the issuance ledger.
/// Leave them alone until the ledger's own retention horizon is reached.
async fn delete_expired_invite_receipts(
    tx: &DatabaseTransaction,
    cutoff: DateTime<Utc>,
) -> Result<u64, DbErr> {
    let backend = tx.get_database_backend();
    let param = parameter(backend)?;
    if backend == DbBackend::Postgres {
        tx.execute_unprepared("LOCK TABLE auth_invite_consumptions IN ROW EXCLUSIVE MODE NOWAIT").await?;
    }
    let predicate = format!(
        "c.expires_at < {param} AND NOT EXISTS (
             SELECT 1 FROM auth_invites i WHERE i.token_hash = c.token_hash)"
    );
    let sql = match backend {
        DbBackend::Postgres => format!(
            "WITH candidates AS MATERIALIZED (
                SELECT c.id FROM auth_invite_consumptions c WHERE {predicate}
                ORDER BY c.id FOR UPDATE OF c NOWAIT
             ) DELETE FROM auth_invite_consumptions c USING candidates
             WHERE c.id = candidates.id"
        ),
        _ => format!("DELETE FROM auth_invite_consumptions AS c WHERE {predicate}"),
    };
    Ok(tx.execute_raw(Statement::from_sql_and_values(
        backend, sql, [cutoff.into()],
    )).await?.rows_affected())
}

/// The session DELETE cascades to history. Pin exact parent IDs first,
/// then use a fresh READ COMMITTED statement to fence their history: a
/// child committed before the parent lock may not be visible to the first
/// snapshot. The parent FOR UPDATE prevents new FK children until commit.
async fn delete_expired_sessions(
    tx: &DatabaseTransaction,
    cutoff: DateTime<Utc>,
) -> Result<u64, DbErr> {
    if tx.get_database_backend() != DbBackend::Postgres {
        return delete_before(tx, "sessions", cutoff, false).await;
    }
    tx.execute_unprepared("LOCK TABLE sessions IN ROW EXCLUSIVE MODE NOWAIT").await?;
    tx.execute_unprepared("LOCK TABLE session_refresh_history IN ROW EXCLUSIVE MODE NOWAIT").await?;
    let rows = tx.query_all_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id FROM sessions WHERE expires_at < $1 ORDER BY id FOR UPDATE NOWAIT",
        [cutoff.into()],
    )).await?;
    let ids = rows.into_iter().map(|row| row.try_get::<Uuid>("", "id"))
        .collect::<Result<Vec<_>, DbErr>>()?;
    if ids.is_empty() {
        return Ok(0);
    }
    // One bound value avoids one query per session or a variable-sized
    // parameter list. Both subsequent statements use exactly the pinned IDs.
    let keys = serde_json::to_string(&ids).map_err(|e| DbErr::Custom(e.to_string()))?;
    tx.query_all_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT h.token_hash FROM session_refresh_history h
         JOIN jsonb_array_elements_text(CAST(CAST($1 AS text) AS jsonb)) AS candidate(id)
           ON h.session_id = candidate.id::uuid
         ORDER BY h.session_id, h.token_hash FOR UPDATE OF h NOWAIT",
        [keys.clone().into()],
    )).await?;
    let result = tx.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "DELETE FROM sessions target
         USING jsonb_array_elements_text(CAST(CAST($1 AS text) AS jsonb)) AS candidate(id)
         WHERE target.id = candidate.id::uuid",
        [keys.into()],
    )).await?;
    Ok(result.rows_affected())
}

/// Lock OAuth owner applications before any child deletion in the sweep.
/// The candidate query is only a hint: all subsequent OAuth deletion is
/// restricted to these exact application IDs. A new candidate on another
/// application is left for the next sweep.
async fn lock_oauth_retention_apps(
    tx: &DatabaseTransaction,
    token_cutoff: DateTime<Utc>,
    code_cutoff: DateTime<Utc>,
) -> Result<Vec<Uuid>, DbErr> {
    tx.execute_unprepared("LOCK TABLE oauth_apps IN ROW SHARE MODE NOWAIT").await?;
    tx.execute_unprepared("LOCK TABLE oauth_tokens, oauth_authorization_codes IN ROW EXCLUSIVE MODE NOWAIT").await?;
    let rows = tx.query_all_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT app.id FROM oauth_apps app
         WHERE EXISTS (SELECT 1 FROM oauth_tokens t WHERE t.app_id = app.id AND t.expires_at < $1)
            OR EXISTS (SELECT 1 FROM oauth_authorization_codes c WHERE c.app_id = app.id AND c.expires_at < $2)
         ORDER BY app.id FOR UPDATE OF app NOWAIT",
        [token_cutoff.into(), code_cutoff.into()],
    )).await?;
    rows.into_iter().map(|row| row.try_get("", "id")).collect()
}

async fn delete_postgres_oauth_artifacts(
    tx: &DatabaseTransaction,
    apps: Vec<Uuid>,
    token_cutoff: DateTime<Utc>,
    code_cutoff: DateTime<Utc>,
    report: &mut AuthRetentionPurge,
) -> Result<(), DbErr> {
    for app in apps {
        // Root first. The credential FK guard retains the app and family
        // root on insertion; a cooperating exchange cannot append here.
        let roots = tx.query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT root.id FROM oauth_tokens root
             WHERE root.app_id = $1 AND root.id = root.family_id AND root.expires_at < $2
               AND NOT EXISTS (SELECT 1 FROM oauth_tokens member
                   WHERE member.family_id = root.id AND member.expires_at >= $2)
             ORDER BY root.id FOR UPDATE OF root NOWAIT",
            [app.into(), token_cutoff.into()],
        )).await?;
        for root in roots {
            let family: Uuid = root.try_get("", "id")?;
            // Do not rely on the earlier MAX/NOT EXISTS snapshot. Lock every
            // member, including a member that became ineligible meanwhile.
            tx.query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT id FROM oauth_tokens WHERE family_id = $1
                 ORDER BY id FOR UPDATE NOWAIT",
                [family.into()],
            )).await?;
            let descendants = tx.execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "DELETE FROM oauth_tokens t WHERE t.family_id = $1 AND t.id <> t.family_id
                   AND NOT EXISTS (SELECT 1 FROM oauth_tokens member
                       WHERE member.family_id = $1 AND member.expires_at >= $2)",
                [family.into(), token_cutoff.into()],
            )).await?;
            let root = tx.execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "DELETE FROM oauth_tokens t WHERE t.id = $1 AND t.family_id = $1
                   AND t.expires_at < $2
                   AND NOT EXISTS (SELECT 1 FROM oauth_tokens member
                       WHERE member.family_id = $1 AND member.id <> $1)",
                [family.into(), token_cutoff.into()],
            )).await?;
            report.oauth_tokens += descendants.rows_affected() + root.rows_affected();
        }
        // Authorization-code replay evidence is kept until its last
        // registered token disappears. The app fence spans both operations.
        report.oauth_authorization_codes += tx.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "WITH candidates AS MATERIALIZED (
                 SELECT c.id FROM oauth_authorization_codes c
                 WHERE c.app_id = $1 AND c.expires_at < $2
                   AND NOT EXISTS (SELECT 1 FROM oauth_tokens t WHERE t.authorization_code_id = c.id)
                 ORDER BY c.id FOR UPDATE OF c NOWAIT
             )
             DELETE FROM oauth_authorization_codes c USING candidates WHERE c.id = candidates.id",
            [app.into(), code_cutoff.into()],
        )).await?.rows_affected();
    }
    Ok(())
}

async fn delete_artifacts(
    tx: &DatabaseTransaction,
    now: DateTime<Utc>,
    sessions_only: bool,
) -> Result<AuthRetentionPurge, DbErr> {
    let common_cutoff = cutoff(now, AUTH_ARTIFACT_RETENTION_DAYS)?;
    let backend = tx.get_database_backend();
    let oauth_cutoff = cutoff(now, OAUTH_TOKEN_RETENTION_DAYS)?;
    let code_cutoff = cutoff(now, OAUTH_AUTHORIZATION_CODE_RETENTION_DAYS)?;
    let apps = if !sessions_only && backend == DbBackend::Postgres {
        lock_oauth_retention_apps(tx, oauth_cutoff, code_cutoff).await?
    } else {
        Vec::new()
    };
    let mut report = AuthRetentionPurge {
        sessions: delete_expired_sessions(tx, common_cutoff).await?,
        ..Default::default()
    };
    if sessions_only {
        return Ok(report);
    }
    // A receipt may be pruned only after its issuance ledger is gone.
    report.invites = delete_before(tx, "auth_invites", common_cutoff, false).await?;
    report.invite_consumptions = delete_expired_invite_receipts(tx, common_cutoff).await?;
    report.email_change_requests =
        delete_before(tx, "auth_email_change_requests", now, true).await?;

    if backend == DbBackend::Postgres {
        delete_postgres_oauth_artifacts(tx, apps, oauth_cutoff, code_cutoff, &mut report).await?;
        return Ok(report);
    }

    // SQLite serializes writers. Retain the complete lineage until its
    // latest member is past retention; descendants precede the root.
    let param = parameter(backend)?;
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

async fn purge(
    db: &DatabaseConnection,
    clock: &ClockGuard,
    sessions_only: bool,
) -> Result<AuthRetentionPurge, DbErr> {
    admit_with_clock(db, clock).await?;
    let tx = match db.get_database_backend() {
        DbBackend::Postgres => db.begin_with_config(Some(IsolationLevel::ReadCommitted), None).await?,
        DbBackend::Sqlite => db.begin().await?,
        _ => return Err(DbErr::Custom("Unsupported auth retention database".into())),
    };
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
    }
    .await;
    finish_purge(tx, result).await
}

/// Complete transactional worker sweep, with a process-lifetime clock latch.
pub async fn purge_expired_auth_artifacts(
    db: &DatabaseConnection,
) -> Result<AuthRetentionPurge, DbErr> {
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
        db.execute_unprepared(
            "CREATE TABLE sessions (id INTEGER PRIMARY KEY, expires_at TEXT NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared("CREATE TABLE auth_application_clock (id INTEGER PRIMARY KEY, host_ms BIGINT NOT NULL, database_ms BIGINT NOT NULL)").await.unwrap();
        db.execute_unprepared("INSERT INTO auth_application_clock VALUES (1, 0, 0)")
            .await
            .unwrap();
        let now = Utc::now();
        for (id, expiry) in [
            (
                1_i32,
                now - Duration::days(AUTH_ARTIFACT_RETENTION_DAYS + 1),
            ),
            (2_i32, now - Duration::days(1)),
            (3_i32, now + Duration::days(1)),
        ] {
            db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO sessions (id, expires_at) VALUES (?, ?)",
                [id.into(), expiry.into()],
            ))
            .await
            .unwrap();
        }
        let count = || async {
            db.query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM sessions".to_string(),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get::<i64>("", "count")
            .unwrap()
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

    async fn postgres_tx(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = db.begin_with_config(Some(IsolationLevel::ReadCommitted), None).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_AUTH_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_retention_fences_apps_and_whole_families_before_deleting() {
        let db = sea_orm::Database::connect(std::env::var("RUSTOK_AUTH_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let schema = format!("auth_retention_fence_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = postgres_tx(&db, &schema).await;
        for table in ["sessions", "auth_invite_consumptions", "auth_invites", "auth_email_change_requests"] {
            let digest = if matches!(table, "auth_invites" | "auth_invite_consumptions") {
                ", token_hash TEXT NOT NULL UNIQUE"
            } else { "" };
            setup.execute_unprepared(&format!("CREATE TABLE {table} (id UUID PRIMARY KEY, expires_at TIMESTAMPTZ NOT NULL{digest})")).await.unwrap();
        }
        setup.execute_unprepared(
            "CREATE TABLE session_refresh_history (
                 token_hash TEXT PRIMARY KEY, session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE);
             CREATE TABLE oauth_apps (id UUID PRIMARY KEY);
             CREATE TABLE oauth_authorization_codes (id UUID PRIMARY KEY, app_id UUID NOT NULL REFERENCES oauth_apps(id), expires_at TIMESTAMPTZ NOT NULL);
             CREATE TABLE oauth_tokens (id UUID PRIMARY KEY, app_id UUID NOT NULL REFERENCES oauth_apps(id),
                 family_id UUID NOT NULL REFERENCES oauth_tokens(id) DEFERRABLE INITIALLY DEFERRED,
                 authorization_code_id UUID, expires_at TIMESTAMPTZ NOT NULL);
             INSERT INTO sessions VALUES
                ('10000000-0000-0000-0000-000000000001', '2000-01-01 UTC'),
                ('10000000-0000-0000-0000-000000000002', '2099-01-01 UTC');
             INSERT INTO session_refresh_history VALUES
                ('expired-session-history', '10000000-0000-0000-0000-000000000001'),
                ('live-session-history', '10000000-0000-0000-0000-000000000002');
             INSERT INTO oauth_apps VALUES ('20000000-0000-0000-0000-000000000001');
             INSERT INTO oauth_authorization_codes VALUES
                ('30000000-0000-0000-0000-000000000001', '20000000-0000-0000-0000-000000000001', '2000-01-01 UTC'),
                ('30000000-0000-0000-0000-000000000002', '20000000-0000-0000-0000-000000000001', '2000-01-01 UTC');
             INSERT INTO oauth_tokens VALUES
                ('40000000-0000-0000-0000-000000000001','20000000-0000-0000-0000-000000000001','40000000-0000-0000-0000-000000000001','30000000-0000-0000-0000-000000000001','2000-01-01 UTC'),
                ('40000000-0000-0000-0000-000000000002','20000000-0000-0000-0000-000000000001','40000000-0000-0000-0000-000000000001','30000000-0000-0000-0000-000000000001','2000-01-01 UTC'),
                ('40000000-0000-0000-0000-000000000003','20000000-0000-0000-0000-000000000001','40000000-0000-0000-0000-000000000003','30000000-0000-0000-0000-000000000002','2000-01-01 UTC'),
                ('40000000-0000-0000-0000-000000000004','20000000-0000-0000-0000-000000000001','40000000-0000-0000-0000-000000000003','30000000-0000-0000-0000-000000000002','2099-01-01 UTC');"
        ).await.unwrap();
        setup.commit().await.unwrap();
        let now = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
        for lock in [
            "LOCK TABLE oauth_apps IN EXCLUSIVE MODE",
            "LOCK TABLE oauth_tokens IN SHARE MODE",
            "LOCK TABLE oauth_authorization_codes IN SHARE MODE",
            "LOCK TABLE sessions IN SHARE MODE",
            "LOCK TABLE session_refresh_history IN SHARE MODE",
            "SELECT token_hash FROM session_refresh_history WHERE token_hash = 'expired-session-history' FOR UPDATE",
            "LOCK TABLE auth_invite_consumptions IN SHARE MODE",
            "LOCK TABLE auth_invites IN SHARE MODE",
            "LOCK TABLE auth_email_change_requests IN SHARE MODE",
            "SELECT id FROM oauth_apps FOR UPDATE",
            "SELECT id FROM sessions FOR UPDATE",
            "SELECT id FROM oauth_tokens WHERE id = '40000000-0000-0000-0000-000000000002' FOR UPDATE",
            "SELECT id FROM oauth_authorization_codes WHERE id = '30000000-0000-0000-0000-000000000001' FOR UPDATE",
        ] {
            let owner = postgres_tx(&db, &schema).await;
            owner.execute_unprepared(lock).await.unwrap();
            let writer = postgres_tx(&db, &schema).await;
            let result = tokio::time::timeout(std::time::Duration::from_secs(3), delete_artifacts(&writer, now, false))
                .await.expect("retention must reject a busy candidate rather than wait");
            assert!(result.is_err(), "{lock}");
            assert!(finish_purge(writer, result).await.is_err());
            owner.rollback().await.unwrap();
            let check = postgres_tx(&db, &schema).await;
            let row = check.query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT (SELECT COUNT(*) FROM sessions) AS sessions,
                        (SELECT COUNT(*) FROM session_refresh_history) AS history,
                        (SELECT COUNT(*) FROM oauth_tokens) AS tokens,
                        (SELECT COUNT(*) FROM oauth_authorization_codes) AS codes",
            )).await.unwrap().unwrap();
            assert_eq!(row.try_get::<i64>("", "sessions").unwrap(), 2);
            assert_eq!(row.try_get::<i64>("", "history").unwrap(), 2);
            assert_eq!(row.try_get::<i64>("", "tokens").unwrap(), 4);
            assert_eq!(row.try_get::<i64>("", "codes").unwrap(), 2);
            check.rollback().await.unwrap();
        }
        let owner = postgres_tx(&db, &schema).await;
        owner.execute_unprepared("SELECT token_hash FROM session_refresh_history WHERE token_hash = 'live-session-history' FOR UPDATE").await.unwrap();
        let writer = postgres_tx(&db, &schema).await;
        let report = tokio::time::timeout(
            std::time::Duration::from_secs(3), delete_artifacts(&writer, now, false),
        ).await.expect("unrelated history must not block retention").unwrap();
        assert_eq!(report.sessions, 1);
        assert_eq!(report.oauth_tokens, 2);
        assert_eq!(report.oauth_authorization_codes, 1);
        writer.commit().await.unwrap();
        owner.rollback().await.unwrap();

        let writer = postgres_tx(&db, &schema).await;
        let report = delete_artifacts(&writer, now, false).await.unwrap();
        assert_eq!(report.oauth_tokens, 0);
        assert_eq!(report.oauth_authorization_codes, 0);
        assert_eq!(report.sessions, 0);
        let history = writer.query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT token_hash FROM session_refresh_history",
        )).await.unwrap().unwrap();
        assert_eq!(history.try_get::<String>("", "token_hash").unwrap(), "live-session-history");
        writer.commit().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
