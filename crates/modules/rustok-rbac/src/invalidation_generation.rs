use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, Statement};
use thiserror::Error;

pub const RBAC_PERMISSION_INVALIDATION_SCOPE: &str = "permissions";
const MAX_DURABLE_GENERATION: i64 = i64::MAX;

#[derive(Debug, Error)]
pub enum RbacInvalidationGenerationError {
    #[error("RBAC invalidation generation database error: {0}")]
    Database(#[source] sea_orm::DbErr),
    #[error("RBAC generation requires PostgreSQL or SQLite")]
    UnsupportedBackend,
    #[error("durable RBAC invalidation generation is missing, invalid or exhausted")]
    MissingOrExhausted,
    #[error("durable RBAC invalidation generation is negative: {0}")]
    Negative(i64),
}

/// Reserve the next durable permission invalidation generation inside the
/// caller-owned authorization transaction. Acquire this global row only
/// after all required tenant, credential, user, role, grant and receipt
/// fences. PostgreSQL must not wait for this late global fence. On contention,
/// roll back the entire owner transaction; never retry only the reservation.
/// Do not acquire a new earlier-ranked identity after reservation.
/// This is a cache recovery watermark, not an authorization grant.
pub async fn reserve_permission_invalidation_generation(
    db: &DatabaseTransaction,
) -> Result<u64, RbacInvalidationGenerationError> {
    let backend = db.get_database_backend();
    let storage_guard = integer_storage_guard(backend)?;
    if backend == DbBackend::Postgres {
        // NOWAIT on the row alone does not protect against a conflicting
        // table lock. Reserve the strongest table mode needed by the UPDATE
        // first, then fence the exact existing singleton without waiting.
        db.execute_unprepared("LOCK TABLE rbac_invalidation_state IN ROW EXCLUSIVE MODE NOWAIT")
            .await
            .map_err(RbacInvalidationGenerationError::Database)?;
        db.query_one_raw(Statement::from_string(
            backend,
            format!("SELECT generation FROM rbac_invalidation_state WHERE scope = '{RBAC_PERMISSION_INVALIDATION_SCOPE}' FOR UPDATE NOWAIT"),
        ))
        .await
        .map_err(RbacInvalidationGenerationError::Database)?
        .ok_or(RbacInvalidationGenerationError::MissingOrExhausted)?;
    }
    let row = db
        .query_one_raw(Statement::from_string(
            backend,
            format!(
                "UPDATE rbac_invalidation_state \
                 SET generation = generation + 1, updated_at = CURRENT_TIMESTAMP \
                 WHERE scope = '{RBAC_PERMISSION_INVALIDATION_SCOPE}' \
                   AND generation >= 0 AND generation < {MAX_DURABLE_GENERATION}{storage_guard} \
                 RETURNING generation"
            ),
        ))
        .await
        .map_err(RbacInvalidationGenerationError::Database)?
        .ok_or(RbacInvalidationGenerationError::MissingOrExhausted)?;
    decode_generation(&row)
}

pub async fn read_permission_invalidation_generation<C>(
    db: &C,
) -> Result<u64, RbacInvalidationGenerationError>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let storage_guard = integer_storage_guard(backend)?;
    let row = db
        .query_one_raw(Statement::from_string(
            backend,
            format!(
                "SELECT generation FROM rbac_invalidation_state \
                 WHERE scope = '{RBAC_PERMISSION_INVALIDATION_SCOPE}'{storage_guard}"
            ),
        ))
        .await
        .map_err(RbacInvalidationGenerationError::Database)?
        .ok_or(RbacInvalidationGenerationError::MissingOrExhausted)?;
    decode_generation(&row)
}

fn integer_storage_guard(backend: DbBackend) -> Result<&'static str, RbacInvalidationGenerationError> {
    match backend {
        DbBackend::Postgres => Ok(""),
        // SQLite's INTEGER affinity permits REAL values. Reject corrupt storage
        // before changing the row, even if a caller later commits an error.
        DbBackend::Sqlite => Ok(" AND typeof(generation) = 'integer'"),
        _ => Err(RbacInvalidationGenerationError::UnsupportedBackend),
    }
}

fn decode_generation(row: &sea_orm::QueryResult) -> Result<u64, RbacInvalidationGenerationError> {
    let generation: i64 = row.try_get("", "generation").map_err(RbacInvalidationGenerationError::Database)?;
    u64::try_from(generation).map_err(|_| RbacInvalidationGenerationError::Negative(generation))
}

#[cfg(test)]
mod tests {
    use super::{
        read_permission_invalidation_generation, reserve_permission_invalidation_generation,
    };
    use sea_orm::{ConnectionTrait, Database, TransactionTrait};

    #[tokio::test]
    async fn reservation_requires_the_durable_generation_schema() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let tx = db.begin().await.unwrap();
        assert!(
            reserve_permission_invalidation_generation(&tx)
                .await
                .is_err()
        );
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn reservation_does_not_heal_a_corrupt_negative_generation() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE rbac_invalidation_state (scope TEXT PRIMARY KEY, generation BIGINT NOT NULL, updated_at TEXT NOT NULL)").await.unwrap();
        db.execute_unprepared(
            "INSERT INTO rbac_invalidation_state VALUES ('permissions', -1, CURRENT_TIMESTAMP)",
        )
        .await
        .unwrap();
        let tx = db.begin().await.unwrap();
        assert!(
            reserve_permission_invalidation_generation(&tx)
                .await
                .is_err()
        );
        tx.commit().await.unwrap();
        assert!(read_permission_invalidation_generation(&db).await.is_err());
    }

    #[tokio::test]
    async fn invalid_storage_or_exhaustion_cannot_mutate_the_generation() {
        use super::RbacInvalidationGenerationError;
        use sea_orm::{DbBackend, Statement};
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE rbac_invalidation_state (scope TEXT PRIMARY KEY, generation BIGINT NOT NULL, updated_at TEXT NOT NULL);
             INSERT INTO rbac_invalidation_state VALUES ('permissions', 0.5, 'original')"
        ).await.unwrap();
        assert!(read_permission_invalidation_generation(&db).await.is_err());
        for (stored, expected_text) in [("0.5", "0.5"), ("9223372036854775807", "9223372036854775807")] {
            db.execute_unprepared(&format!("UPDATE rbac_invalidation_state SET generation = {stored}"))
                .await.unwrap();
            let tx = db.begin().await.unwrap();
            assert!(matches!(
                reserve_permission_invalidation_generation(&tx).await,
                Err(RbacInvalidationGenerationError::MissingOrExhausted)
            ));
            tx.commit().await.unwrap();
            let row = db.query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT CAST(generation AS TEXT) AS stored, updated_at FROM rbac_invalidation_state",
            )).await.unwrap().unwrap();
            assert_eq!(row.try_get::<String>("", "stored").unwrap(), expected_text);
            assert_eq!(row.try_get::<String>("", "updated_at").unwrap(), "original");
        }
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_RBAC_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_generation_reservations_reject_contention_and_rollback_reuses_uncommitted_successor() {
        use sea_orm::{DatabaseConnection, DatabaseTransaction, DbBackend, IsolationLevel};
        use std::time::Duration;

        async fn transaction(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
            let tx = db.begin_with_config(Some(IsolationLevel::ReadCommitted), None).await.unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
            tx
        }

        let db = Database::connect(std::env::var("RUSTOK_RBAC_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let schema = format!("rbac_generation_test_{}", uuid::Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = transaction(&db, &schema).await;
        setup.execute_unprepared(
            "CREATE TABLE rbac_invalidation_state (scope TEXT PRIMARY KEY, generation BIGINT NOT NULL, updated_at TIMESTAMPTZ NOT NULL);
             INSERT INTO rbac_invalidation_state VALUES ('permissions', 0, now())"
        ).await.unwrap();
        setup.commit().await.unwrap();

        for (commit_first, expected) in [(false, 1_u64), (true, 3_u64)] {
            let first = transaction(&db, &schema).await;
            assert_eq!(
                reserve_permission_invalidation_generation(&first).await.unwrap(),
                if commit_first { 2 } else { 1 }
            );
            let second = transaction(&db, &schema).await;
            let error = tokio::time::timeout(
                Duration::from_secs(3),
                reserve_permission_invalidation_generation(&second),
            )
            .await.expect("late generation fence must not wait").unwrap_err();
            let super::RbacInvalidationGenerationError::Database(error) = error else {
                panic!("expected typed database contention");
            };
            assert_eq!(
                rustok_core::resilience::retry::PostgresTransactionConflict::classify(DbBackend::Postgres, &error),
                Some(rustok_core::resilience::retry::PostgresTransactionConflict::LockUnavailable)
            );
            second.rollback().await.unwrap();
            if commit_first {
                first.commit().await.unwrap();
            } else {
                first.rollback().await.unwrap();
            }
            let retry = transaction(&db, &schema).await;
            assert_eq!(reserve_permission_invalidation_generation(&retry).await.unwrap(), expected);
            retry.commit().await.unwrap();
        }

        // A row-level NOWAIT alone would still wait on this table lock.
        let holder = transaction(&db, &schema).await;
        holder.execute_unprepared("LOCK TABLE rbac_invalidation_state IN SHARE MODE").await.unwrap();
        let writer = transaction(&db, &schema).await;
        let error = tokio::time::timeout(
            Duration::from_secs(3),
            reserve_permission_invalidation_generation(&writer),
        )
        .await.expect("late generation table fence must not wait").unwrap_err();
        let super::RbacInvalidationGenerationError::Database(error) = error else {
            panic!("expected typed database contention");
        };
        assert_eq!(
            rustok_core::resilience::retry::PostgresTransactionConflict::classify(DbBackend::Postgres, &error),
            Some(rustok_core::resilience::retry::PostgresTransactionConflict::LockUnavailable)
        );
        writer.rollback().await.unwrap();
        holder.rollback().await.unwrap();

        let verify = transaction(&db, &schema).await;
        assert_eq!(read_permission_invalidation_generation(&verify).await.unwrap(), 3);
        verify.rollback().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }

    #[tokio::test]
    async fn reservation_is_rolled_back_with_the_owner_transaction() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE rbac_invalidation_state (\
             scope TEXT PRIMARY KEY NOT NULL, \
             generation BIGINT NOT NULL, \
             updated_at TEXT NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO rbac_invalidation_state (scope, generation, updated_at) \
             VALUES ('permissions', 0, CURRENT_TIMESTAMP)",
        )
        .await
        .unwrap();

        let tx = db.begin().await.unwrap();
        assert_eq!(
            reserve_permission_invalidation_generation(&tx)
                .await
                .unwrap(),
            1
        );
        tx.rollback().await.unwrap();
        assert_eq!(
            read_permission_invalidation_generation(&db).await.unwrap(),
            0
        );
    }
}
