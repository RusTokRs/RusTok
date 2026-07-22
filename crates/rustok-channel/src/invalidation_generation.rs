use sea_orm::{ConnectionTrait, DbErr, Statement};

use crate::{ChannelError, ChannelResult};

pub const CHANNEL_RESOLUTION_INVALIDATION_SCOPE: &str = "resolution";
const READ_CHANNEL_RESOLUTION_GENERATION_SQL: &str =
    "SELECT generation FROM channel_resolution_invalidation_state WHERE scope = 'resolution'";

pub async fn read_resolution_invalidation_generation<C>(db: &C) -> ChannelResult<u64>
where
    C: ConnectionTrait,
{
    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            READ_CHANNEL_RESOLUTION_GENERATION_SQL.to_string(),
        ))
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
    use super::read_resolution_invalidation_generation;
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
