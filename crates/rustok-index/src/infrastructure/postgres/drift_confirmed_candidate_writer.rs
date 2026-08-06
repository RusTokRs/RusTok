use std::{fmt, str::FromStr};

use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend,
    IsolationLevel, QueryResult, Statement, TransactionTrait,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    EntityKey, IndexDriftConfirmedCandidate, IndexDriftConfirmedMissingEntity,
    IndexDriftConfirmedOrphanLink,
};

use super::{
    drift_finding_inspector::{IndexDriftFindingScope, IndexDriftFindingSeverity},
    drift_finding_writer::{
        IndexDriftDigestFindingRequest, IndexDriftFindingWriteError,
        IndexDriftFindingWriteOutcome, PostgresIndexDriftFindingWriter,
    },
};

const MISSING_ENTITY_CHECK: &str = "index.confirmed_missing_entity";
const ORPHAN_LINK_CHECK_PREFIX: &str = "index.confirmed_orphan_link.";
const MISSING_EVIDENCE_DOMAIN: &[u8] = b"index_confirmed_missing_entity_evidence_v1";
const ORPHAN_EVIDENCE_DOMAIN: &[u8] = b"index_confirmed_orphan_link_evidence_v1";
const ORPHAN_IDENTITY_DOMAIN: &[u8] = b"index_confirmed_orphan_link_identity_v1";

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
/// The adapter derives all finding identity and digest evidence from typed confirmation output,
/// revalidates the exact materialized shape in the same serializable transaction, and delegates to
/// the existing idempotent finding writer. It accepts no raw details JSON, lifecycle state, actor,
/// schedule, cursor, or repair instruction.
#[derive(Clone)]
pub struct PostgresIndexDriftConfirmedCandidateWriter {
    db: DatabaseConnection,
    finding_writer: PostgresIndexDriftFindingWriter,
}

impl PostgresIndexDriftConfirmedCandidateWriter {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            finding_writer: PostgresIndexDriftFindingWriter::new(db.clone()),
            db,
        }
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
                Ok(IndexDriftConfirmedCandidateRecordOutcome::NotRecorded(reason))
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
        let outcome = self
            .finding_writer
            .record_in_transaction(transaction, request)
            .await
            .map_err(map_finding_error)?;
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
    let expected_digest = missing_entity_digest(candidate, b"owner_absent");
    let actual_digest = missing_entity_digest(candidate, b"index_present");
    build_request(
        candidate.key(),
        MISSING_ENTITY_CHECK.to_owned(),
        expected_digest,
        actual_digest,
    )
}

fn orphan_link_finding_request(
    candidate: &IndexDriftConfirmedOrphanLink,
) -> Result<IndexDriftDigestFindingRequest, IndexDriftConfirmedCandidateRecordError> {
    let check_name = format!(
        "{ORPHAN_LINK_CHECK_PREFIX}{}",
        orphan_link_identity_digest(candidate)
    );
    let expected_digest = orphan_link_digest(candidate, b"target_absent");
    let actual_digest = orphan_link_digest(candidate, b"source_link_present");
    build_request(
        candidate.source_key(),
        check_name,
        expected_digest,
        actual_digest,
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
    let length = u64::try_from(value.len()).expect("bounded confirmed-candidate evidence component");
    hasher.update(length.to_be_bytes());
    hasher.update(value);
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
    let key = candidate.key();
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT CAST(source_version AS TEXT) AS source_version_text, is_deleted FROM index_entities WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4 AND entity_id = $5 AND locale_key = $6 LIMIT 1 FOR SHARE",
            entity_values(key),
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
        .query_one(Statement::from_sql_and_values(
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
        .query_one(Statement::from_sql_and_values(
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
        .query_one(Statement::from_sql_and_values(
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

fn entity_values(key: &EntityKey) -> Vec<sea_orm::Value> {
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

fn map_finding_error(
    error: IndexDriftFindingWriteError,
) -> IndexDriftConfirmedCandidateRecordError {
    match error {
        IndexDriftFindingWriteError::UnsupportedBackend => {
            IndexDriftConfirmedCandidateRecordError::UnsupportedBackend
        }
        IndexDriftFindingWriteError::Storage => IndexDriftConfirmedCandidateRecordError::Storage,
        _ => IndexDriftConfirmedCandidateRecordError::FindingContract,
    }
}
