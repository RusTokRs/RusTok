use rustok_api::TenantLocale;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use uuid::Uuid;

use crate::{
    StockLocationTranslationExactLocaleError, StockLocationTranslationExactLocaleResult,
    StockLocationTranslationService,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StockLocationTranslationExactProgressFacts {
    pub resources: u64,
    pub exact_required_units: u64,
    pub complete_resources: u64,
}

#[derive(Debug, FromQueryResult)]
struct StockLocationTranslationExactProgressRow {
    resources: i64,
    exact_required_units: i64,
    complete_resources: i64,
}

impl StockLocationTranslationService {
    pub(crate) async fn read_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> StockLocationTranslationExactLocaleResult<StockLocationTranslationExactProgressFacts> {
        if tenant_id.is_nil() {
            return Err(StockLocationTranslationExactLocaleError::Validation(
                "Inventory translation progress tenant_id must not be nil".to_string(),
            ));
        }
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        if source_locale == target_locale {
            return Err(StockLocationTranslationExactLocaleError::Validation(
                "Inventory translation progress source and target locale must differ".to_string(),
            ));
        }

        query_stock_location_translation_exact_progress(
            self.database(),
            tenant_id,
            &source_locale,
            &target_locale,
        )
        .await
    }
}

async fn query_stock_location_translation_exact_progress<C>(
    db: &C,
    tenant_id: Uuid,
    source_locale: &str,
    target_locale: &str,
) -> StockLocationTranslationExactLocaleResult<StockLocationTranslationExactProgressFacts>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => (
            STOCK_LOCATION_TRANSLATION_PROGRESS_POSTGRES_SQL,
            vec![
                tenant_id.into(),
                source_locale.to_string().into(),
                target_locale.to_string().into(),
            ],
        ),
        _ => (
            STOCK_LOCATION_TRANSLATION_PROGRESS_QUESTION_MARK_SQL,
            vec![
                target_locale.to_string().into(),
                tenant_id.into(),
                source_locale.to_string().into(),
            ],
        ),
    };
    let row = StockLocationTranslationExactProgressRow::find_by_statement(
        Statement::from_sql_and_values(backend, sql, values),
    )
    .one(db)
    .await?
    .ok_or_else(|| {
        StockLocationTranslationExactLocaleError::Validation(
            "Inventory translation progress aggregate returned no row".to_string(),
        )
    })?;

    let facts = StockLocationTranslationExactProgressFacts {
        resources: progress_count(row.resources, "resources")?,
        exact_required_units: progress_count(row.exact_required_units, "exact required units")?,
        complete_resources: progress_count(row.complete_resources, "complete resources")?,
    };
    if facts.exact_required_units > facts.resources || facts.complete_resources > facts.resources {
        return Err(StockLocationTranslationExactLocaleError::Validation(
            "Inventory translation progress aggregate is internally inconsistent".to_string(),
        ));
    }
    Ok(facts)
}

fn canonical_locale(locale: &str) -> StockLocationTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| StockLocationTranslationExactLocaleError::Validation(error.to_string()))
}

fn progress_count(
    value: i64,
    field: &'static str,
) -> StockLocationTranslationExactLocaleResult<u64> {
    u64::try_from(value).map_err(|_| {
        StockLocationTranslationExactLocaleError::Validation(format!(
            "Inventory translation progress {field} must not be negative"
        ))
    })
}

const STOCK_LOCATION_TRANSLATION_PROGRESS_POSTGRES_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS exact_required_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS complete_resources
FROM stock_location_translations AS source_translation
INNER JOIN stock_locations AS stock_location
    ON stock_location.id = source_translation.stock_location_id
LEFT JOIN stock_location_translations AS target_translation
    ON target_translation.stock_location_id = source_translation.stock_location_id
   AND target_translation.locale = $3
WHERE stock_location.tenant_id = $1
  AND stock_location.deleted_at IS NULL
  AND source_translation.locale = $2
"#;

const STOCK_LOCATION_TRANSLATION_PROGRESS_QUESTION_MARK_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS exact_required_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS complete_resources
FROM stock_location_translations AS source_translation
INNER JOIN stock_locations AS stock_location
    ON stock_location.id = source_translation.stock_location_id
LEFT JOIN stock_location_translations AS target_translation
    ON target_translation.stock_location_id = source_translation.stock_location_id
   AND target_translation.locale = ?
WHERE stock_location.tenant_id = ?
  AND stock_location.deleted_at IS NULL
  AND source_translation.locale = ?
"#;
