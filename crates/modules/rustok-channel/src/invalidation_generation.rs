use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, DbErr, Statement};

use crate::{ChannelError, ChannelResult};

pub const CHANNEL_RESOLUTION_INVALIDATION_SCOPE: &str = "resolution";
const READ_CHANNEL_RESOLUTION_GENERATION_SQL: &str =
    "SELECT generation FROM channel_resolution_invalidation_state WHERE scope = 'resolution'";

pub async fn read_resolution_invalidation_generation<C>(db: &C) -> ChannelResult<u64>
where
    C: ConnectionTrait,
{
    read_generation(db, READ_CHANNEL_RESOLUTION_GENERATION_SQL.to_string()).await
}

/// Keep the owner generation stable until the protected SQL effect commits.
/// All channel resolution writers update this row in their own transaction.
/// SQLite reserves its single writer before reading the revision.
pub async fn require_resolution_invalidation_generation_on(
    tx: &DatabaseTransaction,
    expected: u64,
) -> ChannelResult<()> {
    let sql = match tx.get_database_backend() {
        DbBackend::Postgres => format!("{READ_CHANNEL_RESOLUTION_GENERATION_SQL} FOR SHARE"),
        DbBackend::Sqlite => {
            let result = tx.execute_unprepared(
                "UPDATE channel_resolution_invalidation_state SET generation = generation WHERE scope = 'resolution'",
            ).await?;
            if result.rows_affected() != 1 {
                return Err(ChannelError::Database(DbErr::Custom(
                    "channel resolution invalidation state is missing".into(),
                )));
            }
            READ_CHANNEL_RESOLUTION_GENERATION_SQL.to_string()
        }
        backend => {
            return Err(ChannelError::Database(DbErr::Custom(format!(
                "unsupported channel authority backend: {backend:?}"
            ))));
        }
    };
    if read_generation(tx, sql).await? != expected {
        return Err(ChannelError::Conflict(
            "channel resolution authority changed".into(),
        ));
    }
    Ok(())
}

async fn read_generation(db: &impl ConnectionTrait, sql: String) -> ChannelResult<u64> {
    let row = db
        .query_one_raw(Statement::from_string(db.get_database_backend(), sql))
        .await?
        .ok_or_else(|| {
            ChannelError::Database(DbErr::Custom(
                "channel resolution invalidation state is missing".to_string(),
            ))
        })?;
    let generation = row.try_get::<i64>("", "generation")?;
    u64::try_from(generation).map_err(|_| {
        ChannelError::Database(DbErr::Custom(format!(
            "channel resolution invalidation generation is negative: {generation}"
        )))
    })
}

#[cfg(test)]
mod tests {
    use super::{
        read_resolution_invalidation_generation, require_resolution_invalidation_generation_on,
    };
    use sea_orm::{ConnectionTrait, Database, TransactionTrait};

    async fn install_generation_state(db: &sea_orm::DatabaseConnection, generation: i64) {
        db.execute_unprepared(
            "CREATE TABLE channel_resolution_invalidation_state (scope TEXT PRIMARY KEY, generation BIGINT NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared(&format!(
            "INSERT INTO channel_resolution_invalidation_state (scope, generation) VALUES ('resolution', {generation})"
        ))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn transaction_fence_rejects_stale_generation_without_advancing_it() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        install_generation_state(&db, 4).await;
        let tx = db.begin().await.unwrap();
        require_resolution_invalidation_generation_on(&tx, 4)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(
            read_resolution_invalidation_generation(&db).await.unwrap(),
            4
        );

        db.execute_unprepared("UPDATE channel_resolution_invalidation_state SET generation = 6")
            .await
            .unwrap();
        let tx = db.begin().await.unwrap();
        assert!(
            require_resolution_invalidation_generation_on(&tx, 4)
                .await
                .is_err()
        );
        tx.rollback().await.unwrap();
        let tx = db.begin().await.unwrap();
        require_resolution_invalidation_generation_on(&tx, 6)
            .await
            .unwrap();
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn rejects_negative_generation() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        install_generation_state(&db, -1).await;

        let error = read_resolution_invalidation_generation(&db)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("generation is negative"));
    }

    #[tokio::test]
    async fn durable_generation_converges_across_replica_readers_without_pubsub() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        install_generation_state(&db, 0).await;
        let replica_a = db.clone();
        let replica_b = db.clone();

        assert_eq!(
            read_resolution_invalidation_generation(&replica_a)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            read_resolution_invalidation_generation(&replica_b)
                .await
                .unwrap(),
            0
        );

        let committed = db.begin().await.unwrap();
        committed
            .execute_unprepared(
                "UPDATE channel_resolution_invalidation_state SET generation = generation + 1 WHERE scope = 'resolution'",
            )
            .await
            .unwrap();
        committed.commit().await.unwrap();

        assert_eq!(
            read_resolution_invalidation_generation(&replica_a)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            read_resolution_invalidation_generation(&replica_b)
                .await
                .unwrap(),
            1
        );

        let rolled_back = db.begin().await.unwrap();
        rolled_back
            .execute_unprepared(
                "UPDATE channel_resolution_invalidation_state SET generation = generation + 1 WHERE scope = 'resolution'",
            )
            .await
            .unwrap();
        rolled_back.rollback().await.unwrap();

        assert_eq!(
            read_resolution_invalidation_generation(&replica_a)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            read_resolution_invalidation_generation(&replica_b)
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn missing_generation_state_fails_closed_and_recovers_after_restore() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        install_generation_state(&db, 4).await;
        let replica_a = db.clone();
        let replica_b = db.clone();

        db.execute_unprepared("DROP TABLE channel_resolution_invalidation_state")
            .await
            .unwrap();
        assert!(
            read_resolution_invalidation_generation(&replica_a)
                .await
                .is_err()
        );
        assert!(
            read_resolution_invalidation_generation(&replica_b)
                .await
                .is_err()
        );

        install_generation_state(&db, 7).await;
        assert_eq!(
            read_resolution_invalidation_generation(&replica_a)
                .await
                .unwrap(),
            7
        );
        assert_eq!(
            read_resolution_invalidation_generation(&replica_b)
                .await
                .unwrap(),
            7
        );
    }
}
