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
CREATE TABLE channel_index_tombstones (
    tenant_id UUID NOT NULL,
    channel_id UUID NOT NULL,
    source_version BIGINT NOT NULL,
    deleted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, channel_id),
    CONSTRAINT chk_channel_index_tombstones_source_version_positive CHECK (source_version > 0)
);

CREATE OR REPLACE FUNCTION rustok_channel_store_index_tombstone(
    target_tenant_id UUID,
    target_channel_id UUID,
    target_source_version BIGINT
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO channel_index_tombstones (
        tenant_id,
        channel_id,
        source_version,
        deleted_at
    ) VALUES (
        target_tenant_id,
        target_channel_id,
        target_source_version,
        CURRENT_TIMESTAMP
    )
    ON CONFLICT (tenant_id, channel_id) DO UPDATE
    SET source_version = GREATEST(
            channel_index_tombstones.source_version,
            EXCLUDED.source_version
        ),
        deleted_at = CURRENT_TIMESTAMP;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_channel_clear_superseded_index_tombstone(
    target_tenant_id UUID,
    target_channel_id UUID,
    live_source_version BIGINT
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM channel_index_tombstones tombstone
        WHERE tombstone.tenant_id = target_tenant_id
          AND tombstone.channel_id = target_channel_id
          AND tombstone.source_version >= live_source_version
    ) THEN
        RAISE EXCEPTION
            'channel live index revision does not supersede retained tombstone for channel %',
            target_channel_id;
    END IF;

    DELETE FROM channel_index_tombstones
    WHERE tenant_id = target_tenant_id
      AND channel_id = target_channel_id
      AND source_version < live_source_version;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_channel_seed_index_revision_from_tombstone()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    retained_source_version BIGINT;
BEGIN
    SELECT source_version
      INTO retained_source_version
      FROM channel_index_tombstones
     WHERE tenant_id = NEW.tenant_id
       AND channel_id = NEW.id;

    IF retained_source_version IS NOT NULL THEN
        IF retained_source_version = 9223372036854775807 THEN
            RAISE EXCEPTION 'channel index revision exhausted for reused channel %', NEW.id;
        END IF;
        NEW.index_revision := GREATEST(NEW.index_revision, retained_source_version + 1);
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_channels_seed_index_revision
BEFORE INSERT ON channels
FOR EACH ROW
EXECUTE FUNCTION rustok_channel_seed_index_revision_from_tombstone();

CREATE OR REPLACE FUNCTION rustok_channel_capture_index_tombstone()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.index_revision = 9223372036854775807 THEN
        RAISE EXCEPTION 'channel index revision exhausted for deleted channel %', OLD.id;
    END IF;

    PERFORM rustok_channel_store_index_tombstone(
        OLD.tenant_id,
        OLD.id,
        OLD.index_revision + 1
    );
    RETURN OLD;
END;
$$;

CREATE TRIGGER trg_channels_capture_index_tombstone
BEFORE DELETE ON channels
FOR EACH ROW
EXECUTE FUNCTION rustok_channel_capture_index_tombstone();

CREATE OR REPLACE FUNCTION rustok_channel_clear_inserted_index_tombstone()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM rustok_channel_clear_superseded_index_tombstone(
        NEW.tenant_id,
        NEW.id,
        NEW.index_revision
    );
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_channels_clear_index_tombstone
AFTER INSERT ON channels
FOR EACH ROW
EXECUTE FUNCTION rustok_channel_clear_inserted_index_tombstone();

CREATE OR REPLACE FUNCTION rustok_channel_move_index_tombstone()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
       AND OLD.id IS NOT DISTINCT FROM NEW.id THEN
        RETURN NEW;
    END IF;

    PERFORM rustok_channel_store_index_tombstone(
        OLD.tenant_id,
        OLD.id,
        NEW.index_revision
    );
    PERFORM rustok_channel_clear_superseded_index_tombstone(
        NEW.tenant_id,
        NEW.id,
        NEW.index_revision
    );
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_channels_move_index_tombstone
AFTER UPDATE OF id, tenant_id ON channels
FOR EACH ROW
EXECUTE FUNCTION rustok_channel_move_index_tombstone();
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
DROP TRIGGER IF EXISTS trg_channels_move_index_tombstone ON channels;
DROP TRIGGER IF EXISTS trg_channels_clear_index_tombstone ON channels;
DROP TRIGGER IF EXISTS trg_channels_capture_index_tombstone ON channels;
DROP TRIGGER IF EXISTS trg_channels_seed_index_revision ON channels;
DROP FUNCTION IF EXISTS rustok_channel_move_index_tombstone();
DROP FUNCTION IF EXISTS rustok_channel_clear_inserted_index_tombstone();
DROP FUNCTION IF EXISTS rustok_channel_capture_index_tombstone();
DROP FUNCTION IF EXISTS rustok_channel_seed_index_revision_from_tombstone();
DROP FUNCTION IF EXISTS rustok_channel_clear_superseded_index_tombstone(UUID, UUID, BIGINT);
DROP FUNCTION IF EXISTS rustok_channel_store_index_tombstone(UUID, UUID, BIGINT);
DROP TABLE IF EXISTS channel_index_tombstones;
"#,
            )
            .await?;

        Ok(())
    }
}
