use std::collections::BTreeMap;

use rustok_api::normalize_locale_tag;
use sea_orm::{ConnectionTrait, FromQueryResult, Statement};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Debug, FromQueryResult)]
struct VariantAttributeValueTranslationRow {
    id: Uuid,
    value_id: Uuid,
    locale: String,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product migrations require PostgreSQL".to_owned(),
            ));
        }

        let connection = manager.get_connection();
        let rows = VariantAttributeValueTranslationRow::find_by_statement(
            Statement::from_string(
                connection.get_database_backend(),
                r#"
SELECT id, value_id, locale
FROM product_variant_attribute_value_translations
ORDER BY value_id, locale, id
"#
                .to_string(),
            ),
        )
        .all(connection)
        .await?;

        let mut normalized_owners = BTreeMap::<(Uuid, String), Uuid>::new();
        let mut normalized_rows = Vec::with_capacity(rows.len());
        for row in rows {
            let normalized_locale = normalize_locale_tag(&row.locale).ok_or_else(|| {
                DbErr::Migration(format!(
                    "product variant attribute value translation {} for value {} has invalid locale {:?}",
                    row.id, row.value_id, row.locale
                ))
            })?;
            let key = (row.value_id, normalized_locale.clone());
            if let Some(existing_id) = normalized_owners.get(&key) {
                return Err(DbErr::Migration(format!(
                    "product variant attribute value locale normalization collision for value {} locale {} between translations {} and {}",
                    row.value_id, normalized_locale, existing_id, row.id
                )));
            }
            normalized_owners.insert(key, row.id);
            normalized_rows.push((row.id, row.locale, normalized_locale));
        }

        for (id, stored_locale, normalized_locale) in normalized_rows {
            if stored_locale == normalized_locale {
                continue;
            }
            connection
                .execute_raw(Statement::from_sql_and_values(
                    connection.get_database_backend(),
                    "UPDATE product_variant_attribute_value_translations SET locale = $1 WHERE id = $2",
                    vec![normalized_locale.into(), id.into()],
                ))
                .await?;
        }

        connection
            .execute_unprepared(
                r#"
ALTER TABLE product_variant_attribute_values
    ADD COLUMN translation_revision BIGINT NOT NULL DEFAULT 1,
    ADD CONSTRAINT chk_product_variant_attribute_values_translation_revision_positive
        CHECK (translation_revision > 0);

CREATE OR REPLACE FUNCTION rustok_product_touch_variant_attribute_value_translation_revision()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE product_variant_attribute_values
        SET translation_revision = translation_revision + 1,
            updated_at = clock_timestamp()
        WHERE id = NEW.value_id;
    ELSIF OLD.value_text IS DISTINCT FROM NEW.value_text THEN
        UPDATE product_variant_attribute_values
        SET translation_revision = translation_revision + 1,
            updated_at = clock_timestamp()
        WHERE id = NEW.value_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_product_variant_attribute_value_translation_revision
AFTER INSERT OR UPDATE OF value_text ON product_variant_attribute_value_translations
FOR EACH ROW
EXECUTE FUNCTION rustok_product_touch_variant_attribute_value_translation_revision();

CREATE TABLE product_variant_attribute_value_translation_change_journal (
    change_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    root_event_id UUID NOT NULL,
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    variant_id UUID NOT NULL,
    value_id UUID NOT NULL,
    resource_revision VARCHAR(128) NOT NULL,
    lifecycle VARCHAR(16) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_product_variant_attribute_value_translation_change_root_target
        UNIQUE (root_event_id, value_id),
    CONSTRAINT chk_product_variant_attribute_value_translation_change_seq_positive
        CHECK (change_seq > 0),
    CONSTRAINT chk_product_variant_attribute_value_translation_change_root_event_non_nil
        CHECK (root_event_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_variant_attribute_value_translation_change_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_variant_attribute_value_translation_change_product_non_nil
        CHECK (product_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_variant_attribute_value_translation_change_variant_non_nil
        CHECK (variant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_variant_attribute_value_translation_change_value_non_nil
        CHECK (value_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_variant_attribute_value_translation_change_revision_nonblank
        CHECK (length(trim(resource_revision)) > 0),
    CONSTRAINT chk_product_variant_attribute_value_translation_change_lifecycle
        CHECK (lifecycle IN ('active', 'deleted'))
);

CREATE INDEX idx_product_variant_attribute_value_translation_change_tenant_seq
    ON product_variant_attribute_value_translation_change_journal (tenant_id, change_seq);

CREATE INDEX idx_product_variant_attribute_value_translation_change_owner_target_seq
    ON product_variant_attribute_value_translation_change_journal (
        tenant_id,
        product_id,
        variant_id,
        value_id,
        change_seq DESC
    );

WITH live AS (
    SELECT
        pvav.id AS value_id,
        pvav.tenant_id,
        pv.product_id,
        pvav.variant_id,
        pvav.translation_revision,
        md5('rustok-product/variant-attribute-value-translation-backfill/v1:' || pvav.id::text) AS digest
    FROM product_variant_attribute_values pvav
    INNER JOIN product_variants pv
        ON pv.id = pvav.variant_id
       AND pv.tenant_id = pvav.tenant_id
    INNER JOIN product_attributes pa
        ON pa.id = pvav.attribute_id
       AND pa.tenant_id = pvav.tenant_id
    WHERE pa.is_localized = TRUE
      AND pa.value_type IN ('text', 'textarea', 'richtext')
      AND EXISTS (
          SELECT 1
          FROM product_variant_attribute_value_translations pvavt
          WHERE pvavt.value_id = pvav.id
      )
)
INSERT INTO product_variant_attribute_value_translation_change_journal (
    root_event_id,
    tenant_id,
    product_id,
    variant_id,
    value_id,
    resource_revision,
    lifecycle,
    created_at
)
SELECT
    (
        substr(digest, 1, 8) || '-' ||
        substr(digest, 9, 4) || '-' ||
        substr(digest, 13, 4) || '-' ||
        substr(digest, 17, 4) || '-' ||
        substr(digest, 21, 12)
    )::uuid,
    tenant_id,
    product_id,
    variant_id,
    value_id,
    'product-variant-attribute-value-resource-v1:' || translation_revision::text,
    'active',
    CURRENT_TIMESTAMP
FROM live
ORDER BY value_id;
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
DROP TABLE IF EXISTS product_variant_attribute_value_translation_change_journal;
DROP TRIGGER IF EXISTS trg_product_variant_attribute_value_translation_revision
    ON product_variant_attribute_value_translations;
DROP FUNCTION IF EXISTS rustok_product_touch_variant_attribute_value_translation_revision();
ALTER TABLE product_variant_attribute_values
    DROP CONSTRAINT IF EXISTS chk_product_variant_attribute_values_translation_revision_positive,
    DROP COLUMN IF EXISTS translation_revision;
"#,
            )
            .await?;
        Ok(())
    }
}
