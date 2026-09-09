use rustok_api::TenantLocale;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use uuid::Uuid;

use super::collection_translation::{
    CollectionTranslationExactLocaleError, CollectionTranslationExactLocaleResult,
    CollectionTranslationService,
};
use crate::CommerceError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CollectionTranslationExactProgressFacts {
    pub resources: u64,
    pub exact_required_units: u64,
    pub exact_optional_units: u64,
    pub complete_resources: u64,
}

#[derive(Debug, FromQueryResult)]
struct CollectionTranslationExactProgressRow {
    resources: i64,
    exact_required_units: i64,
    exact_optional_units: i64,
    complete_resources: i64,
}

impl CollectionTranslationService {
    /// Aggregates Commerce-owned Collection copy progress in one database statement.
    ///
    /// Inventory matches `list_exact_resources`: active tenant Collections with an
    /// exact source locale participate. Target facts are exact-locale only; blank
    /// values do not count as translated units. Change-cursor bracketing is added
    /// only after the durable owner journal exists.
    pub(crate) async fn read_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> CollectionTranslationExactLocaleResult<CollectionTranslationExactProgressFacts> {
        if tenant_id.is_nil() {
            return Err(CommerceError::Validation(
                "Collection translation progress tenant_id must not be nil".to_owned(),
            )
            .into());
        }
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        if source_locale == target_locale {
            return Err(CommerceError::Validation(
                "Collection translation progress source and target locale must differ".to_owned(),
            )
            .into());
        }

        query_collection_translation_exact_progress(
            self.database(),
            tenant_id,
            &source_locale,
            &target_locale,
        )
        .await
    }
}

async fn query_collection_translation_exact_progress<C>(
    db: &C,
    tenant_id: Uuid,
    source_locale: &str,
    target_locale: &str,
) -> CollectionTranslationExactLocaleResult<CollectionTranslationExactProgressFacts>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => (
            COLLECTION_TRANSLATION_PROGRESS_POSTGRES_SQL,
            vec![
                tenant_id.into(),
                source_locale.to_owned().into(),
                target_locale.to_owned().into(),
            ],
        ),
        _ => (
            COLLECTION_TRANSLATION_PROGRESS_QUESTION_MARK_SQL,
            vec![
                target_locale.to_owned().into(),
                tenant_id.into(),
                source_locale.to_owned().into(),
            ],
        ),
    };
    let statement = Statement::from_sql_and_values(backend, sql, values);
    let row = CollectionTranslationExactProgressRow::find_by_statement(statement)
        .one(db)
        .await?
        .ok_or_else(|| {
            CommerceError::Validation(
                "Collection translation progress aggregate returned no row".to_owned(),
            )
        })?;

    let facts = CollectionTranslationExactProgressFacts {
        resources: progress_count(row.resources, "resources")?,
        exact_required_units: progress_count(row.exact_required_units, "exact required units")?,
        exact_optional_units: progress_count(row.exact_optional_units, "exact optional units")?,
        complete_resources: progress_count(row.complete_resources, "complete resources")?,
    };
    let required_capacity = facts.resources.checked_mul(2).ok_or_else(|| {
        CommerceError::Validation(
            "Collection translation required progress capacity overflow".to_owned(),
        )
    })?;
    if facts.exact_required_units > required_capacity
        || facts.exact_optional_units > facts.resources
        || facts.complete_resources > facts.resources
    {
        return Err(CommerceError::Validation(
            "Collection translation progress aggregate is internally inconsistent".to_owned(),
        )
        .into());
    }
    Ok(facts)
}

fn canonical_locale(locale: &str) -> CollectionTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| CommerceError::Validation(error.to_string()).into())
}

fn progress_count(
    value: i64,
    field: &'static str,
) -> CollectionTranslationExactLocaleResult<u64> {
    u64::try_from(value).map_err(|_| {
        CommerceError::Validation(format!(
            "Collection translation progress {field} must not be negative"
        ))
        .into()
    })
}

const COLLECTION_TRANSLATION_PROGRESS_POSTGRES_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.title, '')) <> '' THEN 1 END)
        + COUNT(CASE WHEN TRIM(COALESCE(target_translation.handle, '')) <> '' THEN 1 END)
        AS exact_required_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.description, '')) <> '' THEN 1 END)
        AS exact_optional_units,
    COUNT(CASE
        WHEN TRIM(COALESCE(target_translation.title, '')) <> ''
         AND TRIM(COALESCE(target_translation.handle, '')) <> ''
        THEN 1
    END) AS complete_resources
FROM collection_translations AS source_translation
INNER JOIN collections AS collection
    ON collection.id = source_translation.collection_id
LEFT JOIN collection_translations AS target_translation
    ON target_translation.collection_id = source_translation.collection_id
   AND target_translation.locale = $3
WHERE collection.tenant_id = $1
  AND collection.deleted_at IS NULL
  AND source_translation.locale = $2
"#;

const COLLECTION_TRANSLATION_PROGRESS_QUESTION_MARK_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.title, '')) <> '' THEN 1 END)
        + COUNT(CASE WHEN TRIM(COALESCE(target_translation.handle, '')) <> '' THEN 1 END)
        AS exact_required_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.description, '')) <> '' THEN 1 END)
        AS exact_optional_units,
    COUNT(CASE
        WHEN TRIM(COALESCE(target_translation.title, '')) <> ''
         AND TRIM(COALESCE(target_translation.handle, '')) <> ''
        THEN 1
    END) AS complete_resources
FROM collection_translations AS source_translation
INNER JOIN collections AS collection
    ON collection.id = source_translation.collection_id
LEFT JOIN collection_translations AS target_translation
    ON target_translation.collection_id = source_translation.collection_id
   AND target_translation.locale = ?
WHERE collection.tenant_id = ?
  AND collection.deleted_at IS NULL
  AND source_translation.locale = ?
"#;
