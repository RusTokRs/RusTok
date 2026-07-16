use sea_orm::{ConnectionTrait, DbErr, Statement};

use crate::{ChannelError, ChannelResult};

pub const CHANNEL_RESOLUTION_INVALIDATION_SCOPE: &str = "resolution";

pub async fn read_resolution_invalidation_generation<C>(db: &C) -> ChannelResult<u64>
where
    C: ConnectionTrait,
{
    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            format!(
                "SELECT generation FROM channel_resolution_invalidation_state WHERE scope = '{}'",
                CHANNEL_RESOLUTION_INVALIDATION_SCOPE
            ),
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
    use sea_orm::{ConnectionTrait, Database};

    #[tokio::test]
    async fn rejects_negative_generation() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE channel_resolution_invalidation_state (scope TEXT PRIMARY KEY, generation BIGINT NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO channel_resolution_invalidation_state (scope, generation) VALUES ('resolution', -1)",
        )
        .await
        .unwrap();

        let error = read_resolution_invalidation_generation(&db)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("generation is negative"));
    }
}
