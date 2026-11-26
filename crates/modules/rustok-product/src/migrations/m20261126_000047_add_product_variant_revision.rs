use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

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
ALTER TABLE product_variants
    ADD COLUMN IF NOT EXISTS revision BIGINT;

-- Preserve the Index watermark while backfilling the independent business predecessor.
DROP TRIGGER IF EXISTS trg_product_variants_bump_index_revision ON product_variants;
DROP TRIGGER IF EXISTS trg_product_variants_bump_business_revision ON product_variants;

UPDATE product_variants
SET revision = index_revision;

ALTER TABLE product_variants
    ALTER COLUMN revision SET DEFAULT 1,
    ALTER COLUMN revision SET NOT NULL;

ALTER TABLE product_variants
    DROP CONSTRAINT IF EXISTS chk_product_variants_revision_positive;

ALTER TABLE product_variants
    ADD CONSTRAINT chk_product_variants_revision_positive CHECK (revision > 0);

CREATE TRIGGER trg_product_variants_bump_index_revision
BEFORE UPDATE ON product_variants
FOR EACH ROW
EXECUTE FUNCTION rustok_product_variant_bump_index_revision();

CREATE OR REPLACE FUNCTION rustok_product_variant_bump_business_revision()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.revision >= 9223372036854775807 THEN
        RAISE EXCEPTION 'product variant business revision exhausted for variant %', OLD.id;
    END IF;

    NEW.revision := OLD.revision + 1;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_product_variants_bump_business_revision
BEFORE UPDATE ON product_variants
FOR EACH ROW
EXECUTE FUNCTION rustok_product_variant_bump_business_revision();

-- Keep recreated Variant identities monotonic across the retained delete tombstone.
CREATE OR REPLACE FUNCTION rustok_product_variant_seed_index_revision_from_tombstone()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    retained_source_version BIGINT;
BEGIN
    SELECT source_version
      INTO retained_source_version
      FROM product_variant_index_tombstones
     WHERE tenant_id = NEW.tenant_id
       AND variant_id = NEW.id;

    IF retained_source_version IS NOT NULL THEN
        IF retained_source_version = 9223372036854775807 THEN
            RAISE EXCEPTION 'product variant index revision exhausted for reused variant %', NEW.id;
        END IF;
        NEW.index_revision := GREATEST(NEW.index_revision, retained_source_version + 1);
    END IF;

    NEW.revision := NEW.index_revision;
    RETURN NEW;
END;
$$;
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
DROP TRIGGER IF EXISTS trg_product_variants_bump_business_revision ON product_variants;
DROP FUNCTION IF EXISTS rustok_product_variant_bump_business_revision();

CREATE OR REPLACE FUNCTION rustok_product_variant_seed_index_revision_from_tombstone()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    retained_source_version BIGINT;
BEGIN
    SELECT source_version
      INTO retained_source_version
      FROM product_variant_index_tombstones
     WHERE tenant_id = NEW.tenant_id
       AND variant_id = NEW.id;

    IF retained_source_version IS NOT NULL THEN
        IF retained_source_version = 9223372036854775807 THEN
            RAISE EXCEPTION 'product variant index revision exhausted for reused variant %', NEW.id;
        END IF;
        NEW.index_revision := GREATEST(NEW.index_revision, retained_source_version + 1);
    END IF;

    RETURN NEW;
END;
$$;

ALTER TABLE product_variants
    DROP CONSTRAINT IF EXISTS chk_product_variants_revision_positive,
    DROP COLUMN IF EXISTS revision;
"#,
            )
            .await?;

        Ok(())
    }
}
