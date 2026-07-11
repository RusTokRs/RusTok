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
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM product_categories
        WHERE assignment_kind = 'primary'
        GROUP BY tenant_id, product_id
        HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION 'cannot migrate product_categories: multiple primary assignments exist';
    END IF;
END $$;

UPDATE products product
SET primary_category_id = assignment.category_id
FROM product_categories assignment
WHERE assignment.tenant_id = product.tenant_id
  AND assignment.product_id = product.id
  AND assignment.assignment_kind = 'primary'
  AND product.primary_category_id IS NULL;

UPDATE product_categories
SET assignment_kind = 'navigation'
WHERE assignment_kind = 'primary';

ALTER TABLE product_categories
    ADD CONSTRAINT chk_product_categories_no_primary_assignment
    CHECK (assignment_kind <> 'primary');

ALTER TABLE product_attribute_values
    ADD CONSTRAINT chk_product_attribute_values_one_scalar
    CHECK (num_nonnulls(
        value_text, value_integer, value_decimal, value_boolean, value_date, value_datetime, value_json
    ) <= 1),
    ADD CONSTRAINT chk_product_attribute_values_detached_storage
    CHECK (detached_at IS NULL);

ALTER TABLE product_variant_attribute_values
    ADD CONSTRAINT chk_product_variant_attribute_values_one_scalar
    CHECK (num_nonnulls(
        value_text, value_integer, value_decimal, value_boolean, value_date, value_datetime, value_json
    ) <= 1),
    ADD CONSTRAINT chk_product_variant_attribute_values_detached_storage
    CHECK (detached_at IS NULL);

CREATE OR REPLACE FUNCTION rustok_product_validate_attribute_value()
RETURNS TRIGGER AS $$
DECLARE
    attribute_type VARCHAR(32);
    attribute_localized BOOLEAN;
    scalar_count INTEGER;
BEGIN
    SELECT value_type, is_localized
      INTO attribute_type, attribute_localized
      FROM product_attributes
     WHERE tenant_id = NEW.tenant_id AND id = NEW.attribute_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'product attribute % is not owned by tenant %', NEW.attribute_id, NEW.tenant_id;
    END IF;

    scalar_count := num_nonnulls(
        NEW.value_text, NEW.value_integer, NEW.value_decimal, NEW.value_boolean,
        NEW.value_date, NEW.value_datetime, NEW.value_json
    );

    IF attribute_localized THEN
        IF attribute_type NOT IN ('text', 'textarea', 'richtext') OR scalar_count <> 0 THEN
            RAISE EXCEPTION 'localized attribute values must use a localized text row';
        END IF;
        RETURN NEW;
    END IF;

    IF attribute_type IN ('text', 'textarea', 'richtext') AND NEW.value_text IS NOT NULL AND scalar_count = 1 THEN
        RETURN NEW;
    ELSIF attribute_type = 'integer' AND NEW.value_integer IS NOT NULL AND scalar_count = 1 THEN
        RETURN NEW;
    ELSIF attribute_type = 'decimal' AND NEW.value_decimal IS NOT NULL AND scalar_count = 1 THEN
        RETURN NEW;
    ELSIF attribute_type = 'boolean' AND NEW.value_boolean IS NOT NULL AND scalar_count = 1 THEN
        RETURN NEW;
    ELSIF attribute_type = 'date' AND NEW.value_date IS NOT NULL AND scalar_count = 1 THEN
        RETURN NEW;
    ELSIF attribute_type = 'datetime' AND NEW.value_datetime IS NOT NULL AND scalar_count = 1 THEN
        RETURN NEW;
    ELSIF attribute_type = 'json' AND NEW.value_json IS NOT NULL AND scalar_count = 1 THEN
        RETURN NEW;
    ELSIF attribute_type IN ('select', 'multiselect') AND scalar_count = 0 THEN
        RETURN NEW;
    END IF;

    RAISE EXCEPTION 'stored product attribute value does not match attribute type %', attribute_type;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_product_attribute_values_validate_type
BEFORE INSERT OR UPDATE ON product_attribute_values
FOR EACH ROW EXECUTE FUNCTION rustok_product_validate_attribute_value();

CREATE TRIGGER trg_product_variant_attribute_values_validate_type
BEFORE INSERT OR UPDATE ON product_variant_attribute_values
FOR EACH ROW EXECUTE FUNCTION rustok_product_validate_attribute_value();

CREATE OR REPLACE FUNCTION rustok_product_validate_attribute_option()
RETURNS TRIGGER AS $$
DECLARE
    expected_attribute_id UUID;
    actual_attribute_id UUID;
    attribute_type VARCHAR(32);
    existing_count INTEGER;
BEGIN
    IF TG_TABLE_NAME = 'product_attribute_value_options' THEN
        SELECT attribute_id INTO expected_attribute_id
        FROM product_attribute_values
        WHERE tenant_id = NEW.tenant_id AND id = NEW.value_id;
    ELSE
        SELECT attribute_id INTO expected_attribute_id
        FROM product_variant_attribute_values
        WHERE tenant_id = NEW.tenant_id AND id = NEW.value_id;
    END IF;

    SELECT attribute_id INTO actual_attribute_id
    FROM product_attribute_options
    WHERE tenant_id = NEW.tenant_id AND id = NEW.option_id AND archived_at IS NULL;

    SELECT value_type INTO attribute_type
    FROM product_attributes
    WHERE tenant_id = NEW.tenant_id AND id = expected_attribute_id;

    IF expected_attribute_id IS NULL
       OR actual_attribute_id IS NULL
       OR actual_attribute_id <> expected_attribute_id
       OR attribute_type NOT IN ('select', 'multiselect') THEN
        RAISE EXCEPTION 'attribute option does not belong to the tenant-owned select attribute value';
    END IF;

    IF attribute_type = 'select' THEN
        PERFORM pg_advisory_xact_lock(hashtextextended(NEW.value_id::text, 0));
        IF TG_TABLE_NAME = 'product_attribute_value_options' THEN
            SELECT COUNT(*) INTO existing_count
            FROM product_attribute_value_options
            WHERE value_id = NEW.value_id AND option_id <> NEW.option_id;
        ELSE
            SELECT COUNT(*) INTO existing_count
            FROM product_variant_attribute_value_options
            WHERE value_id = NEW.value_id AND option_id <> NEW.option_id;
        END IF;
        IF existing_count > 0 THEN
            RAISE EXCEPTION 'select attributes can have exactly one option value';
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_product_attribute_value_options_validate
BEFORE INSERT OR UPDATE ON product_attribute_value_options
FOR EACH ROW EXECUTE FUNCTION rustok_product_validate_attribute_option();

CREATE TRIGGER trg_product_variant_attribute_value_options_validate
BEFORE INSERT OR UPDATE ON product_variant_attribute_value_options
FOR EACH ROW EXECUTE FUNCTION rustok_product_validate_attribute_option();
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // These constraints encode the target catalog contract and are intentionally irreversible.
        Ok(())
    }
}
