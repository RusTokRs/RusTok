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
CREATE TABLE product_index_tombstones (
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    locale TEXT NOT NULL,
    source_version BIGINT NOT NULL,
    deleted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, product_id, locale),
    CONSTRAINT chk_product_index_tombstones_locale_nonempty CHECK (btrim(locale) <> ''),
    CONSTRAINT chk_product_index_tombstones_locale_bounded CHECK (octet_length(locale) <= 128),
    CONSTRAINT chk_product_index_tombstones_source_version_positive CHECK (source_version > 0)
);

CREATE TABLE product_variant_index_tombstones (
    tenant_id UUID NOT NULL,
    variant_id UUID NOT NULL,
    source_version BIGINT NOT NULL,
    deleted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, variant_id),
    CONSTRAINT chk_product_variant_index_tombstones_source_version_positive CHECK (source_version > 0)
);

CREATE OR REPLACE FUNCTION rustok_product_store_index_tombstone(
    target_tenant_id UUID,
    target_product_id UUID,
    target_locale TEXT,
    target_source_version BIGINT
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO product_index_tombstones (
        tenant_id,
        product_id,
        locale,
        source_version,
        deleted_at
    ) VALUES (
        target_tenant_id,
        target_product_id,
        target_locale,
        target_source_version,
        CURRENT_TIMESTAMP
    )
    ON CONFLICT (tenant_id, product_id, locale) DO UPDATE
    SET source_version = GREATEST(
            product_index_tombstones.source_version,
            EXCLUDED.source_version
        ),
        deleted_at = CURRENT_TIMESTAMP;
END;
$$;

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

CREATE OR REPLACE FUNCTION rustok_product_clear_superseded_index_tombstone(
    target_tenant_id UUID,
    target_product_id UUID,
    target_locale TEXT,
    live_source_version BIGINT
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM product_index_tombstones tombstone
        WHERE tombstone.tenant_id = target_tenant_id
          AND tombstone.product_id = target_product_id
          AND tombstone.locale = target_locale
          AND tombstone.source_version >= live_source_version
    ) THEN
        RAISE EXCEPTION
            'product live index revision does not supersede retained tombstone for product % locale %',
            target_product_id,
            target_locale;
    END IF;

    DELETE FROM product_index_tombstones
    WHERE tenant_id = target_tenant_id
      AND product_id = target_product_id
      AND locale = target_locale
      AND source_version < live_source_version;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_product_variant_clear_superseded_index_tombstone(
    target_tenant_id UUID,
    target_variant_id UUID,
    live_source_version BIGINT
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM product_variant_index_tombstones tombstone
        WHERE tombstone.tenant_id = target_tenant_id
          AND tombstone.variant_id = target_variant_id
          AND tombstone.source_version >= live_source_version
    ) THEN
        RAISE EXCEPTION
            'product variant live index revision does not supersede retained tombstone for variant %',
            target_variant_id;
    END IF;

    DELETE FROM product_variant_index_tombstones
    WHERE tenant_id = target_tenant_id
      AND variant_id = target_variant_id
      AND source_version < live_source_version;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_product_seed_index_revision_from_tombstones()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    retained_source_version BIGINT;
BEGIN
    SELECT MAX(source_version)
      INTO retained_source_version
      FROM product_index_tombstones
     WHERE tenant_id = NEW.tenant_id
       AND product_id = NEW.id;

    IF retained_source_version IS NOT NULL THEN
        IF retained_source_version = 9223372036854775807 THEN
            RAISE EXCEPTION 'product index revision exhausted for reused product %', NEW.id;
        END IF;
        NEW.index_revision := GREATEST(NEW.index_revision, retained_source_version + 1);
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_products_seed_index_revision
BEFORE INSERT ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_seed_index_revision_from_tombstones();

CREATE OR REPLACE FUNCTION rustok_product_capture_index_tombstones()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.index_revision = 9223372036854775807 THEN
        RAISE EXCEPTION 'product index revision exhausted for deleted product %', OLD.id;
    END IF;

    INSERT INTO product_index_tombstones (
        tenant_id,
        product_id,
        locale,
        source_version,
        deleted_at
    )
    SELECT
        OLD.tenant_id,
        OLD.id,
        translation.locale,
        OLD.index_revision + 1,
        CURRENT_TIMESTAMP
    FROM product_translations translation
    WHERE translation.tenant_id = OLD.tenant_id
      AND translation.product_id = OLD.id
    ON CONFLICT (tenant_id, product_id, locale) DO UPDATE
    SET source_version = GREATEST(
            product_index_tombstones.source_version,
            EXCLUDED.source_version
        ),
        deleted_at = CURRENT_TIMESTAMP;

    RETURN OLD;
END;
$$;

CREATE TRIGGER trg_products_capture_index_tombstones
BEFORE DELETE ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_capture_index_tombstones();

CREATE OR REPLACE FUNCTION rustok_product_translation_bump_index_revision()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    old_source_version BIGINT;
    new_source_version BIGINT;
    identity_changed BOOLEAN;
BEGIN
    IF TG_OP = 'DELETE' THEN
        UPDATE products
        SET index_revision = index_revision + 1
        WHERE tenant_id = OLD.tenant_id
          AND id = OLD.product_id
        RETURNING index_revision INTO old_source_version;

        IF old_source_version IS NOT NULL THEN
            PERFORM rustok_product_store_index_tombstone(
                OLD.tenant_id,
                OLD.product_id,
                OLD.locale,
                old_source_version
            );
        END IF;
        RETURN OLD;
    END IF;

    IF TG_OP = 'INSERT' THEN
        UPDATE products
        SET index_revision = index_revision + 1
        WHERE tenant_id = NEW.tenant_id
          AND id = NEW.product_id
        RETURNING index_revision INTO new_source_version;

        IF new_source_version IS NULL THEN
            RAISE EXCEPTION 'product translation parent is missing for product %', NEW.product_id;
        END IF;
        PERFORM rustok_product_clear_superseded_index_tombstone(
            NEW.tenant_id,
            NEW.product_id,
            NEW.locale,
            new_source_version
        );
        RETURN NEW;
    END IF;

    identity_changed := OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.product_id IS DISTINCT FROM NEW.product_id
        OR OLD.locale IS DISTINCT FROM NEW.locale;

    IF identity_changed THEN
        UPDATE products
        SET index_revision = index_revision + 1
        WHERE tenant_id = OLD.tenant_id
          AND id = OLD.product_id
        RETURNING index_revision INTO old_source_version;

        IF old_source_version IS NOT NULL THEN
            PERFORM rustok_product_store_index_tombstone(
                OLD.tenant_id,
                OLD.product_id,
                OLD.locale,
                old_source_version
            );
        END IF;

        UPDATE products
        SET index_revision = index_revision + 1
        WHERE tenant_id = NEW.tenant_id
          AND id = NEW.product_id
        RETURNING index_revision INTO new_source_version;
    ELSE
        UPDATE products
        SET index_revision = index_revision + 1
        WHERE tenant_id = NEW.tenant_id
          AND id = NEW.product_id
        RETURNING index_revision INTO new_source_version;
    END IF;

    IF new_source_version IS NULL THEN
        RAISE EXCEPTION 'product translation parent is missing for product %', NEW.product_id;
    END IF;
    PERFORM rustok_product_clear_superseded_index_tombstone(
        NEW.tenant_id,
        NEW.product_id,
        NEW.locale,
        new_source_version
    );
    RETURN NEW;
END;
$$;

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

CREATE TRIGGER trg_product_variants_seed_index_revision
BEFORE INSERT ON product_variants
FOR EACH ROW
EXECUTE FUNCTION rustok_product_variant_seed_index_revision_from_tombstone();

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

CREATE TRIGGER trg_product_variants_capture_index_tombstone
BEFORE DELETE ON product_variants
FOR EACH ROW
EXECUTE FUNCTION rustok_product_variant_capture_index_tombstone();

CREATE OR REPLACE FUNCTION rustok_product_variant_clear_inserted_index_tombstone()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM rustok_product_variant_clear_superseded_index_tombstone(
        NEW.tenant_id,
        NEW.id,
        NEW.index_revision
    );
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_product_variants_clear_index_tombstone
AFTER INSERT ON product_variants
FOR EACH ROW
EXECUTE FUNCTION rustok_product_variant_clear_inserted_index_tombstone();

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

CREATE TRIGGER trg_product_variants_move_index_tombstone
AFTER UPDATE OF id, tenant_id ON product_variants
FOR EACH ROW
EXECUTE FUNCTION rustok_product_variant_move_index_tombstone();
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
DROP TRIGGER IF EXISTS trg_product_variants_move_index_tombstone ON product_variants;
DROP TRIGGER IF EXISTS trg_product_variants_clear_index_tombstone ON product_variants;
DROP TRIGGER IF EXISTS trg_product_variants_capture_index_tombstone ON product_variants;
DROP TRIGGER IF EXISTS trg_product_variants_seed_index_revision ON product_variants;
DROP TRIGGER IF EXISTS trg_products_capture_index_tombstones ON products;
DROP TRIGGER IF EXISTS trg_products_seed_index_revision ON products;

CREATE OR REPLACE FUNCTION rustok_product_translation_bump_index_revision()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND OLD.product_id IS DISTINCT FROM NEW.product_id THEN
        UPDATE products
        SET index_revision = index_revision + 1
        WHERE id = OLD.product_id;

        UPDATE products
        SET index_revision = index_revision + 1
        WHERE id = NEW.product_id;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE products
        SET index_revision = index_revision + 1
        WHERE id = OLD.product_id;
    ELSE
        UPDATE products
        SET index_revision = index_revision + 1
        WHERE id = NEW.product_id;
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$;

DROP FUNCTION IF EXISTS rustok_product_variant_move_index_tombstone();
DROP FUNCTION IF EXISTS rustok_product_variant_clear_inserted_index_tombstone();
DROP FUNCTION IF EXISTS rustok_product_variant_capture_index_tombstone();
DROP FUNCTION IF EXISTS rustok_product_variant_seed_index_revision_from_tombstone();
DROP FUNCTION IF EXISTS rustok_product_capture_index_tombstones();
DROP FUNCTION IF EXISTS rustok_product_seed_index_revision_from_tombstones();
DROP FUNCTION IF EXISTS rustok_product_variant_clear_superseded_index_tombstone(UUID, UUID, BIGINT);
DROP FUNCTION IF EXISTS rustok_product_clear_superseded_index_tombstone(UUID, UUID, TEXT, BIGINT);
DROP FUNCTION IF EXISTS rustok_product_variant_store_index_tombstone(UUID, UUID, BIGINT);
DROP FUNCTION IF EXISTS rustok_product_store_index_tombstone(UUID, UUID, TEXT, BIGINT);
DROP TABLE IF EXISTS product_variant_index_tombstones;
DROP TABLE IF EXISTS product_index_tombstones;
"#,
            )
            .await?;

        Ok(())
    }
}
