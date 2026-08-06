use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product migrations require PostgreSQL".to_owned(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE product_variant_index_tombstones
    ADD COLUMN product_id UUID NULL,
    ADD CONSTRAINT chk_product_variant_index_tombstones_product_id_non_nil
        CHECK (product_id IS NULL OR product_id <> '00000000-0000-0000-0000-000000000000'::uuid);

CREATE INDEX idx_product_variant_index_tombstones_parent
    ON product_variant_index_tombstones (
        tenant_id,
        product_id,
        source_version DESC,
        variant_id
    )
    WHERE product_id IS NOT NULL;

CREATE TABLE product_variant_index_refresh_ledger (
    sequence_no BIGSERIAL NOT NULL,
    refresh_id UUID NOT NULL,
    root_event_id UUID NOT NULL,
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    variant_id UUID NOT NULL,
    source_version BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_product_variant_index_refresh_ledger PRIMARY KEY (refresh_id),
    CONSTRAINT uq_product_variant_index_refresh_root_target
        UNIQUE (root_event_id, variant_id),
    CONSTRAINT uq_product_variant_index_refresh_tenant_sequence
        UNIQUE (tenant_id, sequence_no),
    CONSTRAINT chk_product_variant_index_refresh_sequence_positive
        CHECK (sequence_no > 0),
    CONSTRAINT chk_product_variant_index_refresh_refresh_id_non_nil
        CHECK (refresh_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_variant_index_refresh_root_event_id_non_nil
        CHECK (root_event_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_variant_index_refresh_tenant_id_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_variant_index_refresh_product_id_non_nil
        CHECK (product_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_variant_index_refresh_variant_id_non_nil
        CHECK (variant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_variant_index_refresh_source_version_positive
        CHECK (source_version > 0)
);

CREATE INDEX idx_product_variant_index_refresh_target
    ON product_variant_index_refresh_ledger (
        tenant_id,
        product_id,
        variant_id,
        source_version DESC,
        sequence_no DESC
    );

CREATE OR REPLACE FUNCTION rustok_product_reject_variant_index_refresh_ledger_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'product variant Index refresh ledger is append-only';
    RETURN NULL;
END;
$$;

CREATE TRIGGER trg_product_variant_index_refresh_ledger_update
BEFORE UPDATE ON product_variant_index_refresh_ledger
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_variant_index_refresh_ledger_mutation();

CREATE TRIGGER trg_product_variant_index_refresh_ledger_delete
BEFORE DELETE ON product_variant_index_refresh_ledger
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_variant_index_refresh_ledger_mutation();

CREATE OR REPLACE FUNCTION rustok_product_variant_store_index_tombstone(
    target_tenant_id UUID,
    target_product_id UUID,
    target_variant_id UUID,
    target_source_version BIGINT
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    IF target_tenant_id IS NULL
       OR target_tenant_id = '00000000-0000-0000-0000-000000000000'::uuid
       OR target_product_id IS NULL
       OR target_product_id = '00000000-0000-0000-0000-000000000000'::uuid
       OR target_variant_id IS NULL
       OR target_variant_id = '00000000-0000-0000-0000-000000000000'::uuid
       OR target_source_version <= 0 THEN
        RAISE EXCEPTION 'product variant tombstone identity and source version must be valid';
    END IF;

    INSERT INTO product_variant_index_tombstones (
        tenant_id,
        product_id,
        variant_id,
        source_version,
        deleted_at
    ) VALUES (
        target_tenant_id,
        target_product_id,
        target_variant_id,
        target_source_version,
        CURRENT_TIMESTAMP
    )
    ON CONFLICT (tenant_id, variant_id) DO UPDATE
    SET product_id = CASE
            WHEN EXCLUDED.source_version >= product_variant_index_tombstones.source_version
                THEN EXCLUDED.product_id
            ELSE product_variant_index_tombstones.product_id
        END,
        source_version = GREATEST(
            product_variant_index_tombstones.source_version,
            EXCLUDED.source_version
        ),
        deleted_at = CURRENT_TIMESTAMP;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_product_variant_capture_index_tombstone()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.index_revision = 9223372036854775807 THEN
        RAISE EXCEPTION 'product variant index revision exhausted for deleted variant %', OLD.id;
    END IF;

    PERFORM rustok_product_variant_store_index_tombstone(
        OLD.tenant_id,
        OLD.product_id,
        OLD.id,
        OLD.index_revision + 1
    );
    RETURN OLD;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_product_variant_move_index_tombstone()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
       AND OLD.id IS NOT DISTINCT FROM NEW.id THEN
        RETURN NEW;
    END IF;

    PERFORM rustok_product_variant_store_index_tombstone(
        OLD.tenant_id,
        OLD.product_id,
        OLD.id,
        NEW.index_revision
    );
    PERFORM rustok_product_variant_clear_superseded_index_tombstone(
        NEW.tenant_id,
        NEW.id,
        NEW.index_revision
    );
    RETURN NEW;
END;
$$;

DROP FUNCTION rustok_product_variant_store_index_tombstone(UUID, UUID, BIGINT);
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product migrations require PostgreSQL".to_owned(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE OR REPLACE FUNCTION rustok_product_variant_store_index_tombstone(
    target_tenant_id UUID,
    target_variant_id UUID,
    target_source_version BIGINT
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO product_variant_index_tombstones (
        tenant_id,
        variant_id,
        source_version,
        deleted_at
    ) VALUES (
        target_tenant_id,
        target_variant_id,
        target_source_version,
        CURRENT_TIMESTAMP
    )
    ON CONFLICT (tenant_id, variant_id) DO UPDATE
    SET source_version = GREATEST(
            product_variant_index_tombstones.source_version,
            EXCLUDED.source_version
        ),
        deleted_at = CURRENT_TIMESTAMP;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_product_variant_capture_index_tombstone()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.index_revision = 9223372036854775807 THEN
        RAISE EXCEPTION 'product variant index revision exhausted for deleted variant %', OLD.id;
    END IF;

    PERFORM rustok_product_variant_store_index_tombstone(
        OLD.tenant_id,
        OLD.id,
        OLD.index_revision + 1
    );
    RETURN OLD;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_product_variant_move_index_tombstone()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
       AND OLD.id IS NOT DISTINCT FROM NEW.id THEN
        RETURN NEW;
    END IF;

    PERFORM rustok_product_variant_store_index_tombstone(
        OLD.tenant_id,
        OLD.id,
        NEW.index_revision
    );
    PERFORM rustok_product_variant_clear_superseded_index_tombstone(
        NEW.tenant_id,
        NEW.id,
        NEW.index_revision
    );
    RETURN NEW;
END;
$$;

DROP FUNCTION rustok_product_variant_store_index_tombstone(UUID, UUID, UUID, BIGINT);

DROP TRIGGER IF EXISTS trg_product_variant_index_refresh_ledger_delete
    ON product_variant_index_refresh_ledger;
DROP TRIGGER IF EXISTS trg_product_variant_index_refresh_ledger_update
    ON product_variant_index_refresh_ledger;
DROP TABLE IF EXISTS product_variant_index_refresh_ledger;
DROP FUNCTION IF EXISTS rustok_product_reject_variant_index_refresh_ledger_mutation();

DROP INDEX IF EXISTS idx_product_variant_index_tombstones_parent;
ALTER TABLE product_variant_index_tombstones
    DROP CONSTRAINT IF EXISTS chk_product_variant_index_tombstones_product_id_non_nil,
    DROP COLUMN IF EXISTS product_id;
"#,
            )
            .await?;

        Ok(())
    }
}
