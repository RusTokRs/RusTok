use super::translation::{canonical_translation_locale, validate_locale_pair};
use super::*;

use sea_orm::{DatabaseBackend, FromQueryResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProductTranslationExactProgressFacts {
    pub resources: u64,
    pub exact_required_units: u64,
    pub exact_optional_units: u64,
    pub complete_resources: u64,
}

#[derive(Debug, FromQueryResult)]
struct ProductTranslationExactProgressRow {
    resources: i64,
    exact_required_units: i64,
    exact_optional_units: i64,
    complete_resources: i64,
}

impl CatalogService {
    /// Aggregates Product-owned exact-locale Translation progress in one database statement.
    ///
    /// The source inventory matches `list_product_translation_exact_resources`: only
    /// non-archived Products with the exact source locale participate. Target facts are
    /// exact-locale only and whitespace-only values do not count as translated units.
    /// A single statement gives the provider one database snapshot without requiring a
    /// Product change journal; change-cursor onboarding remains a separate owner slice.
    pub(crate) async fn read_product_translation_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> ProductTranslationExactLocaleResult<ProductTranslationExactProgressFacts> {
        let source_locale = canonical_translation_locale(source_locale)?;
        let target_locale = canonical_translation_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;

        query_product_translation_exact_progress(
            &self.db,
            tenant_id,
            &source_locale,
            &target_locale,
        )
        .await
    }
}

async fn query_product_translation_exact_progress<C>(
    db: &C,
    tenant_id: Uuid,
    source_locale: &str,
    target_locale: &str,
) -> ProductTranslationExactLocaleResult<ProductTranslationExactProgressFacts>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => (
            PRODUCT_TRANSLATION_PROGRESS_POSTGRES_SQL,
            vec![
                tenant_id.into(),
                source_locale.to_owned().into(),
                target_locale.to_owned().into(),
            ],
        ),
        _ => (
            PRODUCT_TRANSLATION_PROGRESS_QUESTION_MARK_SQL,
            vec![
                target_locale.to_owned().into(),
                tenant_id.into(),
                source_locale.to_owned().into(),
            ],
        ),
    };
    let statement = Statement::from_sql_and_values(backend, sql, values);
    let row = ProductTranslationExactProgressRow::find_by_statement(statement)
        .one(db)
        .await?
        .ok_or_else(|| {
            CommerceError::Validation(
                "Product translation progress aggregate returned no row".to_string(),
            )
        })?;

    Ok(ProductTranslationExactProgressFacts {
        resources: progress_count(row.resources, "resources")?,
        exact_required_units: progress_count(row.exact_required_units, "exact required units")?,
        exact_optional_units: progress_count(row.exact_optional_units, "exact optional units")?,
        complete_resources: progress_count(row.complete_resources, "complete resources")?,
    })
}

fn progress_count(value: i64, field: &'static str) -> ProductTranslationExactLocaleResult<u64> {
    u64::try_from(value).map_err(|_| {
        CommerceError::Validation(format!(
            "Product translation progress {field} must not be negative"
        ))
        .into()
    })
}

const PRODUCT_TRANSLATION_PROGRESS_POSTGRES_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.title, '')) <> '' THEN 1 END)
        + COUNT(CASE WHEN TRIM(COALESCE(target_translation.handle, '')) <> '' THEN 1 END)
        AS exact_required_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.description, '')) <> '' THEN 1 END)
        + COUNT(CASE WHEN TRIM(COALESCE(target_translation.meta_title, '')) <> '' THEN 1 END)
        + COUNT(CASE WHEN TRIM(COALESCE(target_translation.meta_description, '')) <> '' THEN 1 END)
        AS exact_optional_units,
    COUNT(CASE
        WHEN TRIM(COALESCE(target_translation.title, '')) <> ''
         AND TRIM(COALESCE(target_translation.handle, '')) <> ''
        THEN 1
    END) AS complete_resources
FROM product_translations AS source_translation
INNER JOIN products AS product
    ON product.id = source_translation.product_id
   AND product.tenant_id = source_translation.tenant_id
LEFT JOIN product_translations AS target_translation
    ON target_translation.product_id = source_translation.product_id
   AND target_translation.tenant_id = source_translation.tenant_id
   AND target_translation.locale = $3
WHERE source_translation.tenant_id = $1
  AND source_translation.locale = $2
  AND product.status <> 'archived'
"#;

const PRODUCT_TRANSLATION_PROGRESS_QUESTION_MARK_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.title, '')) <> '' THEN 1 END)
        + COUNT(CASE WHEN TRIM(COALESCE(target_translation.handle, '')) <> '' THEN 1 END)
        AS exact_required_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.description, '')) <> '' THEN 1 END)
        + COUNT(CASE WHEN TRIM(COALESCE(target_translation.meta_title, '')) <> '' THEN 1 END)
        + COUNT(CASE WHEN TRIM(COALESCE(target_translation.meta_description, '')) <> '' THEN 1 END)
        AS exact_optional_units,
    COUNT(CASE
        WHEN TRIM(COALESCE(target_translation.title, '')) <> ''
         AND TRIM(COALESCE(target_translation.handle, '')) <> ''
        THEN 1
    END) AS complete_resources
FROM product_translations AS source_translation
INNER JOIN products AS product
    ON product.id = source_translation.product_id
   AND product.tenant_id = source_translation.tenant_id
LEFT JOIN product_translations AS target_translation
    ON target_translation.product_id = source_translation.product_id
   AND target_translation.tenant_id = source_translation.tenant_id
   AND target_translation.locale = ?
WHERE source_translation.tenant_id = ?
  AND source_translation.locale = ?
  AND product.status <> 'archived'
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    const TENANT_ID: &str = "00000000-0000-0000-0000-000000000001";

    #[tokio::test]
    async fn aggregate_counts_only_active_exact_source_products_and_nonblank_targets() {
        let db = Database::connect("sqlite::memory:").await.expect("sqlite");
        db.execute_unprepared(
            r#"
CREATE TABLE products (
    id TEXT PRIMARY KEY,
    tenant_id BLOB NOT NULL,
    status TEXT NOT NULL
);
CREATE TABLE product_translations (
    id TEXT PRIMARY KEY,
    product_id TEXT NOT NULL,
    tenant_id BLOB NOT NULL,
    locale TEXT NOT NULL,
    title TEXT NOT NULL,
    handle TEXT NOT NULL,
    description TEXT NULL,
    meta_title TEXT NULL,
    meta_description TEXT NULL
);

INSERT INTO products (id, tenant_id, status) VALUES
('00000000-0000-0000-0000-000000000011', X'00000000000000000000000000000001', 'active'),
('00000000-0000-0000-0000-000000000012', X'00000000000000000000000000000001', 'draft'),
('00000000-0000-0000-0000-000000000013', X'00000000000000000000000000000001', 'archived'),
('00000000-0000-0000-0000-000000000014', X'00000000000000000000000000000001', 'active');

INSERT INTO product_translations
(id, product_id, tenant_id, locale, title, handle, description, meta_title, meta_description)
VALUES
('00000000-0000-0000-0000-000000000101', '00000000-0000-0000-0000-000000000011', X'00000000000000000000000000000001', 'en', 'One', 'one', NULL, NULL, NULL),
('00000000-0000-0000-0000-000000000102', '00000000-0000-0000-0000-000000000011', X'00000000000000000000000000000001', 'fr', 'Un', 'un', 'Description', '   ', NULL),
('00000000-0000-0000-0000-000000000103', '00000000-0000-0000-0000-000000000012', X'00000000000000000000000000000001', 'en', 'Two', 'two', NULL, NULL, NULL),
('00000000-0000-0000-0000-000000000104', '00000000-0000-0000-0000-000000000012', X'00000000000000000000000000000001', 'fr', '   ', 'deux', '', NULL, '   '),
('00000000-0000-0000-0000-000000000105', '00000000-0000-0000-0000-000000000013', X'00000000000000000000000000000001', 'en', 'Archived', 'archived', NULL, NULL, NULL),
('00000000-0000-0000-0000-000000000106', '00000000-0000-0000-0000-000000000013', X'00000000000000000000000000000001', 'fr', 'Archive', 'archive', 'ignored', 'ignored', 'ignored'),
('00000000-0000-0000-0000-000000000107', '00000000-0000-0000-0000-000000000014', X'00000000000000000000000000000001', 'fr', 'Target only', 'target-only', 'ignored', 'ignored', 'ignored');
"#,
        )
        .await
        .expect("fixtures");

        let facts = query_product_translation_exact_progress(
            &db,
            Uuid::parse_str(TENANT_ID).expect("tenant uuid"),
            "en",
            "fr",
        )
        .await
        .expect("progress");

        assert_eq!(facts.resources, 2);
        assert_eq!(facts.exact_required_units, 3);
        assert_eq!(facts.exact_optional_units, 1);
        assert_eq!(facts.complete_resources, 1);
    }
}
