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
use std::{sync::OnceLock, time::Duration};

use chrono::{DateTime, Utc};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, DbErr, Statement, TransactionTrait};

use crate::jwt_clock::{ClockGuard, ClockSample};

// All application DB consumers in one process share the same latch. Production
// assumes a single trusted primary. Admission evidence is persisted there.
static APPLICATION_CLOCK: OnceLock<ClockGuard> = OnceLock::new();

// Never borrow the application pool for a nested clock write: a fully occupied
// application pool could otherwise deadlock while domain transactions wait.
struct LateClockStore {
    db: DatabaseConnection,
    source_backend: DbBackend,
    sqlite_binding: Option<String>,
}
static LATE_CLOCK_STORE: OnceLock<LateClockStore> = OnceLock::new();

/// A cancelled or failed commit may have reached durable storage. Do not let
/// this process continue credential admission on an uncertain outcome.
struct ClockCommit<'a> {
    clock: &'a ClockGuard,
    confirmed: bool,
}
impl Drop for ClockCommit<'_> {
    fn drop(&mut self) {
        if !self.confirmed {
            self.clock.trip();
        }
    }
}

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
    postgres_identity: Option<String>,
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
    let identity = if backend == DbBackend::Postgres {
        // Refuse replicas and identify the actual server and relation
        // namespace, not merely the configured URI (which can route).
        ", json_build_array(current_database(), current_schema(), inet_server_addr()::text, inet_server_port(), to_regclass('auth_application_clock')::oid)::text AS primary_identity, pg_is_in_recovery() AS in_recovery"
    } else { "" };
    let before = clock.credential_sample(ClockSample::now).ok_or_else(clock_unavailable)?;
    let row = db.query_one_raw(Statement::from_string(
        backend, format!("SELECT {expression} AS now_ms{identity}"),
    )).await.map_err(|_| clock_unavailable())?;
    let after = clock.credential_sample(ClockSample::now).ok_or_else(clock_unavailable)?;
    let row = row.ok_or_else(clock_unavailable)?;
    let db_ms: i64 = row.try_get("", "now_ms").map_err(|_| clock_unavailable())?;
    let postgres_identity = if backend == DbBackend::Postgres {
        if row.try_get::<bool>("", "in_recovery").map_err(|_| clock_unavailable())? {
            return Err(clock_unavailable());
        }
        Some(row.try_get("", "primary_identity").map_err(|_| clock_unavailable())?)
    } else { None };
    if !clock.validate_db(before, after, db_ms) {
        return Err(clock_unavailable());
    }
    Ok(ClockObservation {
        host_before: before.utc().ok_or_else(clock_unavailable)?,
        host_after: after.utc().ok_or_else(clock_unavailable)?,
        database_now: DateTime::<Utc>::from_timestamp_millis(db_ms).ok_or_else(clock_unavailable)?,
        postgres_identity,
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

/// Configure the independent writer pool before serving requests or running
/// operator retention. PostgreSQL uses the same trusted primary URI. SQLite
/// optionally uses a separately provisioned persistent file; its own writer
/// must never contend with the application's SQLite writer.
pub async fn configure_late_observation_store(
    source: &DatabaseConnection, primary_uri: &str,
) -> Result<(), DbErr> {
    let sidecar = match std::env::var("RUSTOK_AUTH_SQLITE_CLOCK_DATABASE_URL") {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(_) => return Err(clock_unavailable()),
    };
    let source_backend = source.get_database_backend();
    let uri = match (source_backend, sidecar.as_deref()) {
        (DbBackend::Postgres, None) => primary_uri,
        (DbBackend::Sqlite, Some(uri)) if uri.starts_with("sqlite://") && uri.ends_with("?mode=rw")
            && !uri[..uri.len() - "?mode=rw".len()].contains('?') => uri,
        (DbBackend::Sqlite, None) => return ensure_legacy_allowed(source).await,
        _ => return Err(clock_unavailable()),
    };
    if LATE_CLOCK_STORE.get().is_some() {
        return Err(clock_unavailable());
    }
    let mut options = ConnectOptions::new(uri.to_string());
    options.max_connections(1).min_connections(0)
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    let db = Database::connect(options).await.map_err(|_| clock_unavailable())?;
    if db.get_database_backend() != source_backend {
        return Err(clock_unavailable());
    }
    if source_backend == DbBackend::Sqlite {
        let source_path = sqlite_main_file(source).await?;
        let clock_path = sqlite_main_file(&db).await?;
        if source_path == clock_path {
            return Err(clock_unavailable());
        }
        // Canonical paths alone do not detect hard links. On Unix compare
        // device/inode too, before opening an independent writer transaction.
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let a = std::fs::metadata(&source_path).map_err(|_| clock_unavailable())?;
            let b = std::fs::metadata(&clock_path).map_err(|_| clock_unavailable())?;
            if a.dev() == b.dev() && a.ino() == b.ino() {
                return Err(clock_unavailable());
            }
        }
        let row = db.query_one_raw(Statement::from_string(DbBackend::Sqlite,
            "SELECT COUNT(*) AS present FROM sqlite_master WHERE name IN ('users', 'sessions', 'auth_jwt_generation_fence')".to_string(),
        )).await?.ok_or_else(clock_unavailable)?;
        if row.try_get::<i64>("", "present")? != 0 {
            return Err(clock_unavailable());
        }
    }
    let sqlite_binding = if source_backend == DbBackend::Sqlite {
        let binding = sqlite_binding(source).await?.ok_or_else(clock_unavailable)?;
        if sqlite_binding(&db).await?.as_deref() != Some(binding.as_str()) {
            return Err(clock_unavailable());
        }
        db.execute_unprepared("PRAGMA synchronous = FULL").await?;
        Some(binding)
    } else { None };
    let store = LateClockStore { db, source_backend, sqlite_binding };
    // Refuse missing, stale or wrongly routed stores at startup. No automatic
    // initialization or fallback after an explicitly configured store fails.
    observe_durably(source, &store, application_clock()).await?;
    LATE_CLOCK_STORE.set(store).map_err(|_| clock_unavailable())
}

// An offline binding in both files makes omission of the sidecar configuration
// a denial rather than a fallback to the old application-only floor.
async fn sqlite_binding(db: &impl ConnectionTrait) -> Result<Option<String>, DbErr> {
    let present = db.query_one_raw(Statement::from_string(DbBackend::Sqlite,
        "SELECT COUNT(*) AS present FROM sqlite_master WHERE type = 'table' AND name = 'auth_application_clock_sidecar_binding'".to_string(),
    )).await?.ok_or_else(clock_unavailable)?.try_get::<i64>("", "present")?;
    if present == 0 { return Ok(None); }
    let row = db.query_one_raw(Statement::from_string(DbBackend::Sqlite,
        "SELECT binding_id FROM auth_application_clock_sidecar_binding WHERE id = 1".to_string(),
    )).await?.ok_or_else(clock_unavailable)?;
    let binding: String = row.try_get("", "binding_id")?;
    let id = uuid::Uuid::parse_str(&binding).map_err(|_| clock_unavailable())?;
    if id.is_nil() || id.to_string() != binding {
        return Err(clock_unavailable());
    }
    Ok(Some(binding))
}

async fn ensure_legacy_allowed(db: &impl ConnectionTrait) -> Result<(), DbErr> {
    if db.get_database_backend() == DbBackend::Sqlite && sqlite_binding(db).await?.is_some() {
        return Err(clock_unavailable());
    }
    Ok(())
}

async fn sqlite_main_file(db: &DatabaseConnection) -> Result<std::path::PathBuf, DbErr> {
    let rows = db.query_all_raw(Statement::from_string(DbBackend::Sqlite,
        "PRAGMA database_list".to_string(),
    )).await?;
    for row in rows {
        if row.try_get::<String>("", "name")? == "main" {
            let path: String = row.try_get("", "file")?;
            if path.is_empty() {
                return Err(clock_unavailable());
            }
            return std::fs::canonicalize(path).map_err(|_| clock_unavailable());
        }
    }
    Err(clock_unavailable())
}

async fn persist_observation(
    db: &DatabaseConnection, clock: &ClockGuard,
    prior: Option<&ClockObservation>, source_floor: Option<ClockFloor>,
    expected_binding: Option<&str>,
) -> Result<ClockObservation, DbErr> {
    let tx = db.begin().await?;
    if tx.get_database_backend() == DbBackend::Postgres {
        tx.execute_unprepared("SET LOCAL synchronous_commit = 'on'").await?;
    } else if source_floor.is_some() {
        if sqlite_binding(&tx).await?.as_deref() != expected_binding || expected_binding.is_none() {
            return Err(clock_unavailable());
        }
        // Sidecar connections must use FULL (or EXTRA) synchronous commit.
        let row = tx.query_one_raw(Statement::from_string(DbBackend::Sqlite,
            "PRAGMA synchronous".to_string(),
        )).await?.ok_or_else(clock_unavailable)?;
        if row.try_get::<i64>("", "synchronous")? < 2 {
            return Err(clock_unavailable());
        }
    }
    // Acquire the serialization fence before sampling to avoid stale writes.
    let floor = read_floor(&tx, true).await?;
    if source_floor.is_some_and(|source| floor.host_ms < source.host_ms
        || floor.database_ms < source.database_ms) {
        return Err(clock_unavailable());
    }
    let started = clock.credential_sample(ClockSample::now).ok_or_else(clock_unavailable)?;
    let mut sample = observe(&tx, clock).await?;
    if let Some(prior) = prior {
        if prior.postgres_identity != sample.postgres_identity {
            return Err(clock_unavailable());
        }
        if sample.database_now < prior.database_now {
            clock.trip();
            return Err(clock_unavailable());
        }
    }
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
    let mut outcome = ClockCommit { clock, confirmed: false };
    tx.commit().await?;
    outcome.confirmed = true;
    let after = clock.credential_sample(ClockSample::now).ok_or_else(clock_unavailable)?;
    // Slow commit is inconclusive for admission, even if its floor survived.
    if !clock.validate_db(started, after, sample.database_now.timestamp_millis()) {
        return Err(clock_unavailable());
    }
    // Only expose the precision that was independently persisted.
    sample.host_after = DateTime::<Utc>::from_timestamp_millis(sample.host_after.timestamp_millis())
        .ok_or_else(clock_unavailable)?;
    Ok(sample)
}

async fn observe_durably(
    source: &impl ConnectionTrait, store: &LateClockStore, clock: &ClockGuard,
) -> Result<ClockObservation, DbErr> {
    if source.get_database_backend() != store.source_backend {
        return Err(clock_unavailable());
    }
    // Sample the supplied connection BEFORE acquiring the independent clock
    // lock. Never acquire a domain connection/lock while holding that lock.
    let prior = observe(source, clock).await?;
    let source_floor = if store.source_backend == DbBackend::Sqlite {
        if sqlite_binding(source).await?.as_deref() != store.sqlite_binding.as_deref()
            || store.sqlite_binding.is_none() {
            return Err(clock_unavailable());
        }
        Some(read_floor(source, false).await?)
    } else { None };
    persist_observation(&store.db, clock, Some(&prior), source_floor, store.sqlite_binding.as_deref()).await
}

/// Commit the admission sample independently of domain mutations. Call on
/// a pool before acquiring any domain transaction or row lock.
pub async fn admit_now(db: &DatabaseConnection) -> Result<DateTime<Utc>, DbErr> {
    admit_with_clock(db, application_clock()).await
}

pub(crate) async fn admit_with_clock(db: &DatabaseConnection, clock: &ClockGuard) -> Result<DateTime<Utc>, DbErr> {
    let sample = match LATE_CLOCK_STORE.get() {
        Some(store) => observe_durably(db, store, clock).await?,
        None => {
            ensure_legacy_allowed(db).await?;
            persist_observation(db, clock, None, None, None).await?
        },
    };
    Ok(sample.host_after)
}

pub(crate) async fn observe_with_floor(
    db: &impl ConnectionTrait, clock: &ClockGuard,
) -> Result<ClockObservation, DbErr> {
    if let Some(store) = LATE_CLOCK_STORE.get() {
        return observe_durably(db, store, clock).await;
    }
    // Compatibility for unmanaged embedders and SQLite without a sidecar.
    // No independent late-observation guarantee in this mode.
    ensure_legacy_allowed(db).await?;
    let floor = read_floor(db, false).await?;
    let sample = observe(db, clock).await?;
    check_floor(floor, &sample, clock)?;
    Ok(sample)
}

/// Corroborate host time on the supplied connection or transaction and, when
/// configured, commit a fresh independently checked sample before returning.
/// Do not substitute SQL transaction-start time (now()).
/// Credential timestamps remain on the host UTC timeline; DB time is evidence,
/// not an alternate source for extending a credential deadline.
pub async fn verified_now(db: &impl ConnectionTrait) -> Result<DateTime<Utc>, DbErr> {
    Ok(observe_with_floor(db, application_clock()).await?.host_after)
}

/// Keep the signing/lease check live, but do not replace an independently
/// persisted decision timestamp with an unpersisted later wall observation.
/// Unmanaged embedders retain their existing injected-clock contract.
pub fn decision_time(verified: DateTime<Utc>, live: DateTime<Utc>) -> Result<DateTime<Utc>, DbErr> {
    if LATE_CLOCK_STORE.get().is_none() {
        return Ok(live);
    }
    if live < verified {
        application_clock().trip();
        return Err(clock_unavailable());
    }
    if live.signed_duration_since(verified) > chrono::Duration::seconds(1) {
        return Err(clock_unavailable());
    }
    Ok(verified)
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
    async fn separate_writer_persists_late_sample_despite_domain_rollback() {
        let source = Database::connect("sqlite::memory:").await.unwrap();
        let independent = Database::connect("sqlite::memory:").await.unwrap();
        for db in [&source, &independent] {
            db.execute_unprepared("CREATE TABLE auth_application_clock (id INTEGER PRIMARY KEY, host_ms BIGINT NOT NULL, database_ms BIGINT NOT NULL)").await.unwrap();
            db.execute_unprepared("INSERT INTO auth_application_clock VALUES (1, 0, 0)").await.unwrap();
            db.execute_unprepared("PRAGMA synchronous = FULL").await.unwrap();
            db.execute_unprepared("CREATE TABLE auth_application_clock_sidecar_binding (id INTEGER PRIMARY KEY, binding_id TEXT NOT NULL)").await.unwrap();
            db.execute_unprepared("INSERT INTO auth_application_clock_sidecar_binding VALUES (1, 'f04286fc-6b01-4c91-b79d-0b987cbb6a02')").await.unwrap();
        }
        source.execute_unprepared("CREATE TABLE domain_value (id INTEGER PRIMARY KEY)").await.unwrap();
        let store = LateClockStore { db: independent.clone(), source_backend: DbBackend::Sqlite,
            sqlite_binding: Some("f04286fc-6b01-4c91-b79d-0b987cbb6a02".into()) };
        assert!(ensure_legacy_allowed(&source).await.is_err());
        let tx = source.begin().await.unwrap();
        tx.execute_unprepared("INSERT INTO domain_value VALUES (1)").await.unwrap();
        let sample = observe_durably(&tx, &store, &ClockGuard::new()).await.unwrap();
        tx.rollback().await.unwrap();
        let floor = read_floor(&independent, false).await.unwrap();
        assert_eq!(floor.host_ms, sample.host_after.timestamp_millis());
        assert_eq!(floor.database_ms, sample.database_now.timestamp_millis());
        assert_eq!(read_floor(&source, false).await.unwrap().host_ms, 0);
        independent.execute_unprepared("UPDATE auth_application_clock_sidecar_binding SET binding_id = 'f04286fc-6b01-4c91-b79d-0b987cbb6a03'").await.unwrap();
        assert!(observe_durably(&source, &store, &ClockGuard::new()).await.is_err());
        independent.execute_unprepared("UPDATE auth_application_clock_sidecar_binding SET binding_id = 'f04286fc-6b01-4c91-b79d-0b987cbb6a02'").await.unwrap();
        independent.execute_unprepared("UPDATE auth_application_clock SET database_ms = database_ms + 60000").await.unwrap();
        let restarted = ClockGuard::new();
        assert!(observe_durably(&source, &store, &restarted).await.is_err());
        assert!(restarted.faulted());
    }

    #[test]
    fn uncertain_clock_commit_fences_process() {
        let clock = ClockGuard::new();
        drop(ClockCommit { clock: &clock, confirmed: true });
        assert!(!clock.faulted());
        drop(ClockCommit { clock: &clock, confirmed: false });
        assert!(clock.faulted());
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
