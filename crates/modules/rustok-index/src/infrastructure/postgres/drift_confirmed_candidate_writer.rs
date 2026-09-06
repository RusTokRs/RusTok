use std::{fmt, str::FromStr};

use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend,
    IsolationLevel, QueryResult, Statement, TransactionTrait, Value as SqlValue,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    EntityKey, IndexDriftConfirmedCandidate, IndexDriftConfirmedMissingEntity,
    IndexDriftConfirmedOrphanLink,
};

use super::{
    drift_finding_inspector::{IndexDriftFindingScope, IndexDriftFindingSeverity},
    drift_finding_writer::{IndexDriftDigestFindingRequest, IndexDriftFindingWriteOutcome},
};

const MISSING_ENTITY_CHECK: &str = "index.confirmed_missing_entity";
const ORPHAN_LINK_CHECK_PREFIX: &str = "index.confirmed_orphan_link.";
const MISSING_EVIDENCE_DOMAIN: &[u8] = b"index_confirmed_missing_entity_evidence_v1";
const ORPHAN_EVIDENCE_DOMAIN: &[u8] = b"index_confirmed_orphan_link_evidence_v1";
const ORPHAN_IDENTITY_DOMAIN: &[u8] = b"index_confirmed_orphan_link_identity_v1";
const FINDING_DETAILS_CONTRACT: &str = "index_drift_digest_finding_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftConfirmedCandidateNotRecordedReason {
    MaterializedChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftConfirmedCandidateRecordOutcome {
    Recorded(IndexDriftFindingWriteOutcome),
    NotRecorded(IndexDriftConfirmedCandidateNotRecordedReason),
}

impl IndexDriftConfirmedCandidateRecordOutcome {
    pub fn is_recorded(&self) -> bool {
        matches!(self, Self::Recorded(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftConfirmedCandidateRecordError {
    #[error("confirmed Index drift candidate writer requires PostgreSQL")]
    UnsupportedBackend,
    #[error("confirmed Index drift candidate evidence is outside the bounded contract")]
    InvalidEvidence,
    #[error("confirmed Index drift candidate storage operation must be retried")]
    Storage,
    #[error("confirmed Index drift candidate conflicts with the finding storage contract")]
    FindingContract,
}

impl IndexDriftConfirmedCandidateRecordError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Storage)
    }
}

/// Index-owned write boundary for one already confirmed stale entity or orphan link.
///
/// All finding identity and evidence are derived from typed confirmation output. The adapter
/// revalidates the exact materialized shape and applies the established finding-key/state contract
/// in one serializable transaction. It accepts no raw details JSON, lifecycle state, actor,
/// schedule, cursor, or repair instruction.
#[derive(Clone)]
pub struct PostgresIndexDriftConfirmedCandidateWriter {
    db: DatabaseConnection,
}

impl PostgresIndexDriftConfirmedCandidateWriter {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn record_confirmed_candidate(
        &self,
        candidate: &IndexDriftConfirmedCandidate,
    ) -> Result<IndexDriftConfirmedCandidateRecordOutcome, IndexDriftConfirmedCandidateRecordError>
    {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(IndexDriftConfirmedCandidateRecordError::UnsupportedBackend);
        }
        let request = finding_request(candidate)?;
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::Serializable),
                Some(AccessMode::ReadWrite),
            )
            .await
            .map_err(|_| IndexDriftConfirmedCandidateRecordError::Storage)?;

        let result = self
            .record_in_transaction(&transaction, candidate, &request)
            .await;
        match result {
            Ok(IndexDriftConfirmedCandidateRecordOutcome::Recorded(outcome)) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| IndexDriftConfirmedCandidateRecordError::Storage)?;
                Ok(IndexDriftConfirmedCandidateRecordOutcome::Recorded(outcome))
            }
            Ok(IndexDriftConfirmedCandidateRecordOutcome::NotRecorded(reason)) => {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| IndexDriftConfirmedCandidateRecordError::Storage)?;
                Ok(IndexDriftConfirmedCandidateRecordOutcome::NotRecorded(
                    reason,
                ))
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }

    async fn record_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        candidate: &IndexDriftConfirmedCandidate,
        request: &IndexDriftDigestFindingRequest,
    ) -> Result<IndexDriftConfirmedCandidateRecordOutcome, IndexDriftConfirmedCandidateRecordError>
    {
        if !materialized_candidate_matches(transaction, candidate).await? {
            return Ok(IndexDriftConfirmedCandidateRecordOutcome::NotRecorded(
                IndexDriftConfirmedCandidateNotRecordedReason::MaterializedChanged,
            ));
        }
        let outcome = record_finding_in_transaction(transaction, request).await?;
        Ok(IndexDriftConfirmedCandidateRecordOutcome::Recorded(outcome))
    }
}

impl fmt::Debug for PostgresIndexDriftConfirmedCandidateWriter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexDriftConfirmedCandidateWriter")
            .finish_non_exhaustive()
    }
}

/// Constructs the internal writer without publishing a runtime or transport capability.
pub fn materialize_postgres_index_drift_confirmed_candidate_writer(
    db: DatabaseConnection,
) -> Result<PostgresIndexDriftConfirmedCandidateWriter, IndexDriftConfirmedCandidateRecordError> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(IndexDriftConfirmedCandidateRecordError::UnsupportedBackend);
    }
    Ok(PostgresIndexDriftConfirmedCandidateWriter::new(db))
}

fn finding_request(
    candidate: &IndexDriftConfirmedCandidate,
) -> Result<IndexDriftDigestFindingRequest, IndexDriftConfirmedCandidateRecordError> {
    match candidate {
        IndexDriftConfirmedCandidate::MissingEntity(candidate) => {
            missing_entity_finding_request(candidate)
        }
        IndexDriftConfirmedCandidate::OrphanLink(candidate) => {
            orphan_link_finding_request(candidate)
        }
    }
}

fn missing_entity_finding_request(
    candidate: &IndexDriftConfirmedMissingEntity,
) -> Result<IndexDriftDigestFindingRequest, IndexDriftConfirmedCandidateRecordError> {
    build_request(
        candidate.key(),
        MISSING_ENTITY_CHECK.to_owned(),
        missing_entity_digest(candidate, b"owner_absent"),
        missing_entity_digest(candidate, b"index_present"),
    )
}

fn orphan_link_finding_request(
    candidate: &IndexDriftConfirmedOrphanLink,
) -> Result<IndexDriftDigestFindingRequest, IndexDriftConfirmedCandidateRecordError> {
    build_request(
        candidate.source_key(),
        format!(
            "{ORPHAN_LINK_CHECK_PREFIX}{}",
            orphan_link_identity_digest(candidate)
        ),
        orphan_link_digest(candidate, b"target_absent"),
        orphan_link_digest(candidate, b"source_link_present"),
    )
}

fn build_request(
    key: &EntityKey,
    check_name: String,
    expected_digest: String,
    actual_digest: String,
) -> Result<IndexDriftDigestFindingRequest, IndexDriftConfirmedCandidateRecordError> {
    if expected_digest == actual_digest {
        return Err(IndexDriftConfirmedCandidateRecordError::InvalidEvidence);
    }
    IndexDriftDigestFindingRequest::new(
        key.tenant_id,
        check_name,
        IndexDriftFindingSeverity::Error,
        finding_scope(key),
        expected_digest,
        actual_digest,
    )
    .map_err(|_| IndexDriftConfirmedCandidateRecordError::InvalidEvidence)
}

fn finding_scope(key: &EntityKey) -> IndexDriftFindingScope {
    match &key.locale {
        Some(locale) => IndexDriftFindingScope::Entity {
            schema: key.schema.clone(),
            entity_id: key.entity_id,
            locale: locale.clone(),
        },
        None => IndexDriftFindingScope::EntityWithoutLocale {
            schema: key.schema.clone(),
            entity_id: key.entity_id,
        },
    }
}

fn missing_entity_digest(candidate: &IndexDriftConfirmedMissingEntity, state: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, MISSING_EVIDENCE_DOMAIN);
    hash_component(&mut hasher, state);
    hash_entity_key(&mut hasher, candidate.key());
    hash_component(
        &mut hasher,
        &candidate.indexed_source_version().to_be_bytes(),
    );
    hash_component(
        &mut hasher,
        &candidate.absence_source_version().to_be_bytes(),
    );
    hex::encode(hasher.finalize())
}

fn orphan_link_identity_digest(candidate: &IndexDriftConfirmedOrphanLink) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, ORPHAN_IDENTITY_DOMAIN);
    hash_component(&mut hasher, candidate.link_name().as_str().as_bytes());
    hash_component(&mut hasher, &candidate.ordinal().to_be_bytes());
    hash_linked_key(&mut hasher, candidate.target());
    hash_component(
        &mut hasher,
        &candidate.target_absence_source_version().to_be_bytes(),
    );
    hex::encode(hasher.finalize())
}

fn orphan_link_digest(candidate: &IndexDriftConfirmedOrphanLink, state: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, ORPHAN_EVIDENCE_DOMAIN);
    hash_component(&mut hasher, state);
    hash_entity_key(&mut hasher, candidate.source_key());
    hash_component(
        &mut hasher,
        &candidate.indexed_source_version().to_be_bytes(),
    );
    hash_component(&mut hasher, candidate.link_name().as_str().as_bytes());
    hash_component(&mut hasher, &candidate.ordinal().to_be_bytes());
    hash_linked_key(&mut hasher, candidate.target());
    hash_component(
        &mut hasher,
        &candidate.target_absence_source_version().to_be_bytes(),
    );
    hex::encode(hasher.finalize())
}

fn hash_entity_key(hasher: &mut Sha256, key: &EntityKey) {
    hash_component(hasher, key.tenant_id.as_bytes());
    hash_schema(hasher, &key.schema);
    hash_component(hasher, key.entity_id.as_bytes());
    hash_locale(hasher, key.locale.as_ref());
}

fn hash_linked_key(hasher: &mut Sha256, key: &crate::LinkedEntityKey) {
    hash_schema(hasher, &key.schema);
    hash_component(hasher, key.entity_id.as_bytes());
    hash_locale(hasher, key.locale.as_ref());
}

fn hash_schema(hasher: &mut Sha256, schema: &crate::SchemaRef) {
    hash_component(hasher, schema.module.as_str().as_bytes());
    hash_component(hasher, schema.entity.as_str().as_bytes());
    hash_component(hasher, &schema.version.get().to_be_bytes());
}

fn hash_locale(hasher: &mut Sha256, locale: Option<&crate::LocaleKey>) {
    match locale {
        Some(locale) => {
            hash_component(hasher, b"locale");
            hash_component(hasher, locale.as_str().as_bytes());
        }
        None => hash_component(hasher, b"no_locale"),
    }
}

fn hash_component(hasher: &mut Sha256, value: &[u8]) {
    let length =
        u64::try_from(value.len()).expect("bounded confirmed-candidate evidence component");
    hasher.update(length.to_be_bytes());
    hasher.update(value);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PersistedFindingScope {
    scope_kind: String,
    module_name: Option<String>,
    entity_name: Option<String>,
    schema_version: Option<i64>,
    entity_id: Option<Uuid>,
    locale_key: Option<String>,
}

impl PersistedFindingScope {
    fn from_scope(
        scope: &IndexDriftFindingScope,
    ) -> Result<Self, IndexDriftConfirmedCandidateRecordError> {
        match scope {
            IndexDriftFindingScope::Global => Ok(Self {
                scope_kind: "global".to_owned(),
                module_name: None,
                entity_name: None,
                schema_version: None,
                entity_id: None,
                locale_key: None,
            }),
            IndexDriftFindingScope::Schema { schema } => Ok(Self {
                scope_kind: "schema".to_owned(),
                module_name: Some(schema.module.as_str().to_owned()),
                entity_name: Some(schema.entity.as_str().to_owned()),
                schema_version: Some(i64::from(schema.version.get())),
                entity_id: None,
                locale_key: None,
            }),
            IndexDriftFindingScope::Entity {
                schema,
                entity_id,
                locale,
            } => Ok(Self {
                scope_kind: "entity".to_owned(),
                module_name: Some(schema.module.as_str().to_owned()),
                entity_name: Some(schema.entity.as_str().to_owned()),
                schema_version: Some(i64::from(schema.version.get())),
                entity_id: Some(*entity_id),
                locale_key: Some(locale.as_str().to_owned()),
            }),
            IndexDriftFindingScope::EntityWithoutLocale { schema, entity_id } => Ok(Self {
                scope_kind: "entity".to_owned(),
                module_name: Some(schema.module.as_str().to_owned()),
                entity_name: Some(schema.entity.as_str().to_owned()),
                schema_version: Some(i64::from(schema.version.get())),
                entity_id: Some(*entity_id),
                locale_key: None,
            }),
        }
    }
}

#[derive(Debug)]
struct StoredFinding {
    finding_id: Uuid,
    check_name: String,
    state: String,
    scope: PersistedFindingScope,
}

async fn record_finding_in_transaction(
    transaction: &DatabaseTransaction,
    request: &IndexDriftDigestFindingRequest,
) -> Result<IndexDriftFindingWriteOutcome, IndexDriftConfirmedCandidateRecordError> {
    lock_finding_key(transaction, request).await?;
    let expected_scope = PersistedFindingScope::from_scope(request.scope())?;
    if let Some(existing) = load_existing_finding(transaction, request).await? {
        return refresh_existing_finding(transaction, request, &expected_scope, existing).await;
    }

    let finding_id = Uuid::new_v4();
    let inserted = transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO index_consistency_findings (tenant_id, finding_id, finding_key, check_name, severity, state, scope_kind, module_name, entity_name, schema_version, entity_id, locale_key, expected_digest, actual_digest, details) VALUES ($1, $2, $3, $4, $5, 'open', $6, $7, $8, $9, $10, $11, $12, $13, $14) ON CONFLICT (tenant_id, finding_key) DO NOTHING",
            insert_values(request, finding_id, &expected_scope),
        ))
        .await
        .map_err(|_| IndexDriftConfirmedCandidateRecordError::Storage)?;
    if inserted.rows_affected() == 1 {
        return Ok(IndexDriftFindingWriteOutcome::Created {
            finding_id,
            finding_key: request.finding_key().to_owned(),
        });
    }

    let existing = load_existing_finding(transaction, request)
        .await?
        .ok_or(IndexDriftConfirmedCandidateRecordError::Storage)?;
    refresh_existing_finding(transaction, request, &expected_scope, existing).await
}

async fn lock_finding_key(
    transaction: &DatabaseTransaction,
    request: &IndexDriftDigestFindingRequest,
) -> Result<(), IndexDriftConfirmedCandidateRecordError> {
    let lock_key = format!(
        "index-drift-finding\u{1f}{}\u{1f}{}",
        request.tenant_id(),
        request.finding_key(),
    );
    transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![lock_key.into()],
        ))
        .await
        .map_err(|_| IndexDriftConfirmedCandidateRecordError::Storage)?;
    Ok(())
}

async fn load_existing_finding(
    transaction: &DatabaseTransaction,
    request: &IndexDriftDigestFindingRequest,
) -> Result<Option<StoredFinding>, IndexDriftConfirmedCandidateRecordError> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT finding_id, check_name, state, scope_kind, module_name, entity_name, CAST(schema_version AS BIGINT) AS schema_version_value, entity_id, locale_key FROM index_consistency_findings WHERE tenant_id = $1 AND finding_key = $2 FOR UPDATE",
            vec![request.tenant_id().into(), request.finding_key().to_owned().into()],
        ))
        .await
        .map_err(|_| IndexDriftConfirmedCandidateRecordError::Storage)?
        .map(decode_stored_finding)
        .transpose()
}

fn decode_stored_finding(
    row: QueryResult,
) -> Result<StoredFinding, IndexDriftConfirmedCandidateRecordError> {
    let finding_id = row
        .try_get::<Uuid>("", "finding_id")
        .map_err(|_| IndexDriftConfirmedCandidateRecordError::FindingContract)?;
    if finding_id.is_nil() {
        return Err(IndexDriftConfirmedCandidateRecordError::FindingContract);
    }
    Ok(StoredFinding {
        finding_id,
        check_name: row
            .try_get("", "check_name")
            .map_err(|_| IndexDriftConfirmedCandidateRecordError::FindingContract)?,
        state: row
            .try_get("", "state")
            .map_err(|_| IndexDriftConfirmedCandidateRecordError::FindingContract)?,
        scope: PersistedFindingScope {
            scope_kind: row
                .try_get("", "scope_kind")
                .map_err(|_| IndexDriftConfirmedCandidateRecordError::FindingContract)?,
            module_name: row
                .try_get("", "module_name")
                .map_err(|_| IndexDriftConfirmedCandidateRecordError::FindingContract)?,
            entity_name: row
                .try_get("", "entity_name")
                .map_err(|_| IndexDriftConfirmedCandidateRecordError::FindingContract)?,
            schema_version: row
                .try_get("", "schema_version_value")
                .map_err(|_| IndexDriftConfirmedCandidateRecordError::FindingContract)?,
            entity_id: row
                .try_get("", "entity_id")
                .map_err(|_| IndexDriftConfirmedCandidateRecordError::FindingContract)?,
            locale_key: row
                .try_get("", "locale_key")
                .map_err(|_| IndexDriftConfirmedCandidateRecordError::FindingContract)?,
        },
    })
}

async fn refresh_existing_finding(
    transaction: &DatabaseTransaction,
    request: &IndexDriftDigestFindingRequest,
    expected_scope: &PersistedFindingScope,
    existing: StoredFinding,
) -> Result<IndexDriftFindingWriteOutcome, IndexDriftConfirmedCandidateRecordError> {
    if existing.check_name != request.check_name() || existing.scope != *expected_scope {
        return Err(IndexDriftConfirmedCandidateRecordError::FindingContract);
    }
    let (sql, outcome) = match existing.state.as_str() {
        "open" => (
            "UPDATE index_consistency_findings SET severity = $3, expected_digest = $4, actual_digest = $5, details = $6, last_detected_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND finding_id = $2 AND finding_key = $7 AND state = 'open'",
            IndexDriftFindingWriteOutcome::Refreshed {
                finding_id: existing.finding_id,
                finding_key: request.finding_key().to_owned(),
            },
        ),
        "resolved" => (
            "UPDATE index_consistency_findings SET severity = $3, state = 'open', expected_digest = $4, actual_digest = $5, details = $6, last_detected_at = CURRENT_TIMESTAMP, closed_at = NULL WHERE tenant_id = $1 AND finding_id = $2 AND finding_key = $7 AND state = 'resolved'",
            IndexDriftFindingWriteOutcome::Reopened {
                finding_id: existing.finding_id,
                finding_key: request.finding_key().to_owned(),
            },
        ),
        "ignored" => (
            "UPDATE index_consistency_findings SET severity = $3, expected_digest = $4, actual_digest = $5, details = $6, last_detected_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND finding_id = $2 AND finding_key = $7 AND state = 'ignored'",
            IndexDriftFindingWriteOutcome::Suppressed {
                finding_id: existing.finding_id,
                finding_key: request.finding_key().to_owned(),
            },
        ),
        _ => return Err(IndexDriftConfirmedCandidateRecordError::FindingContract),
    };
    let updated = transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            update_values(request, existing.finding_id),
        ))
        .await
        .map_err(|_| IndexDriftConfirmedCandidateRecordError::Storage)?;
    if updated.rows_affected() != 1 {
        return Err(IndexDriftConfirmedCandidateRecordError::Storage);
    }
    Ok(outcome)
}

fn insert_values(
    request: &IndexDriftDigestFindingRequest,
    finding_id: Uuid,
    scope: &PersistedFindingScope,
) -> Vec<SqlValue> {
    vec![
        request.tenant_id().into(),
        finding_id.into(),
        request.finding_key().to_owned().into(),
        request.check_name().to_owned().into(),
        severity_value(request.severity()).to_owned().into(),
        scope.scope_kind.clone().into(),
        scope.module_name.clone().into(),
        scope.entity_name.clone().into(),
        scope.schema_version.into(),
        scope.entity_id.into(),
        scope.locale_key.clone().into(),
        request.expected_digest().to_owned().into(),
        request.actual_digest().to_owned().into(),
        SqlValue::Json(Some(Box::new(json!({
            "contract": FINDING_DETAILS_CONTRACT
        })))),
    ]
}

fn update_values(request: &IndexDriftDigestFindingRequest, finding_id: Uuid) -> Vec<SqlValue> {
    vec![
        request.tenant_id().into(),
        finding_id.into(),
        severity_value(request.severity()).to_owned().into(),
        request.expected_digest().to_owned().into(),
        request.actual_digest().to_owned().into(),
        SqlValue::Json(Some(Box::new(json!({
            "contract": FINDING_DETAILS_CONTRACT
        })))),
        request.finding_key().to_owned().into(),
    ]
}

fn severity_value(value: IndexDriftFindingSeverity) -> &'static str {
    match value {
        IndexDriftFindingSeverity::Info => "info",
        IndexDriftFindingSeverity::Warning => "warning",
        IndexDriftFindingSeverity::Error => "error",
    }
}

async fn materialized_candidate_matches(
    transaction: &DatabaseTransaction,
    candidate: &IndexDriftConfirmedCandidate,
) -> Result<bool, IndexDriftConfirmedCandidateRecordError> {
    match candidate {
        IndexDriftConfirmedCandidate::MissingEntity(candidate) => {
            materialized_missing_entity_matches(transaction, candidate).await
        }
        IndexDriftConfirmedCandidate::OrphanLink(candidate) => {
            materialized_orphan_link_matches(transaction, candidate).await
        }
    }
}

async fn materialized_missing_entity_matches(
    transaction: &DatabaseTransaction,
    candidate: &IndexDriftConfirmedMissingEntity,
) -> Result<bool, IndexDriftConfirmedCandidateRecordError> {
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT CAST(source_version AS TEXT) AS source_version_text, is_deleted FROM index_entities WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4 AND entity_id = $5 AND locale_key = $6 LIMIT 1 FOR SHARE",
            entity_values(candidate.key()),
        ))
        .await
        .map_err(|_| IndexDriftConfirmedCandidateRecordError::Storage)?;
    let Some(row) = row else {
        return Ok(false);
    };
    let source_version = positive_source_version(&row)?;
    let is_deleted = row
        .try_get::<bool>("", "is_deleted")
        .map_err(|_| IndexDriftConfirmedCandidateRecordError::FindingContract)?;
    Ok(!is_deleted && source_version == candidate.indexed_source_version())
}

async fn materialized_orphan_link_matches(
    transaction: &DatabaseTransaction,
    candidate: &IndexDriftConfirmedOrphanLink,
) -> Result<bool, IndexDriftConfirmedCandidateRecordError> {
    if !materialized_source_matches(transaction, candidate).await? {
        return Ok(false);
    }
    if !materialized_link_matches(transaction, candidate).await? {
        return Ok(false);
    }
    materialized_target_absent(transaction, candidate).await
}

async fn materialized_source_matches(
    transaction: &DatabaseTransaction,
    candidate: &IndexDriftConfirmedOrphanLink,
) -> Result<bool, IndexDriftConfirmedCandidateRecordError> {
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT CAST(source_version AS TEXT) AS source_version_text, is_deleted FROM index_entities WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4 AND entity_id = $5 AND locale_key = $6 LIMIT 1 FOR SHARE",
            entity_values(candidate.source_key()),
        ))
        .await
        .map_err(|_| IndexDriftConfirmedCandidateRecordError::Storage)?;
    let Some(row) = row else {
        return Ok(false);
    };
    let source_version = positive_source_version(&row)?;
    let is_deleted = row
        .try_get::<bool>("", "is_deleted")
        .map_err(|_| IndexDriftConfirmedCandidateRecordError::FindingContract)?;
    Ok(!is_deleted && source_version == candidate.indexed_source_version())
}

async fn materialized_link_matches(
    transaction: &DatabaseTransaction,
    candidate: &IndexDriftConfirmedOrphanLink,
) -> Result<bool, IndexDriftConfirmedCandidateRecordError> {
    let source = candidate.source_key();
    let target = candidate.target();
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT 1 AS link_match FROM index_links WHERE tenant_id = $1 AND source_module = $2 AND source_entity = $3 AND source_schema_version = $4 AND source_entity_id = $5 AND source_locale_key = $6 AND CAST(source_version AS TEXT) = $7 AND link_name = $8 AND ordinal = $9 AND target_module = $10 AND target_entity = $11 AND target_schema_version = $12 AND target_entity_id = $13 AND target_locale_key = $14 LIMIT 1 FOR SHARE",
            vec![
                source.tenant_id.into(),
                source.schema.module.as_str().to_owned().into(),
                source.schema.entity.as_str().to_owned().into(),
                i64::from(source.schema.version.get()).into(),
                source.entity_id.into(),
                persisted_locale(source.locale.as_ref()).into(),
                candidate.indexed_source_version().to_string().into(),
                candidate.link_name().as_str().to_owned().into(),
                i64::from(candidate.ordinal()).into(),
                target.schema.module.as_str().to_owned().into(),
                target.schema.entity.as_str().to_owned().into(),
                i64::from(target.schema.version.get()).into(),
                target.entity_id.into(),
                persisted_locale(target.locale.as_ref()).into(),
            ],
        ))
        .await
        .map_err(|_| IndexDriftConfirmedCandidateRecordError::Storage)?;
    Ok(row.is_some())
}

async fn materialized_target_absent(
    transaction: &DatabaseTransaction,
    candidate: &IndexDriftConfirmedOrphanLink,
) -> Result<bool, IndexDriftConfirmedCandidateRecordError> {
    let target = candidate.target();
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT CAST(source_version AS TEXT) AS source_version_text, is_deleted FROM index_entities WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4 AND entity_id = $5 AND locale_key = $6 LIMIT 1 FOR SHARE",
            vec![
                candidate.source_key().tenant_id.into(),
                target.schema.module.as_str().to_owned().into(),
                target.schema.entity.as_str().to_owned().into(),
                i64::from(target.schema.version.get()).into(),
                target.entity_id.into(),
                persisted_locale(target.locale.as_ref()).into(),
            ],
        ))
        .await
        .map_err(|_| IndexDriftConfirmedCandidateRecordError::Storage)?;
    let Some(row) = row else {
        return Ok(true);
    };
    let _ = positive_source_version(&row)?;
    let is_deleted = row
        .try_get::<bool>("", "is_deleted")
        .map_err(|_| IndexDriftConfirmedCandidateRecordError::FindingContract)?;
    Ok(is_deleted)
}

fn entity_values(key: &EntityKey) -> Vec<SqlValue> {
    vec![
        key.tenant_id.into(),
        key.schema.module.as_str().to_owned().into(),
        key.schema.entity.as_str().to_owned().into(),
        i64::from(key.schema.version.get()).into(),
        key.entity_id.into(),
        persisted_locale(key.locale.as_ref()).into(),
    ]
}

fn positive_source_version(
    row: &QueryResult,
) -> Result<u64, IndexDriftConfirmedCandidateRecordError> {
    let value = row
        .try_get::<String>("", "source_version_text")
        .map_err(|_| IndexDriftConfirmedCandidateRecordError::FindingContract)?;
    u64::from_str(&value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or(IndexDriftConfirmedCandidateRecordError::FindingContract)
}

fn persisted_locale(locale: Option<&crate::LocaleKey>) -> String {
    locale
        .map(|value| value.as_str().to_owned())
        .unwrap_or_default()
}
