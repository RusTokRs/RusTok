use std::{fmt, sync::Arc};

use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value as SqlValue,
};
use sha2::{Digest, Sha256};

use crate::{
    EntityKey, IndexDriftAuthorizedRepairCommand, IndexDriftRepairAuthorizer,
    IndexDriftRepairEvidence, IndexDriftRepairEvidenceReader, IndexDriftRepairEvidenceState,
    IndexDriftRepairFailure, IndexDriftRepairFinding, IndexDriftRepairOwner,
    IndexDriftRepairOwnerOutcome, IndexDriftRepairOwnerRegistry,
    IndexDriftRepairReservationOutcome, IndexDriftRepairService, IndexDriftRepairStore,
    IndexDriftRepairStoreCompletionOutcome, IndexDriftRepairTarget, IndexDriftRepairTargetKind,
    IndexDriftRepairTicket, IndexMutation, IndexSourceAbsenceError, IndexSourceError,
    IndexSourceFailureKind, IndexSourceLoadRequest, SchemaRegistry,
    SharedIndexSourceAbsenceRegistry, SharedIndexSourceRegistry,
};

use super::{
    drift_repair::materialize_postgres_index_drift_repair_store,
    drift_repair_recovery::{
        RecoveryAwareIndexDriftRepairOwner, RecoveryAwareIndexDriftRepairStore,
    },
    mutation_store::{
        MutationApplyOutcome, MutationDelivery, MutationStorageError, PostgresMutationStore,
    },
};

const OWNER_NAME: &str = "index_missing_entity_delete_owner";
const DELIVERY_SOURCE: &str = "index_drift_repair_missing_entity";
const EVIDENCE_DOMAIN: &[u8] = b"index_missing_entity_repair_evidence_v1";
const OWNER_RECEIPT_DOMAIN: &[u8] = b"index_missing_entity_repair_owner_receipt_v1";
const TARGET_UNSUPPORTED: &str = "index_drift_repair_target_unsupported";
const SOURCE_UNAVAILABLE: &str = "index_drift_repair_source_unavailable";
const SOURCE_REJECTED: &str = "index_drift_repair_source_rejected";
const SOURCE_CONTRACT_INVALID: &str = "index_drift_repair_source_contract_invalid";
const SOURCE_CHANGED: &str = "index_drift_repair_source_changed";
const ABSENCE_UNAVAILABLE: &str = "index_drift_repair_absence_unavailable";
const MATERIALIZED_UNAVAILABLE: &str = "index_drift_repair_materialized_unavailable";
const OWNER_UNAVAILABLE: &str = "index_drift_repair_owner_unavailable";
const OWNER_CONTRACT_INVALID: &str = "index_drift_repair_owner_contract_invalid";
const UNSUPPORTED_BACKEND: &str = "index_drift_repair_unsupported_backend";
const COMPONENTS_INVALID: &str = "index_drift_repair_components_invalid";

#[derive(Clone)]
pub struct PostgresIndexDriftMissingEntityEvidenceReader {
    db: DatabaseConnection,
    sources: SharedIndexSourceRegistry,
    absence: SharedIndexSourceAbsenceRegistry,
}

impl PostgresIndexDriftMissingEntityEvidenceReader {
    pub fn new(
        db: DatabaseConnection,
        sources: SharedIndexSourceRegistry,
        absence: SharedIndexSourceAbsenceRegistry,
    ) -> Result<Self, IndexDriftRepairFailure> {
        ensure_postgres(&db)?;
        Ok(Self {
            db,
            sources,
            absence,
        })
    }

    async fn capture(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
        phase: MissingEntityEvidencePhase,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
        let target = exact_missing_entity_target(authorized, finding)?;
        let first_authority = self.load_authority(target.key).await?;
        let materialized = self.load_materialized(target.key).await?;
        let second_authority = self.load_authority(target.key).await?;
        if first_authority != second_authority {
            return Err(retryable_failure(SOURCE_CHANGED));
        }

        let state = classify_evidence(target, first_authority, materialized, phase);
        let digest = evidence_digest(target, first_authority, materialized, state);
        IndexDriftRepairEvidence::new(state, digest)
            .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))
    }

    async fn load_authority(
        &self,
        key: &EntityKey,
    ) -> Result<MissingEntityAuthority, IndexDriftRepairFailure> {
        let request = IndexSourceLoadRequest::new(vec![key.clone()])
            .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
        let batch = self.sources.load(request).await.map_err(map_source_error)?;
        let mut mutations = batch.into_mutations();
        if mutations.len() > 1 {
            return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
        }
        match mutations.pop() {
            Some(IndexMutation::Upsert { record, .. }) => {
                if &record.key != key || record.source_version == 0 {
                    return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
                }
                Ok(MissingEntityAuthority::Present(record.source_version))
            }
            Some(IndexMutation::Delete {
                key: returned_key,
                source_version,
                ..
            }) => {
                if &returned_key != key || source_version == 0 {
                    return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
                }
                Ok(MissingEntityAuthority::Absent(source_version))
            }
            None => {
                if self.absence.provider_for_schema(&key.schema).is_none() {
                    return Err(permanent_failure(ABSENCE_UNAVAILABLE));
                }
                let watermark = self
                    .absence
                    .load(key.clone())
                    .await
                    .map_err(map_absence_error)?
                    .ok_or_else(|| permanent_failure(ABSENCE_UNAVAILABLE))?;
                if watermark.key() != key || watermark.source_version() == 0 {
                    return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
                }
                Ok(MissingEntityAuthority::Absent(watermark.source_version()))
            }
        }
    }

    async fn load_materialized(
        &self,
        key: &EntityKey,
    ) -> Result<MissingEntityMaterialized, IndexDriftRepairFailure> {
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT CAST(source_version AS TEXT) AS source_version_text, is_deleted FROM index_entities WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4 AND entity_id = $5 AND locale_key = $6 LIMIT 1",
                entity_values(key),
            ))
            .await
            .map_err(|_| retryable_failure(MATERIALIZED_UNAVAILABLE))?;
        let Some(row) = row else {
            return Ok(MissingEntityMaterialized::Missing);
        };
        let source_version = positive_source_version(&row)?;
        let is_deleted = row
            .try_get::<bool>("", "is_deleted")
            .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
        Ok(if is_deleted {
            MissingEntityMaterialized::Deleted(source_version)
        } else {
            MissingEntityMaterialized::Live(source_version)
        })
    }
}

#[async_trait]
impl IndexDriftRepairEvidenceReader for PostgresIndexDriftMissingEntityEvidenceReader {
    async fn capture_before(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
        self.capture(authorized, finding, MissingEntityEvidencePhase::Before)
            .await
    }

    async fn capture_after(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
        before: &IndexDriftRepairEvidence,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
        if before.state() != IndexDriftRepairEvidenceState::Repairable {
            return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
        }
        self.capture(authorized, finding, MissingEntityEvidencePhase::After)
            .await
    }
}

impl fmt::Debug for PostgresIndexDriftMissingEntityEvidenceReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexDriftMissingEntityEvidenceReader")
            .field("source_count", &self.sources.len())
            .field("absence_provider_count", &self.absence.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct PostgresIndexDriftMissingEntityRepairOwner {
    mutation_store: PostgresMutationStore,
    schemas: Arc<SchemaRegistry>,
}

impl PostgresIndexDriftMissingEntityRepairOwner {
    pub fn new(
        db: DatabaseConnection,
        schemas: Arc<SchemaRegistry>,
    ) -> Result<Self, IndexDriftRepairFailure> {
        ensure_postgres(&db)?;
        Ok(Self {
            mutation_store: PostgresMutationStore::new(db),
            schemas,
        })
    }
}

#[async_trait]
impl IndexDriftRepairOwner for PostgresIndexDriftMissingEntityRepairOwner {
    fn owner_name(&self) -> &str {
        OWNER_NAME
    }

    fn target_kind(&self) -> IndexDriftRepairTargetKind {
        IndexDriftRepairTargetKind::MissingEntity
    }

    async fn repair(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
        before: &IndexDriftRepairEvidence,
    ) -> Result<IndexDriftRepairOwnerOutcome, IndexDriftRepairFailure> {
        let target = exact_missing_entity_target(authorized, finding)?;
        if before.state() != IndexDriftRepairEvidenceState::Repairable
            || target.absence_source_version <= target.indexed_source_version
        {
            return IndexDriftRepairOwnerOutcome::not_applied("missing_entity_not_repairable")
                .map_err(|_| permanent_failure(OWNER_CONTRACT_INVALID));
        }

        let mutation = IndexMutation::Delete {
            event_id: authorized.command().command_id(),
            key: target.key.clone(),
            source_version: target.absence_source_version,
        };
        let delivery = MutationDelivery::from_event(DELIVERY_SOURCE, mutation)
            .map_err(|_| permanent_failure(OWNER_CONTRACT_INVALID))?;
        let outcome = self
            .mutation_store
            .apply(self.schemas.as_ref(), &delivery)
            .await
            .map_err(map_mutation_error)?;
        let receipt = owner_receipt_digest(authorized, finding, target, &outcome);
        IndexDriftRepairOwnerOutcome::applied(receipt)
            .map_err(|_| permanent_failure(OWNER_CONTRACT_INVALID))
    }
}

impl fmt::Debug for PostgresIndexDriftMissingEntityRepairOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexDriftMissingEntityRepairOwner")
            .field("schema_count", &self.schemas.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct MissingEntityOnlyRepairStore {
    inner: Arc<dyn IndexDriftRepairStore>,
}

#[async_trait]
impl IndexDriftRepairStore for MissingEntityOnlyRepairStore {
    async fn reserve(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
    ) -> Result<IndexDriftRepairReservationOutcome, IndexDriftRepairFailure> {
        if authorized.command().target().kind() != IndexDriftRepairTargetKind::MissingEntity {
            return Err(permanent_failure(TARGET_UNSUPPORTED));
        }
        self.inner.reserve(authorized).await
    }

    async fn complete(
        &self,
        ticket: &IndexDriftRepairTicket,
        completion: &crate::IndexDriftRepairCompletion,
    ) -> Result<IndexDriftRepairStoreCompletionOutcome, IndexDriftRepairFailure> {
        self.inner.complete(ticket, completion).await
    }
}

/// Composes the first concrete targeted-repair path without publishing a runtime extension.
///
/// Only confirmed missing-entity findings are accepted. The supplied authorizer remains the owner
/// of operator admission; this helper adds the exact source/absence/materialized evidence reader,
/// one idempotent PostgreSQL delete owner, an active-recovery fence around the owner call, and the
/// durable repair store behind missing-only and recovery-aware gates.
pub fn materialize_postgres_index_drift_missing_entity_repair_service(
    authorizer: Arc<dyn IndexDriftRepairAuthorizer>,
    db: DatabaseConnection,
    schemas: Arc<SchemaRegistry>,
    sources: SharedIndexSourceRegistry,
    absence: SharedIndexSourceAbsenceRegistry,
) -> Result<IndexDriftRepairService, IndexDriftRepairFailure> {
    ensure_postgres(&db)?;
    let evidence = Arc::new(PostgresIndexDriftMissingEntityEvidenceReader::new(
        db.clone(),
        sources,
        absence,
    )?);
    let base_owner: Arc<dyn IndexDriftRepairOwner> = Arc::new(
        PostgresIndexDriftMissingEntityRepairOwner::new(db.clone(), schemas)?,
    );
    let owner: Arc<dyn IndexDriftRepairOwner> =
        Arc::new(RecoveryAwareIndexDriftRepairOwner::new(db.clone(), base_owner)?);
    let owners = IndexDriftRepairOwnerRegistry::new([owner])
        .map_err(|_| permanent_failure(COMPONENTS_INVALID))?;
    let store = materialize_postgres_index_drift_repair_store(db.clone())?;
    let recovery_store: Arc<dyn IndexDriftRepairStore> =
        Arc::new(RecoveryAwareIndexDriftRepairStore::new(db, store)?);
    let gated_store: Arc<dyn IndexDriftRepairStore> = Arc::new(MissingEntityOnlyRepairStore {
        inner: recovery_store,
    });
    Ok(IndexDriftRepairService::new_boxed(
        authorizer,
        evidence,
        owners,
        gated_store,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissingEntityAuthority {
    Present(u64),
    Absent(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissingEntityMaterialized {
    Missing,
    Live(u64),
    Deleted(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissingEntityEvidencePhase {
    Before,
    After,
}

#[derive(Clone, Copy)]
struct MissingEntityTargetRef<'a> {
    key: &'a EntityKey,
    indexed_source_version: u64,
    absence_source_version: u64,
}

fn exact_missing_entity_target<'a>(
    authorized: &'a IndexDriftAuthorizedRepairCommand,
    finding: &'a IndexDriftRepairFinding,
) -> Result<MissingEntityTargetRef<'a>, IndexDriftRepairFailure> {
    if authorized.command().finding_id() != finding.finding_id()
        || authorized.command().target() != finding.target()
    {
        return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
    }
    match finding.target() {
        IndexDriftRepairTarget::MissingEntity {
            key,
            indexed_source_version,
            absence_source_version,
        } => Ok(MissingEntityTargetRef {
            key,
            indexed_source_version: *indexed_source_version,
            absence_source_version: *absence_source_version,
        }),
        IndexDriftRepairTarget::OrphanLink { .. } => Err(permanent_failure(TARGET_UNSUPPORTED)),
    }
}

fn classify_evidence(
    target: MissingEntityTargetRef<'_>,
    authority: MissingEntityAuthority,
    materialized: MissingEntityMaterialized,
    phase: MissingEntityEvidencePhase,
) -> IndexDriftRepairEvidenceState {
    match (phase, authority, materialized) {
        (
            MissingEntityEvidencePhase::Before,
            MissingEntityAuthority::Absent(absence_version),
            MissingEntityMaterialized::Live(indexed_version),
        ) if absence_version == target.absence_source_version
            && indexed_version == target.indexed_source_version
            && absence_version > indexed_version =>
        {
            IndexDriftRepairEvidenceState::Repairable
        }
        (
            MissingEntityEvidencePhase::Before,
            MissingEntityAuthority::Absent(absence_version),
            MissingEntityMaterialized::Deleted(indexed_version),
        ) if absence_version == target.absence_source_version
            && indexed_version == target.absence_source_version =>
        {
            // A retry may begin after the idempotent delete committed but before the repair receipt.
            // Keep the before phase repairable so the same command UUID reaches the inbox duplicate
            // path and can still produce admitted after evidence.
            IndexDriftRepairEvidenceState::Repairable
        }
        (
            MissingEntityEvidencePhase::After,
            MissingEntityAuthority::Absent(absence_version),
            MissingEntityMaterialized::Deleted(indexed_version),
        ) if absence_version == target.absence_source_version
            && indexed_version == target.absence_source_version =>
        {
            IndexDriftRepairEvidenceState::Converged
        }
        _ => IndexDriftRepairEvidenceState::Changed,
    }
}

fn evidence_digest(
    target: MissingEntityTargetRef<'_>,
    authority: MissingEntityAuthority,
    materialized: MissingEntityMaterialized,
    state: IndexDriftRepairEvidenceState,
) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, EVIDENCE_DOMAIN);
    hash_entity_key(&mut hasher, target.key);
    hash_component(&mut hasher, &target.indexed_source_version.to_be_bytes());
    hash_component(&mut hasher, &target.absence_source_version.to_be_bytes());
    match authority {
        MissingEntityAuthority::Present(version) => {
            hash_component(&mut hasher, b"owner_present");
            hash_component(&mut hasher, &version.to_be_bytes());
        }
        MissingEntityAuthority::Absent(version) => {
            hash_component(&mut hasher, b"owner_absent");
            hash_component(&mut hasher, &version.to_be_bytes());
        }
    }
    match materialized {
        MissingEntityMaterialized::Missing => hash_component(&mut hasher, b"index_missing"),
        MissingEntityMaterialized::Live(version) => {
            hash_component(&mut hasher, b"index_live");
            hash_component(&mut hasher, &version.to_be_bytes());
        }
        MissingEntityMaterialized::Deleted(version) => {
            hash_component(&mut hasher, b"index_deleted");
            hash_component(&mut hasher, &version.to_be_bytes());
        }
    }
    hash_component(
        &mut hasher,
        match state {
            IndexDriftRepairEvidenceState::Repairable => b"repairable",
            IndexDriftRepairEvidenceState::Converged => b"converged",
            IndexDriftRepairEvidenceState::Changed => b"changed",
        },
    );
    hex::encode(hasher.finalize())
}

fn owner_receipt_digest(
    authorized: &IndexDriftAuthorizedRepairCommand,
    finding: &IndexDriftRepairFinding,
    target: MissingEntityTargetRef<'_>,
    outcome: &MutationApplyOutcome,
) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, OWNER_RECEIPT_DOMAIN);
    hash_component(&mut hasher, authorized.command().command_id().as_bytes());
    hash_component(&mut hasher, finding.finding_id().as_bytes());
    hash_entity_key(&mut hasher, target.key);
    hash_component(&mut hasher, &target.indexed_source_version.to_be_bytes());
    hash_component(&mut hasher, &target.absence_source_version.to_be_bytes());
    match outcome {
        MutationApplyOutcome::Applied { source_version } => {
            hash_component(&mut hasher, b"applied");
            hash_component(&mut hasher, &source_version.to_be_bytes());
        }
        MutationApplyOutcome::Duplicate { source_version } => {
            hash_component(&mut hasher, b"duplicate");
            hash_component(&mut hasher, &source_version.to_be_bytes());
        }
        MutationApplyOutcome::StaleIgnored {
            incoming_source_version,
            current_source_version,
        } => {
            hash_component(&mut hasher, b"stale_ignored");
            hash_component(&mut hasher, &incoming_source_version.to_be_bytes());
            hash_component(&mut hasher, &current_source_version.to_be_bytes());
        }
    }
    hex::encode(hasher.finalize())
}

fn entity_values(key: &EntityKey) -> Vec<SqlValue> {
    vec![
        key.tenant_id.into(),
        key.schema.module.as_str().to_owned().into(),
        key.schema.entity.as_str().to_owned().into(),
        i64::from(key.schema.version.get()).into(),
        key.entity_id.into(),
        key.locale
            .as_ref()
            .map_or_else(String::new, |locale| locale.as_str().to_owned())
            .into(),
    ]
}

fn positive_source_version(row: &QueryResult) -> Result<u64, IndexDriftRepairFailure> {
    let value = row
        .try_get::<String>("", "source_version_text")
        .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
    let parsed = value
        .parse::<u64>()
        .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
    if parsed == 0 {
        return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
    }
    Ok(parsed)
}

fn hash_entity_key(hasher: &mut Sha256, key: &EntityKey) {
    hash_component(hasher, key.tenant_id.as_bytes());
    hash_component(hasher, key.schema.module.as_str().as_bytes());
    hash_component(hasher, key.schema.entity.as_str().as_bytes());
    hash_component(hasher, &key.schema.version.get().to_be_bytes());
    hash_component(hasher, key.entity_id.as_bytes());
    match &key.locale {
        Some(locale) => {
            hash_component(hasher, b"locale");
            hash_component(hasher, locale.as_str().as_bytes());
        }
        None => hash_component(hasher, b"no_locale"),
    }
}

fn hash_component(hasher: &mut Sha256, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("bounded repair digest component");
    hasher.update(length.to_be_bytes());
    hasher.update(value);
}

fn map_source_error(error: IndexSourceError) -> IndexDriftRepairFailure {
    match error {
        IndexSourceError::SourceFailure { failure, .. } => match failure.kind() {
            IndexSourceFailureKind::Retryable => retryable_failure(SOURCE_UNAVAILABLE),
            IndexSourceFailureKind::Permanent => permanent_failure(SOURCE_REJECTED),
        },
        _ => permanent_failure(SOURCE_CONTRACT_INVALID),
    }
}

fn map_absence_error(error: IndexSourceAbsenceError) -> IndexDriftRepairFailure {
    match error {
        IndexSourceAbsenceError::ProviderFailure { failure, .. } => match failure.kind() {
            IndexSourceFailureKind::Retryable => retryable_failure(SOURCE_UNAVAILABLE),
            IndexSourceFailureKind::Permanent => permanent_failure(SOURCE_REJECTED),
        },
        IndexSourceAbsenceError::UnknownSchemaProvider(_) => permanent_failure(ABSENCE_UNAVAILABLE),
        _ => permanent_failure(SOURCE_CONTRACT_INVALID),
    }
}

fn map_mutation_error(error: MutationStorageError) -> IndexDriftRepairFailure {
    match error {
        MutationStorageError::DeliveryInProgress { .. }
        | MutationStorageError::Storage(_)
        | MutationStorageError::ConcurrentMutationConflict
        | MutationStorageError::InboxCompletionLost => retryable_failure(OWNER_UNAVAILABLE),
        MutationStorageError::Validation(_)
        | MutationStorageError::InvalidDelivery { .. }
        | MutationStorageError::DeliveryConflict
        | MutationStorageError::DeliveryRejected
        | MutationStorageError::InvalidStoredSourceVersion { .. }
        | MutationStorageError::SqliteSourceVersionOutOfRange { .. }
        | MutationStorageError::Serialization(_) => permanent_failure(OWNER_CONTRACT_INVALID),
    }
}

fn ensure_postgres(db: &DatabaseConnection) -> Result<(), IndexDriftRepairFailure> {
    if db.get_database_backend() == DbBackend::Postgres {
        Ok(())
    } else {
        Err(permanent_failure(UNSUPPORTED_BACKEND))
    }
}

fn retryable_failure(code: &str) -> IndexDriftRepairFailure {
    IndexDriftRepairFailure::retryable(code).expect("static missing-entity repair code is valid")
}

fn permanent_failure(code: &str) -> IndexDriftRepairFailure {
    IndexDriftRepairFailure::permanent(code).expect("static missing-entity repair code is valid")
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::{EntityName, LocaleKey, ModuleName, SchemaRef, SchemaVersion};

    fn key() -> EntityKey {
        EntityKey {
            tenant_id: Uuid::from_u128(1),
            schema: SchemaRef {
                module: ModuleName::new("catalog").expect("module"),
                entity: EntityName::new("product").expect("entity"),
                version: SchemaVersion::new(1),
            },
            entity_id: Uuid::from_u128(2),
            locale: Some(LocaleKey::new("en").expect("locale")),
        }
    }

    #[test]
    fn evidence_is_repairable_only_for_monotonic_exact_absence() {
        let key = key();
        let target = MissingEntityTargetRef {
            key: &key,
            indexed_source_version: 7,
            absence_source_version: 8,
        };
        assert_eq!(
            classify_evidence(
                target,
                MissingEntityAuthority::Absent(8),
                MissingEntityMaterialized::Live(7),
                MissingEntityEvidencePhase::Before,
            ),
            IndexDriftRepairEvidenceState::Repairable,
        );
        assert_eq!(
            classify_evidence(
                target,
                MissingEntityAuthority::Absent(7),
                MissingEntityMaterialized::Live(7),
                MissingEntityEvidencePhase::Before,
            ),
            IndexDriftRepairEvidenceState::Changed,
        );
    }

    #[test]
    fn committed_delete_is_repairable_before_and_converged_after() {
        let key = key();
        let target = MissingEntityTargetRef {
            key: &key,
            indexed_source_version: 7,
            absence_source_version: 8,
        };
        assert_eq!(
            classify_evidence(
                target,
                MissingEntityAuthority::Absent(8),
                MissingEntityMaterialized::Deleted(8),
                MissingEntityEvidencePhase::Before,
            ),
            IndexDriftRepairEvidenceState::Repairable,
        );
        assert_eq!(
            classify_evidence(
                target,
                MissingEntityAuthority::Absent(8),
                MissingEntityMaterialized::Deleted(8),
                MissingEntityEvidencePhase::After,
            ),
            IndexDriftRepairEvidenceState::Converged,
        );
    }

    #[test]
    fn physical_absence_is_never_convergence() {
        let key = key();
        let target = MissingEntityTargetRef {
            key: &key,
            indexed_source_version: 7,
            absence_source_version: 8,
        };
        assert_eq!(
            classify_evidence(
                target,
                MissingEntityAuthority::Absent(8),
                MissingEntityMaterialized::Missing,
                MissingEntityEvidencePhase::After,
            ),
            IndexDriftRepairEvidenceState::Changed,
        );
    }

    #[test]
    fn evidence_digest_binds_state_and_materialized_version() {
        let key = key();
        let target = MissingEntityTargetRef {
            key: &key,
            indexed_source_version: 7,
            absence_source_version: 8,
        };
        let before = evidence_digest(
            target,
            MissingEntityAuthority::Absent(8),
            MissingEntityMaterialized::Live(7),
            IndexDriftRepairEvidenceState::Repairable,
        );
        let after = evidence_digest(
            target,
            MissingEntityAuthority::Absent(8),
            MissingEntityMaterialized::Deleted(8),
            IndexDriftRepairEvidenceState::Converged,
        );
        assert_eq!(before.len(), 64);
        assert_eq!(after.len(), 64);
        assert_ne!(before, after);
    }
}
