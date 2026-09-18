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
-- 1. Product Variant Axes table
CREATE TABLE IF NOT EXISTS product_variant_axes (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    attribute_id UUID NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    FOREIGN KEY (tenant_id, product_id)
        REFERENCES products(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, attribute_id)
        REFERENCES product_attributes(tenant_id, id) ON DELETE RESTRICT,

    CONSTRAINT uq_product_variant_axes_tenant_id UNIQUE (tenant_id, id),
    CONSTRAINT uq_product_variant_axes_product_attribute UNIQUE (tenant_id, product_id, attribute_id),
    CONSTRAINT uq_product_variant_axes_product_position UNIQUE (tenant_id, product_id, position)
);

-- 2. Product Variant Axis Values table (allowed values per axis)
CREATE TABLE IF NOT EXISTS product_variant_axis_values (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    axis_id UUID NOT NULL,
    option_id UUID NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    FOREIGN KEY (tenant_id, axis_id)
        REFERENCES product_variant_axes(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, option_id)
        REFERENCES product_attribute_options(tenant_id, id) ON DELETE RESTRICT,

    CONSTRAINT uq_product_variant_axis_values_tenant_id UNIQUE (tenant_id, id),
    CONSTRAINT uq_product_variant_axis_values_axis_option UNIQUE (tenant_id, axis_id, option_id),
    CONSTRAINT uq_product_variant_axis_values_axis_position UNIQUE (tenant_id, axis_id, position)
);

-- 3. Channel-Aware Product Attribute Groups
CREATE TABLE IF NOT EXISTS product_attribute_groups (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    code VARCHAR(128) NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT uq_product_attribute_groups_tenant_id UNIQUE (tenant_id, id),
    CONSTRAINT uq_product_attribute_groups_tenant_code UNIQUE (tenant_id, code)
);

CREATE TABLE IF NOT EXISTS product_attribute_group_translations (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    group_id UUID NOT NULL,
    locale VARCHAR(32) NOT NULL,
    label VARCHAR(255) NOT NULL,

    FOREIGN KEY (tenant_id, group_id)
        REFERENCES product_attribute_groups(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT uq_product_attribute_group_translations UNIQUE (tenant_id, group_id, locale)
);

CREATE TABLE IF NOT EXISTS product_attribute_group_attributes (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    group_id UUID NOT NULL,
    attribute_id UUID NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,

    PRIMARY KEY (tenant_id, group_id, attribute_id),
    FOREIGN KEY (tenant_id, group_id)
        REFERENCES product_attribute_groups(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, attribute_id)
        REFERENCES product_attributes(tenant_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS product_attribute_group_channel_settings (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    group_id UUID NOT NULL,
    channel_id UUID NOT NULL,
    is_visible BOOLEAN NOT NULL DEFAULT TRUE,
    position INTEGER,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    FOREIGN KEY (tenant_id, group_id)
        REFERENCES product_attribute_groups(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT uq_product_attribute_group_channel_settings UNIQUE (tenant_id, group_id, channel_id)
);

-- 4. Variant Axis Policies on category_attributes and product_attribute_schema_attributes
ALTER TABLE category_attributes
    ADD COLUMN IF NOT EXISTS variant_axis_policy VARCHAR(32) NOT NULL DEFAULT 'forbidden',
    ADD COLUMN IF NOT EXISTS default_variant_axis BOOLEAN NOT NULL DEFAULT FALSE;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_category_attributes_variant_axis_policy') THEN
        ALTER TABLE category_attributes
            ADD CONSTRAINT chk_category_attributes_variant_axis_policy
            CHECK (variant_axis_policy IN ('forbidden', 'allowed', 'required'));
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_category_attributes_axis_default_consistency') THEN
        ALTER TABLE category_attributes
            ADD CONSTRAINT chk_category_attributes_axis_default_consistency
            CHECK (variant_axis_policy <> 'forbidden' OR default_variant_axis = FALSE);
    END IF;
END $$;

ALTER TABLE product_attribute_schema_attributes
    ADD COLUMN IF NOT EXISTS variant_axis_policy VARCHAR(32) NOT NULL DEFAULT 'forbidden',
    ADD COLUMN IF NOT EXISTS default_variant_axis BOOLEAN NOT NULL DEFAULT FALSE;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_schema_attributes_variant_axis_policy') THEN
        ALTER TABLE product_attribute_schema_attributes
            ADD CONSTRAINT chk_schema_attributes_variant_axis_policy
            CHECK (variant_axis_policy IN ('forbidden', 'allowed', 'required'));
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_schema_attributes_axis_default_consistency') THEN
        ALTER TABLE product_attribute_schema_attributes
            ADD CONSTRAINT chk_schema_attributes_axis_default_consistency
            CHECK (variant_axis_policy <> 'forbidden' OR default_variant_axis = FALSE);
    END IF;
END $$;

-- 5. Option-Attribute Ownership Validation Trigger
CREATE OR REPLACE FUNCTION rustok_product_validate_axis_value_option()
RETURNS TRIGGER AS $$
DECLARE
    axis_attribute_id UUID;
    option_attribute_id UUID;
BEGIN
    SELECT attribute_id INTO axis_attribute_id
    FROM product_variant_axes
    WHERE tenant_id = NEW.tenant_id AND id = NEW.axis_id;

    SELECT attribute_id INTO option_attribute_id
    FROM product_attribute_options
    WHERE tenant_id = NEW.tenant_id AND id = NEW.option_id;

    IF axis_attribute_id IS NULL OR option_attribute_id IS NULL
       OR axis_attribute_id <> option_attribute_id THEN
        RAISE EXCEPTION 'axis value option % does not belong to axis % attribute',
            NEW.option_id, NEW.axis_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_axis_value_option_ownership ON product_variant_axis_values;
CREATE CONSTRAINT TRIGGER trg_axis_value_option_ownership
AFTER INSERT OR UPDATE ON product_variant_axis_values
FOR EACH ROW EXECUTE FUNCTION rustok_product_validate_axis_value_option();

-- 6. Canonical Combination Identity Computation Function
CREATE OR REPLACE FUNCTION rustok_product_compute_combination_identity(
    p_variant_id UUID,
    p_product_id UUID,
    p_tenant_id  UUID
) RETURNS TEXT AS $$
DECLARE
    result TEXT;
    axes_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO axes_count
    FROM product_variant_axes
    WHERE tenant_id = p_tenant_id AND product_id = p_product_id;

    IF axes_count = 0 THEN
        RETURN NULL;
    END IF;

    SELECT string_agg(
        pva.attribute_id::text || ':' || pvao.option_id::text,
        ';' ORDER BY pva.attribute_id
    ) INTO result
    FROM product_variant_attribute_values pva
    JOIN product_variant_attribute_value_options pvao
        ON pvao.tenant_id = pva.tenant_id AND pvao.value_id = pva.id
    JOIN product_variant_axes ax
        ON ax.tenant_id = pva.tenant_id
        AND ax.product_id = p_product_id
        AND ax.attribute_id = pva.attribute_id
    WHERE pva.tenant_id = p_tenant_id
      AND pva.variant_id = p_variant_id;

    RETURN result;
END;
$$ LANGUAGE plpgsql STABLE;

-- 7. Trigger to Maintain Combination Identity
CREATE OR REPLACE FUNCTION rustok_product_maintain_combination_identity()
RETURNS TRIGGER AS $$
DECLARE
    v_variant_id UUID;
    v_tenant_id UUID;
    v_product_id UUID;
BEGIN
    IF TG_OP = 'DELETE' THEN
        SELECT pva.variant_id, pva.tenant_id INTO v_variant_id, v_tenant_id
        FROM product_variant_attribute_values pva
        WHERE pva.id = OLD.value_id;
    ELSE
        SELECT pva.variant_id, pva.tenant_id INTO v_variant_id, v_tenant_id
        FROM product_variant_attribute_values pva
        WHERE pva.id = NEW.value_id;
    END IF;

    IF v_variant_id IS NOT NULL THEN
        SELECT pv.product_id INTO v_product_id
        FROM product_variants pv
        WHERE pv.tenant_id = v_tenant_id AND pv.id = v_variant_id;

        IF v_product_id IS NOT NULL THEN
            UPDATE product_variants
            SET combination_identity = rustok_product_compute_combination_identity(
                v_variant_id, v_product_id, v_tenant_id
            )
            WHERE tenant_id = v_tenant_id AND id = v_variant_id;
        END IF;
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    ELSE
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_maintain_combination_identity ON product_variant_attribute_value_options;
CREATE CONSTRAINT TRIGGER trg_maintain_combination_identity
AFTER INSERT OR UPDATE OR DELETE
ON product_variant_attribute_value_options
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW
EXECUTE FUNCTION rustok_product_maintain_combination_identity();

-- 8. Deferred Completeness Constraint Trigger
CREATE OR REPLACE FUNCTION rustok_product_validate_variant_axis_completeness()
RETURNS TRIGGER AS $$
DECLARE
    configured_count INTEGER;
    assigned_count   INTEGER;
    v_variant_id     UUID;
    v_tenant_id      UUID;
    v_product_id     UUID;
BEGIN
    IF TG_OP = 'DELETE' THEN
        SELECT pva.variant_id, pva.tenant_id INTO v_variant_id, v_tenant_id
        FROM product_variant_attribute_values pva
        WHERE pva.id = OLD.value_id;
    ELSE
        SELECT pva.variant_id, pva.tenant_id INTO v_variant_id, v_tenant_id
        FROM product_variant_attribute_values pva
        WHERE pva.id = NEW.value_id;
    END IF;

    IF v_variant_id IS NULL THEN
        IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
    END IF;

    SELECT product_id INTO v_product_id
    FROM product_variants
    WHERE tenant_id = v_tenant_id AND id = v_variant_id;

    IF v_product_id IS NULL THEN
        IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
    END IF;

    SELECT COUNT(*) INTO configured_count
    FROM product_variant_axes
    WHERE tenant_id = v_tenant_id AND product_id = v_product_id;

    IF configured_count = 0 THEN
        IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
    END IF;

    SELECT COUNT(DISTINCT ax.id) INTO assigned_count
    FROM product_variant_axes ax
    JOIN product_variant_attribute_values pva
        ON pva.tenant_id = ax.tenant_id
        AND pva.variant_id = v_variant_id
        AND pva.attribute_id = ax.attribute_id
    JOIN product_variant_attribute_value_options pvao
        ON pvao.tenant_id = pva.tenant_id
        AND pvao.value_id = pva.id
    WHERE ax.tenant_id = v_tenant_id
      AND ax.product_id = v_product_id;

    IF assigned_count <> configured_count THEN
        RAISE EXCEPTION
            'variant % has % axis assignments but product has % configured axes',
            v_variant_id, assigned_count, configured_count;
    END IF;

    IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_variant_axis_completeness ON product_variant_attribute_value_options;
CREATE CONSTRAINT TRIGGER trg_variant_axis_completeness
AFTER INSERT OR UPDATE OR DELETE
ON product_variant_attribute_value_options
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW
EXECUTE FUNCTION rustok_product_validate_variant_axis_completeness();
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP TRIGGER IF EXISTS trg_variant_axis_completeness ON product_variant_attribute_value_options;
DROP FUNCTION IF EXISTS rustok_product_validate_variant_axis_completeness();
DROP TRIGGER IF EXISTS trg_maintain_combination_identity ON product_variant_attribute_value_options;
DROP FUNCTION IF EXISTS rustok_product_maintain_combination_identity();
DROP FUNCTION IF EXISTS rustok_product_compute_combination_identity(UUID, UUID, UUID);
DROP TRIGGER IF EXISTS trg_axis_value_option_ownership ON product_variant_axis_values;
DROP FUNCTION IF EXISTS rustok_product_validate_axis_value_option();

ALTER TABLE product_attribute_schema_attributes
    DROP CONSTRAINT IF EXISTS chk_schema_attributes_axis_default_consistency,
    DROP CONSTRAINT IF EXISTS chk_schema_attributes_variant_axis_policy,
    DROP COLUMN IF EXISTS default_variant_axis,
    DROP COLUMN IF EXISTS variant_axis_policy;

ALTER TABLE category_attributes
    DROP CONSTRAINT IF EXISTS chk_category_attributes_axis_default_consistency,
    DROP CONSTRAINT IF EXISTS chk_category_attributes_variant_axis_policy,
    DROP COLUMN IF EXISTS default_variant_axis,
    DROP COLUMN IF EXISTS variant_axis_policy;

DROP TABLE IF EXISTS product_attribute_group_channel_settings CASCADE;
DROP TABLE IF EXISTS product_attribute_group_attributes CASCADE;
DROP TABLE IF EXISTS product_attribute_group_translations CASCADE;
DROP TABLE IF EXISTS product_attribute_groups CASCADE;
DROP TABLE IF EXISTS product_variant_axis_values CASCADE;
DROP TABLE IF EXISTS product_variant_axes CASCADE;
"#,
            )
            .await?;

        Ok(())
    }
}
