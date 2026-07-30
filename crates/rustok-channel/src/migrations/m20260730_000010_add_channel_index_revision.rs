use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-channel migrations require PostgreSQL".to_owned(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE channels
    ADD COLUMN index_revision BIGINT NOT NULL DEFAULT 1,
    ADD CONSTRAINT chk_channels_index_revision_positive CHECK (index_revision > 0);

CREATE OR REPLACE FUNCTION rustok_channel_bump_index_revision()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.index_revision = 9223372036854775807 THEN
        RAISE EXCEPTION 'channel index revision exhausted for channel %', OLD.id;
    END IF;
    NEW.index_revision := OLD.index_revision + 1;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_channels_bump_index_revision
BEFORE UPDATE ON channels
FOR EACH ROW
EXECUTE FUNCTION rustok_channel_bump_index_revision();
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-channel migrations require PostgreSQL".to_owned(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP TRIGGER IF EXISTS trg_channels_bump_index_revision ON channels;
DROP FUNCTION IF EXISTS rustok_channel_bump_index_revision();
ALTER TABLE channels
    DROP CONSTRAINT IF EXISTS chk_channels_index_revision_positive,
    DROP COLUMN IF EXISTS index_revision;
"#,
            )
            .await?;
        Ok(())
    }
}
