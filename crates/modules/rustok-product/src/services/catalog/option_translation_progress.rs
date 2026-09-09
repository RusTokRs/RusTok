use super::*;

use rustok_api::TenantLocale;
use sea_orm::{DbBackend, FromQueryResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProductOptionTranslationExactProgressFacts {
    pub resources: u64,
    pub required_units: u64,
    pub exact_required_units: u64,
    pub complete_resources: u64,
}

#[derive(Debug, FromQueryResult)]
struct ProductOptionTranslationExactProgressRow {
    resources: i64,
    source_value_units: i64,
    target_title_exact_units: i64,
    target_value_exact_units: i64,
    complete_resources: i64,
    invalid_partial_option_id: Option<Uuid>,
}

impl CatalogService {
    /// Aggregates exact-locale Product Option translation progress in one owner statement.
    ///
    /// Inventory matches `product/option`: a non-archived Option participates only when
    /// its exact source title exists and every current value has an exact source row.
    /// One required unit is the Option title and one required unit belongs to every
    /// current Option Value. Blank target strings remain exact rows structurally, but do
    /// not count as translated units. Structurally partial target aggregates fail closed.
    pub(crate) async fn read_product_option_translation_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> ProductOptionTranslationExactLocaleResult<ProductOptionTranslationExactProgressFacts> {
        let source_locale = canonical_progress_locale(source_locale)?;
        let target_locale = canonical_progress_locale(target_locale)?;
        if source_locale == target_locale {
            return Err(CommerceError::Validation(
                "Product option translation source and target locale must differ".to_string(),
            )
            .into());
        }
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(CommerceError::Validation(
                "Product option translation progress requires PostgreSQL".to_string(),
            )
            .into());
        }

        let row = ProductOptionTranslationExactProgressRow::find_by_statement(
            Statement::from_sql_and_values(
                DbBackend::Postgres,
                PRODUCT_OPTION_TRANSLATION_PROGRESS_SQL,
                vec![
                    tenant_id.into(),
                    source_locale.into(),
                    target_locale.clone().into(),
                ],
            ),
        )
        .one(&self.db)
        .await?
        .ok_or_else(|| {
            CommerceError::Validation(
                "Product Option translation progress aggregate returned no row".to_string(),
            )
        })?;

        if let Some(option_id) = row.invalid_partial_option_id {
            return Err(ProductOptionTranslationExactLocaleError::IncompleteLocale {
                option_id,
                locale: target_locale,
            });
        }

        let resources = progress_count(row.resources, "resources")?;
        let source_value_units = progress_count(row.source_value_units, "source value units")?;
        let target_title_exact_units =
            progress_count(row.target_title_exact_units, "target title exact units")?;
        let target_value_exact_units =
            progress_count(row.target_value_exact_units, "target value exact units")?;
        let required_units = resources.checked_add(source_value_units).ok_or_else(|| {
            CommerceError::Validation(
                "Product Option translation required unit count overflow".to_string(),
            )
        })?;
        let exact_required_units = target_title_exact_units
            .checked_add(target_value_exact_units)
            .ok_or_else(|| {
                CommerceError::Validation(
                    "Product Option translation exact required unit count overflow".to_string(),
                )
            })?;
        let complete_resources = progress_count(row.complete_resources, "complete resources")?;
        if exact_required_units > required_units || complete_resources > resources {
            return Err(CommerceError::Validation(
                "Product Option translation progress aggregate violated owner bounds".to_string(),
            )
            .into());
        }

        Ok(ProductOptionTranslationExactProgressFacts {
            resources,
            required_units,
            exact_required_units,
            complete_resources,
        })
    }
}

fn canonical_progress_locale(locale: &str) -> ProductOptionTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| CommerceError::Validation(error.to_string()).into())
}

fn progress_count(
    value: i64,
    field: &'static str,
) -> ProductOptionTranslationExactLocaleResult<u64> {
    u64::try_from(value).map_err(|_| {
        CommerceError::Validation(format!(
            "Product Option translation progress {field} must not be negative"
        ))
        .into()
    })
}

const PRODUCT_OPTION_TRANSLATION_PROGRESS_SQL: &str = r#"
WITH inventory AS (
    SELECT option_row.id
    FROM product_options AS option_row
    INNER JOIN products AS product
        ON product.id = option_row.product_id
    INNER JOIN product_option_translations AS source_title
        ON source_title.option_id = option_row.id
       AND source_title.locale = $2
    WHERE product.tenant_id = $1
      AND product.status <> 'archived'
      AND NOT EXISTS (
          SELECT 1
          FROM product_option_values AS source_option_value
          WHERE source_option_value.option_id = option_row.id
            AND NOT EXISTS (
                SELECT 1
                FROM product_option_value_translations AS source_value
                WHERE source_value.value_id = source_option_value.id
                  AND source_value.locale = $2
            )
      )
)
SELECT
    (SELECT COUNT(*) FROM inventory) AS resources,
    (
        SELECT COUNT(*)
        FROM product_option_values AS option_value
        INNER JOIN inventory ON inventory.id = option_value.option_id
    ) AS source_value_units,
    (
        SELECT COUNT(*)
        FROM inventory
        INNER JOIN product_option_translations AS target_title
            ON target_title.option_id = inventory.id
           AND target_title.locale = $3
        WHERE TRIM(target_title.title) <> ''
    ) AS target_title_exact_units,
    (
        SELECT COUNT(*)
        FROM product_option_values AS option_value
        INNER JOIN inventory ON inventory.id = option_value.option_id
        INNER JOIN product_option_value_translations AS target_value
            ON target_value.value_id = option_value.id
           AND target_value.locale = $3
        WHERE TRIM(target_value.value) <> ''
    ) AS target_value_exact_units,
    (
        SELECT COUNT(*)
        FROM inventory
        INNER JOIN product_option_translations AS target_title
            ON target_title.option_id = inventory.id
           AND target_title.locale = $3
        WHERE TRIM(target_title.title) <> ''
          AND NOT EXISTS (
              SELECT 1
              FROM product_option_values AS option_value
              WHERE option_value.option_id = inventory.id
                AND NOT EXISTS (
                    SELECT 1
                    FROM product_option_value_translations AS target_value
                    WHERE target_value.value_id = option_value.id
                      AND target_value.locale = $3
                      AND TRIM(target_value.value) <> ''
                )
          )
    ) AS complete_resources,
    (
        SELECT inventory.id
        FROM inventory
        WHERE (
            EXISTS (
                SELECT 1
                FROM product_option_translations AS target_title
                WHERE target_title.option_id = inventory.id
                  AND target_title.locale = $3
            )
            OR EXISTS (
                SELECT 1
                FROM product_option_values AS option_value
                INNER JOIN product_option_value_translations AS target_value
                    ON target_value.value_id = option_value.id
                   AND target_value.locale = $3
                WHERE option_value.option_id = inventory.id
            )
        )
        AND (
            NOT EXISTS (
                SELECT 1
                FROM product_option_translations AS target_title
                WHERE target_title.option_id = inventory.id
                  AND target_title.locale = $3
            )
            OR EXISTS (
                SELECT 1
                FROM product_option_values AS option_value
                WHERE option_value.option_id = inventory.id
                  AND NOT EXISTS (
                      SELECT 1
                      FROM product_option_value_translations AS target_value
                      WHERE target_value.value_id = option_value.id
                        AND target_value.locale = $3
                  )
            )
        )
        ORDER BY inventory.id
        LIMIT 1
    ) AS invalid_partial_option_id
"#;
