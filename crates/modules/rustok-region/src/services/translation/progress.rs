use rustok_api::TenantLocale;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use uuid::Uuid;

use crate::{RegionError, RegionTranslationExactLocaleResult, RegionTranslationService};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RegionTranslationExactProgressFacts {
    pub resources: u64,
    pub exact_required_units: u64,
    pub complete_resources: u64,
}

#[derive(Debug, FromQueryResult)]
struct RegionTranslationExactProgressRow {
    resources: i64,
    exact_required_units: i64,
    complete_resources: i64,
}

impl RegionTranslationService {
    pub(crate) async fn read_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> RegionTranslationExactLocaleResult<RegionTranslationExactProgressFacts> {
        if tenant_id.is_nil() {
            return Err(RegionError::Validation(
                "Region translation progress tenant_id must not be nil".to_string(),
            )
            .into());
        }
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        if source_locale == target_locale {
            return Err(RegionError::Validation(
                "Region translation progress source and target locale must differ".to_string(),
            )
            .into());
        }

        query_region_translation_exact_progress(
            self.database(),
            tenant_id,
            &source_locale,
            &target_locale,
        )
        .await
    }
}

async fn query_region_translation_exact_progress<C>(
    db: &C,
    tenant_id: Uuid,
    source_locale: &str,
    target_locale: &str,
) -> RegionTranslationExactLocaleResult<RegionTranslationExactProgressFacts>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => (
            REGION_TRANSLATION_PROGRESS_POSTGRES_SQL,
            vec![
                tenant_id.into(),
                source_locale.to_string().into(),
                target_locale.to_string().into(),
            ],
        ),
        _ => (
            REGION_TRANSLATION_PROGRESS_QUESTION_MARK_SQL,
            vec![
                target_locale.to_string().into(),
                tenant_id.into(),
                source_locale.to_string().into(),
            ],
        ),
    };
    let row = RegionTranslationExactProgressRow::find_by_statement(Statement::from_sql_and_values(
        backend, sql, values,
    ))
    .one(db)
    .await?
    .ok_or_else(|| {
        RegionError::Validation("Region translation progress aggregate returned no row".to_string())
    })?;

    let facts = RegionTranslationExactProgressFacts {
        resources: progress_count(row.resources, "resources")?,
        exact_required_units: progress_count(row.exact_required_units, "exact required units")?,
        complete_resources: progress_count(row.complete_resources, "complete resources")?,
    };
    if facts.exact_required_units > facts.resources || facts.complete_resources > facts.resources {
        return Err(RegionError::Validation(
            "Region translation progress aggregate is internally inconsistent".to_string(),
        )
        .into());
    }
    Ok(facts)
}

fn canonical_locale(locale: &str) -> RegionTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| RegionError::Validation(error.to_string()).into())
}

fn progress_count(value: i64, field: &'static str) -> RegionTranslationExactLocaleResult<u64> {
    u64::try_from(value).map_err(|_| {
        RegionError::Validation(format!(
            "Region translation progress {field} must not be negative"
        ))
        .into()
    })
}

const REGION_TRANSLATION_PROGRESS_POSTGRES_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS exact_required_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS complete_resources
FROM region_translations AS source_translation
INNER JOIN regions AS region
    ON region.id = source_translation.region_id
LEFT JOIN region_translations AS target_translation
    ON target_translation.region_id = source_translation.region_id
   AND target_translation.locale = $3
WHERE region.tenant_id = $1
  AND source_translation.locale = $2
"#;

const REGION_TRANSLATION_PROGRESS_QUESTION_MARK_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS exact_required_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS complete_resources
FROM region_translations AS source_translation
INNER JOIN regions AS region
    ON region.id = source_translation.region_id
LEFT JOIN region_translations AS target_translation
    ON target_translation.region_id = source_translation.region_id
   AND target_translation.locale = ?
WHERE region.tenant_id = ?
  AND source_translation.locale = ?
"#;
