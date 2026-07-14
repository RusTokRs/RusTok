use sea_orm::{ConnectionTrait, Statement};

use crate::error::{Error, Result};

const RBAC_PERMISSION_SCOPE: &str = "permissions";
const MAX_DURABLE_GENERATION: i64 = i64::MAX;

/// Reserve the next durable RBAC invalidation generation on the caller-owned
/// connection. When `db` is a transaction, the generation advances atomically
/// with the authorization mutation and is rolled back with it.
pub(crate) async fn reserve_rbac_invalidation_generation<C>(db: &C) -> Result<u64>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let update = db
        .execute(Statement::from_string(
            backend,
            format!(
                "UPDATE rbac_invalidation_state \
                 SET generation = generation + 1, updated_at = CURRENT_TIMESTAMP \
                 WHERE scope = '{RBAC_PERMISSION_SCOPE}' \
                   AND generation < {MAX_DURABLE_GENERATION}"
            ),
        ))
        .await?;

    if update.rows_affected() != 1 {
        return Err(Error::Cache(
            "durable RBAC invalidation generation is missing or exhausted".to_string(),
        ));
    }

    read_rbac_invalidation_generation(db).await
}

pub(crate) async fn read_rbac_invalidation_generation<C>(db: &C) -> Result<u64>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let row = db
        .query_one(Statement::from_string(
            backend,
            format!(
                "SELECT generation FROM rbac_invalidation_state \
                 WHERE scope = '{RBAC_PERMISSION_SCOPE}'"
            ),
        ))
        .await?
        .ok_or_else(|| {
            Error::Cache("durable RBAC invalidation generation row is missing".to_string())
        })?;
    let generation: i64 = row.try_get("", "generation")?;
    u64::try_from(generation).map_err(|_| {
        Error::Cache(format!(
            "durable RBAC invalidation generation is negative: {generation}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::{read_rbac_invalidation_generation, reserve_rbac_invalidation_generation};
    use rustok_migrations::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::TransactionTrait;

    #[tokio::test]
    async fn durable_generation_commits_and_rolls_back_with_the_owner_transaction() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 0);

        let rolled_back = db.begin().await.unwrap();
        assert_eq!(
            reserve_rbac_invalidation_generation(&rolled_back)
                .await
                .unwrap(),
            1
        );
        rolled_back.rollback().await.unwrap();
        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 0);

        let committed = db.begin().await.unwrap();
        assert_eq!(
            reserve_rbac_invalidation_generation(&committed)
                .await
                .unwrap(),
            1
        );
        committed.commit().await.unwrap();
        assert_eq!(read_rbac_invalidation_generation(&db).await.unwrap(), 1);
    }
}
