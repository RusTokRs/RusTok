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
    position INTEGER NOT NULL DEFAULT 0 CHECK (position >= 0),
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
    position INTEGER NOT NULL DEFAULT 0 CHECK (position >= 0),
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
$$ LANGUAGE plpgsql VOLATILE;

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

-- 9. Complete state validation. The earlier option-row trigger cannot see a value
-- row after it is cascade-deleted, and it does not run when only axis configuration
-- changes. These deferred triggers make PostgreSQL the final authority for exact
-- configuration/assignment matching, allowed-option membership, derived identity,
-- and the single default-variant rule.
CREATE OR REPLACE FUNCTION rustok_product_assert_variant_axis_state(
    p_tenant_id UUID,
    p_product_id UUID,
    p_variant_id UUID
) RETURNS VOID AS $$
DECLARE
    configured_count INTEGER;
    invalid_assignment_count INTEGER;
    variant_count INTEGER;
    stored_identity TEXT;
    computed_identity TEXT;
BEGIN
    SELECT COUNT(*) INTO configured_count
    FROM product_variant_axes
    WHERE tenant_id = p_tenant_id AND product_id = p_product_id;

    SELECT combination_identity INTO stored_identity
    FROM product_variants
    WHERE tenant_id = p_tenant_id AND id = p_variant_id;

    IF NOT FOUND THEN
        RETURN;
    END IF;

    IF configured_count = 0 THEN
        SELECT COUNT(*) INTO variant_count
        FROM product_variants
        WHERE tenant_id = p_tenant_id AND product_id = p_product_id;
        IF variant_count <> 1 THEN
            RAISE EXCEPTION
                'product % without variant axes must own exactly one default variant',
                p_product_id;
        END IF;
        -- Combination identity is derived state, not a caller-controlled field. Normalize an
        -- old or direct write so the default-variant unique index and persisted state agree.
        IF stored_identity IS NOT NULL THEN
            UPDATE product_variants
            SET combination_identity = NULL
            WHERE tenant_id = p_tenant_id AND id = p_variant_id;
        END IF;
        RETURN;
    END IF;

    SELECT COUNT(*) INTO invalid_assignment_count
    FROM (
        SELECT axis.id
        FROM product_variant_axes axis
        LEFT JOIN product_variant_attribute_values value
          ON value.tenant_id = axis.tenant_id
         AND value.variant_id = p_variant_id
         AND value.attribute_id = axis.attribute_id
        LEFT JOIN product_variant_attribute_value_options selected
          ON selected.tenant_id = value.tenant_id
         AND selected.value_id = value.id
        LEFT JOIN product_variant_axis_values allowed
          ON allowed.tenant_id = axis.tenant_id
         AND allowed.axis_id = axis.id
         AND allowed.option_id = selected.option_id
        WHERE axis.tenant_id = p_tenant_id
          AND axis.product_id = p_product_id
        GROUP BY axis.id, value.id, value.detached_at
        HAVING value.id IS NULL
            OR value.detached_at IS NOT NULL
            OR COUNT(selected.option_id) <> 1
            OR COUNT(allowed.option_id) <> 1
    ) invalid_axis_assignments;

    IF invalid_assignment_count <> 0 THEN
        RAISE EXCEPTION
            'variant % does not have exactly one active allowed assignment for every configured axis',
            p_variant_id;
    END IF;

    computed_identity := rustok_product_compute_combination_identity(
        p_variant_id,
        p_product_id,
        p_tenant_id
    );
    -- Synchronize derived identity here as well as in the option-row maintenance trigger.
    -- Constraint triggers may run in a different deferred-event order, so relying only on the
    -- maintenance trigger can observe a complete assignment set before it has refreshed the
    -- stored value. A duplicate final identity still fails the Product unique index.
    IF stored_identity IS DISTINCT FROM computed_identity THEN
        UPDATE product_variants
        SET combination_identity = computed_identity
        WHERE tenant_id = p_tenant_id AND id = p_variant_id;
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION rustok_product_assert_variant_axis_state_for_variant(
    p_tenant_id UUID,
    p_variant_id UUID
) RETURNS VOID AS $$
DECLARE
    v_product_id UUID;
BEGIN
    SELECT product_id INTO v_product_id
    FROM product_variants
    WHERE tenant_id = p_tenant_id AND id = p_variant_id;
    IF v_product_id IS NOT NULL THEN
        PERFORM rustok_product_assert_variant_axis_state(p_tenant_id, v_product_id, p_variant_id);
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION rustok_product_assert_default_variant_state(
    p_tenant_id UUID,
    p_product_id UUID
) RETURNS VOID AS $$
DECLARE
    configured_count INTEGER;
    variant_count INTEGER;
    product_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM products
        WHERE tenant_id = p_tenant_id AND id = p_product_id
    ) INTO product_exists;
    -- A deferred child-row trigger also runs while its Product is being deleted. The final
    -- state has no Product to constrain, so do not turn a legitimate cascade into an error.
    IF NOT product_exists THEN
        RETURN;
    END IF;
    SELECT COUNT(*) INTO configured_count
    FROM product_variant_axes
    WHERE tenant_id = p_tenant_id AND product_id = p_product_id;
    IF configured_count <> 0 THEN
        RETURN;
    END IF;
    SELECT COUNT(*) INTO variant_count
    FROM product_variants
    WHERE tenant_id = p_tenant_id AND product_id = p_product_id;
    IF variant_count <> 1 THEN
        RAISE EXCEPTION
            'product % without variant axes must own exactly one default variant',
            p_product_id;
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION rustok_product_assert_product_variant_axis_state(
    p_tenant_id UUID,
    p_product_id UUID
) RETURNS VOID AS $$
DECLARE
    variant_row RECORD;
BEGIN
    -- Deferred child triggers run during a Product cascade. A deleted owner has no remaining
    -- aggregate state to validate.
    IF NOT EXISTS (
        SELECT 1 FROM products
        WHERE tenant_id = p_tenant_id AND id = p_product_id
    ) THEN
        RETURN;
    END IF;

    PERFORM rustok_product_assert_default_variant_state(p_tenant_id, p_product_id);

    IF EXISTS (
        SELECT 1
        FROM product_variant_axes axis
        LEFT JOIN product_variant_axis_values allowed
          ON allowed.tenant_id = axis.tenant_id AND allowed.axis_id = axis.id
        WHERE axis.tenant_id = p_tenant_id
          AND axis.product_id = p_product_id
          AND allowed.id IS NULL
    ) THEN
        RAISE EXCEPTION
            'product % has a variant axis without an allowed option', p_product_id;
    END IF;

    IF EXISTS (
        SELECT 1
        FROM product_variant_axes axis
        LEFT JOIN product_attributes attribute
          ON attribute.tenant_id = axis.tenant_id AND attribute.id = axis.attribute_id
        WHERE axis.tenant_id = p_tenant_id
          AND axis.product_id = p_product_id
          AND (
              attribute.id IS NULL
              OR attribute.archived_at IS NOT NULL
              OR attribute.value_type <> 'select'
              OR attribute.scope NOT IN ('variant', 'both')
          )
    ) THEN
        RAISE EXCEPTION
            'product % has an inactive or non-select variant axis attribute', p_product_id;
    END IF;

    IF EXISTS (
        SELECT 1
        FROM product_variant_axes axis
        JOIN product_variant_axis_values allowed
          ON allowed.tenant_id = axis.tenant_id AND allowed.axis_id = axis.id
        LEFT JOIN product_attribute_options attribute_option
          ON attribute_option.tenant_id = allowed.tenant_id
         AND attribute_option.id = allowed.option_id
        WHERE axis.tenant_id = p_tenant_id
          AND axis.product_id = p_product_id
          AND (
              attribute_option.id IS NULL
              OR attribute_option.archived_at IS NOT NULL
              OR attribute_option.attribute_id <> axis.attribute_id
          )
    ) THEN
        RAISE EXCEPTION
            'product % has an inactive or mismatched variant axis option', p_product_id;
    END IF;

    FOR variant_row IN
        SELECT id FROM product_variants
        WHERE tenant_id = p_tenant_id AND product_id = p_product_id
    LOOP
        PERFORM rustok_product_assert_variant_axis_state(
            p_tenant_id,
            p_product_id,
            variant_row.id
        );
    END LOOP;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION rustok_product_assert_variant_axis_state_for_axis(
    p_tenant_id UUID,
    p_axis_id UUID
) RETURNS VOID AS $$
DECLARE
    v_product_id UUID;
BEGIN
    SELECT product_id INTO v_product_id
    FROM product_variant_axes
    WHERE tenant_id = p_tenant_id AND id = p_axis_id;
    IF v_product_id IS NOT NULL THEN
        PERFORM rustok_product_assert_product_variant_axis_state(p_tenant_id, v_product_id);
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION rustok_product_validate_variant_axis_state_from_axis_attribute()
RETURNS TRIGGER AS $$
DECLARE
    product_row RECORD;
BEGIN
    FOR product_row IN
        SELECT DISTINCT product_id
        FROM product_variant_axes
        WHERE tenant_id = NEW.tenant_id AND attribute_id = NEW.id
    LOOP
        PERFORM rustok_product_assert_product_variant_axis_state(NEW.tenant_id, product_row.product_id);
    END LOOP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION rustok_product_validate_variant_axis_state_from_axis_option()
RETURNS TRIGGER AS $$
DECLARE
    product_row RECORD;
BEGIN
    FOR product_row IN
        SELECT DISTINCT axis.product_id
        FROM product_variant_axis_values allowed
        JOIN product_variant_axes axis
          ON axis.tenant_id = allowed.tenant_id AND axis.id = allowed.axis_id
        WHERE allowed.tenant_id = NEW.tenant_id AND allowed.option_id = NEW.id
    LOOP
        PERFORM rustok_product_assert_product_variant_axis_state(NEW.tenant_id, product_row.product_id);
    END LOOP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION rustok_product_validate_variant_axis_state_from_value()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' OR TG_OP = 'UPDATE' THEN
        PERFORM rustok_product_assert_variant_axis_state_for_variant(OLD.tenant_id, OLD.variant_id);
    END IF;
    IF TG_OP = 'INSERT' OR TG_OP = 'UPDATE' THEN
        PERFORM rustok_product_assert_variant_axis_state_for_variant(NEW.tenant_id, NEW.variant_id);
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION rustok_product_validate_variant_axis_state_from_variant()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' OR TG_OP = 'UPDATE' THEN
        PERFORM rustok_product_assert_default_variant_state(OLD.tenant_id, OLD.product_id);
    END IF;
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    PERFORM rustok_product_assert_variant_axis_state(NEW.tenant_id, NEW.product_id, NEW.id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION rustok_product_validate_variant_axis_state_from_axis()
RETURNS TRIGGER AS $$
BEGIN
    -- An UPDATE can move an axis between Products. Validate both final aggregates rather than
    -- only NEW, otherwise the old Product can retain an invalid no-axis/default state.
    IF TG_OP = 'DELETE' OR TG_OP = 'UPDATE' THEN
        PERFORM rustok_product_assert_product_variant_axis_state(OLD.tenant_id, OLD.product_id);
    END IF;
    IF TG_OP = 'INSERT' OR TG_OP = 'UPDATE' THEN
        PERFORM rustok_product_assert_product_variant_axis_state(NEW.tenant_id, NEW.product_id);
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION rustok_product_validate_variant_axis_state_from_axis_value()
RETURNS TRIGGER AS $$
BEGIN
    -- Like axis changes, an allowed-option update can move a row between axes. Revalidate both
    -- owners so neither axis can be left with no allowed values or an invalid Variant assignment.
    IF TG_OP = 'DELETE' OR TG_OP = 'UPDATE' THEN
        PERFORM rustok_product_assert_variant_axis_state_for_axis(OLD.tenant_id, OLD.axis_id);
    END IF;
    IF TG_OP = 'INSERT' OR TG_OP = 'UPDATE' THEN
        PERFORM rustok_product_assert_variant_axis_state_for_axis(NEW.tenant_id, NEW.axis_id);
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END;
$$ LANGUAGE plpgsql;

-- Existing catalog rows predate these triggers. Validate them before installing the
-- constraint triggers so a successful migration means every persisted product already obeys
-- the same invariants as subsequent writes. Operators must explicitly remediate ambiguous
-- legacy multi-variant products instead of silently losing a variant or inventing an axis.
DO $$
DECLARE
    product_row RECORD;
BEGIN
    FOR product_row IN SELECT tenant_id, id FROM products LOOP
        PERFORM rustok_product_assert_product_variant_axis_state(
            product_row.tenant_id,
            product_row.id
        );
    END LOOP;
END $$;

DROP TRIGGER IF EXISTS trg_variant_axis_state_from_variant ON product_variants;
CREATE CONSTRAINT TRIGGER trg_variant_axis_state_from_variant
AFTER INSERT OR UPDATE OR DELETE ON product_variants
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION rustok_product_validate_variant_axis_state_from_variant();

DROP TRIGGER IF EXISTS trg_variant_axis_state_from_value ON product_variant_attribute_values;
CREATE CONSTRAINT TRIGGER trg_variant_axis_state_from_value
AFTER INSERT OR UPDATE OR DELETE ON product_variant_attribute_values
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION rustok_product_validate_variant_axis_state_from_value();

DROP TRIGGER IF EXISTS trg_variant_axis_state_from_axis_attribute ON product_attributes;
CREATE CONSTRAINT TRIGGER trg_variant_axis_state_from_axis_attribute
AFTER UPDATE ON product_attributes
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION rustok_product_validate_variant_axis_state_from_axis_attribute();

DROP TRIGGER IF EXISTS trg_variant_axis_state_from_axis_option ON product_attribute_options;
CREATE CONSTRAINT TRIGGER trg_variant_axis_state_from_axis_option
AFTER UPDATE ON product_attribute_options
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION rustok_product_validate_variant_axis_state_from_axis_option();

DROP TRIGGER IF EXISTS trg_variant_axis_state_from_axis ON product_variant_axes;
CREATE CONSTRAINT TRIGGER trg_variant_axis_state_from_axis
AFTER INSERT OR UPDATE OR DELETE ON product_variant_axes
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION rustok_product_validate_variant_axis_state_from_axis();

DROP TRIGGER IF EXISTS trg_variant_axis_state_from_axis_value ON product_variant_axis_values;
CREATE CONSTRAINT TRIGGER trg_variant_axis_state_from_axis_value
AFTER INSERT OR UPDATE OR DELETE ON product_variant_axis_values
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION rustok_product_validate_variant_axis_state_from_axis_value();
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
DROP TRIGGER IF EXISTS trg_variant_axis_state_from_axis_option ON product_attribute_options;
DROP TRIGGER IF EXISTS trg_variant_axis_state_from_axis_attribute ON product_attributes;
DROP TRIGGER IF EXISTS trg_variant_axis_state_from_axis_value ON product_variant_axis_values;
DROP TRIGGER IF EXISTS trg_variant_axis_state_from_axis ON product_variant_axes;
DROP TRIGGER IF EXISTS trg_variant_axis_state_from_value ON product_variant_attribute_values;
DROP TRIGGER IF EXISTS trg_variant_axis_state_from_variant ON product_variants;
DROP FUNCTION IF EXISTS rustok_product_validate_variant_axis_state_from_axis_option();
DROP FUNCTION IF EXISTS rustok_product_validate_variant_axis_state_from_axis_attribute();
DROP FUNCTION IF EXISTS rustok_product_validate_variant_axis_state_from_axis_value();
DROP FUNCTION IF EXISTS rustok_product_assert_variant_axis_state_for_axis(UUID, UUID);
DROP FUNCTION IF EXISTS rustok_product_validate_variant_axis_state_from_axis();
DROP FUNCTION IF EXISTS rustok_product_validate_variant_axis_state_from_value();
DROP FUNCTION IF EXISTS rustok_product_assert_product_variant_axis_state(UUID, UUID);
DROP FUNCTION IF EXISTS rustok_product_assert_default_variant_state(UUID, UUID);
DROP FUNCTION IF EXISTS rustok_product_assert_variant_axis_state_for_variant(UUID, UUID);
DROP FUNCTION IF EXISTS rustok_product_validate_variant_axis_state_from_variant();
DROP FUNCTION IF EXISTS rustok_product_assert_variant_axis_state(UUID, UUID, UUID);
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
