use sea_orm::{ConnectionTrait, DatabaseTransaction, Statement};
use thiserror::Error;

pub const RBAC_PERMISSION_INVALIDATION_SCOPE: &str = "permissions";
const MAX_DURABLE_GENERATION: i64 = i64::MAX;

#[derive(Debug, Error)]
pub enum RbacInvalidationGenerationError {
    #[error("RBAC invalidation generation database error: {0}")]
    Database(String),
    #[error("durable RBAC invalidation generation is missing or exhausted")]
    MissingOrExhausted,
    #[error("durable RBAC invalidation generation is negative: {0}")]
    Negative(i64),
}

/// Reserve the next durable permission invalidation generation inside the
/// caller-owned authorization transaction.
pub async fn reserve_permission_invalidation_generation(
    db: &DatabaseTransaction,
) -> Result<u64, RbacInvalidationGenerationError> {
    let backend = db.get_database_backend();
    let update = db
        .execute(Statement::from_string(
            backend,
            format!(
                "UPDATE rbac_invalidation_state \
                 SET generation = generation + 1, updated_at = CURRENT_TIMESTAMP \
                 WHERE scope = '{RBAC_PERMISSION_INVALIDATION_SCOPE}' \
                   AND generation < {MAX_DURABLE_GENERATION}"
            ),
        ))
        .await
        .map_err(database_error)?;

    if update.rows_affected() != 1 {
        return Err(RbacInvalidationGenerationError::MissingOrExhausted);
    }

    read_permission_invalidation_generation(db).await
}

pub async fn read_permission_invalidation_generation<C>(
    db: &C,
) -> Result<u64, RbacInvalidationGenerationError>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let row = db
        .query_one(Statement::from_string(
            backend,
            format!(
                "SELECT generation FROM rbac_invalidation_state \
                 WHERE scope = '{RBAC_PERMISSION_INVALIDATION_SCOPE}'"
            ),
        ))
        .await
        .map_err(database_error)?
        .ok_or(RbacInvalidationGenerationError::MissingOrExhausted)?;
    let generation: i64 = row.try_get("", "generation").map_err(database_error)?;
    u64::try_from(generation).map_err(|_| RbacInvalidationGenerationError::Negative(generation))
}

fn database_error(error: impl std::fmt::Display) -> RbacInvalidationGenerationError {
    RbacInvalidationGenerationError::Database(error.to_string())
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
