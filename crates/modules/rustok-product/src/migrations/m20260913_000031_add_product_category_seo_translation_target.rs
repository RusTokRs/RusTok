use std::collections::BTreeMap;

use rustok_api::normalize_locale_tag;
use sea_orm::{ConnectionTrait, FromQueryResult, Statement};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Debug, FromQueryResult)]
struct CategorySeoTranslationRow {
    tenant_id: Uuid,
    category_id: Uuid,
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
        let rows = CategorySeoTranslationRow::find_by_statement(Statement::from_string(
            DatabaseBackend::Postgres,
            r#"
SELECT tenant_id, category_id, locale
FROM catalog_category_seo_translations
ORDER BY tenant_id, category_id, locale
"#
            .to_string(),
        ))
        .all(connection)
        .await?;

        let mut normalized_owners = BTreeMap::<(Uuid, Uuid, String), String>::new();
        let mut normalized_rows = Vec::with_capacity(rows.len());
        for row in rows {
            let normalized_locale = normalize_locale_tag(&row.locale).ok_or_else(|| {
                DbErr::Migration(format!(
                    "Product Category SEO translation for tenant {} category {} has invalid locale {:?}",
                    row.tenant_id, row.category_id, row.locale
                ))
            })?;
            let key = (row.tenant_id, row.category_id, normalized_locale.clone());
            if let Some(existing_locale) = normalized_owners.get(&key) {
                return Err(DbErr::Migration(format!(
                    "Product Category SEO locale normalization collision for tenant {} category {} locale {} between stored locales {:?} and {:?}",
                    row.tenant_id, row.category_id, normalized_locale, existing_locale, row.locale
                )));
            }
            normalized_owners.insert(key, row.locale.clone());
            normalized_rows.push((
                row.tenant_id,
                row.category_id,
                row.locale,
                normalized_locale,
            ));
        }

        for (tenant_id, category_id, stored_locale, normalized_locale) in normalized_rows {
            if stored_locale == normalized_locale {
                continue;
            }
            connection
                .execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    r#"
UPDATE catalog_category_seo_translations
SET locale = $1
WHERE tenant_id = $2 AND category_id = $3 AND locale = $4
"#,
                    vec![
                        normalized_locale.into(),
                        tenant_id.into(),
                        category_id.into(),
                        stored_locale.into(),
                    ],
                ))
                .await?;
        }

        connection
            .execute_unprepared(
                r#"
ALTER TABLE catalog_categories
    ADD COLUMN category_seo_translation_revision BIGINT NOT NULL DEFAULT 1,
    ADD CONSTRAINT chk_catalog_categories_category_seo_translation_revision_positive
        CHECK (category_seo_translation_revision > 0);

CREATE OR REPLACE FUNCTION rustok_product_touch_category_seo_translation_revision()
RETURNS TRIGGER AS $$
DECLARE
    owner_tenant_id UUID;
    owner_category_id UUID;
BEGIN
    owner_tenant_id := COALESCE(NEW.tenant_id, OLD.tenant_id);
    owner_category_id := COALESCE(NEW.category_id, OLD.category_id);

    IF TG_OP = 'INSERT'
       OR TG_OP = 'DELETE'
       OR OLD.meta_title IS DISTINCT FROM NEW.meta_title
       OR OLD.meta_description IS DISTINCT FROM NEW.meta_description THEN
        UPDATE catalog_categories
        SET category_seo_translation_revision = category_seo_translation_revision + 1,
            updated_at = clock_timestamp()
        WHERE tenant_id = owner_tenant_id AND id = owner_category_id;
    END IF;

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_product_category_seo_translation_revision
AFTER INSERT OR UPDATE OF meta_title, meta_description OR DELETE
ON catalog_category_seo_translations
FOR EACH ROW
EXECUTE FUNCTION rustok_product_touch_category_seo_translation_revision();

CREATE TABLE product_category_seo_translation_change_journal (
    change_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    root_event_id UUID NOT NULL,
    tenant_id UUID NOT NULL,
    category_id UUID NOT NULL,
    resource_revision VARCHAR(128) NOT NULL,
    lifecycle VARCHAR(16) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_product_category_seo_translation_change_root_target
        UNIQUE (root_event_id, category_id),
    CONSTRAINT chk_product_category_seo_translation_change_seq_positive
        CHECK (change_seq > 0),
    CONSTRAINT chk_product_category_seo_translation_change_root_event_non_nil
        CHECK (root_event_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_category_seo_translation_change_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_category_seo_translation_change_category_non_nil
        CHECK (category_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_category_seo_translation_change_revision_nonblank
        CHECK (length(trim(resource_revision)) > 0),
    CONSTRAINT chk_product_category_seo_translation_change_lifecycle
        CHECK (lifecycle IN ('active', 'archived', 'deleted'))
);

CREATE INDEX idx_product_category_seo_translation_change_tenant_seq
    ON product_category_seo_translation_change_journal (tenant_id, change_seq);

CREATE INDEX idx_product_category_seo_translation_change_category_seq
    ON product_category_seo_translation_change_journal (
        tenant_id,
        category_id,
        change_seq DESC
    );

WITH live AS (
    SELECT
        category.tenant_id,
        category.id AS category_id,
        category.category_seo_translation_revision,
        CASE
            WHEN category.deleted_at IS NOT NULL THEN 'deleted'
            WHEN category.is_active THEN 'active'
            ELSE 'archived'
        END AS lifecycle,
        md5('rustok-product/category-seo-translation-backfill/v1:' || category.tenant_id::text || ':' || category.id::text) AS digest
    FROM catalog_categories category
    WHERE EXISTS (
        SELECT 1
        FROM catalog_category_seo_translations seo
        WHERE seo.tenant_id = category.tenant_id
          AND seo.category_id = category.id
          AND (
              NULLIF(BTRIM(seo.meta_title), '') IS NOT NULL
              OR NULLIF(BTRIM(seo.meta_description), '') IS NOT NULL
          )
    )
)
INSERT INTO product_category_seo_translation_change_journal (
    root_event_id,
    tenant_id,
    category_id,
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
    category_id,
    'product-category-seo-resource-v1:' || category_seo_translation_revision::text,
    lifecycle,
    CURRENT_TIMESTAMP
FROM live
ORDER BY tenant_id, category_id;
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
DROP TABLE IF EXISTS product_category_seo_translation_change_journal;
DROP TRIGGER IF EXISTS trg_product_category_seo_translation_revision
    ON catalog_category_seo_translations;
DROP FUNCTION IF EXISTS rustok_product_touch_category_seo_translation_revision();
ALTER TABLE catalog_categories
    DROP CONSTRAINT IF EXISTS chk_catalog_categories_category_seo_translation_revision_positive,
    DROP COLUMN IF EXISTS category_seo_translation_revision;
"#,
            )
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_normalization_matches_shared_platform_contract() {
        assert_eq!(normalize_locale_tag(" EN_us ").as_deref(), Some("en-US"));
        assert_eq!(normalize_locale_tag("ru_RU").as_deref(), Some("ru-RU"));
        assert!(normalize_locale_tag(" ").is_none());
    }
}
