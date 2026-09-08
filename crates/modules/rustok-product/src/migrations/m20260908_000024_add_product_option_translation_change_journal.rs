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
CREATE TABLE product_option_translation_tombstones (
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    option_id UUID NOT NULL,
    deleted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, option_id),
    CONSTRAINT chk_product_option_translation_tombstone_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_option_translation_tombstone_product_non_nil
        CHECK (product_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_option_translation_tombstone_option_non_nil
        CHECK (option_id <> '00000000-0000-0000-0000-000000000000'::uuid)
);

CREATE INDEX idx_product_option_translation_tombstone_product
    ON product_option_translation_tombstones (tenant_id, product_id, option_id);

CREATE TABLE product_option_translation_change_journal (
    change_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    root_event_id UUID NOT NULL,
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    option_id UUID NOT NULL,
    resource_revision VARCHAR(96) NOT NULL,
    lifecycle VARCHAR(16) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_product_option_translation_change_root_target
        UNIQUE (root_event_id, option_id),
    CONSTRAINT chk_product_option_translation_change_seq_positive
        CHECK (change_seq > 0),
    CONSTRAINT chk_product_option_translation_change_root_event_non_nil
        CHECK (root_event_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_option_translation_change_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_option_translation_change_product_non_nil
        CHECK (product_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_option_translation_change_option_non_nil
        CHECK (option_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_option_translation_change_revision_nonblank
        CHECK (length(trim(resource_revision)) > 0),
    CONSTRAINT chk_product_option_translation_change_lifecycle
        CHECK (lifecycle IN ('active', 'archived', 'deleted'))
);

CREATE INDEX idx_product_option_translation_change_tenant_seq
    ON product_option_translation_change_journal (tenant_id, change_seq);

CREATE INDEX idx_product_option_translation_change_product_target_seq
    ON product_option_translation_change_journal (
        tenant_id,
        product_id,
        option_id,
        change_seq DESC
    );

CREATE INDEX idx_product_option_translation_change_target_seq
    ON product_option_translation_change_journal (
        tenant_id,
        option_id,
        change_seq DESC
    );

CREATE OR REPLACE FUNCTION rustok_product_option_store_translation_tombstone(
    target_tenant_id UUID,
    target_product_id UUID,
    target_option_id UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO product_option_translation_tombstones (
        tenant_id,
        product_id,
        option_id,
        deleted_at
    ) VALUES (
        target_tenant_id,
        target_product_id,
        target_option_id,
        CURRENT_TIMESTAMP
    )
    ON CONFLICT (tenant_id, option_id) DO UPDATE
    SET product_id = EXCLUDED.product_id,
        deleted_at = CURRENT_TIMESTAMP;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_product_option_capture_translation_tombstone()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    target_tenant_id UUID;
BEGIN
    SELECT tenant_id
      INTO target_tenant_id
      FROM products
     WHERE id = OLD.product_id;

    IF target_tenant_id IS NULL THEN
        RAISE EXCEPTION
            'product option translation tombstone parent is missing for option % product %',
            OLD.id,
            OLD.product_id;
    END IF;

    PERFORM rustok_product_option_store_translation_tombstone(
        target_tenant_id,
        OLD.product_id,
        OLD.id
    );
    RETURN OLD;
END;
$$;

CREATE TRIGGER trg_product_options_capture_translation_tombstone
BEFORE DELETE ON product_options
FOR EACH ROW
EXECUTE FUNCTION rustok_product_option_capture_translation_tombstone();

CREATE OR REPLACE FUNCTION rustok_product_capture_option_translation_tombstones()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO product_option_translation_tombstones (
        tenant_id,
        product_id,
        option_id,
        deleted_at
    )
    SELECT
        OLD.tenant_id,
        OLD.id,
        option_row.id,
        CURRENT_TIMESTAMP
    FROM product_options option_row
    WHERE option_row.product_id = OLD.id
    ON CONFLICT (tenant_id, option_id) DO UPDATE
    SET product_id = EXCLUDED.product_id,
        deleted_at = CURRENT_TIMESTAMP;

    RETURN OLD;
END;
$$;

CREATE TRIGGER trg_products_capture_option_translation_tombstones
BEFORE DELETE ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_capture_option_translation_tombstones();

CREATE OR REPLACE FUNCTION rustok_product_option_clear_translation_tombstone()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    target_tenant_id UUID;
BEGIN
    SELECT tenant_id
      INTO target_tenant_id
      FROM products
     WHERE id = NEW.product_id;

    IF target_tenant_id IS NULL THEN
        RAISE EXCEPTION
            'product option translation tombstone parent is missing for inserted option % product %',
            NEW.id,
            NEW.product_id;
    END IF;

    DELETE FROM product_option_translation_tombstones
    WHERE tenant_id = target_tenant_id
      AND option_id = NEW.id;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_product_options_clear_translation_tombstone
AFTER INSERT ON product_options
FOR EACH ROW
EXECUTE FUNCTION rustok_product_option_clear_translation_tombstone();
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
DROP TRIGGER IF EXISTS trg_product_options_clear_translation_tombstone ON product_options;
DROP TRIGGER IF EXISTS trg_products_capture_option_translation_tombstones ON products;
DROP TRIGGER IF EXISTS trg_product_options_capture_translation_tombstone ON product_options;
DROP FUNCTION IF EXISTS rustok_product_option_clear_translation_tombstone();
DROP FUNCTION IF EXISTS rustok_product_capture_option_translation_tombstones();
DROP FUNCTION IF EXISTS rustok_product_option_capture_translation_tombstone();
DROP FUNCTION IF EXISTS rustok_product_option_store_translation_tombstone(UUID, UUID, UUID);
DROP TABLE IF EXISTS product_option_translation_change_journal;
DROP TABLE IF EXISTS product_option_translation_tombstones;
"#,
            )
            .await?;
        Ok(())
    }
}
