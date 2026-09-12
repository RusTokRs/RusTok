use rustok_api::TenantLocale;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use uuid::Uuid;

use crate::{
    PriceListTranslationExactLocaleError, PriceListTranslationExactLocaleResult,
    PriceListTranslationService,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PriceListTranslationExactProgressFacts {
    pub resources: u64,
    pub exact_required_units: u64,
    pub exact_optional_units: u64,
    pub complete_resources: u64,
}

#[derive(Debug, FromQueryResult)]
struct PriceListTranslationExactProgressRow {
    resources: i64,
    exact_required_units: i64,
    exact_optional_units: i64,
    complete_resources: i64,
}

impl PriceListTranslationService {
    pub(crate) async fn read_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> PriceListTranslationExactLocaleResult<PriceListTranslationExactProgressFacts> {
        if tenant_id.is_nil() {
            return Err(PriceListTranslationExactLocaleError::Validation(
                "Pricing translation progress tenant_id must not be nil".to_string(),
            ));
        }
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        if source_locale == target_locale {
            return Err(PriceListTranslationExactLocaleError::Validation(
                "Pricing translation progress source and target locale must differ".to_string(),
            ));
        }

        query_price_list_translation_exact_progress(
            self.database(),
            tenant_id,
            &source_locale,
            &target_locale,
        )
        .await
    }
}

async fn query_price_list_translation_exact_progress<C>(
    db: &C,
    tenant_id: Uuid,
    source_locale: &str,
    target_locale: &str,
) -> PriceListTranslationExactLocaleResult<PriceListTranslationExactProgressFacts>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => (
            PRICE_LIST_TRANSLATION_PROGRESS_POSTGRES_SQL,
            vec![
                tenant_id.into(),
                source_locale.to_string().into(),
                target_locale.to_string().into(),
            ],
        ),
        _ => (
            PRICE_LIST_TRANSLATION_PROGRESS_QUESTION_MARK_SQL,
            vec![
                target_locale.to_string().into(),
                tenant_id.into(),
                source_locale.to_string().into(),
            ],
        ),
    };
    let row = PriceListTranslationExactProgressRow::find_by_statement(
        Statement::from_sql_and_values(backend, sql, values),
    )
    .one(db)
    .await?
    .ok_or_else(|| {
        PriceListTranslationExactLocaleError::Validation(
            "Pricing translation progress aggregate returned no row".to_string(),
        )
    })?;

    let facts = PriceListTranslationExactProgressFacts {
        resources: progress_count(row.resources, "resources")?,
        exact_required_units: progress_count(row.exact_required_units, "exact required units")?,
        exact_optional_units: progress_count(row.exact_optional_units, "exact optional units")?,
        complete_resources: progress_count(row.complete_resources, "complete resources")?,
    };
    if facts.exact_required_units > facts.resources
        || facts.exact_optional_units > facts.resources
        || facts.complete_resources > facts.resources
    {
        return Err(PriceListTranslationExactLocaleError::Validation(
            "Pricing translation progress aggregate is internally inconsistent".to_string(),
        ));
    }
    Ok(facts)
}

fn canonical_locale(locale: &str) -> PriceListTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| PriceListTranslationExactLocaleError::Validation(error.to_string()))
}

fn progress_count(
    value: i64,
    field: &'static str,
) -> PriceListTranslationExactLocaleResult<u64> {
    u64::try_from(value).map_err(|_| {
        PriceListTranslationExactLocaleError::Validation(format!(
            "Pricing translation progress {field} must not be negative"
        ))
    })
}

const PRICE_LIST_TRANSLATION_PROGRESS_POSTGRES_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS exact_required_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.description, '')) <> '' THEN 1 END)
        AS exact_optional_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS complete_resources
FROM price_list_translations AS source_translation
INNER JOIN price_lists AS price_list
    ON price_list.id = source_translation.price_list_id
LEFT JOIN price_list_translations AS target_translation
    ON target_translation.price_list_id = source_translation.price_list_id
   AND target_translation.locale = $3
WHERE price_list.tenant_id = $1
  AND source_translation.locale = $2
"#;

const PRICE_LIST_TRANSLATION_PROGRESS_QUESTION_MARK_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS exact_required_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.description, '')) <> '' THEN 1 END)
        AS exact_optional_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS complete_resources
FROM price_list_translations AS source_translation
INNER JOIN price_lists AS price_list
    ON price_list.id = source_translation.price_list_id
LEFT JOIN price_list_translations AS target_translation
    ON target_translation.price_list_id = source_translation.price_list_id
   AND target_translation.locale = ?
WHERE price_list.tenant_id = ?
  AND source_translation.locale = ?
"#;
