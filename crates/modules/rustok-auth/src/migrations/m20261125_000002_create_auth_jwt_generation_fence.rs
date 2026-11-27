use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, Statement};

/// One durable authority per Auth database. Never reseed an existing fence.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE TABLE auth_jwt_generation_fence (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                generation BIGINT NOT NULL CHECK (generation >= 0),
                payload TEXT NOT NULL,
                tag TEXT NOT NULL,
                CHECK ((generation = 0 AND payload = '' AND tag = '')
                    OR (generation > 0 AND payload <> '' AND length(tag) = 64))
            )",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO auth_jwt_generation_fence (id, generation, payload, tag)
             VALUES (1, 0, '', '')",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let row = db
            .query_one_raw(Statement::from_string(
                db.get_database_backend(),
                "SELECT generation FROM auth_jwt_generation_fence WHERE id = 1".to_string(),
            ))
            .await?
            .ok_or_else(|| DbErr::Custom("JWT generation fence is missing".into()))?;
        let generation: i64 = row.try_get("", "generation")?;
        if generation != 0 {
            return Err(DbErr::Custom(
                "Cannot remove an enrolled JWT generation fence".into(),
            ));
        }
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("auth_jwt_generation_fence"))
                    .to_owned(),
            )
            .await
    }
}
