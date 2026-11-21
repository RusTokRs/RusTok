use std::{fmt, sync::Arc};

use async_trait::async_trait;
use rust_decimal::Decimal;
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend,
    IsolationLevel, QueryResult, Statement, TransactionTrait, Value as SqlValue,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    EntityKey, IndexDriftAuthorizedRepairCommand, IndexDriftRepairAuthorizer,
    IndexDriftRepairEvidence, IndexDriftRepairEvidenceReader, IndexDriftRepairEvidenceState,
    IndexDriftRepairFailure, IndexDriftRepairFinding, IndexDriftRepairOwner,
    IndexDriftRepairOwnerOutcome, IndexDriftRepairOwnerRegistry,
    IndexDriftRepairReservationOutcome, IndexDriftRepairService, IndexDriftRepairStore,
    IndexDriftRepairStoreCompletionOutcome, IndexDriftRepairTarget, IndexDriftRepairTargetKind,
    IndexDriftRepairTicket, IndexMutation, IndexSourceAbsenceError, IndexSourceError,
    IndexSourceFailureKind, IndexSourceLoadRequest, LinkName, LinkedEntityKey,
    SharedIndexSourceAbsenceRegistry, SharedIndexSourceRegistry,
};

use super::{
    drift_repair::materialize_postgres_index_drift_repair_store,
    drift_repair_recovery::{
        RecoveryAwareIndexDriftRepairOwner, RecoveryAwareIndexDriftRepairStore,
    },
    mutation_store::lock_entity_key,
};

const OWNER_NAME: &str = "index_orphan_link_remove_owner";
const DELIVERY_SOURCE: &str = "index_drift_repair_orphan_link";
const EVIDENCE_DOMAIN: &[u8] = b"index_orphan_link_repair_evidence_v1";
const MUTATION_DOMAIN: &[u8] = b"index_orphan_link_removal_mutation_v1";
const OWNER_RECEIPT_DOMAIN: &[u8] = b"index_orphan_link_repair_owner_receipt_v1";
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
pub struct PostgresIndexDriftOrphanLinkEvidenceReader {
    db: DatabaseConnection,
    sources: SharedIndexSourceRegistry,
    absence: SharedIndexSourceAbsenceRegistry,
}

impl PostgresIndexDriftOrphanLinkEvidenceReader {
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
        phase: OrphanLinkEvidencePhase,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
        let target = exact_orphan_link_target(authorized, finding)?;
        let command_id = authorized.command().command_id();
        // A committed delivery proves past work, not current convergence. It permits only
        // read-only receipt recovery, even when the original owner revision is no longer live.
        if phase == OrphanLinkEvidencePhase::Before
            && read_delivery_state(&self.db, command_id, target, MATERIALIZED_UNAVAILABLE).await?
                == OrphanMutationDeliveryState::Applied
        {
            return recovery_evidence(command_id, target);
        }
        let first_source = self.load_source_authority(target).await?;
        let first_target = self.load_target_authority(target).await?;
        let materialized = self.load_materialized(target, command_id).await?;
        if phase == OrphanLinkEvidencePhase::Before
            && materialized.delivery == OrphanMutationDeliveryState::Applied
        {
            // The mutation may have committed between the preflight and this snapshot.
            return recovery_evidence(command_id, target);
        }
        let second_source = self.load_source_authority(target).await?;
        let second_target = self.load_target_authority(target).await?;
        if first_source != second_source || first_target != second_target {
            return Err(retryable_failure(SOURCE_CHANGED));
        }

        let state = classify_evidence(target, first_source, first_target, materialized, phase);
        let digest = evidence_digest(target, first_source, first_target, materialized, state);
        IndexDriftRepairEvidence::new(state, digest)
            .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))
    }

    async fn load_source_authority(
        &self,
        target: OrphanLinkTargetRef<'_>,
    ) -> Result<OrphanSourceAuthority, IndexDriftRepairFailure> {
        let request = IndexSourceLoadRequest::new(vec![target.source_key.clone()])
            .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
        let batch = self.sources.load(request).await.map_err(map_source_error)?;
        let mut mutations = batch.into_mutations();
        if mutations.len() > 1 {
            return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
        }
        match mutations.pop() {
            Some(IndexMutation::Upsert { record, .. }) => {
                if &record.key != target.source_key || record.source_version == 0 {
                    return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
                }
                let exact_link_present = record_has_exact_link(
                    &record.links,
                    target.link_name,
                    target.ordinal,
                    target.linked_target,
                )?;
                Ok(OrphanSourceAuthority::Present {
                    source_version: record.source_version,
                    exact_link_present,
                })
            }
            Some(IndexMutation::Delete {
                key,
                source_version,
                ..
            }) => {
                if &key != target.source_key || source_version == 0 {
                    return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
                }
                Ok(OrphanSourceAuthority::Absent)
            }
            None => Ok(OrphanSourceAuthority::Absent),
        }
    }

    async fn load_target_authority(
        &self,
        target: OrphanLinkTargetRef<'_>,
    ) -> Result<OrphanTargetAuthority, IndexDriftRepairFailure> {
        let key = target.target_key();
        let request = IndexSourceLoadRequest::new(vec![key.clone()])
            .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
        let batch = self.sources.load(request).await.map_err(map_source_error)?;
        let mut mutations = batch.into_mutations();
        if mutations.len() > 1 {
            return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
        }
        match mutations.pop() {
            Some(IndexMutation::Upsert { record, .. }) => {
                if record.key != key || record.source_version == 0 {
                    return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
                }
                Ok(OrphanTargetAuthority::Present)
            }
            Some(IndexMutation::Delete {
                key: returned_key,
                source_version,
                ..
            }) => {
                if returned_key != key || source_version == 0 {
                    return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
                }
                Ok(OrphanTargetAuthority::Absent(source_version))
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
                if watermark.key() != &key || watermark.source_version() == 0 {
                    return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
                }
                Ok(OrphanTargetAuthority::Absent(watermark.source_version()))
            }
        }
    }

    async fn load_materialized(
        &self,
        target: OrphanLinkTargetRef<'_>,
        command_id: Uuid,
    ) -> Result<OrphanMaterialized, IndexDriftRepairFailure> {
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
            .map_err(|_| retryable_failure(MATERIALIZED_UNAVAILABLE))?;
        let result = self
            .load_materialized_in_transaction(&transaction, target, command_id)
            .await;
        match result {
            Ok(value) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| retryable_failure(MATERIALIZED_UNAVAILABLE))?;
                Ok(value)
            }
            Err(error) => {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| retryable_failure(MATERIALIZED_UNAVAILABLE))?;
                Err(error)
            }
        }
    }

    async fn load_materialized_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        target: OrphanLinkTargetRef<'_>,
        command_id: Uuid,
    ) -> Result<OrphanMaterialized, IndexDriftRepairFailure> {
        let source =
            read_materialized_record(transaction, target.source_key, MATERIALIZED_UNAVAILABLE)
                .await?;
        let target_record =
            read_materialized_record(transaction, &target.target_key(), MATERIALIZED_UNAVAILABLE)
                .await?;

        let link_row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT target_module, target_entity, target_schema_version, target_entity_id, target_locale_key FROM index_links WHERE tenant_id = $1 AND source_module = $2 AND source_entity = $3 AND source_schema_version = $4 AND source_entity_id = $5 AND source_locale_key = $6 AND source_version = $7 AND link_name = $8 AND ordinal = $9 LIMIT 1",
                link_identity_values(target),
            ))
            .await
            .map_err(|_| retryable_failure(MATERIALIZED_UNAVAILABLE))?;
        let link = match link_row {
            None => OrphanMaterializedLink::Absent,
            Some(row) if row_matches_target(&row, target.linked_target)? => {
                OrphanMaterializedLink::Exact
            }
            Some(_) => OrphanMaterializedLink::Changed,
        };

        let delivery =
            read_delivery_state(transaction, command_id, target, MATERIALIZED_UNAVAILABLE).await?;

        Ok(OrphanMaterialized {
            source,
            target: target_record,
            link,
            delivery,
        })
    }
}

#[async_trait]
impl IndexDriftRepairEvidenceReader for PostgresIndexDriftOrphanLinkEvidenceReader {
    async fn capture_before(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
        self.capture(authorized, finding, OrphanLinkEvidencePhase::Before)
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
        self.capture(authorized, finding, OrphanLinkEvidencePhase::After)
            .await
    }
}

impl fmt::Debug for PostgresIndexDriftOrphanLinkEvidenceReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexDriftOrphanLinkEvidenceReader")
            .field("source_count", &self.sources.len())
            .field("absence_provider_count", &self.absence.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndexOrphanLinkRemovalOutcome {
    Applied,
    Duplicate,
}

/// Owner-provided, transactional proof for exact destructive orphan removal.
///
/// Lock (or prove immutable) the source revision and exact edge and the target's absence revision
/// in `transaction`, including when the target row does not exist. The locks must serialize with
/// the owners' writes, not just Index's materialization, and remain effective until commit or
/// rollback. Implementations must refuse unsupported owner/scope pairs or moving authority; a
/// point-in-time lookup or a transaction against an independent store is not a fence.
/// If an owner writes both stores synchronously, it must acquire owner locks before Index locks;
/// two owners that participate in one repair must agree on a stable owner-lock order.
/// Admission also requires permanent absence, or an owner guarantee that a later resurrection
/// advances the affected source revisions to restore their edges. Index redelivery is idempotent.
#[async_trait]
pub trait PostgresIndexOrphanLinkAuthorityFence: Send + Sync {
    async fn verify_and_lock(
        &self,
        transaction: &DatabaseTransaction,
        target: &IndexDriftRepairTarget,
    ) -> Result<(), IndexDriftRepairFailure>;
}

#[derive(Clone)]
struct PostgresIndexOrphanLinkMutationStore {
    db: DatabaseConnection,
    authority: Arc<dyn PostgresIndexOrphanLinkAuthorityFence>,
}

impl PostgresIndexOrphanLinkMutationStore {
    async fn apply(
        &self,
        command_id: Uuid,
        repair_target: &IndexDriftRepairTarget,
        target: OrphanLinkTargetRef<'_>,
    ) -> Result<IndexOrphanLinkRemovalOutcome, IndexDriftRepairFailure> {
        let payload_digest = link_removal_payload_digest(command_id, target);
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::ReadCommitted),
                Some(AccessMode::ReadWrite),
            )
            .await
            .map_err(|_| retryable_failure(OWNER_UNAVAILABLE))?;
        let result = async {
            transaction
                .execute_unprepared(
                    "SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '25s'",
                )
                .await
                .map_err(|_| retryable_failure(OWNER_UNAVAILABLE))?;
            // The owner locks precede Index entity locks. Both sets remain held through commit.
            tokio::time::timeout(
                std::time::Duration::from_secs(30),
                self.authority.verify_and_lock(&transaction, repair_target),
            )
            .await
            .map_err(|_| retryable_failure(OWNER_UNAVAILABLE))??;
            self.apply_in_transaction(&transaction, command_id, target, &payload_digest)
                .await
        }
        .await;
        match result {
            Ok(outcome) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| retryable_failure(OWNER_UNAVAILABLE))?;
                Ok(outcome)
            }
            Err(error) => {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| retryable_failure(OWNER_UNAVAILABLE))?;
                Err(error)
            }
        }
    }

    async fn apply_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        command_id: Uuid,
        target: OrphanLinkTargetRef<'_>,
        payload_digest: &str,
    ) -> Result<IndexOrphanLinkRemovalOutcome, IndexDriftRepairFailure> {
        let inserted = transaction
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO index_inbox (tenant_id, source_name, delivery_id, mutation_kind, module_name, entity_name, schema_version, entity_id, locale_key, source_version, payload_hash, state, attempt_count) VALUES ($1, $2, $3, 'delete', $4, $5, $6, $7, $8, $9, $10, 'pending', 1) ON CONFLICT (tenant_id, source_name, delivery_id) DO NOTHING",
                vec![
                    target.source_key.tenant_id.into(),
                    DELIVERY_SOURCE.to_owned().into(),
                    command_id.to_string().into(),
                    target.source_key.schema.module.as_str().to_owned().into(),
                    target.source_key.schema.entity.as_str().to_owned().into(),
                    i64::from(target.source_key.schema.version.get()).into(),
                    target.source_key.entity_id.into(),
                    locale_text(target.source_key.locale.as_ref()).into(),
                    Decimal::from(target.indexed_source_version).into(),
                    payload_digest.to_owned().into(),
                ],
            ))
            .await
            .map_err(|_| retryable_failure(OWNER_UNAVAILABLE))?;
        if inserted.rows_affected() == 0 {
            return recover_applied_delivery(transaction, command_id, target).await;
        }

        // Serialize against the normal mutation writer for both keys, including a target that has
        // no row yet. Take locks in stable order and read the current projection after locking.
        let target_key = target.target_key();
        let mut keys = [target.source_key, &target_key];
        keys.sort();
        for key in keys {
            lock_entity_key(transaction, key, DbBackend::Postgres)
                .await
                .map_err(|_| retryable_failure(OWNER_UNAVAILABLE))?;
        }
        require_exact_live_source(transaction, target).await?;
        let target_record =
            read_materialized_record(transaction, &target_key, OWNER_UNAVAILABLE).await?;
        if !target_absent_at(target_record, target.target_absence_source_version) {
            return Err(retryable_failure(SOURCE_CHANGED));
        }
        require_exact_link(transaction, target).await?;

        let deleted = transaction
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "DELETE FROM index_links WHERE tenant_id = $1 AND source_module = $2 AND source_entity = $3 AND source_schema_version = $4 AND source_entity_id = $5 AND source_locale_key = $6 AND source_version = $7 AND link_name = $8 AND ordinal = $9 AND target_module = $10 AND target_entity = $11 AND target_schema_version = $12 AND target_entity_id = $13 AND target_locale_key = $14",
                exact_link_values(target),
            ))
            .await
            .map_err(|_| retryable_failure(OWNER_UNAVAILABLE))?;
        if deleted.rows_affected() != 1 {
            return Err(retryable_failure(OWNER_UNAVAILABLE));
        }

        complete_delivery(transaction, command_id, target, payload_digest).await?;
        Ok(IndexOrphanLinkRemovalOutcome::Applied)
    }
}

async fn read_delivery_state<C: ConnectionTrait>(
    db: &C,
    command_id: Uuid,
    target: OrphanLinkTargetRef<'_>,
    unavailable_code: &str,
) -> Result<OrphanMutationDeliveryState, IndexDriftRepairFailure> {
    let row = db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT mutation_kind, module_name, entity_name, schema_version, entity_id, locale_key, CAST(source_version AS TEXT) AS source_version_text, payload_hash, state FROM index_inbox WHERE tenant_id = $1 AND source_name = $2 AND delivery_id = $3 LIMIT 1",
        vec![
            target.source_key.tenant_id.into(),
            DELIVERY_SOURCE.to_owned().into(),
            command_id.to_string().into(),
        ],
    )).await.map_err(|_| retryable_failure(unavailable_code))?;
    match row {
        None => Ok(OrphanMutationDeliveryState::Missing),
        Some(row) => decode_delivery_state(
            &row,
            target,
            &link_removal_payload_digest(command_id, target),
        ),
    }
}

async fn recover_applied_delivery<C: ConnectionTrait>(
    db: &C,
    command_id: Uuid,
    target: OrphanLinkTargetRef<'_>,
) -> Result<IndexOrphanLinkRemovalOutcome, IndexDriftRepairFailure> {
    match read_delivery_state(db, command_id, target, OWNER_UNAVAILABLE).await? {
        OrphanMutationDeliveryState::Applied => Ok(IndexOrphanLinkRemovalOutcome::Duplicate),
        // An absent witness is never authority to execute the old destructive command again.
        OrphanMutationDeliveryState::Missing => Err(retryable_failure(OWNER_UNAVAILABLE)),
    }
}

impl fmt::Debug for PostgresIndexOrphanLinkMutationStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexOrphanLinkMutationStore")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct PostgresIndexDriftOrphanLinkRepairOwner {
    mutations: PostgresIndexOrphanLinkMutationStore,
}

impl PostgresIndexDriftOrphanLinkRepairOwner {
    pub fn new(
        db: DatabaseConnection,
        authority: Arc<dyn PostgresIndexOrphanLinkAuthorityFence>,
    ) -> Result<Self, IndexDriftRepairFailure> {
        ensure_postgres(&db)?;
        Ok(Self {
            mutations: PostgresIndexOrphanLinkMutationStore { db, authority },
        })
    }
}

#[async_trait]
impl IndexDriftRepairOwner for PostgresIndexDriftOrphanLinkRepairOwner {
    fn owner_name(&self) -> &str {
        OWNER_NAME
    }

    fn target_kind(&self) -> IndexDriftRepairTargetKind {
        IndexDriftRepairTargetKind::OrphanLink
    }

    async fn repair(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
        before: &IndexDriftRepairEvidence,
    ) -> Result<IndexDriftRepairOwnerOutcome, IndexDriftRepairFailure> {
        let target = exact_orphan_link_target(authorized, finding)?;
        if before.state() != IndexDriftRepairEvidenceState::Repairable {
            return IndexDriftRepairOwnerOutcome::not_applied("orphan_link_not_repairable")
                .map_err(|_| permanent_failure(OWNER_CONTRACT_INVALID));
        }
        let command_id = authorized.command().command_id();
        let outcome = if before.digest() == recovery_evidence_digest(command_id, target) {
            recover_applied_delivery(&self.mutations.db, command_id, target).await?
        } else {
            self.mutations
                .apply(command_id, authorized.command().target(), target)
                .await?
        };
        let receipt = owner_receipt_digest(authorized, finding, target, outcome);
        IndexDriftRepairOwnerOutcome::applied(receipt)
            .map_err(|_| permanent_failure(OWNER_CONTRACT_INVALID))
    }
}

impl fmt::Debug for PostgresIndexDriftOrphanLinkRepairOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexDriftOrphanLinkRepairOwner")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct OrphanLinkOnlyRepairStore {
    inner: Arc<dyn IndexDriftRepairStore>,
}

#[async_trait]
impl IndexDriftRepairStore for OrphanLinkOnlyRepairStore {
    async fn reserve(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
    ) -> Result<IndexDriftRepairReservationOutcome, IndexDriftRepairFailure> {
        if authorized.command().target().kind() != IndexDriftRepairTargetKind::OrphanLink {
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

/// Composes a bounded confirmed-orphan-link repair path without publishing a runtime extension.
pub fn materialize_postgres_index_drift_orphan_link_repair_service(
    authorizer: Arc<dyn IndexDriftRepairAuthorizer>,
    db: DatabaseConnection,
    sources: SharedIndexSourceRegistry,
    absence: SharedIndexSourceAbsenceRegistry,
    authority: Arc<dyn PostgresIndexOrphanLinkAuthorityFence>,
) -> Result<IndexDriftRepairService, IndexDriftRepairFailure> {
    ensure_postgres(&db)?;
    let evidence = Arc::new(PostgresIndexDriftOrphanLinkEvidenceReader::new(
        db.clone(),
        sources,
        absence,
    )?);
    let base_owner: Arc<dyn IndexDriftRepairOwner> = Arc::new(
        PostgresIndexDriftOrphanLinkRepairOwner::new(db.clone(), authority)?,
    );
    let owner: Arc<dyn IndexDriftRepairOwner> = Arc::new(RecoveryAwareIndexDriftRepairOwner::new(
        db.clone(),
        base_owner,
    )?);
    let owners = IndexDriftRepairOwnerRegistry::new([owner])
        .map_err(|_| permanent_failure(COMPONENTS_INVALID))?;
    let store = materialize_postgres_index_drift_repair_store(db.clone())?;
    let recovery_store: Arc<dyn IndexDriftRepairStore> =
        Arc::new(RecoveryAwareIndexDriftRepairStore::new(db, store)?);
    let gated_store: Arc<dyn IndexDriftRepairStore> = Arc::new(OrphanLinkOnlyRepairStore {
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
enum OrphanSourceAuthority {
    Present {
        source_version: u64,
        exact_link_present: bool,
    },
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrphanTargetAuthority {
    Present,
    Absent(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrphanMaterializedSource {
    Missing,
    Live(u64),
    Deleted(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrphanMaterializedLink {
    Exact,
    Absent,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrphanMutationDeliveryState {
    Missing,
    Applied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrphanMaterialized {
    source: OrphanMaterializedSource,
    target: OrphanMaterializedSource,
    link: OrphanMaterializedLink,
    delivery: OrphanMutationDeliveryState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrphanLinkEvidencePhase {
    Before,
    After,
}

#[derive(Clone, Copy)]
struct OrphanLinkTargetRef<'a> {
    source_key: &'a EntityKey,
    indexed_source_version: u64,
    link_name: &'a LinkName,
    ordinal: u32,
    linked_target: &'a LinkedEntityKey,
    target_absence_source_version: u64,
}

impl OrphanLinkTargetRef<'_> {
    fn target_key(self) -> EntityKey {
        EntityKey {
            tenant_id: self.source_key.tenant_id,
            schema: self.linked_target.schema.clone(),
            entity_id: self.linked_target.entity_id,
            locale: self.linked_target.locale.clone(),
        }
    }
}

fn exact_orphan_link_target<'a>(
    authorized: &'a IndexDriftAuthorizedRepairCommand,
    finding: &'a IndexDriftRepairFinding,
) -> Result<OrphanLinkTargetRef<'a>, IndexDriftRepairFailure> {
    if authorized.command().finding_id() != finding.finding_id()
        || authorized.command().target() != finding.target()
    {
        return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
    }
    match finding.target() {
        IndexDriftRepairTarget::OrphanLink {
            source_key,
            indexed_source_version,
            link_name,
            ordinal,
            target,
            target_absence_source_version,
        } => Ok(OrphanLinkTargetRef {
            source_key,
            indexed_source_version: *indexed_source_version,
            link_name,
            ordinal: *ordinal,
            linked_target: target,
            target_absence_source_version: *target_absence_source_version,
        }),
        IndexDriftRepairTarget::MissingEntity { .. } => Err(permanent_failure(TARGET_UNSUPPORTED)),
    }
}

fn classify_evidence(
    target: OrphanLinkTargetRef<'_>,
    source: OrphanSourceAuthority,
    target_authority: OrphanTargetAuthority,
    materialized: OrphanMaterialized,
    phase: OrphanLinkEvidencePhase,
) -> IndexDriftRepairEvidenceState {
    let authority_admitted = matches!(
        source,
        OrphanSourceAuthority::Present {
            source_version,
            exact_link_present: true,
        } if source_version == target.indexed_source_version
    ) && matches!(
        target_authority,
        OrphanTargetAuthority::Absent(version)
            if version == target.target_absence_source_version
    );
    let source_admitted =
        materialized.source == OrphanMaterializedSource::Live(target.indexed_source_version);
    if !authority_admitted
        || !source_admitted
        || !target_absent_at(materialized.target, target.target_absence_source_version)
    {
        return IndexDriftRepairEvidenceState::Changed;
    }
    match (phase, materialized.link, materialized.delivery) {
        (
            OrphanLinkEvidencePhase::Before,
            OrphanMaterializedLink::Exact,
            OrphanMutationDeliveryState::Missing,
        ) => IndexDriftRepairEvidenceState::Repairable,
        (
            OrphanLinkEvidencePhase::After,
            OrphanMaterializedLink::Absent,
            OrphanMutationDeliveryState::Applied,
        ) => IndexDriftRepairEvidenceState::Converged,
        _ => IndexDriftRepairEvidenceState::Changed,
    }
}

fn evidence_digest(
    target: OrphanLinkTargetRef<'_>,
    source: OrphanSourceAuthority,
    target_authority: OrphanTargetAuthority,
    materialized: OrphanMaterialized,
    state: IndexDriftRepairEvidenceState,
) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, EVIDENCE_DOMAIN);
    hash_orphan_target(&mut hasher, target);
    match source {
        OrphanSourceAuthority::Present {
            source_version,
            exact_link_present,
        } => {
            hash_component(&mut hasher, b"source_present");
            hash_component(&mut hasher, &source_version.to_be_bytes());
            hash_component(
                &mut hasher,
                if exact_link_present {
                    b"link_present"
                } else {
                    b"link_absent"
                },
            );
        }
        OrphanSourceAuthority::Absent => hash_component(&mut hasher, b"source_absent"),
    }
    match target_authority {
        OrphanTargetAuthority::Present => hash_component(&mut hasher, b"target_present"),
        OrphanTargetAuthority::Absent(version) => {
            hash_component(&mut hasher, b"target_absent");
            hash_component(&mut hasher, &version.to_be_bytes());
        }
    }
    match materialized.source {
        OrphanMaterializedSource::Missing => hash_component(&mut hasher, b"index_source_missing"),
        OrphanMaterializedSource::Live(version) => {
            hash_component(&mut hasher, b"index_source_live");
            hash_component(&mut hasher, &version.to_be_bytes());
        }
        OrphanMaterializedSource::Deleted(version) => {
            hash_component(&mut hasher, b"index_source_deleted");
            hash_component(&mut hasher, &version.to_be_bytes());
        }
    }
    match materialized.target {
        OrphanMaterializedSource::Missing => hash_component(&mut hasher, b"index_target_missing"),
        OrphanMaterializedSource::Live(version) => {
            hash_component(&mut hasher, b"index_target_live");
            hash_component(&mut hasher, &version.to_be_bytes());
        }
        OrphanMaterializedSource::Deleted(version) => {
            hash_component(&mut hasher, b"index_target_deleted");
            hash_component(&mut hasher, &version.to_be_bytes());
        }
    }
    hash_component(
        &mut hasher,
        match materialized.link {
            OrphanMaterializedLink::Exact => b"index_link_exact",
            OrphanMaterializedLink::Absent => b"index_link_absent",
            OrphanMaterializedLink::Changed => b"index_link_changed",
        },
    );
    hash_component(
        &mut hasher,
        match materialized.delivery {
            OrphanMutationDeliveryState::Missing => b"delivery_missing",
            OrphanMutationDeliveryState::Applied => b"delivery_applied",
        },
    );
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

fn recovery_evidence_digest(command_id: Uuid, target: OrphanLinkTargetRef<'_>) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, EVIDENCE_DOMAIN);
    hash_component(&mut hasher, b"resume_applied_delivery");
    hash_component(
        &mut hasher,
        link_removal_payload_digest(command_id, target).as_bytes(),
    );
    hex::encode(hasher.finalize())
}

fn recovery_evidence(
    command_id: Uuid,
    target: OrphanLinkTargetRef<'_>,
) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
    IndexDriftRepairEvidence::new(
        IndexDriftRepairEvidenceState::Repairable,
        recovery_evidence_digest(command_id, target),
    )
    .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))
}

fn link_removal_payload_digest(command_id: Uuid, target: OrphanLinkTargetRef<'_>) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, MUTATION_DOMAIN);
    hash_component(&mut hasher, command_id.as_bytes());
    hash_orphan_target(&mut hasher, target);
    hex::encode(hasher.finalize())
}

fn owner_receipt_digest(
    authorized: &IndexDriftAuthorizedRepairCommand,
    finding: &IndexDriftRepairFinding,
    target: OrphanLinkTargetRef<'_>,
    outcome: IndexOrphanLinkRemovalOutcome,
) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, OWNER_RECEIPT_DOMAIN);
    hash_component(&mut hasher, authorized.command().command_id().as_bytes());
    hash_component(&mut hasher, finding.finding_id().as_bytes());
    hash_orphan_target(&mut hasher, target);
    hash_component(
        &mut hasher,
        match outcome {
            IndexOrphanLinkRemovalOutcome::Applied => b"applied",
            IndexOrphanLinkRemovalOutcome::Duplicate => b"duplicate",
        },
    );
    hex::encode(hasher.finalize())
}

fn hash_orphan_target(hasher: &mut Sha256, target: OrphanLinkTargetRef<'_>) {
    hash_entity_key(hasher, target.source_key);
    hash_component(hasher, &target.indexed_source_version.to_be_bytes());
    hash_component(hasher, target.link_name.as_str().as_bytes());
    hash_component(hasher, &target.ordinal.to_be_bytes());
    hash_linked_key(hasher, target.linked_target);
    hash_component(hasher, &target.target_absence_source_version.to_be_bytes());
}

fn record_has_exact_link(
    links: &[crate::IndexLinkValue],
    link_name: &LinkName,
    ordinal: u32,
    target: &LinkedEntityKey,
) -> Result<bool, IndexDriftRepairFailure> {
    let mut values = links.iter().filter(|value| &value.name == link_name);
    let Some(value) = values.next() else {
        return Ok(false);
    };
    if values.next().is_some() {
        return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
    }
    let ordinal =
        usize::try_from(ordinal).map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
    Ok(value.targets.get(ordinal) == Some(target))
}

fn decode_delivery_state(
    row: &QueryResult,
    target: OrphanLinkTargetRef<'_>,
    payload_digest: &str,
) -> Result<OrphanMutationDeliveryState, IndexDriftRepairFailure> {
    let mutation_kind = required_text(row, "mutation_kind")?;
    let module_name = required_text(row, "module_name")?;
    let entity_name = required_text(row, "entity_name")?;
    let schema_version = row
        .try_get::<i32>("", "schema_version")
        .map_err(|_| permanent_failure(OWNER_CONTRACT_INVALID))?;
    let entity_id = row
        .try_get::<Uuid>("", "entity_id")
        .map_err(|_| permanent_failure(OWNER_CONTRACT_INVALID))?;
    let locale_key = row
        .try_get::<String>("", "locale_key")
        .map_err(|_| permanent_failure(OWNER_CONTRACT_INVALID))?;
    let source_version = positive_source_version(row)?;
    let stored_payload = required_text(row, "payload_hash")?;
    if mutation_kind != "delete"
        || module_name != target.source_key.schema.module.as_str()
        || entity_name != target.source_key.schema.entity.as_str()
        || u32::try_from(schema_version).ok() != Some(target.source_key.schema.version.get())
        || entity_id != target.source_key.entity_id
        || locale_key != locale_text(target.source_key.locale.as_ref())
        || source_version != target.indexed_source_version
        || stored_payload != payload_digest
    {
        return Err(permanent_failure(OWNER_CONTRACT_INVALID));
    }
    match required_text(row, "state")?.as_str() {
        "applied" => Ok(OrphanMutationDeliveryState::Applied),
        "pending" | "processing" => Err(retryable_failure(OWNER_UNAVAILABLE)),
        "rejected" => Err(permanent_failure(OWNER_CONTRACT_INVALID)),
        _ => Err(permanent_failure(OWNER_CONTRACT_INVALID)),
    }
}

async fn require_exact_live_source(
    transaction: &DatabaseTransaction,
    target: OrphanLinkTargetRef<'_>,
) -> Result<(), IndexDriftRepairFailure> {
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT CAST(source_version AS TEXT) AS source_version_text, is_deleted FROM index_entities WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4 AND entity_id = $5 AND locale_key = $6 LIMIT 1 FOR UPDATE",
            entity_values(target.source_key),
        ))
        .await
        .map_err(|_| retryable_failure(OWNER_UNAVAILABLE))?
        .ok_or_else(|| retryable_failure(OWNER_UNAVAILABLE))?;
    let source_version = positive_source_version(&row)?;
    let is_deleted = row
        .try_get::<bool>("", "is_deleted")
        .map_err(|_| permanent_failure(OWNER_CONTRACT_INVALID))?;
    if is_deleted || source_version != target.indexed_source_version {
        return Err(retryable_failure(OWNER_UNAVAILABLE));
    }
    Ok(())
}

async fn require_exact_link(
    transaction: &DatabaseTransaction,
    target: OrphanLinkTargetRef<'_>,
) -> Result<(), IndexDriftRepairFailure> {
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT target_module, target_entity, target_schema_version, target_entity_id, target_locale_key FROM index_links WHERE tenant_id = $1 AND source_module = $2 AND source_entity = $3 AND source_schema_version = $4 AND source_entity_id = $5 AND source_locale_key = $6 AND source_version = $7 AND link_name = $8 AND ordinal = $9 LIMIT 1 FOR UPDATE",
            link_identity_values(target),
        ))
        .await
        .map_err(|_| retryable_failure(OWNER_UNAVAILABLE))?
        .ok_or_else(|| retryable_failure(OWNER_UNAVAILABLE))?;
    if !row_matches_target(&row, target.linked_target)? {
        return Err(retryable_failure(OWNER_UNAVAILABLE));
    }
    Ok(())
}

async fn complete_delivery(
    transaction: &DatabaseTransaction,
    command_id: Uuid,
    target: OrphanLinkTargetRef<'_>,
    payload_digest: &str,
) -> Result<(), IndexDriftRepairFailure> {
    let updated = transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE index_inbox SET state = 'applied', completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, lease_owner = NULL, lease_expires_at = NULL, error_code = NULL, error_details = NULL WHERE tenant_id = $1 AND source_name = $2 AND delivery_id = $3 AND payload_hash = $4 AND state = 'pending'",
            vec![
                target.source_key.tenant_id.into(),
                DELIVERY_SOURCE.to_owned().into(),
                command_id.to_string().into(),
                payload_digest.to_owned().into(),
            ],
        ))
        .await
        .map_err(|_| retryable_failure(OWNER_UNAVAILABLE))?;
    if updated.rows_affected() != 1 {
        return Err(retryable_failure(OWNER_UNAVAILABLE));
    }
    Ok(())
}

async fn read_materialized_record(
    transaction: &DatabaseTransaction,
    key: &EntityKey,
    unavailable_code: &'static str,
) -> Result<OrphanMaterializedSource, IndexDriftRepairFailure> {
    let row = transaction.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT CAST(source_version AS TEXT) AS source_version_text, is_deleted FROM index_entities \
         WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 \
         AND schema_version = $4 AND entity_id = $5 AND locale_key = $6",
        entity_values(key),
    )).await.map_err(|_| retryable_failure(unavailable_code))?;
    let Some(row) = row else {
        return Ok(OrphanMaterializedSource::Missing);
    };
    let version = positive_source_version(&row)?;
    if row
        .try_get::<bool>("", "is_deleted")
        .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?
    {
        Ok(OrphanMaterializedSource::Deleted(version))
    } else {
        Ok(OrphanMaterializedSource::Live(version))
    }
}

fn target_absent_at(target: OrphanMaterializedSource, absence_revision: u64) -> bool {
    match target {
        OrphanMaterializedSource::Missing => true,
        OrphanMaterializedSource::Deleted(version) => version <= absence_revision,
        OrphanMaterializedSource::Live(_) => false,
    }
}

fn row_matches_target(
    row: &QueryResult,
    target: &LinkedEntityKey,
) -> Result<bool, IndexDriftRepairFailure> {
    let module = required_text(row, "target_module")?;
    let entity = required_text(row, "target_entity")?;
    let version = row
        .try_get::<i32>("", "target_schema_version")
        .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
    let entity_id = row
        .try_get::<Uuid>("", "target_entity_id")
        .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
    let locale = row
        .try_get::<String>("", "target_locale_key")
        .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
    Ok(module == target.schema.module.as_str()
        && entity == target.schema.entity.as_str()
        && u32::try_from(version).ok() == Some(target.schema.version.get())
        && entity_id == target.entity_id
        && locale == locale_text(target.locale.as_ref()))
}

fn entity_values(key: &EntityKey) -> Vec<SqlValue> {
    vec![
        key.tenant_id.into(),
        key.schema.module.as_str().to_owned().into(),
        key.schema.entity.as_str().to_owned().into(),
        i64::from(key.schema.version.get()).into(),
        key.entity_id.into(),
        locale_text(key.locale.as_ref()).into(),
    ]
}

fn link_identity_values(target: OrphanLinkTargetRef<'_>) -> Vec<SqlValue> {
    let mut values = entity_values(target.source_key);
    values.push(Decimal::from(target.indexed_source_version).into());
    values.push(target.link_name.as_str().to_owned().into());
    values.push(i64::from(target.ordinal).into());
    values
}

fn exact_link_values(target: OrphanLinkTargetRef<'_>) -> Vec<SqlValue> {
    let mut values = link_identity_values(target);
    values.push(
        target
            .linked_target
            .schema
            .module
            .as_str()
            .to_owned()
            .into(),
    );
    values.push(
        target
            .linked_target
            .schema
            .entity
            .as_str()
            .to_owned()
            .into(),
    );
    values.push(i64::from(target.linked_target.schema.version.get()).into());
    values.push(target.linked_target.entity_id.into());
    values.push(locale_text(target.linked_target.locale.as_ref()).into());
    values
}

fn locale_text(locale: Option<&crate::LocaleKey>) -> String {
    locale.map_or_else(String::new, |value| value.as_str().to_owned())
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

fn required_text(row: &QueryResult, column: &str) -> Result<String, IndexDriftRepairFailure> {
    let value = row
        .try_get::<String>("", column)
        .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
    if value.is_empty() {
        return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
    }
    Ok(value)
}

fn hash_entity_key(hasher: &mut Sha256, key: &EntityKey) {
    hash_component(hasher, key.tenant_id.as_bytes());
    hash_component(hasher, key.schema.module.as_str().as_bytes());
    hash_component(hasher, key.schema.entity.as_str().as_bytes());
    hash_component(hasher, &key.schema.version.get().to_be_bytes());
    hash_component(hasher, key.entity_id.as_bytes());
    hash_component(hasher, locale_text(key.locale.as_ref()).as_bytes());
}

fn hash_linked_key(hasher: &mut Sha256, key: &LinkedEntityKey) {
    hash_component(hasher, key.schema.module.as_str().as_bytes());
    hash_component(hasher, key.schema.entity.as_str().as_bytes());
    hash_component(hasher, &key.schema.version.get().to_be_bytes());
    hash_component(hasher, key.entity_id.as_bytes());
    hash_component(hasher, locale_text(key.locale.as_ref()).as_bytes());
}

fn hash_component(hasher: &mut Sha256, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("bounded orphan-link repair digest component");
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

fn ensure_postgres(db: &DatabaseConnection) -> Result<(), IndexDriftRepairFailure> {
    if db.get_database_backend() == DbBackend::Postgres {
        Ok(())
    } else {
        Err(permanent_failure(UNSUPPORTED_BACKEND))
    }
}

fn retryable_failure(code: &str) -> IndexDriftRepairFailure {
    IndexDriftRepairFailure::retryable(code)
        .expect("static orphan-link repair code is valid: invariant validated at startup")
}

fn permanent_failure(code: &str) -> IndexDriftRepairFailure {
    IndexDriftRepairFailure::permanent(code)
        .expect("static orphan-link repair code is valid: invariant validated at startup")
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::{EntityName, LocaleKey, ModuleName, SchemaRef, SchemaVersion};

    fn target() -> (EntityKey, LinkedEntityKey, LinkName) {
        let source = EntityKey {
            tenant_id: Uuid::from_u128(1),
            schema: SchemaRef {
                module: ModuleName::new("catalog").expect("module"),
                entity: EntityName::new("product").expect("entity"),
                version: SchemaVersion::new(1),
            },
            entity_id: Uuid::from_u128(2),
            locale: Some(LocaleKey::new("en").expect("locale")),
        };
        let linked = LinkedEntityKey {
            schema: SchemaRef {
                module: ModuleName::new("catalog").expect("module"),
                entity: EntityName::new("variant").expect("entity"),
                version: SchemaVersion::new(1),
            },
            entity_id: Uuid::from_u128(3),
            locale: Some(LocaleKey::new("en").expect("locale")),
        };
        (source, linked, LinkName::new("variants").expect("link"))
    }

    #[test]
    fn exact_link_is_repairable_before() {
        let (source, linked, link_name) = target();
        let target = OrphanLinkTargetRef {
            source_key: &source,
            indexed_source_version: 7,
            link_name: &link_name,
            ordinal: 1,
            linked_target: &linked,
            target_absence_source_version: 9,
        };
        assert_eq!(
            classify_evidence(
                target,
                OrphanSourceAuthority::Present {
                    source_version: 7,
                    exact_link_present: true,
                },
                OrphanTargetAuthority::Absent(9),
                OrphanMaterialized {
                    source: OrphanMaterializedSource::Live(7),
                    target: OrphanMaterializedSource::Missing,
                    link: OrphanMaterializedLink::Exact,
                    delivery: OrphanMutationDeliveryState::Missing,
                },
                OrphanLinkEvidencePhase::Before,
            ),
            IndexDriftRepairEvidenceState::Repairable,
        );
    }

    #[test]
    fn applied_absence_is_not_a_fresh_repair_but_can_converge() {
        let (source, linked, link_name) = target();
        let target = OrphanLinkTargetRef {
            source_key: &source,
            indexed_source_version: 7,
            link_name: &link_name,
            ordinal: 1,
            linked_target: &linked,
            target_absence_source_version: 9,
        };
        let materialized = OrphanMaterialized {
            source: OrphanMaterializedSource::Live(7),
            target: OrphanMaterializedSource::Missing,
            link: OrphanMaterializedLink::Absent,
            delivery: OrphanMutationDeliveryState::Applied,
        };
        let source_authority = OrphanSourceAuthority::Present {
            source_version: 7,
            exact_link_present: true,
        };
        let target_authority = OrphanTargetAuthority::Absent(9);
        // capture() handles an exact applied delivery through read-only receipt recovery.
        // The live-evidence classifier must never admit another destructive repair for it.
        assert_eq!(
            classify_evidence(
                target,
                source_authority,
                target_authority,
                materialized,
                OrphanLinkEvidencePhase::Before,
            ),
            IndexDriftRepairEvidenceState::Changed,
        );
        assert_eq!(
            classify_evidence(
                target,
                source_authority,
                target_authority,
                materialized,
                OrphanLinkEvidencePhase::After,
            ),
            IndexDriftRepairEvidenceState::Converged,
        );
    }

    #[test]
    fn newer_target_projection_refuses_a_stale_absence_watermark() {
        assert!(target_absent_at(OrphanMaterializedSource::Missing, 9));
        assert!(target_absent_at(OrphanMaterializedSource::Deleted(9), 9));
        assert!(!target_absent_at(OrphanMaterializedSource::Deleted(10), 9));
        assert!(!target_absent_at(OrphanMaterializedSource::Live(10), 9));
    }

    #[test]
    fn absent_link_without_command_delivery_is_not_convergence() {
        let (source, linked, link_name) = target();
        let target = OrphanLinkTargetRef {
            source_key: &source,
            indexed_source_version: 7,
            link_name: &link_name,
            ordinal: 1,
            linked_target: &linked,
            target_absence_source_version: 9,
        };
        assert_eq!(
            classify_evidence(
                target,
                OrphanSourceAuthority::Present {
                    source_version: 7,
                    exact_link_present: true,
                },
                OrphanTargetAuthority::Absent(9),
                OrphanMaterialized {
                    source: OrphanMaterializedSource::Live(7),
                    target: OrphanMaterializedSource::Missing,
                    link: OrphanMaterializedLink::Absent,
                    delivery: OrphanMutationDeliveryState::Missing,
                },
                OrphanLinkEvidencePhase::After,
            ),
            IndexDriftRepairEvidenceState::Changed,
        );
    }
}
