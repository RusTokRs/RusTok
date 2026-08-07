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
CREATE TABLE channel_index_identity_generations (
    tenant_id UUID PRIMARY KEY,
    generation BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT chk_channel_index_identity_generation_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_channel_index_identity_generation_positive
        CHECK (generation > 0)
);

CREATE OR REPLACE FUNCTION rustok_channel_bump_index_identity_generation(target_tenant_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    previous_generation BIGINT;
    lock_key TEXT;
BEGIN
    IF target_tenant_id IS NULL
       OR target_tenant_id = '00000000-0000-0000-0000-000000000000'::uuid
    THEN
        RAISE EXCEPTION 'Channel Index identity generation tenant is invalid';
    END IF;

    lock_key := target_tenant_id::text || E'\x1fchannel-index-identity-generation';
    PERFORM pg_advisory_xact_lock(hashtextextended(lock_key, 0));

    SELECT generation
      INTO previous_generation
      FROM channel_index_identity_generations
     WHERE tenant_id = target_tenant_id
     FOR UPDATE;

    IF previous_generation IS NULL THEN
        INSERT INTO channel_index_identity_generations (tenant_id, generation, updated_at)
        VALUES (target_tenant_id, 1, CURRENT_TIMESTAMP);
        RETURN;
    END IF;

    IF previous_generation = 9223372036854775807 THEN
        RAISE EXCEPTION 'Channel Index identity generation exhausted for tenant %', target_tenant_id;
    END IF;

    UPDATE channel_index_identity_generations
       SET generation = previous_generation + 1,
           updated_at = CURRENT_TIMESTAMP
     WHERE tenant_id = target_tenant_id;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_channel_track_index_identity_generation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    old_tenant UUID;
    new_tenant UUID;
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM rustok_channel_bump_index_identity_generation(NEW.tenant_id);
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        PERFORM rustok_channel_bump_index_identity_generation(OLD.tenant_id);
        RETURN OLD;
    END IF;

    IF OLD.id IS NOT DISTINCT FROM NEW.id
       AND OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
       AND lower(btrim(OLD.slug)) IS NOT DISTINCT FROM lower(btrim(NEW.slug))
    THEN
        RETURN NEW;
    END IF;

    IF OLD.tenant_id = NEW.tenant_id THEN
        PERFORM rustok_channel_bump_index_identity_generation(NEW.tenant_id);
        RETURN NEW;
    END IF;

    old_tenant := OLD.tenant_id;
    new_tenant := NEW.tenant_id;
    IF old_tenant::text < new_tenant::text THEN
        PERFORM rustok_channel_bump_index_identity_generation(old_tenant);
        PERFORM rustok_channel_bump_index_identity_generation(new_tenant);
    ELSE
        PERFORM rustok_channel_bump_index_identity_generation(new_tenant);
        PERFORM rustok_channel_bump_index_identity_generation(old_tenant);
    END IF;

    RETURN NEW;
END;
$$;

-- Install the trigger before seeding the baseline. Both trigger writes and the seed call the exact
-- same advisory-locked bump function, so no first-row race can lose an identity mutation. A tenant
-- touched concurrently may receive an extra initial generation; generations are monotonic freshness
-- fences, not business counters, so preserving every ordering fence is the stronger invariant.
CREATE TRIGGER trg_channels_track_index_identity_generation
AFTER INSERT OR DELETE OR UPDATE OF id, tenant_id, slug ON channels
FOR EACH ROW
EXECUTE FUNCTION rustok_channel_track_index_identity_generation();

SELECT rustok_channel_bump_index_identity_generation(seed.tenant_id)
FROM (
    SELECT DISTINCT tenant_id
    FROM channels
) seed
ORDER BY seed.tenant_id;
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
DROP TRIGGER IF EXISTS trg_channels_track_index_identity_generation ON channels;
DROP FUNCTION IF EXISTS rustok_channel_track_index_identity_generation();
DROP FUNCTION IF EXISTS rustok_channel_bump_index_identity_generation(UUID);
DROP TABLE IF EXISTS channel_index_identity_generations;
"#,
            )
            .await?;
        Ok(())
    }
}
