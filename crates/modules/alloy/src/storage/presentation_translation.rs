use async_trait::async_trait;
use rustok_api::{RuntimeLocale, StoredLocale};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait, FromQueryResult,
    QueryFilter, QuerySelect, Statement, TransactionTrait,
};
use thiserror::Error;
use uuid::Uuid;

use super::{
    SeaOrmScriptPresentationStore, ScriptPresentationStoreError, ScriptsColumn, ScriptsEntity,
};

pub const ALLOY_SCRIPT_PRESENTATION_RESOURCE_STATE_TABLE: &str =
    "alloy_script_presentation_translation_resource_state";
pub const ALLOY_SCRIPT_PRESENTATION_CHANGE_JOURNAL_TABLE: &str =
    "alloy_script_presentation_translation_change_journal";
pub const ALLOY_SCRIPT_PRESENTATION_APPLY_RECEIPTS_TABLE: &str =
    "alloy_script_presentation_translation_apply_receipts";
pub const MAX_ALLOY_SCRIPT_PRESENTATION_CHANGE_PAGE: u16 = 200;

const CHANGE_CURSOR_VERSION: &str = "v1";
const RESOURCE_REVISION_PREFIX: &str = "presentation";
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptPresentationTranslationChangeLifecycle {
    Active,
    Deleted,
}

impl ScriptPresentationTranslationChangeLifecycle {
    fn parse(value: &str) -> ScriptPresentationTranslationResult<Self> {
        match value {
            "active" => Ok(Self::Active),
            "deleted" => Ok(Self::Deleted),
            other => Err(ScriptPresentationTranslationError::OwnerInvariant(format!(
                "Alloy script presentation change journal contains invalid lifecycle `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptPresentationTranslationChangeRecord {
    pub change_seq: u64,
    pub script_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: ScriptPresentationTranslationChangeLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptPresentationTranslationChangePage {
    pub changes: Vec<ScriptPresentationTranslationChangeRecord>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScriptPresentationTranslationExactProgress {
    pub required_units: u64,
    pub exact_required_units: u64,
    pub optional_units: u64,
    pub exact_optional_units: u64,
    pub resources: u64,
    pub complete_resources: u64,
}

impl ScriptPresentationTranslationExactProgress {
    fn validate(&self) -> ScriptPresentationTranslationResult<()> {
        if self.exact_required_units > self.required_units
            || self.exact_optional_units > self.optional_units
            || self.complete_resources > self.resources
        {
            return Err(ScriptPresentationTranslationError::OwnerInvariant(
                "Alloy script presentation translation progress violates owner bounds".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptPresentationTranslationProgressSnapshot {
    pub facts: ScriptPresentationTranslationExactProgress,
    pub owner_change_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptPresentationTranslationOperationContext {
    pub idempotency_key: String,
    pub proposal_id: String,
    pub approval_receipt_id: String,
    pub request_fingerprint: String,
}

impl ScriptPresentationTranslationOperationContext {
    fn validate(&self) -> ScriptPresentationTranslationResult<()> {
        validate_bounded_nonblank(&self.idempotency_key, "idempotency_key", 255)?;
        validate_bounded_nonblank(&self.proposal_id, "proposal_id", 255)?;
        validate_bounded_nonblank(&self.approval_receipt_id, "approval_receipt_id", 255)?;
        validate_bounded_nonblank(&self.request_fingerprint, "request_fingerprint", 128)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptPresentationTranslationApply {
    pub operation: ScriptPresentationTranslationOperationContext,
    pub source_locale: String,
    pub target_locale: String,
    pub target_description: Option<String>,
    pub expected_resource_revision: String,
    pub expected_source_copy_revision: i64,
    pub expected_target_copy_revision: Option<i64>,
}

impl ScriptPresentationTranslationApply {
    fn validate(
        &self,
    ) -> ScriptPresentationTranslationResult<(RuntimeLocale, RuntimeLocale)> {
        self.operation.validate()?;
        validate_bounded_nonblank(
            &self.expected_resource_revision,
            "expected_resource_revision",
            128,
        )?;
        if self.expected_source_copy_revision <= 0 {
            return Err(ScriptPresentationTranslationError::Invalid(
                "Alloy script presentation expected source copy revision must be positive"
                    .to_string(),
            ));
        }
        if self
            .expected_target_copy_revision
            .is_some_and(|revision| revision <= 0)
        {
            return Err(ScriptPresentationTranslationError::Invalid(
                "Alloy script presentation expected target copy revision must be positive"
                    .to_string(),
            ));
        }
        let source_locale = RuntimeLocale::new(&self.source_locale).map_err(|error| {
            ScriptPresentationTranslationError::Invalid(format!(
                "Alloy script presentation source locale is invalid: {error}"
            ))
        })?;
        let target_locale = RuntimeLocale::new(&self.target_locale).map_err(|error| {
            ScriptPresentationTranslationError::Invalid(format!(
                "Alloy script presentation target locale is invalid: {error}"
            ))
        })?;
        if source_locale == target_locale {
            return Err(ScriptPresentationTranslationError::Invalid(
                "Alloy script presentation source and target locale must differ".to_string(),
            ));
        }
        Ok((source_locale, target_locale))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptPresentationTranslationApplyReceipt {
    pub operation_id: Uuid,
    pub script_id: Uuid,
    pub resource_revision: String,
    pub target_locale: String,
    pub target_copy_revision: i64,
    pub target_description: Option<String>,
}

#[derive(Debug, Error)]
pub enum ScriptPresentationTranslationError {
    #[error("invalid Alloy script presentation translation request: {0}")]
    Invalid(String),
    #[error("Alloy script presentation requires PostgreSQL")]
    UnsupportedBackend,
    #[error("Alloy script was not found: {0}")]
    ScriptNotFound(Uuid),
    #[error("Alloy script presentation source locale was not found: {locale} for {script_id}")]
    SourceLocaleNotFound { script_id: Uuid, locale: String },
    #[error("Alloy script presentation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },
    #[error("Alloy script presentation idempotency key was reused for another request")]
    IdempotencyConflict,
    #[error("Alloy script presentation owner invariant failed: {0}")]
    OwnerInvariant(String),
    #[error("Alloy script presentation storage failed: {0}")]
    Storage(String),
}

impl From<sea_orm::DbErr> for ScriptPresentationTranslationError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Storage(error.to_string())
    }
}

pub type ScriptPresentationTranslationResult<T> =
    Result<T, ScriptPresentationTranslationError>;

#[async_trait]
pub trait ScriptPresentationTranslationChangeOwnerPort: Send + Sync {
    async fn read_change_highwater(&self) -> ScriptPresentationTranslationResult<Option<u64>>;

    async fn read_changes(
        &self,
        after: Option<&str>,
        limit: u16,
    ) -> ScriptPresentationTranslationResult<ScriptPresentationTranslationChangePage>;

    async fn read_exact_progress(
        &self,
        source_locale: &str,
        target_locale: &str,
    ) -> ScriptPresentationTranslationResult<ScriptPresentationTranslationProgressSnapshot>;
}

#[derive(Clone)]
pub struct SeaOrmScriptPresentationTranslationStore {
    db: DatabaseConnection,
    tenant_id: Uuid,
    presentations: SeaOrmScriptPresentationStore,
}

impl SeaOrmScriptPresentationTranslationStore {
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

    /// Applies one exact target locale through the canonical presentation CAS while reserving
    /// and completing a durable owner receipt in the same transaction. Receipt replay is checked
    /// before live revisions, so a committed retry remains idempotent after later owner changes.
    pub async fn apply_exact_locale(
        &self,
        script_id: Uuid,
        request: ScriptPresentationTranslationApply,
    ) -> ScriptPresentationTranslationResult<ScriptPresentationTranslationApplyReceipt> {
        self.ensure_postgres()?;
        if script_id.is_nil() {
            return Err(ScriptPresentationTranslationError::Invalid(
                "Alloy script presentation script_id must not be nil".to_string(),
            ));
        }
        let (source_runtime_locale, target_runtime_locale) = request.validate()?;
        let source_locale = StoredLocale::from(source_runtime_locale);
        let target_locale = StoredLocale::from(target_runtime_locale.clone());
        let operation_id = Uuid::new_v4();
        let transaction = self.db.begin().await?;

        transaction
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                format!(
                    r#"
INSERT INTO {ALLOY_SCRIPT_PRESENTATION_APPLY_RECEIPTS_TABLE} (
    id, tenant_id, script_id, idempotency_key, proposal_id, approval_receipt_id,
    request_fingerprint, source_locale, target_locale, expected_resource_revision,
    expected_source_copy_revision, expected_target_copy_revision, requested_description,
    completed
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, FALSE)
ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
"#
                ),
                vec![
                    operation_id.into(),
                    self.tenant_id.into(),
                    script_id.into(),
                    request.operation.idempotency_key.clone().into(),
                    request.operation.proposal_id.clone().into(),
                    request.operation.approval_receipt_id.clone().into(),
                    request.operation.request_fingerprint.clone().into(),
                    source_locale.as_str().to_owned().into(),
                    target_locale.as_str().to_owned().into(),
                    request.expected_resource_revision.clone().into(),
                    request.expected_source_copy_revision.into(),
                    request.expected_target_copy_revision.into(),
                    request.target_description.clone().into(),
                ],
            ))
            .await?;

        let durable = ApplyReceiptRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!(
                r#"
SELECT id, script_id, idempotency_key, proposal_id, approval_receipt_id,
       request_fingerprint, source_locale, target_locale, expected_resource_revision,
       expected_source_copy_revision, expected_target_copy_revision, requested_description,
       completed, resource_revision, target_copy_revision, target_description
FROM {ALLOY_SCRIPT_PRESENTATION_APPLY_RECEIPTS_TABLE}
WHERE tenant_id = $1 AND idempotency_key = $2
FOR UPDATE
"#
            ),
            vec![
                self.tenant_id.into(),
                request.operation.idempotency_key.clone().into(),
            ],
        ))
        .one(&transaction)
        .await?
        .ok_or_else(|| {
            ScriptPresentationTranslationError::OwnerInvariant(
                "Alloy script presentation apply admission completed without a receipt".to_string(),
            )
        })?;

        validate_receipt_identity(&durable, script_id, &request, &source_locale, &target_locale)?;
        if durable.id != operation_id {
            if durable.completed {
                let receipt = receipt_from_row(durable)?;
                transaction.commit().await?;
                return Ok(receipt);
            }
            return Err(ScriptPresentationTranslationError::OwnerInvariant(
                "Alloy script presentation durable apply receipt is incomplete".to_string(),
            ));
        }

        let script_exists = ScriptsEntity::find_by_id(script_id)
            .filter(ScriptsColumn::TenantId.eq(self.tenant_id))
            .lock_exclusive()
            .one(&transaction)
            .await?
            .is_some();
        if !script_exists {
            return Err(ScriptPresentationTranslationError::ScriptNotFound(script_id));
        }

        let source = load_presentation_row(
            &transaction,
            self.tenant_id,
            script_id,
            source_locale.as_str(),
        )
        .await?
        .ok_or_else(|| ScriptPresentationTranslationError::SourceLocaleNotFound {
            script_id,
            locale: source_locale.as_str().to_string(),
        })?;
        if source.copy_revision != request.expected_source_copy_revision {
            return Err(ScriptPresentationTranslationError::RevisionConflict {
                revision: "source",
            });
        }

        let state = load_resource_state(&transaction, self.tenant_id, script_id)
            .await?
            .ok_or_else(|| {
                ScriptPresentationTranslationError::OwnerInvariant(
                    "Alloy script presentation source exists without translation resource state"
                        .to_string(),
                )
            })?;
        let current_resource_revision = resource_revision(state.revision)?;
        if current_resource_revision != request.expected_resource_revision {
            return Err(ScriptPresentationTranslationError::RevisionConflict {
                revision: "resource",
            });
        }

        let current_target = load_presentation_row(
            &transaction,
            self.tenant_id,
            script_id,
            target_locale.as_str(),
        )
        .await?;
        if current_target.as_ref().map(|row| row.copy_revision)
            != request.expected_target_copy_revision
        {
            return Err(ScriptPresentationTranslationError::RevisionConflict {
                revision: "target",
            });
        }

        let target = match current_target {
            Some(current) => self
                .presentations
                .compare_and_set_source_in_transaction(
                    &transaction,
                    self.tenant_id,
                    script_id,
                    &target_runtime_locale,
                    current.copy_revision,
                    request.target_description.clone(),
                )
                .await
                .map_err(map_presentation_store_error)?,
            None => self
                .presentations
                .create_source_in_transaction(
                    &transaction,
                    self.tenant_id,
                    script_id,
                    target_runtime_locale,
                    request.target_description.clone(),
                )
                .await
                .map_err(map_presentation_store_error)?,
        };

        let state_after = load_resource_state(&transaction, self.tenant_id, script_id)
            .await?
            .ok_or_else(|| {
                ScriptPresentationTranslationError::OwnerInvariant(
                    "Alloy script presentation apply removed translation resource state".to_string(),
                )
            })?;
        let receipt = ScriptPresentationTranslationApplyReceipt {
            operation_id,
            script_id,
            resource_revision: resource_revision(state_after.revision)?,
            target_locale: target.locale.as_str().to_string(),
            target_copy_revision: target.copy_revision,
            target_description: target.description,
        };

        let completed = transaction
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                format!(
                    r#"
UPDATE {ALLOY_SCRIPT_PRESENTATION_APPLY_RECEIPTS_TABLE}
SET completed = TRUE,
    resource_revision = $2,
    target_copy_revision = $3,
    target_description = $4,
    completed_at = CURRENT_TIMESTAMP
WHERE id = $1 AND completed = FALSE
"#
                ),
                vec![
                    operation_id.into(),
                    receipt.resource_revision.clone().into(),
                    receipt.target_copy_revision.into(),
                    receipt.target_description.clone().into(),
                ],
            ))
            .await?;
        if completed.rows_affected() != 1 {
            return Err(ScriptPresentationTranslationError::OwnerInvariant(
                "Alloy script presentation apply receipt completion lost its durable lease"
                    .to_string(),
            ));
        }

        transaction.commit().await?;
        Ok(receipt)
    }

    fn ensure_postgres(&self) -> ScriptPresentationTranslationResult<()> {
        if self.db.get_database_backend() != DatabaseBackend::Postgres {
            return Err(ScriptPresentationTranslationError::UnsupportedBackend);
        }
        Ok(())
    }

    async fn read_progress_facts(
        &self,
        source_locale: &str,
        target_locale: &str,
    ) -> ScriptPresentationTranslationResult<ScriptPresentationTranslationExactProgress> {
        let row = ProgressRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target.description, '')) <> '' THEN 1 END)
        AS exact_optional_units
FROM alloy_script_presentations AS source
INNER JOIN scripts AS script
    ON script.id = source.script_id
   AND script.tenant_id = source.tenant_id
LEFT JOIN alloy_script_presentations AS target
    ON target.tenant_id = source.tenant_id
   AND target.script_id = source.script_id
   AND target.locale = $3
WHERE source.tenant_id = $1
  AND source.locale = $2
"#,
            vec![
                self.tenant_id.into(),
                source_locale.to_owned().into(),
                target_locale.to_owned().into(),
            ],
        ))
        .one(&self.db)
        .await?
        .ok_or_else(|| {
            ScriptPresentationTranslationError::OwnerInvariant(
                "Alloy script presentation progress aggregate returned no row".to_string(),
            )
        })?;
        let resources = nonnegative_count(row.resources, "resources")?;
        let exact_optional_units =
            nonnegative_count(row.exact_optional_units, "exact_optional_units")?;
        let facts = ScriptPresentationTranslationExactProgress {
            required_units: 0,
            exact_required_units: 0,
            optional_units: resources,
            exact_optional_units,
            resources,
            // `description` is optional. Resource completeness is therefore governed by the
            // empty required-field set; optional progress remains visible independently.
            complete_resources: resources,
        };
        facts.validate()?;
        Ok(facts)
    }

    async fn read_change_records(
        &self,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> ScriptPresentationTranslationResult<Vec<ScriptPresentationTranslationChangeRecord>> {
        if through_seq < after_seq {
            return Err(ScriptPresentationTranslationError::Invalid(
                "Alloy script presentation change high-water precedes the cursor".to_string(),
            ));
        }
        if through_seq == 0 {
            return Ok(Vec::new());
        }
        let rows = ChangeRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!(
                r#"
SELECT change_seq, script_id, resource_revision, lifecycle
FROM {ALLOY_SCRIPT_PRESENTATION_CHANGE_JOURNAL_TABLE}
WHERE tenant_id = $1
  AND change_seq > $2
  AND change_seq <= $3
ORDER BY change_seq ASC
LIMIT $4
"#
            ),
            vec![
                self.tenant_id.into(),
                i64::try_from(after_seq)
                    .map_err(|_| invalid_sequence("after"))?
                    .into(),
                i64::try_from(through_seq)
                    .map_err(|_| invalid_sequence("through"))?
                    .into(),
                i64::from(limit).into(),
            ],
        ))
        .all(&self.db)
        .await?;
        rows.into_iter().map(change_from_row).collect()
    }
}

#[async_trait]
impl ScriptPresentationTranslationChangeOwnerPort for SeaOrmScriptPresentationTranslationStore {
    async fn read_change_highwater(&self) -> ScriptPresentationTranslationResult<Option<u64>> {
        self.ensure_postgres()?;
        let row = HighwaterRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!(
                "SELECT MAX(change_seq) AS highwater FROM {ALLOY_SCRIPT_PRESENTATION_CHANGE_JOURNAL_TABLE} WHERE tenant_id = $1"
            ),
            vec![self.tenant_id.into()],
        ))
        .one(&self.db)
        .await?
        .ok_or_else(|| {
            ScriptPresentationTranslationError::OwnerInvariant(
                "Alloy script presentation change high-water query returned no row".to_string(),
            )
        })?;
        row.highwater
            .map(|value| positive_sequence(value, "high-water"))
            .transpose()
    }

    async fn read_changes(
        &self,
        after: Option<&str>,
        limit: u16,
    ) -> ScriptPresentationTranslationResult<ScriptPresentationTranslationChangePage> {
        self.ensure_postgres()?;
        validate_change_limit(limit)?;
        let parsed = after.map(parse_change_cursor).transpose()?;
        let (through, cursor_after) = match parsed {
            Some((through, cursor_after)) if through == cursor_after => {
                let current = self
                    .read_change_highwater()
                    .await?
                    .unwrap_or(cursor_after)
                    .max(cursor_after);
                (current, cursor_after)
            }
            Some(cursor) => cursor,
            None => (self.read_change_highwater().await?.unwrap_or(0), 0),
        };
        if through == 0 {
            return Ok(ScriptPresentationTranslationChangePage {
                changes: Vec::new(),
                next_cursor: None,
            });
        }

        let changes = self
            .read_change_records(cursor_after, through, limit)
            .await?;
        let last_seq = changes.last().map(|change| change.change_seq);
        let next_cursor = Some(match last_seq {
            Some(last_seq) if last_seq < through => change_cursor(through, last_seq)?,
            _ => change_cursor(through, through)?,
        });
        Ok(ScriptPresentationTranslationChangePage {
            changes,
            next_cursor,
        })
    }

    async fn read_exact_progress(
        &self,
        source_locale: &str,
        target_locale: &str,
    ) -> ScriptPresentationTranslationResult<ScriptPresentationTranslationProgressSnapshot> {
        self.ensure_postgres()?;
        let source_locale = RuntimeLocale::new(source_locale).map_err(|error| {
            ScriptPresentationTranslationError::Invalid(format!(
                "Alloy script presentation source locale is invalid: {error}"
            ))
        })?;
        let target_locale = RuntimeLocale::new(target_locale).map_err(|error| {
            ScriptPresentationTranslationError::Invalid(format!(
                "Alloy script presentation target locale is invalid: {error}"
            ))
        })?;
        if source_locale == target_locale {
            return Err(ScriptPresentationTranslationError::Invalid(
                "Alloy script presentation source and target locale must differ".to_string(),
            ));
        }

        for _ in 0..PROGRESS_STABILITY_ATTEMPTS {
            let before = self.read_change_highwater().await?;
            let facts = self
                .read_progress_facts(source_locale.as_str(), target_locale.as_str())
                .await?;
            let after = self.read_change_highwater().await?;
            if before != after {
                continue;
            }
            return Ok(ScriptPresentationTranslationProgressSnapshot {
                facts,
                owner_change_cursor: after
                    .map(|change_seq| change_cursor(change_seq, change_seq))
                    .transpose()?,
            });
        }

        Err(ScriptPresentationTranslationError::Storage(
            "Alloy script presentation progress changed while it was being aggregated".to_string(),
        ))
    }
}

#[derive(Debug, FromQueryResult)]
struct PresentationRow {
    copy_revision: i64,
}

#[derive(Debug, FromQueryResult)]
struct ResourceStateRow {
    revision: i64,
}

#[derive(Debug, FromQueryResult)]
struct HighwaterRow {
    highwater: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct ChangeRow {
    change_seq: i64,
    script_id: Uuid,
    resource_revision: String,
    lifecycle: String,
}

#[derive(Debug, FromQueryResult)]
struct ProgressRow {
    resources: i64,
    exact_optional_units: i64,
}

#[derive(Debug, FromQueryResult)]
struct ApplyReceiptRow {
    id: Uuid,
    script_id: Uuid,
    idempotency_key: String,
    proposal_id: String,
    approval_receipt_id: String,
    request_fingerprint: String,
    source_locale: String,
    target_locale: String,
    expected_resource_revision: String,
    expected_source_copy_revision: i64,
    expected_target_copy_revision: Option<i64>,
    requested_description: Option<String>,
    completed: bool,
    resource_revision: Option<String>,
    target_copy_revision: Option<i64>,
    target_description: Option<String>,
}

async fn load_presentation_row<C>(
    connection: &C,
    tenant_id: Uuid,
    script_id: Uuid,
    locale: &str,
) -> ScriptPresentationTranslationResult<Option<PresentationRow>>
where
    C: ConnectionTrait,
{
    Ok(PresentationRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
SELECT copy_revision
FROM alloy_script_presentations
WHERE tenant_id = $1 AND script_id = $2 AND locale = $3
FOR UPDATE
"#,
        vec![
            tenant_id.into(),
            script_id.into(),
            locale.to_owned().into(),
        ],
    ))
    .one(connection)
    .await?)
}

async fn load_resource_state<C>(
    connection: &C,
    tenant_id: Uuid,
    script_id: Uuid,
) -> ScriptPresentationTranslationResult<Option<ResourceStateRow>>
where
    C: ConnectionTrait,
{
    Ok(ResourceStateRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!(
            "SELECT revision FROM {ALLOY_SCRIPT_PRESENTATION_RESOURCE_STATE_TABLE} WHERE tenant_id = $1 AND script_id = $2 FOR UPDATE"
        ),
        vec![tenant_id.into(), script_id.into()],
    ))
    .one(connection)
    .await?)
}

fn validate_receipt_identity(
    row: &ApplyReceiptRow,
    script_id: Uuid,
    request: &ScriptPresentationTranslationApply,
    source_locale: &StoredLocale,
    target_locale: &StoredLocale,
) -> ScriptPresentationTranslationResult<()> {
    let matches = row.script_id == script_id
        && row.idempotency_key == request.operation.idempotency_key
        && row.proposal_id == request.operation.proposal_id
        && row.approval_receipt_id == request.operation.approval_receipt_id
        && row.request_fingerprint == request.operation.request_fingerprint
        && row.source_locale == source_locale.as_str()
        && row.target_locale == target_locale.as_str()
        && row.expected_resource_revision == request.expected_resource_revision
        && row.expected_source_copy_revision == request.expected_source_copy_revision
        && row.expected_target_copy_revision == request.expected_target_copy_revision
        && row.requested_description == request.target_description;
    if matches {
        Ok(())
    } else {
        Err(ScriptPresentationTranslationError::IdempotencyConflict)
    }
}

fn receipt_from_row(
    row: ApplyReceiptRow,
) -> ScriptPresentationTranslationResult<ScriptPresentationTranslationApplyReceipt> {
    if !row.completed {
        return Err(ScriptPresentationTranslationError::OwnerInvariant(
            "Alloy script presentation replay receipt is incomplete".to_string(),
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
    Ok(ScriptPresentationTranslationApplyReceipt {
        operation_id: row.id,
        script_id: row.script_id,
        resource_revision,
        target_locale: row.target_locale,
        target_copy_revision,
        target_description: row.target_description,
    })
}

fn map_presentation_store_error(
    error: ScriptPresentationStoreError,
) -> ScriptPresentationTranslationError {
    match error {
        ScriptPresentationStoreError::NotFound => {
            ScriptPresentationTranslationError::RevisionConflict { revision: "target" }
        }
        ScriptPresentationStoreError::AlreadyExists
        | ScriptPresentationStoreError::RevisionConflict { .. } => {
            ScriptPresentationTranslationError::RevisionConflict { revision: "target" }
        }
        ScriptPresentationStoreError::InvalidStoredLocale => {
            ScriptPresentationTranslationError::OwnerInvariant(
                "canonical presentation store rejected a normalized runtime locale".to_string(),
            )
        }
        ScriptPresentationStoreError::Storage(message) => {
            ScriptPresentationTranslationError::Storage(message)
        }
    }
}

fn resource_revision(revision: i64) -> ScriptPresentationTranslationResult<String> {
    let revision = positive_sequence(revision, "resource revision")?;
    Ok(format!("{RESOURCE_REVISION_PREFIX}:{revision}"))
}

fn change_from_row(
    row: ChangeRow,
) -> ScriptPresentationTranslationResult<ScriptPresentationTranslationChangeRecord> {
    if row.script_id.is_nil() || row.resource_revision.trim().is_empty() {
        return Err(ScriptPresentationTranslationError::OwnerInvariant(
            "Alloy script presentation change row contains invalid identity".to_string(),
        ));
    }
    Ok(ScriptPresentationTranslationChangeRecord {
        change_seq: positive_sequence(row.change_seq, "change")?,
        script_id: row.script_id,
        resource_revision: row.resource_revision,
        lifecycle: ScriptPresentationTranslationChangeLifecycle::parse(&row.lifecycle)?,
    })
}

fn validate_change_limit(limit: u16) -> ScriptPresentationTranslationResult<()> {
    if limit == 0 || limit > MAX_ALLOY_SCRIPT_PRESENTATION_CHANGE_PAGE {
        return Err(ScriptPresentationTranslationError::Invalid(format!(
            "Alloy script presentation change page limit must be between 1 and {MAX_ALLOY_SCRIPT_PRESENTATION_CHANGE_PAGE}"
        )));
    }
    Ok(())
}

fn change_cursor(through: u64, after: u64) -> ScriptPresentationTranslationResult<String> {
    if through == 0 || after == 0 || after > through {
        return Err(ScriptPresentationTranslationError::OwnerInvariant(
            "Alloy script presentation change cursor bounds are invalid".to_string(),
        ));
    }
    Ok(format!("{CHANGE_CURSOR_VERSION}:{through}:{after}"))
}

fn parse_change_cursor(cursor: &str) -> ScriptPresentationTranslationResult<(u64, u64)> {
    let mut parts = cursor.split(':');
    let version = parts.next();
    let through = parts.next().and_then(|value| value.parse::<u64>().ok());
    let after = parts.next().and_then(|value| value.parse::<u64>().ok());
    if version != Some(CHANGE_CURSOR_VERSION)
        || parts.next().is_some()
        || through.is_none()
        || after.is_none()
    {
        return Err(ScriptPresentationTranslationError::Invalid(
            "Alloy script presentation change cursor is invalid".to_string(),
        ));
    }
    let through = through.unwrap_or_default();
    let after = after.unwrap_or_default();
    if through == 0 || after == 0 || after > through {
        return Err(ScriptPresentationTranslationError::Invalid(
            "Alloy script presentation change cursor bounds are invalid".to_string(),
        ));
    }
    Ok((through, after))
}

fn positive_sequence(value: i64, field: &str) -> ScriptPresentationTranslationResult<u64> {
    let value = u64::try_from(value).map_err(|_| invalid_sequence(field))?;
    if value == 0 {
        return Err(invalid_sequence(field));
    }
    Ok(value)
}

fn nonnegative_count(value: i64, field: &str) -> ScriptPresentationTranslationResult<u64> {
    u64::try_from(value).map_err(|_| {
        ScriptPresentationTranslationError::OwnerInvariant(format!(
            "Alloy script presentation progress {field} must not be negative"
        ))
    })
}

fn invalid_sequence(field: &str) -> ScriptPresentationTranslationError {
    ScriptPresentationTranslationError::OwnerInvariant(format!(
        "Alloy script presentation {field} must be positive"
    ))
}

fn validate_bounded_nonblank(
    value: &str,
    field: &str,
    max_bytes: usize,
) -> ScriptPresentationTranslationResult<()> {
    if value.trim().is_empty() || value.len() > max_bytes {
        return Err(ScriptPresentationTranslationError::Invalid(format!(
            "Alloy script presentation {field} must be nonblank and at most {max_bytes} bytes"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_change_cursor_round_trips() {
        let cursor = change_cursor(42, 17).expect("valid cursor");
        assert_eq!(parse_change_cursor(&cursor).unwrap(), (42, 17));
        assert_eq!(parse_change_cursor("v1:42:42").unwrap(), (42, 42));
    }

    #[test]
    fn invalid_change_cursors_fail_closed() {
        for cursor in ["", "v2:1:1", "v1:0:0", "v1:1:0", "v1:1:2", "v1:x:1"] {
            assert!(parse_change_cursor(cursor).is_err(), "{cursor}");
        }
    }
}
