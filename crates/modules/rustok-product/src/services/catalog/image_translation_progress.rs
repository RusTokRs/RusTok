use super::*;

use rustok_api::TenantLocale;
use sea_orm::{DatabaseBackend, FromQueryResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProductImageTranslationExactProgressFacts {
    pub resources: u64,
    pub exact_optional_units: u64,
}

#[derive(Debug, FromQueryResult)]
struct ProductImageTranslationExactProgressRow {
    resources: i64,
    exact_optional_units: i64,
}

impl CatalogService {
    /// Aggregates Product-owned exact-locale Image translation progress in one
    /// database statement.
    ///
    /// Inventory matches the neutral `product/image` provider: only Images under
    /// non-archived tenant-owned Products with an exact source locale row
    /// participate. Image alt text is nullable in the owner model, therefore it
    /// is an optional translation unit; NULL, empty, and whitespace-only exact
    /// target alt text do not count as translated.
    pub(crate) async fn read_product_image_translation_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> ProductImageTranslationExactLocaleResult<ProductImageTranslationExactProgressFacts> {
        let source_locale = canonical_progress_locale(source_locale)?;
        let target_locale = canonical_progress_locale(target_locale)?;
        if source_locale == target_locale {
            return Err(CommerceError::Validation(
                "Product image translation source and target locale must differ".to_string(),
            )
            .into());
        }

        query_product_image_translation_exact_progress(
            &self.db,
            tenant_id,
            &source_locale,
            &target_locale,
        )
        .await
    }
}

async fn query_product_image_translation_exact_progress<C>(
    db: &C,
    tenant_id: Uuid,
    source_locale: &str,
    target_locale: &str,
) -> ProductImageTranslationExactLocaleResult<ProductImageTranslationExactProgressFacts>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => (
            PRODUCT_IMAGE_TRANSLATION_PROGRESS_POSTGRES_SQL,
            vec![
                tenant_id.into(),
                source_locale.to_owned().into(),
                target_locale.to_owned().into(),
            ],
        ),
        _ => (
            PRODUCT_IMAGE_TRANSLATION_PROGRESS_QUESTION_MARK_SQL,
            vec![
                target_locale.to_owned().into(),
                tenant_id.into(),
                source_locale.to_owned().into(),
            ],
        ),
    };
    let statement = Statement::from_sql_and_values(backend, sql, values);
    let row = ProductImageTranslationExactProgressRow::find_by_statement(statement)
        .one(db)
        .await?
        .ok_or_else(|| {
            CommerceError::Validation(
                "Product Image translation progress aggregate returned no row".to_string(),
            )
        })?;

    let resources = progress_count(row.resources, "resources")?;
    let exact_optional_units = progress_count(row.exact_optional_units, "exact optional units")?;
    if exact_optional_units > resources {
        return Err(CommerceError::Validation(
            "Product Image translation progress aggregate violated owner bounds".to_string(),
        )
        .into());
    }

    Ok(ProductImageTranslationExactProgressFacts {
        resources,
        exact_optional_units,
    })
}

fn canonical_progress_locale(
    locale: &str,
) -> ProductImageTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| CommerceError::Validation(error.to_string()).into())
}

fn progress_count(
    value: i64,
    field: &'static str,
) -> ProductImageTranslationExactLocaleResult<u64> {
    u64::try_from(value).map_err(|_| {
        CommerceError::Validation(format!(
            "Product Image translation progress {field} must not be negative"
        ))
        .into()
    })
}

const PRODUCT_IMAGE_TRANSLATION_PROGRESS_POSTGRES_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.alt_text, '')) <> '' THEN 1 END)
        AS exact_optional_units
FROM product_image_translations AS source_translation
INNER JOIN product_images AS image
    ON image.id = source_translation.image_id
INNER JOIN products AS product
    ON product.id = image.product_id
LEFT JOIN product_image_translations AS target_translation
    ON target_translation.image_id = source_translation.image_id
   AND target_translation.locale = $3
WHERE product.tenant_id = $1
  AND source_translation.locale = $2
  AND product.status <> 'archived'
"#;

const PRODUCT_IMAGE_TRANSLATION_PROGRESS_QUESTION_MARK_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.alt_text, '')) <> '' THEN 1 END)
        AS exact_optional_units
FROM product_image_translations AS source_translation
INNER JOIN product_images AS image
    ON image.id = source_translation.image_id
INNER JOIN products AS product
    ON product.id = image.product_id
LEFT JOIN product_image_translations AS target_translation
    ON target_translation.image_id = source_translation.image_id
   AND target_translation.locale = ?
WHERE product.tenant_id = ?
  AND source_translation.locale = ?
  AND product.status <> 'archived'
"#;
