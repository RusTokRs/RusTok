use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "ALTER TABLE auth_jwt_generation_fence ADD COLUMN replica_grants TEXT NOT NULL DEFAULT '{}'",
        ).await?;
        db.execute_unprepared(
            "ALTER TABLE auth_jwt_generation_fence ADD COLUMN untracked_until_ms BIGINT NOT NULL DEFAULT 0 CHECK (untracked_until_ms >= 0)",
        ).await?;
        db.execute_unprepared(
            "UPDATE auth_jwt_generation_fence SET untracked_until_ms = lease_until_ms WHERE id = 1",
        ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let row = db.query_one_raw(Statement::from_string(
            db.get_database_backend(),
            "SELECT generation, lease_until_ms, drain_until_ms, untracked_until_ms, replica_grants FROM auth_jwt_generation_fence WHERE id = 1".to_string(),
        )).await?.ok_or_else(|| DbErr::Custom("JWT generation fence is missing".into()))?;
        let generation: i64 = row.try_get("", "generation")?;
        let lease: i64 = row.try_get("", "lease_until_ms")?;
        let drain: i64 = row.try_get("", "drain_until_ms")?;
        let untracked: i64 = row.try_get("", "untracked_until_ms")?;
        let grants: String = row.try_get("", "replica_grants")?;
        if generation != 0 || lease != 0 || drain != 0 || untracked != 0 || grants != "{}" {
            return Err(DbErr::Custom("Cannot remove an exercised JWT replica fence".into()));
        }
        db.execute_unprepared("ALTER TABLE auth_jwt_generation_fence DROP COLUMN untracked_until_ms").await?;
        db.execute_unprepared("ALTER TABLE auth_jwt_generation_fence DROP COLUMN replica_grants").await?;
        Ok(())
    }
}
