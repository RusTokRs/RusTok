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
CREATE TABLE product_index_refresh_relay_cursors (
    tenant_id UUID NOT NULL,
    stream_kind TEXT NOT NULL,
    last_sequence_no BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_product_index_refresh_relay_cursors
        PRIMARY KEY (tenant_id, stream_kind),
    CONSTRAINT chk_product_index_refresh_relay_cursor_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_index_refresh_relay_cursor_stream_kind
        CHECK (stream_kind IN ('locale', 'variant')),
    CONSTRAINT chk_product_index_refresh_relay_cursor_non_negative
        CHECK (last_sequence_no >= 0)
);

CREATE OR REPLACE FUNCTION rustok_product_guard_index_refresh_relay_cursor()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.tenant_id <> OLD.tenant_id OR NEW.stream_kind <> OLD.stream_kind THEN
        RAISE EXCEPTION 'product Index refresh relay cursor identity is immutable';
    END IF;
    IF NEW.last_sequence_no < OLD.last_sequence_no THEN
        RAISE EXCEPTION 'product Index refresh relay cursor cannot move backwards';
    END IF;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_product_reject_index_refresh_relay_cursor_delete()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'product Index refresh relay cursor cannot be deleted';
    RETURN NULL;
END;
$$;

CREATE TRIGGER trg_product_index_refresh_relay_cursor_update
BEFORE UPDATE ON product_index_refresh_relay_cursors
FOR EACH ROW
EXECUTE FUNCTION rustok_product_guard_index_refresh_relay_cursor();

CREATE TRIGGER trg_product_index_refresh_relay_cursor_delete
BEFORE DELETE ON product_index_refresh_relay_cursors
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_index_refresh_relay_cursor_delete();
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
DROP TRIGGER IF EXISTS trg_product_index_refresh_relay_cursor_delete
    ON product_index_refresh_relay_cursors;
DROP TRIGGER IF EXISTS trg_product_index_refresh_relay_cursor_update
    ON product_index_refresh_relay_cursors;
DROP TABLE IF EXISTS product_index_refresh_relay_cursors;
DROP FUNCTION IF EXISTS rustok_product_reject_index_refresh_relay_cursor_delete();
DROP FUNCTION IF EXISTS rustok_product_guard_index_refresh_relay_cursor();
"#,
            )
            .await?;

        Ok(())
    }
}
