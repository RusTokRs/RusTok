use rustok_api::StoredLocale;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, FromQueryResult, Statement};
use uuid::Uuid;

use crate::model::ScriptPresentation;

use super::{
    ALLOY_SCRIPT_PRESENTATION_APPLY_RECEIPTS_TABLE,
    ALLOY_SCRIPT_PRESENTATION_RESOURCE_STATE_TABLE, MAX_ALLOY_SCRIPT_PRESENTATION_CHANGE_PAGE,
    SeaOrmScriptPresentationStore, ScriptPresentationStore,
    ScriptPresentationTranslationApplyReceipt, ScriptPresentationTranslationError,
    ScriptPresentationTranslationResult,
};

const RESOURCE_REVISION_PREFIX: &str = "presentation";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptPresentationTranslationResourceSummary {
    pub script_id: Uuid,
    pub resource_revision: String,
    pub exact_locales: Vec<StoredLocale>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptPresentationTranslationResourcePage {
    pub resources: Vec<ScriptPresentationTranslationResourceSummary>,
    pub next_after: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptPresentationTranslationExactResource {
    pub script_id: Uuid,
    pub resource_revision: String,
    pub exact_locales: Vec<StoredLocale>,
    pub source: ScriptPresentation,
    pub target: Option<ScriptPresentation>,
}

/// Alloy-owned exact-resource projection used by the neutral Translation host adapter.
///
/// All SQL remains in Alloy. This projection is deliberately contract-neutral: it does not
/// depend on `rustok-translation-targets`, and it exposes only Script presentation copy plus
/// owner revisions. Operational Script state never crosses this boundary.
#[derive(Clone)]
pub struct SeaOrmScriptPresentationTranslationTargetOwner {
    db: DatabaseConnection,
    tenant_id: Uuid,
    presentations: SeaOrmScriptPresentationStore,
}

impl SeaOrmScriptPresentationTranslationTargetOwner {
    pub fn new(
        db: DatabaseConnection,
        tenant_id: Uuid,
    ) -> ScriptPresentationTranslationResult<Self> {
        if tenant_id.is_nil() {
            return Err(ScriptPresentationTranslationError::Invalid(
                "Alloy script presentation tenant_id must not be nil".to_string(),
            ));
        }
        Ok(Self {
            presentations: SeaOrmScriptPresentationStore::new(db.clone()),
            db,
            tenant_id,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub async fn list_exact_resources(
        &self,
        source_locale: &StoredLocale,
        after: Option<Uuid>,
        limit: u16,
    ) -> ScriptPresentationTranslationResult<ScriptPresentationTranslationResourcePage> {
        self.ensure_postgres()?;
        if source_locale.is_unknown_provenance() {
            return Err(ScriptPresentationTranslationError::Invalid(
                "Alloy script presentation exact source locale cannot use unknown provenance"
                    .to_string(),
            ));
        }
        if limit == 0 || limit > MAX_ALLOY_SCRIPT_PRESENTATION_CHANGE_PAGE {
            return Err(ScriptPresentationTranslationError::Invalid(format!(
                "Alloy script presentation resource page limit must be between 1 and {MAX_ALLOY_SCRIPT_PRESENTATION_CHANGE_PAGE}"
            )));
        }

        let fetch_limit = i64::from(limit) + 1;
        let (sql, values) = match after {
            Some(after) => (
                format!(
                    r#"
SELECT
    source.script_id,
    state.revision,
    string_agg(DISTINCT all_copy.locale, ',' ORDER BY all_copy.locale) AS exact_locales
FROM alloy_script_presentations AS source
INNER JOIN scripts AS script
    ON script.id = source.script_id
   AND script.tenant_id = source.tenant_id
INNER JOIN {ALLOY_SCRIPT_PRESENTATION_RESOURCE_STATE_TABLE} AS state
    ON state.tenant_id = source.tenant_id
   AND state.script_id = source.script_id
INNER JOIN alloy_script_presentations AS all_copy
    ON all_copy.tenant_id = source.tenant_id
   AND all_copy.script_id = source.script_id
   AND all_copy.locale <> 'und'
WHERE source.tenant_id = $1
  AND source.locale = $2
  AND source.script_id > $3
GROUP BY source.script_id, state.revision
ORDER BY source.script_id ASC
LIMIT $4
"#
                ),
                vec![
                    self.tenant_id.into(),
                    source_locale.as_str().to_owned().into(),
                    after.into(),
                    fetch_limit.into(),
                ],
            ),
            None => (
                format!(
                    r#"
SELECT
    source.script_id,
    state.revision,
    string_agg(DISTINCT all_copy.locale, ',' ORDER BY all_copy.locale) AS exact_locales
FROM alloy_script_presentations AS source
INNER JOIN scripts AS script
    ON script.id = source.script_id
   AND script.tenant_id = source.tenant_id
INNER JOIN {ALLOY_SCRIPT_PRESENTATION_RESOURCE_STATE_TABLE} AS state
    ON state.tenant_id = source.tenant_id
   AND state.script_id = source.script_id
INNER JOIN alloy_script_presentations AS all_copy
    ON all_copy.tenant_id = source.tenant_id
   AND all_copy.script_id = source.script_id
   AND all_copy.locale <> 'und'
WHERE source.tenant_id = $1
  AND source.locale = $2
GROUP BY source.script_id, state.revision
ORDER BY source.script_id ASC
LIMIT $3
"#
                ),
                vec![
                    self.tenant_id.into(),
                    source_locale.as_str().to_owned().into(),
                    fetch_limit.into(),
                ],
            ),
        };

        let mut rows = ResourceListRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            values,
        ))
        .all(&self.db)
        .await?;
        let has_more = rows.len() > usize::from(limit);
        if has_more {
            rows.truncate(usize::from(limit));
        }
        let next_after = has_more.then(|| rows.last().map(|row| row.script_id)).flatten();
        let resources = rows
            .into_iter()
            .map(resource_summary_from_row)
            .collect::<ScriptPresentationTranslationResult<Vec<_>>>()?;
        Ok(ScriptPresentationTranslationResourcePage {
            resources,
            next_after,
        })
    }

    pub async fn read_exact_resource(
        &self,
        script_id: Uuid,
        source_locale: &StoredLocale,
        target_locale: &StoredLocale,
    ) -> ScriptPresentationTranslationResult<ScriptPresentationTranslationExactResource> {
        self.ensure_postgres()?;
        if script_id.is_nil() {
            return Err(ScriptPresentationTranslationError::Invalid(
                "Alloy script presentation script_id must not be nil".to_string(),
            ));
        }
        if source_locale.is_unknown_provenance() || target_locale.is_unknown_provenance() {
            return Err(ScriptPresentationTranslationError::Invalid(
                "Alloy script presentation exact locales cannot use unknown provenance"
                    .to_string(),
            ));
        }
        if source_locale == target_locale {
            return Err(ScriptPresentationTranslationError::Invalid(
                "Alloy script presentation source and target locale must differ".to_string(),
            ));
        }

        let source = self
            .presentations
            .find_exact(self.tenant_id, script_id, source_locale)
            .await
            .map_err(map_presentation_store_error)?
            .ok_or_else(|| ScriptPresentationTranslationError::SourceLocaleNotFound {
                script_id,
                locale: source_locale.as_str().to_string(),
            })?;
        let target = self
            .presentations
            .find_exact(self.tenant_id, script_id, target_locale)
            .await
            .map_err(map_presentation_store_error)?;
        let exact_locales = self
            .presentations
            .list_for_script(self.tenant_id, script_id)
            .await
            .map_err(map_presentation_store_error)?
            .into_iter()
            .filter(|presentation| !presentation.locale.is_unknown_provenance())
            .map(|presentation| presentation.locale)
            .collect::<Vec<_>>();
        let state = ResourceStateRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!(
                "SELECT revision FROM {ALLOY_SCRIPT_PRESENTATION_RESOURCE_STATE_TABLE} WHERE tenant_id = $1 AND script_id = $2"
            ),
            vec![self.tenant_id.into(), script_id.into()],
        ))
        .one(&self.db)
        .await?
        .ok_or_else(|| {
            ScriptPresentationTranslationError::OwnerInvariant(
                "Alloy script presentation source exists without Translation resource state"
                    .to_string(),
            )
        })?;

        Ok(ScriptPresentationTranslationExactResource {
            script_id,
            resource_revision: resource_revision(state.revision)?,
            exact_locales,
            source,
            target,
        })
    }

    /// Returns a committed exact replay before callers consult live revisions.
    ///
    /// `request_fingerprint` is computed by the trusted host adapter from the complete neutral
    /// Translation patch. Binding it to tenant + idempotency key + Script identity is therefore
    /// sufficient to reject mismatched key reuse without duplicating target-contract types here.
    pub async fn replay_completed_apply(
        &self,
        script_id: Uuid,
        idempotency_key: &str,
        request_fingerprint: &str,
    ) -> ScriptPresentationTranslationResult<Option<ScriptPresentationTranslationApplyReceipt>> {
        self.ensure_postgres()?;
        if script_id.is_nil() {
            return Err(ScriptPresentationTranslationError::Invalid(
                "Alloy script presentation script_id must not be nil".to_string(),
            ));
        }
        if idempotency_key.trim().is_empty() || request_fingerprint.trim().is_empty() {
            return Err(ScriptPresentationTranslationError::Invalid(
                "Alloy script presentation replay identity must be nonblank".to_string(),
            ));
        }

        let row = ReplayReceiptRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!(
                r#"
SELECT id, script_id, request_fingerprint, target_locale, completed,
       resource_revision, target_copy_revision, target_description
FROM {ALLOY_SCRIPT_PRESENTATION_APPLY_RECEIPTS_TABLE}
WHERE tenant_id = $1 AND idempotency_key = $2
"#
            ),
            vec![self.tenant_id.into(), idempotency_key.to_owned().into()],
        ))
        .one(&self.db)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        if row.script_id != script_id || row.request_fingerprint != request_fingerprint {
            return Err(ScriptPresentationTranslationError::IdempotencyConflict);
        }
        if !row.completed {
            return Err(ScriptPresentationTranslationError::OwnerInvariant(
                "Alloy script presentation durable apply receipt is incomplete".to_string(),
            ));
        }
        let resource_revision = row.resource_revision.ok_or_else(|| {
            ScriptPresentationTranslationError::OwnerInvariant(
                "Alloy script presentation replay receipt is missing resource revision".to_string(),
            )
        })?;
        let target_copy_revision = row.target_copy_revision.filter(|revision| *revision > 0).ok_or_else(
            || {
                ScriptPresentationTranslationError::OwnerInvariant(
                    "Alloy script presentation replay receipt is missing target copy revision"
                        .to_string(),
                )
            },
        )?;
        Ok(Some(ScriptPresentationTranslationApplyReceipt {
            operation_id: row.id,
            script_id: row.script_id,
            resource_revision,
            target_locale: row.target_locale,
            target_copy_revision,
            target_description: row.target_description,
        }))
    }

    fn ensure_postgres(&self) -> ScriptPresentationTranslationResult<()> {
        if self.db.get_database_backend() != DatabaseBackend::Postgres {
            return Err(ScriptPresentationTranslationError::UnsupportedBackend);
        }
        Ok(())
    }
}

#[derive(Debug, FromQueryResult)]
struct ResourceListRow {
    script_id: Uuid,
    revision: i64,
    exact_locales: String,
}

#[derive(Debug, FromQueryResult)]
struct ResourceStateRow {
    revision: i64,
}

#[derive(Debug, FromQueryResult)]
struct ReplayReceiptRow {
    id: Uuid,
    script_id: Uuid,
    request_fingerprint: String,
    target_locale: String,
    completed: bool,
    resource_revision: Option<String>,
    target_copy_revision: Option<i64>,
    target_description: Option<String>,
}

fn resource_summary_from_row(
    row: ResourceListRow,
) -> ScriptPresentationTranslationResult<ScriptPresentationTranslationResourceSummary> {
    if row.script_id.is_nil() {
        return Err(ScriptPresentationTranslationError::OwnerInvariant(
            "Alloy script presentation resource contains a nil Script id".to_string(),
        ));
    }
    let exact_locales = row
        .exact_locales
        .split(',')
        .filter(|locale| !locale.is_empty())
        .map(|locale| {
            StoredLocale::new(locale).map_err(|error| {
                ScriptPresentationTranslationError::OwnerInvariant(format!(
                    "Alloy script presentation resource contains invalid exact locale: {error}"
                ))
            })
        })
        .collect::<ScriptPresentationTranslationResult<Vec<_>>>()?;
    if exact_locales.iter().any(StoredLocale::is_unknown_provenance) {
        return Err(ScriptPresentationTranslationError::OwnerInvariant(
            "Alloy script presentation concrete exact inventory contains unknown provenance"
                .to_string(),
        ));
    }
    Ok(ScriptPresentationTranslationResourceSummary {
        script_id: row.script_id,
        resource_revision: resource_revision(row.revision)?,
        exact_locales,
    })
}

fn resource_revision(revision: i64) -> ScriptPresentationTranslationResult<String> {
    if revision <= 0 {
        return Err(ScriptPresentationTranslationError::OwnerInvariant(
            "Alloy script presentation resource revision must be positive".to_string(),
        ));
    }
    Ok(format!("{RESOURCE_REVISION_PREFIX}:{revision}"))
}

fn map_presentation_store_error(
    error: super::ScriptPresentationStoreError,
) -> ScriptPresentationTranslationError {
    match error {
        super::ScriptPresentationStoreError::NotFound => {
            ScriptPresentationTranslationError::OwnerInvariant(
                "Alloy script presentation owner row disappeared during exact read".to_string(),
            )
        }
        super::ScriptPresentationStoreError::AlreadyExists
        | super::ScriptPresentationStoreError::RevisionConflict { .. } => {
            ScriptPresentationTranslationError::OwnerInvariant(
                "Alloy script presentation exact read observed an unexpected write conflict"
                    .to_string(),
            )
        }
        super::ScriptPresentationStoreError::InvalidStoredLocale => {
            ScriptPresentationTranslationError::OwnerInvariant(
                "Alloy script presentation contains an invalid stored locale".to_string(),
            )
        }
        super::ScriptPresentationStoreError::Storage(message) => {
            ScriptPresentationTranslationError::Storage(message)
        }
    }
}
