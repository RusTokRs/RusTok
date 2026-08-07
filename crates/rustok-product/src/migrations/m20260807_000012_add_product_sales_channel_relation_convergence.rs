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
CREATE TABLE product_sales_channel_index_relation_convergence_requests (
    sequence_no BIGSERIAL NOT NULL,
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    product_source_version BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_product_sales_channel_index_relation_convergence_requests
        PRIMARY KEY (tenant_id, sequence_no),
    CONSTRAINT uq_product_sales_channel_index_relation_convergence_target_version
        UNIQUE (tenant_id, product_id, product_source_version),
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_request_sequence_positive
        CHECK (sequence_no > 0),
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_request_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_request_product_non_nil
        CHECK (product_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_request_source_positive
        CHECK (product_source_version > 0)
);

CREATE INDEX idx_product_sales_channel_index_relation_convergence_request_target
    ON product_sales_channel_index_relation_convergence_requests (
        tenant_id,
        product_id,
        product_source_version DESC,
        sequence_no DESC
    );

CREATE TABLE product_sales_channel_index_relation_convergence_state (
    tenant_id UUID PRIMARY KEY,
    visibility_cursor BIGINT NOT NULL DEFAULT 0,
    channel_identity_generation BIGINT NULL,
    sweep_generation BIGINT NULL,
    sweep_after_product_id UUID NULL,
    lease_token UUID NULL,
    lease_expires_at TIMESTAMPTZ NULL,
    available_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    attempt_count BIGINT NOT NULL DEFAULT 0,
    last_error TEXT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_state_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_visibility_cursor
        CHECK (visibility_cursor >= 0),
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_channel_generation
        CHECK (channel_identity_generation IS NULL OR channel_identity_generation >= 0),
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_sweep_generation
        CHECK (sweep_generation IS NULL OR sweep_generation >= 0),
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_sweep_order
        CHECK (
            channel_identity_generation IS NULL
            OR sweep_generation IS NULL
            OR sweep_generation >= channel_identity_generation
        ),
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_sweep_cursor
        CHECK (sweep_after_product_id IS NULL OR sweep_generation IS NOT NULL),
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_lease_pair
        CHECK ((lease_token IS NULL) = (lease_expires_at IS NULL)),
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_lease_token_non_nil
        CHECK (
            lease_token IS NULL
            OR lease_token <> '00000000-0000-0000-0000-000000000000'::uuid
        ),
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_attempt_count
        CHECK (attempt_count >= 0),
    CONSTRAINT chk_product_sales_channel_index_relation_convergence_error_bounded
        CHECK (last_error IS NULL OR octet_length(last_error) BETWEEN 1 AND 1024)
);

CREATE INDEX idx_product_sales_channel_index_relation_convergence_due
    ON product_sales_channel_index_relation_convergence_state (
        available_at,
        lease_expires_at,
        updated_at,
        tenant_id
    );

CREATE OR REPLACE FUNCTION rustok_product_reject_channel_relation_convergence_request_mutation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'Product-SalesChannel convergence requests are append-only';
    RETURN NULL;
END;
$$;

CREATE TRIGGER trg_product_channel_relation_convergence_request_update
BEFORE UPDATE ON product_sales_channel_index_relation_convergence_requests
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_channel_relation_convergence_request_mutation();

CREATE TRIGGER trg_product_channel_relation_convergence_request_delete
BEFORE DELETE ON product_sales_channel_index_relation_convergence_requests
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_channel_relation_convergence_request_mutation();

CREATE OR REPLACE FUNCTION rustok_product_guard_channel_relation_convergence_state()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    next_visibility_sequence BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.visibility_cursor <> 0
           OR NEW.channel_identity_generation IS NOT NULL
           OR NEW.sweep_generation IS NOT NULL
           OR NEW.sweep_after_product_id IS NOT NULL
           OR NEW.lease_token IS NOT NULL
           OR NEW.lease_expires_at IS NOT NULL
           OR NEW.attempt_count <> 0
           OR NEW.last_error IS NOT NULL
        THEN
            RAISE EXCEPTION 'Product-SalesChannel convergence state must start from the canonical empty checkpoint';
        END IF;
        RETURN NEW;
    END IF;

    IF NEW.tenant_id <> OLD.tenant_id THEN
        RAISE EXCEPTION 'Product-SalesChannel convergence tenant identity is immutable';
    END IF;
    IF NEW.visibility_cursor < OLD.visibility_cursor THEN
        RAISE EXCEPTION 'Product-SalesChannel convergence visibility cursor cannot regress';
    END IF;
    IF NEW.visibility_cursor > OLD.visibility_cursor THEN
        SELECT MIN(request.sequence_no)
          INTO next_visibility_sequence
          FROM product_sales_channel_index_relation_convergence_requests request
         WHERE request.tenant_id = NEW.tenant_id
           AND request.sequence_no > OLD.visibility_cursor;
        IF next_visibility_sequence IS NULL
           OR NEW.visibility_cursor <> next_visibility_sequence
           OR OLD.lease_token IS NULL
           OR NEW.lease_token IS NOT NULL
        THEN
            RAISE EXCEPTION 'Product-SalesChannel convergence visibility cursor must advance exactly one leased request';
        END IF;
    END IF;

    IF OLD.channel_identity_generation IS NOT NULL
       AND NEW.channel_identity_generation IS NULL
    THEN
        RAISE EXCEPTION 'Product-SalesChannel convergence Channel generation cannot be cleared';
    END IF;
    IF OLD.channel_identity_generation IS NOT NULL
       AND NEW.channel_identity_generation < OLD.channel_identity_generation
    THEN
        RAISE EXCEPTION 'Product-SalesChannel convergence Channel generation cannot regress';
    END IF;
    IF NEW.channel_identity_generation IS DISTINCT FROM OLD.channel_identity_generation THEN
        IF OLD.sweep_generation IS NULL
           OR NEW.channel_identity_generation IS DISTINCT FROM OLD.sweep_generation
           OR NEW.sweep_generation IS NOT NULL
           OR NEW.sweep_after_product_id IS NOT NULL
           OR OLD.lease_token IS NULL
           OR NEW.lease_token IS NOT NULL
        THEN
            RAISE EXCEPTION 'Product-SalesChannel convergence Channel generation may advance only by completing the leased sweep';
        END IF;
    END IF;

    IF OLD.sweep_generation IS NOT NULL
       AND NEW.sweep_generation IS NOT NULL
       AND NEW.sweep_generation <> OLD.sweep_generation
    THEN
        RAISE EXCEPTION 'Product-SalesChannel convergence in-progress sweep generation is immutable';
    END IF;
    IF OLD.sweep_generation IS NULL AND NEW.sweep_generation IS NOT NULL THEN
        IF NEW.sweep_after_product_id IS NOT NULL
           OR NEW.lease_token IS NULL
           OR NEW.attempt_count <> OLD.attempt_count + 1
           OR (
               OLD.channel_identity_generation IS NOT NULL
               AND NEW.sweep_generation < OLD.channel_identity_generation
           )
        THEN
            RAISE EXCEPTION 'Product-SalesChannel convergence sweep must start from a new leased generation';
        END IF;
    END IF;
    IF NEW.sweep_after_product_id IS DISTINCT FROM OLD.sweep_after_product_id THEN
        IF NEW.sweep_generation IS NULL THEN
            IF NEW.sweep_after_product_id IS NOT NULL THEN
                RAISE EXCEPTION 'Product-SalesChannel convergence completed sweep cursor must clear';
            END IF;
        ELSE
            IF OLD.sweep_generation IS NULL
               OR NEW.sweep_generation <> OLD.sweep_generation
               OR NEW.sweep_after_product_id IS NULL
               OR OLD.lease_token IS NULL
               OR NEW.lease_token IS NOT NULL
               OR (
                   OLD.sweep_after_product_id IS NOT NULL
                   AND NEW.sweep_after_product_id <= OLD.sweep_after_product_id
               )
            THEN
                RAISE EXCEPTION 'Product-SalesChannel convergence sweep cursor must advance strictly while completing a leased page';
            END IF;
        END IF;
    END IF;
    IF OLD.sweep_generation IS NOT NULL AND NEW.sweep_generation IS NULL THEN
        IF NEW.channel_identity_generation IS DISTINCT FROM OLD.sweep_generation
           OR NEW.sweep_after_product_id IS NOT NULL
           OR OLD.lease_token IS NULL
           OR NEW.lease_token IS NOT NULL
        THEN
            RAISE EXCEPTION 'Product-SalesChannel convergence sweep may clear only after checkpointing its generation';
        END IF;
    END IF;

    IF NEW.lease_token IS DISTINCT FROM OLD.lease_token THEN
        IF NEW.lease_token IS NOT NULL THEN
            IF NEW.attempt_count <> OLD.attempt_count + 1 THEN
                RAISE EXCEPTION 'Product-SalesChannel convergence lease acquisition must advance attempt count exactly once';
            END IF;
        ELSIF OLD.lease_token IS NULL THEN
            RAISE EXCEPTION 'Product-SalesChannel convergence lease clear requires an existing lease';
        END IF;
    ELSIF NEW.attempt_count <> OLD.attempt_count THEN
        RAISE EXCEPTION 'Product-SalesChannel convergence attempt count may change only with lease acquisition';
    END IF;
    IF NEW.lease_expires_at IS DISTINCT FROM OLD.lease_expires_at
       AND NEW.lease_token IS NOT DISTINCT FROM OLD.lease_token
    THEN
        RAISE EXCEPTION 'Product-SalesChannel convergence lease expiry may change only with lease ownership';
    END IF;
    IF NEW.attempt_count < OLD.attempt_count THEN
        RAISE EXCEPTION 'Product-SalesChannel convergence attempt count cannot regress';
    END IF;

    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_product_reject_channel_relation_convergence_state_delete()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'Product-SalesChannel convergence state cannot be deleted';
    RETURN NULL;
END;
$$;

CREATE TRIGGER trg_product_channel_relation_convergence_state_insert_update
BEFORE INSERT OR UPDATE ON product_sales_channel_index_relation_convergence_state
FOR EACH ROW
EXECUTE FUNCTION rustok_product_guard_channel_relation_convergence_state();

CREATE TRIGGER trg_product_channel_relation_convergence_state_delete
BEFORE DELETE ON product_sales_channel_index_relation_convergence_state
FOR EACH ROW
EXECUTE FUNCTION rustok_product_reject_channel_relation_convergence_state_delete();

CREATE OR REPLACE FUNCTION rustok_product_enqueue_channel_relation_convergence()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO product_sales_channel_index_relation_convergence_state (tenant_id)
    VALUES (NEW.tenant_id)
    ON CONFLICT (tenant_id) DO NOTHING;

    IF TG_OP = 'INSERT'
       OR OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
       OR OLD.id IS DISTINCT FROM NEW.id
       OR (OLD.metadata #> '{channel_visibility}')
            IS DISTINCT FROM (NEW.metadata #> '{channel_visibility}')
    THEN
        INSERT INTO product_sales_channel_index_relation_convergence_requests (
            tenant_id,
            product_id,
            product_source_version
        ) VALUES (
            NEW.tenant_id,
            NEW.id,
            NEW.index_revision
        )
        ON CONFLICT (tenant_id, product_id, product_source_version) DO NOTHING;
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_products_enqueue_channel_relation_convergence_insert
AFTER INSERT ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_enqueue_channel_relation_convergence();

CREATE TRIGGER trg_products_enqueue_channel_relation_convergence_update
AFTER UPDATE OF metadata, tenant_id, id ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_enqueue_channel_relation_convergence();

INSERT INTO product_sales_channel_index_relation_convergence_state (tenant_id)
SELECT DISTINCT tenant_id
FROM products
ORDER BY tenant_id
ON CONFLICT (tenant_id) DO NOTHING;
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
DROP TRIGGER IF EXISTS trg_products_enqueue_channel_relation_convergence_update ON products;
DROP TRIGGER IF EXISTS trg_products_enqueue_channel_relation_convergence_insert ON products;
DROP TRIGGER IF EXISTS trg_product_channel_relation_convergence_state_delete
    ON product_sales_channel_index_relation_convergence_state;
DROP TRIGGER IF EXISTS trg_product_channel_relation_convergence_state_insert_update
    ON product_sales_channel_index_relation_convergence_state;
DROP TRIGGER IF EXISTS trg_product_channel_relation_convergence_request_delete
    ON product_sales_channel_index_relation_convergence_requests;
DROP TRIGGER IF EXISTS trg_product_channel_relation_convergence_request_update
    ON product_sales_channel_index_relation_convergence_requests;
DROP FUNCTION IF EXISTS rustok_product_enqueue_channel_relation_convergence();
DROP FUNCTION IF EXISTS rustok_product_reject_channel_relation_convergence_state_delete();
DROP FUNCTION IF EXISTS rustok_product_guard_channel_relation_convergence_state();
DROP FUNCTION IF EXISTS rustok_product_reject_channel_relation_convergence_request_mutation();
DROP TABLE IF EXISTS product_sales_channel_index_relation_convergence_state;
DROP TABLE IF EXISTS product_sales_channel_index_relation_convergence_requests;
"#,
            )
            .await?;
        Ok(())
    }
}
