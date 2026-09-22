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
CREATE TABLE product_index_locale_refresh_ledger (
    sequence_no BIGSERIAL NOT NULL,
    refresh_id UUID NOT NULL,
    root_event_id UUID NOT NULL,
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    locale TEXT NOT NULL,
    source_version BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_product_index_locale_refresh_ledger PRIMARY KEY (refresh_id),
    CONSTRAINT uq_product_index_locale_refresh_root_target
        UNIQUE (root_event_id, product_id, locale),
    CONSTRAINT uq_product_index_locale_refresh_tenant_sequence
        UNIQUE (tenant_id, sequence_no),
    CONSTRAINT chk_product_index_locale_refresh_sequence_positive
        CHECK (sequence_no > 0),
    CONSTRAINT chk_product_index_locale_refresh_refresh_id_non_nil
        CHECK (refresh_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_index_locale_refresh_root_event_id_non_nil
        CHECK (root_event_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_index_locale_refresh_tenant_id_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_index_locale_refresh_product_id_non_nil
        CHECK (product_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_index_locale_refresh_locale_nonempty
        CHECK (btrim(locale) <> ''),
    CONSTRAINT chk_product_index_locale_refresh_locale_bounded
        CHECK (octet_length(locale) <= 128),
    CONSTRAINT chk_product_index_locale_refresh_source_version_positive
        CHECK (source_version > 0)
);

CREATE INDEX idx_product_index_locale_refresh_target
    ON product_index_locale_refresh_ledger (
        tenant_id,
        product_id,
        locale,
        source_version DESC,
        sequence_no DESC
    );

CREATE OR REPLACE FUNCTION rustok_product_reject_index_locale_refresh_ledger_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'product Index locale refresh ledger is append-only';
    RETURN NULL;
END;
$$;

CREATE TRIGGER trg_product_index_locale_refresh_ledger_update
BEFORE UPDATE ON product_index_locale_refresh_ledger
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_index_locale_refresh_ledger_mutation();

CREATE TRIGGER trg_product_index_locale_refresh_ledger_delete
BEFORE DELETE ON product_index_locale_refresh_ledger
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_index_locale_refresh_ledger_mutation();
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
DROP TRIGGER IF EXISTS trg_product_index_locale_refresh_ledger_delete
    ON product_index_locale_refresh_ledger;
DROP TRIGGER IF EXISTS trg_product_index_locale_refresh_ledger_update
    ON product_index_locale_refresh_ledger;
DROP TABLE IF EXISTS product_index_locale_refresh_ledger;
DROP FUNCTION IF EXISTS rustok_product_reject_index_locale_refresh_ledger_mutation();
"#,
            )
            .await?;

        Ok(())
    }
}
