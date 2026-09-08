use super::*;

use rustok_api::TenantLocale;
use sea_orm::{DatabaseBackend, FromQueryResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProductVariantTranslationExactProgressFacts {
    pub resources: u64,
    pub exact_optional_units: u64,
}

#[derive(Debug, FromQueryResult)]
struct ProductVariantTranslationExactProgressRow {
    resources: i64,
    exact_optional_units: i64,
}

impl CatalogService {
    /// Aggregates Product-owned exact-locale Variant translation progress in one
    /// database statement.
    ///
    /// The inventory matches the neutral `product/variant` provider: only
    /// Variants under non-archived Products with the exact source locale
    /// participate. Variant title is nullable in the owner model, therefore it is
    /// an optional translation unit; whitespace-only exact target titles do not
    /// count as translated.
    pub(crate) async fn read_product_variant_translation_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> ProductVariantTranslationExactLocaleResult<ProductVariantTranslationExactProgressFacts>
    {
        let source_locale = canonical_progress_locale(source_locale)?;
        let target_locale = canonical_progress_locale(target_locale)?;
        if source_locale == target_locale {
            return Err(CommerceError::Validation(
                "Product variant translation source and target locale must differ".to_string(),
            )
            .into());
        }

        query_product_variant_translation_exact_progress(
            &self.db,
            tenant_id,
            &source_locale,
            &target_locale,
        )
        .await
    }
}

async fn query_product_variant_translation_exact_progress<C>(
    db: &C,
    tenant_id: Uuid,
    source_locale: &str,
    target_locale: &str,
) -> ProductVariantTranslationExactLocaleResult<ProductVariantTranslationExactProgressFacts>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => (
            PRODUCT_VARIANT_TRANSLATION_PROGRESS_POSTGRES_SQL,
            vec![
                tenant_id.into(),
                source_locale.to_owned().into(),
                target_locale.to_owned().into(),
            ],
        ),
        _ => (
            PRODUCT_VARIANT_TRANSLATION_PROGRESS_QUESTION_MARK_SQL,
            vec![
                target_locale.to_owned().into(),
                tenant_id.into(),
                source_locale.to_owned().into(),
            ],
        ),
    };
    let statement = Statement::from_sql_and_values(backend, sql, values);
    let row = ProductVariantTranslationExactProgressRow::find_by_statement(statement)
        .one(db)
        .await?
        .ok_or_else(|| {
            CommerceError::Validation(
                "Product Variant translation progress aggregate returned no row".to_string(),
            )
        })?;

    Ok(ProductVariantTranslationExactProgressFacts {
        resources: progress_count(row.resources, "resources")?,
        exact_optional_units: progress_count(row.exact_optional_units, "exact optional units")?,
    })
}

fn canonical_progress_locale(
    locale: &str,
) -> ProductVariantTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| CommerceError::Validation(error.to_string()).into())
}

fn progress_count(
    value: i64,
    field: &'static str,
) -> ProductVariantTranslationExactLocaleResult<u64> {
    u64::try_from(value).map_err(|_| {
        CommerceError::Validation(format!(
            "Product Variant translation progress {field} must not be negative"
        ))
        .into()
    })
}

const PRODUCT_VARIANT_TRANSLATION_PROGRESS_POSTGRES_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.title, '')) <> '' THEN 1 END)
        AS exact_optional_units
FROM product_variant_translations AS source_translation
INNER JOIN product_variants AS variant
    ON variant.id = source_translation.variant_id
INNER JOIN products AS product
    ON product.id = variant.product_id
   AND product.tenant_id = variant.tenant_id
LEFT JOIN product_variant_translations AS target_translation
    ON target_translation.variant_id = source_translation.variant_id
   AND target_translation.locale = $3
WHERE variant.tenant_id = $1
  AND source_translation.locale = $2
  AND product.status <> 'archived'
"#;

const PRODUCT_VARIANT_TRANSLATION_PROGRESS_QUESTION_MARK_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.title, '')) <> '' THEN 1 END)
        AS exact_optional_units
FROM product_variant_translations AS source_translation
INNER JOIN product_variants AS variant
    ON variant.id = source_translation.variant_id
INNER JOIN products AS product
    ON product.id = variant.product_id
   AND product.tenant_id = variant.tenant_id
LEFT JOIN product_variant_translations AS target_translation
    ON target_translation.variant_id = source_translation.variant_id
   AND target_translation.locale = ?
WHERE variant.tenant_id = ?
  AND source_translation.locale = ?
  AND product.status <> 'archived'
"#;
