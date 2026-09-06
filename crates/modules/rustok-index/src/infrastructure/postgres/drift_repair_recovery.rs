use std::{fmt, sync::Arc};

use async_trait::async_trait;
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend,
    IsolationLevel, QueryResult, Statement, TransactionTrait,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    EntityKey, IndexDriftAuthorizedRepairCommand, IndexDriftAuthorizedRepairRecoveryCommand,
    IndexDriftRepairCommand, IndexDriftRepairCompletion, IndexDriftRepairFailure,
    IndexDriftRepairFinding, IndexDriftRepairOwner, IndexDriftRepairOwnerOutcome,
    IndexDriftRepairRecoveryAction, IndexDriftRepairRecoveryFailure,
    IndexDriftRepairRecoveryReceipt, IndexDriftRepairRecoveryState, IndexDriftRepairRecoveryStore,
    IndexDriftRepairRecoveryStoreOutcome, IndexDriftRepairReservationOutcome,
    IndexDriftRepairStore, IndexDriftRepairStoreCompletionOutcome, IndexDriftRepairTarget,
    IndexDriftRepairTargetKind, IndexDriftRepairTicket, LinkedEntityKey, LocaleKey, SchemaRef,
};

const COMMAND_TABLE: &str = "index_consistency_finding_repair_commands";
const DECISION_TABLE: &str = "index_consistency_finding_repair_recovery_decisions";
const STORAGE_UNAVAILABLE: &str = "index_drift_repair_recovery_storage_unavailable";
const STORED_CONTRACT_INVALID: &str = "index_drift_repair_recovery_stored_contract_invalid";
const COMMAND_ID_CONFLICT: &str = "index_drift_repair_command_id_conflict";
const DECISION_ID_CONFLICT: &str = "index_drift_repair_recovery_decision_id_conflict";
const RECOVERY_REQUIRED: &str = "index_drift_repair_recovery_required";
const RECOVERY_PAUSED: &str = "index_drift_repair_recovery_paused";
const RECOVERY_ABANDONED: &str = "index_drift_repair_recovery_abandoned";
const UNSUPPORTED_BACKEND: &str = "index_drift_repair_unsupported_backend";
const COMMAND_PAYLOAD_DOMAIN: &[u8] = b"index_drift_repair_command_v1";

#[derive(Clone)]
pub struct PostgresIndexDriftRepairRecoveryStore {
    db: DatabaseConnection,
}

impl PostgresIndexDriftRepairRecoveryStore {
    pub fn new(db: DatabaseConnection) -> Result<Self, IndexDriftRepairRecoveryFailure> {
        ensure_postgres_recovery(&db)?;
        Ok(Self { db })
    }

    async fn apply_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        authorized: &IndexDriftAuthorizedRepairRecoveryCommand,
    ) -> Result<IndexDriftRepairRecoveryStoreOutcome, IndexDriftRepairRecoveryFailure> {
        let command = authorized.command();
        lock_command_recovery(transaction, command.tenant_id(), command.command_id()).await?;
        let Some(stored_command) =
            load_command_identity(transaction, command.tenant_id(), command.command_id()).await?
        else {
            return Ok(IndexDriftRepairRecoveryStoreOutcome::NotFound);
        };
        if stored_command.finding_id != command.finding_id()
            || stored_command.payload_digest != command.payload_digest()
        {
            return Err(permanent_recovery_failure(COMMAND_ID_CONFLICT));
        }
        if stored_command.state == "completed" {
            return Ok(IndexDriftRepairRecoveryStoreOutcome::AlreadyCompleted);
        }
        if stored_command.state != "prepared" {
            return Err(permanent_recovery_failure(STORED_CONTRACT_INVALID));
        }

        if let Some(existing) = load_decision_by_id(
            transaction,
            command.tenant_id(),
            command.command_id(),
            command.decision_id(),
        )
        .await?
        {
            if !existing.matches_operator_command(command) {
                return Err(permanent_recovery_failure(DECISION_ID_CONFLICT));
            }
            return Ok(IndexDriftRepairRecoveryStoreOutcome::AlreadyApplied(
                existing.into_receipt()?,
            ));
        }

        let latest =
            load_latest_decision(transaction, command.tenant_id(), command.command_id()).await?;
        let current_revision = latest.as_ref().map(|value| value.revision);
        if current_revision != command.expected_revision() {
            return Ok(IndexDriftRepairRecoveryStoreOutcome::StaleRevision { current_revision });
        }
        let current_state = latest.as_ref().map(|value| value.state);
        let next_state = match (command.action(), current_state) {
            (IndexDriftRepairRecoveryAction::Resume, None)
            | (
                IndexDriftRepairRecoveryAction::Resume,
                Some(IndexDriftRepairRecoveryState::Paused),
            ) => IndexDriftRepairRecoveryState::Active,
            (
                IndexDriftRepairRecoveryAction::Pause,
                Some(IndexDriftRepairRecoveryState::Active),
            ) => IndexDriftRepairRecoveryState::Paused,
            (IndexDriftRepairRecoveryAction::Abandon, None)
            | (
                IndexDriftRepairRecoveryAction::Abandon,
                Some(IndexDriftRepairRecoveryState::Active),
            )
            | (
                IndexDriftRepairRecoveryAction::Abandon,
                Some(IndexDriftRepairRecoveryState::Paused),
            ) => IndexDriftRepairRecoveryState::Abandoned,
            _ => {
                return Ok(IndexDriftRepairRecoveryStoreOutcome::InvalidTransition {
                    current_state,
                });
            }
        };

        if command.action() == IndexDriftRepairRecoveryAction::Resume
            && !exact_finding_is_open_recovery(
                transaction,
                command.tenant_id(),
                command.finding_id(),
            )
            .await?
        {
            return Ok(IndexDriftRepairRecoveryStoreOutcome::FindingNotOpen);
        }

        let revision = match current_revision {
            Some(value) => value
                .checked_add(1)
                .ok_or_else(|| permanent_recovery_failure(STORED_CONTRACT_INVALID))?,
            None => 0,
        };
        insert_decision(
            transaction,
            DecisionInsert {
                tenant_id: command.tenant_id(),
                command_id: command.command_id(),
                decision_id: command.decision_id(),
                finding_id: command.finding_id(),
                payload_digest: command.payload_digest(),
                revision,
                action: recovery_action_text(command.action()),
                previous_state: optional_recovery_state_text(current_state),
                new_state: recovery_state_text(next_state),
                actor_kind: command.actor().kind(),
                actor_subject: command.actor().subject(),
                reason: command.reason(),
            },
        )
        .await?;

        Ok(IndexDriftRepairRecoveryStoreOutcome::Applied(
            IndexDriftRepairRecoveryReceipt::new(
                command.decision_id(),
                command.command_id(),
                command.finding_id(),
                revision,
                command.action(),
                current_state,
                next_state,
            )
            .map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))?,
        ))
    }
}

#[async_trait]
impl IndexDriftRepairRecoveryStore for PostgresIndexDriftRepairRecoveryStore {
    async fn apply(
        &self,
        authorized: &IndexDriftAuthorizedRepairRecoveryCommand,
    ) -> Result<IndexDriftRepairRecoveryStoreOutcome, IndexDriftRepairRecoveryFailure> {
        ensure_postgres_recovery(&self.db)?;
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::Serializable),
                Some(AccessMode::ReadWrite),
            )
            .await
            .map_err(|_| retryable_recovery_failure(STORAGE_UNAVAILABLE))?;
        let result = self.apply_in_transaction(&transaction, authorized).await;
        match result {
            Ok(outcome) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| retryable_recovery_failure(STORAGE_UNAVAILABLE))?;
                Ok(outcome)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

impl fmt::Debug for PostgresIndexDriftRepairRecoveryStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexDriftRepairRecoveryStore")
            .finish_non_exhaustive()
    }
}

pub fn materialize_postgres_index_drift_repair_recovery_store(
    db: DatabaseConnection,
) -> Result<Arc<dyn IndexDriftRepairRecoveryStore>, IndexDriftRepairRecoveryFailure> {
    Ok(Arc::new(PostgresIndexDriftRepairRecoveryStore::new(db)?))
}

#[derive(Clone)]
pub struct RecoveryAwareIndexDriftRepairStore {
    db: DatabaseConnection,
    inner: Arc<dyn IndexDriftRepairStore>,
}

impl RecoveryAwareIndexDriftRepairStore {
    pub fn new(
        db: DatabaseConnection,
        inner: Arc<dyn IndexDriftRepairStore>,
    ) -> Result<Self, IndexDriftRepairFailure> {
        ensure_postgres_repair(&db)?;
        Ok(Self { db, inner })
    }

    async fn gate_command(
        &self,
        command: &IndexDriftRepairCommand,
    ) -> Result<(), IndexDriftRepairFailure> {
        let payload_digest = command_payload_digest(command);
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::Serializable),
                Some(AccessMode::ReadWrite),
            )
            .await
            .map_err(|_| retryable_repair_failure(STORAGE_UNAVAILABLE))?;
        let result = async {
            lock_command_repair(&transaction, command.tenant_id(), command.command_id()).await?;
            let Some(stored) = load_command_identity_repair(
                &transaction,
                command.tenant_id(),
                command.command_id(),
            )
            .await?
            else {
                return Ok(());
            };
            if stored.finding_id != command.finding_id() || stored.payload_digest != payload_digest
            {
                return Err(permanent_repair_failure(COMMAND_ID_CONFLICT));
            }
            if stored.state == "completed" {
                return Ok(());
            }
            if stored.state != "prepared" {
                return Err(permanent_repair_failure(STORED_CONTRACT_INVALID));
            }
            require_active_repair_state(&transaction, command.tenant_id(), command.command_id())
                .await
        }
        .await;
        finish_repair_transaction(transaction, result).await
    }

    async fn ensure_initial_active(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
    ) -> Result<(), IndexDriftRepairFailure> {
        let command = authorized.command();
        let payload_digest = command_payload_digest(command);
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::Serializable),
                Some(AccessMode::ReadWrite),
            )
            .await
            .map_err(|_| retryable_repair_failure(STORAGE_UNAVAILABLE))?;
        let result = async {
            lock_command_repair(&transaction, command.tenant_id(), command.command_id()).await?;
            let stored = load_command_identity_repair(
                &transaction,
                command.tenant_id(),
                command.command_id(),
            )
            .await?
            .ok_or_else(|| permanent_repair_failure(STORED_CONTRACT_INVALID))?;
            if stored.finding_id != command.finding_id()
                || stored.payload_digest != payload_digest
                || stored.state != "prepared"
            {
                return Err(permanent_repair_failure(STORED_CONTRACT_INVALID));
            }
            if let Some(latest) =
                load_latest_decision_repair(&transaction, command.tenant_id(), command.command_id())
                    .await?
            {
                return match latest.state {
                    IndexDriftRepairRecoveryState::Active => Ok(()),
                    IndexDriftRepairRecoveryState::Paused => {
                        Err(permanent_repair_failure(RECOVERY_PAUSED))
                    }
                    IndexDriftRepairRecoveryState::Abandoned => {
                        Err(permanent_repair_failure(RECOVERY_ABANDONED))
                    }
                };
            }
            insert_decision_repair(
                &transaction,
                DecisionInsert {
                    tenant_id: command.tenant_id(),
                    command_id: command.command_id(),
                    decision_id: command.command_id(),
                    finding_id: command.finding_id(),
                    payload_digest: &payload_digest,
                    revision: 0,
                    action: "activate",
                    previous_state: "unclassified",
                    new_state: "active",
                    actor_kind: command.actor().kind(),
                    actor_subject: command.actor().subject(),
                    reason: command.reason(),
                },
            )
            .await
        }
        .await;
        finish_repair_transaction(transaction, result).await
    }

    async fn gate_ticket(
        &self,
        ticket: &IndexDriftRepairTicket,
    ) -> Result<(), IndexDriftRepairFailure> {
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::Serializable),
                Some(AccessMode::ReadWrite),
            )
            .await
            .map_err(|_| retryable_repair_failure(STORAGE_UNAVAILABLE))?;
        let result = async {
            lock_command_repair(&transaction, ticket.tenant_id(), ticket.command_id()).await?;
            let stored =
                load_command_identity_repair(&transaction, ticket.tenant_id(), ticket.command_id())
                    .await?
                    .ok_or_else(|| permanent_repair_failure(STORED_CONTRACT_INVALID))?;
            if stored.finding_id != ticket.finding_id()
                || stored.payload_digest != ticket.reservation_digest()
            {
                return Err(permanent_repair_failure(COMMAND_ID_CONFLICT));
            }
            if stored.state == "completed" {
                return Ok(());
            }
            if stored.state != "prepared" {
                return Err(permanent_repair_failure(STORED_CONTRACT_INVALID));
            }
            require_active_repair_state(&transaction, ticket.tenant_id(), ticket.command_id()).await
        }
        .await;
        finish_repair_transaction(transaction, result).await
    }
}

#[async_trait]
impl IndexDriftRepairStore for RecoveryAwareIndexDriftRepairStore {
    async fn reserve(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
    ) -> Result<IndexDriftRepairReservationOutcome, IndexDriftRepairFailure> {
        self.gate_command(authorized.command()).await?;
        let outcome = self.inner.reserve(authorized).await?;
        if matches!(
            &outcome,
            IndexDriftRepairReservationOutcome::Reserved { .. }
        ) {
            self.ensure_initial_active(authorized).await?;
        }
        Ok(outcome)
    }

    async fn complete(
        &self,
        ticket: &IndexDriftRepairTicket,
        completion: &IndexDriftRepairCompletion,
    ) -> Result<IndexDriftRepairStoreCompletionOutcome, IndexDriftRepairFailure> {
        self.gate_ticket(ticket).await?;
        self.inner.complete(ticket, completion).await
    }
}

impl fmt::Debug for RecoveryAwareIndexDriftRepairStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveryAwareIndexDriftRepairStore")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct RecoveryAwareIndexDriftRepairOwner {
    db: DatabaseConnection,
    inner: Arc<dyn IndexDriftRepairOwner>,
}

impl RecoveryAwareIndexDriftRepairOwner {
    pub fn new(
        db: DatabaseConnection,
        inner: Arc<dyn IndexDriftRepairOwner>,
    ) -> Result<Self, IndexDriftRepairFailure> {
        ensure_postgres_repair(&db)?;
        Ok(Self { db, inner })
    }
}

#[async_trait]
impl IndexDriftRepairOwner for RecoveryAwareIndexDriftRepairOwner {
    fn owner_name(&self) -> &str {
        self.inner.owner_name()
    }

    fn target_kind(&self) -> IndexDriftRepairTargetKind {
        self.inner.target_kind()
    }

    async fn repair(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
        before: &crate::IndexDriftRepairEvidence,
    ) -> Result<IndexDriftRepairOwnerOutcome, IndexDriftRepairFailure> {
        let command = authorized.command();
        let payload_digest = command_payload_digest(command);
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::Serializable),
                Some(AccessMode::ReadWrite),
            )
            .await
            .map_err(|_| retryable_repair_failure(STORAGE_UNAVAILABLE))?;
        let result = async {
            lock_command_repair(&transaction, command.tenant_id(), command.command_id()).await?;
            let stored = load_command_identity_repair(
                &transaction,
                command.tenant_id(),
                command.command_id(),
            )
            .await?
            .ok_or_else(|| permanent_repair_failure(STORED_CONTRACT_INVALID))?;
            if stored.finding_id != command.finding_id()
                || stored.finding_id != finding.finding_id()
                || stored.payload_digest != payload_digest
                || stored.state != "prepared"
            {
                return Err(permanent_repair_failure(COMMAND_ID_CONFLICT));
            }
            require_active_repair_state(&transaction, command.tenant_id(), command.command_id())
                .await?;
            self.inner.repair(authorized, finding, before).await
        }
        .await;
        match result {
            Ok(outcome) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| retryable_repair_failure(STORAGE_UNAVAILABLE))?;
                Ok(outcome)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

impl fmt::Debug for RecoveryAwareIndexDriftRepairOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveryAwareIndexDriftRepairOwner")
            .field("owner_name", &self.inner.owner_name())
            .field("target_kind", &self.inner.target_kind())
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct StoredCommandIdentity {
    finding_id: Uuid,
    payload_digest: String,
    state: String,
}

#[derive(Clone)]
struct LatestDecision {
    revision: u64,
    state: IndexDriftRepairRecoveryState,
}

#[derive(Clone)]
struct StoredDecision {
    command_id: Uuid,
    decision_id: Uuid,
    finding_id: Uuid,
    payload_digest: String,
    revision: u64,
    action: String,
    previous_state: String,
    new_state: String,
    actor_kind: String,
    actor_subject: String,
    reason: String,
}

impl StoredDecision {
    fn matches_operator_command(&self, command: &crate::IndexDriftRepairRecoveryCommand) -> bool {
        self.command_id == command.command_id()
            && self.decision_id == command.decision_id()
            && self.finding_id == command.finding_id()
            && self.payload_digest == command.payload_digest()
            && self.expected_revision() == command.expected_revision()
            && self.action == recovery_action_text(command.action())
            && self.actor_kind == command.actor().kind()
            && self.actor_subject == command.actor().subject()
            && self.reason == command.reason()
    }

    fn expected_revision(&self) -> Option<u64> {
        self.revision.checked_sub(1)
    }

    fn into_receipt(
        self,
    ) -> Result<IndexDriftRepairRecoveryReceipt, IndexDriftRepairRecoveryFailure> {
        let action = decode_operator_action(&self.action)?;
        let previous_state = decode_optional_state(&self.previous_state)?;
        let current_state = decode_state(&self.new_state)?;
        IndexDriftRepairRecoveryReceipt::new(
            self.decision_id,
            self.command_id,
            self.finding_id,
            self.revision,
            action,
            previous_state,
            current_state,
        )
        .map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))
    }
}

struct DecisionInsert<'a> {
    tenant_id: Uuid,
    command_id: Uuid,
    decision_id: Uuid,
    finding_id: Uuid,
    payload_digest: &'a str,
    revision: u64,
    action: &'a str,
    previous_state: &'a str,
    new_state: &'a str,
    actor_kind: &'a str,
    actor_subject: &'a str,
    reason: &'a str,
}

async fn finish_repair_transaction(
    transaction: DatabaseTransaction,
    result: Result<(), IndexDriftRepairFailure>,
) -> Result<(), IndexDriftRepairFailure> {
    match result {
        Ok(()) => {
            transaction
                .commit()
                .await
                .map_err(|_| retryable_repair_failure(STORAGE_UNAVAILABLE))?;
            Ok(())
        }
        Err(error) => {
            let _ = transaction.rollback().await;
            Err(error)
        }
    }
}

async fn load_command_identity(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    command_id: Uuid,
) -> Result<Option<StoredCommandIdentity>, IndexDriftRepairRecoveryFailure> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT finding_id, payload_digest, state FROM {COMMAND_TABLE} WHERE tenant_id = $1 AND command_id = $2 FOR UPDATE"
            ),
            vec![tenant_id.into(), command_id.into()],
        ))
        .await
        .map_err(|_| retryable_recovery_failure(STORAGE_UNAVAILABLE))?
        .map(decode_command_identity_recovery)
        .transpose()
}

async fn load_command_identity_repair(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    command_id: Uuid,
) -> Result<Option<StoredCommandIdentity>, IndexDriftRepairFailure> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT finding_id, payload_digest, state FROM {COMMAND_TABLE} WHERE tenant_id = $1 AND command_id = $2 FOR UPDATE"
            ),
            vec![tenant_id.into(), command_id.into()],
        ))
        .await
        .map_err(|_| retryable_repair_failure(STORAGE_UNAVAILABLE))?
        .map(decode_command_identity_repair)
        .transpose()
}

async fn load_latest_decision(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    command_id: Uuid,
) -> Result<Option<LatestDecision>, IndexDriftRepairRecoveryFailure> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT revision, new_state FROM {DECISION_TABLE} WHERE tenant_id = $1 AND command_id = $2 ORDER BY revision DESC LIMIT 1 FOR UPDATE"
            ),
            vec![tenant_id.into(), command_id.into()],
        ))
        .await
        .map_err(|_| retryable_recovery_failure(STORAGE_UNAVAILABLE))?
        .map(decode_latest_decision_recovery)
        .transpose()
}

async fn load_latest_decision_repair(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    command_id: Uuid,
) -> Result<Option<LatestDecision>, IndexDriftRepairFailure> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT revision, new_state FROM {DECISION_TABLE} WHERE tenant_id = $1 AND command_id = $2 ORDER BY revision DESC LIMIT 1 FOR UPDATE"
            ),
            vec![tenant_id.into(), command_id.into()],
        ))
        .await
        .map_err(|_| retryable_repair_failure(STORAGE_UNAVAILABLE))?
        .map(decode_latest_decision_repair)
        .transpose()
}

async fn load_decision_by_id(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    command_id: Uuid,
    decision_id: Uuid,
) -> Result<Option<StoredDecision>, IndexDriftRepairRecoveryFailure> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT command_id, decision_id, finding_id, payload_digest, revision, action, previous_state, new_state, actor_kind, actor_subject, reason FROM {DECISION_TABLE} WHERE tenant_id = $1 AND command_id = $2 AND decision_id = $3 FOR UPDATE"
            ),
            vec![tenant_id.into(), command_id.into(), decision_id.into()],
        ))
        .await
        .map_err(|_| retryable_recovery_failure(STORAGE_UNAVAILABLE))?
        .map(decode_stored_decision)
        .transpose()
}

async fn insert_decision(
    transaction: &DatabaseTransaction,
    decision: DecisionInsert<'_>,
) -> Result<(), IndexDriftRepairRecoveryFailure> {
    let revision = i64::try_from(decision.revision)
        .map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))?;
    let inserted = transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "INSERT INTO {DECISION_TABLE} (tenant_id, command_id, decision_id, finding_id, payload_digest, revision, action, previous_state, new_state, actor_kind, actor_subject, reason) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"
            ),
            vec![
                decision.tenant_id.into(),
                decision.command_id.into(),
                decision.decision_id.into(),
                decision.finding_id.into(),
                decision.payload_digest.to_owned().into(),
                revision.into(),
                decision.action.to_owned().into(),
                decision.previous_state.to_owned().into(),
                decision.new_state.to_owned().into(),
                decision.actor_kind.to_owned().into(),
                decision.actor_subject.to_owned().into(),
                decision.reason.to_owned().into(),
            ],
        ))
        .await
        .map_err(|_| retryable_recovery_failure(STORAGE_UNAVAILABLE))?;
    if inserted.rows_affected() != 1 {
        return Err(retryable_recovery_failure(STORAGE_UNAVAILABLE));
    }
    Ok(())
}

async fn insert_decision_repair(
    transaction: &DatabaseTransaction,
    decision: DecisionInsert<'_>,
) -> Result<(), IndexDriftRepairFailure> {
    let revision = i64::try_from(decision.revision)
        .map_err(|_| permanent_repair_failure(STORED_CONTRACT_INVALID))?;
    let inserted = transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "INSERT INTO {DECISION_TABLE} (tenant_id, command_id, decision_id, finding_id, payload_digest, revision, action, previous_state, new_state, actor_kind, actor_subject, reason) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"
            ),
            vec![
                decision.tenant_id.into(),
                decision.command_id.into(),
                decision.decision_id.into(),
                decision.finding_id.into(),
                decision.payload_digest.to_owned().into(),
                revision.into(),
                decision.action.to_owned().into(),
                decision.previous_state.to_owned().into(),
                decision.new_state.to_owned().into(),
                decision.actor_kind.to_owned().into(),
                decision.actor_subject.to_owned().into(),
                decision.reason.to_owned().into(),
            ],
        ))
        .await
        .map_err(|_| retryable_repair_failure(STORAGE_UNAVAILABLE))?;
    if inserted.rows_affected() != 1 {
        return Err(retryable_repair_failure(STORAGE_UNAVAILABLE));
    }
    Ok(())
}

async fn exact_finding_is_open_recovery(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    finding_id: Uuid,
) -> Result<bool, IndexDriftRepairRecoveryFailure> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT finding_id FROM index_consistency_findings WHERE tenant_id = $1 AND finding_id = $2 AND state = 'open' LIMIT 1 FOR SHARE",
            vec![tenant_id.into(), finding_id.into()],
        ))
        .await
        .map(|row| row.is_some())
        .map_err(|_| retryable_recovery_failure(STORAGE_UNAVAILABLE))
}

async fn require_active_repair_state(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    command_id: Uuid,
) -> Result<(), IndexDriftRepairFailure> {
    match load_latest_decision_repair(transaction, tenant_id, command_id).await? {
        Some(LatestDecision {
            state: IndexDriftRepairRecoveryState::Active,
            ..
        }) => Ok(()),
        Some(LatestDecision {
            state: IndexDriftRepairRecoveryState::Paused,
            ..
        }) => Err(permanent_repair_failure(RECOVERY_PAUSED)),
        Some(LatestDecision {
            state: IndexDriftRepairRecoveryState::Abandoned,
            ..
        }) => Err(permanent_repair_failure(RECOVERY_ABANDONED)),
        None => Err(permanent_repair_failure(RECOVERY_REQUIRED)),
    }
}

async fn lock_command_recovery(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    command_id: Uuid,
) -> Result<(), IndexDriftRepairRecoveryFailure> {
    let key = command_lock_key(tenant_id, command_id);
    transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![key.into()],
        ))
        .await
        .map_err(|_| retryable_recovery_failure(STORAGE_UNAVAILABLE))?;
    Ok(())
}

async fn lock_command_repair(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    command_id: Uuid,
) -> Result<(), IndexDriftRepairFailure> {
    let key = command_lock_key(tenant_id, command_id);
    transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![key.into()],
        ))
        .await
        .map_err(|_| retryable_repair_failure(STORAGE_UNAVAILABLE))?;
    Ok(())
}

fn command_lock_key(tenant_id: Uuid, command_id: Uuid) -> String {
    format!("index-drift-repair-command\u{1f}{tenant_id}\u{1f}{command_id}")
}

fn decode_command_identity_recovery(
    row: QueryResult,
) -> Result<StoredCommandIdentity, IndexDriftRepairRecoveryFailure> {
    let finding_id = row
        .try_get::<Uuid>("", "finding_id")
        .map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))?;
    let payload_digest = row
        .try_get::<String>("", "payload_digest")
        .map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))?;
    let state = row
        .try_get::<String>("", "state")
        .map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))?;
    if finding_id.is_nil() || !valid_digest(&payload_digest) || state.is_empty() {
        return Err(permanent_recovery_failure(STORED_CONTRACT_INVALID));
    }
    Ok(StoredCommandIdentity {
        finding_id,
        payload_digest,
        state,
    })
}

fn decode_command_identity_repair(
    row: QueryResult,
) -> Result<StoredCommandIdentity, IndexDriftRepairFailure> {
    let finding_id = row
        .try_get::<Uuid>("", "finding_id")
        .map_err(|_| permanent_repair_failure(STORED_CONTRACT_INVALID))?;
    let payload_digest = row
        .try_get::<String>("", "payload_digest")
        .map_err(|_| permanent_repair_failure(STORED_CONTRACT_INVALID))?;
    let state = row
        .try_get::<String>("", "state")
        .map_err(|_| permanent_repair_failure(STORED_CONTRACT_INVALID))?;
    if finding_id.is_nil() || !valid_digest(&payload_digest) || state.is_empty() {
        return Err(permanent_repair_failure(STORED_CONTRACT_INVALID));
    }
    Ok(StoredCommandIdentity {
        finding_id,
        payload_digest,
        state,
    })
}

fn decode_latest_decision_recovery(
    row: QueryResult,
) -> Result<LatestDecision, IndexDriftRepairRecoveryFailure> {
    let revision = decode_revision_recovery(&row)?;
    let state = decode_state(
        &row.try_get::<String>("", "new_state")
            .map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))?,
    )?;
    Ok(LatestDecision { revision, state })
}

fn decode_latest_decision_repair(
    row: QueryResult,
) -> Result<LatestDecision, IndexDriftRepairFailure> {
    let raw_revision = row
        .try_get::<i64>("", "revision")
        .map_err(|_| permanent_repair_failure(STORED_CONTRACT_INVALID))?;
    let revision = u64::try_from(raw_revision)
        .map_err(|_| permanent_repair_failure(STORED_CONTRACT_INVALID))?;
    let state_text = row
        .try_get::<String>("", "new_state")
        .map_err(|_| permanent_repair_failure(STORED_CONTRACT_INVALID))?;
    let state = decode_state_repair(&state_text)?;
    Ok(LatestDecision { revision, state })
}

fn decode_stored_decision(
    row: QueryResult,
) -> Result<StoredDecision, IndexDriftRepairRecoveryFailure> {
    let command_id = row
        .try_get::<Uuid>("", "command_id")
        .map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))?;
    let decision_id = row
        .try_get::<Uuid>("", "decision_id")
        .map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))?;
    let finding_id = row
        .try_get::<Uuid>("", "finding_id")
        .map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))?;
    let payload_digest = row
        .try_get::<String>("", "payload_digest")
        .map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))?;
    if command_id.is_nil()
        || decision_id.is_nil()
        || finding_id.is_nil()
        || !valid_digest(&payload_digest)
    {
        return Err(permanent_recovery_failure(STORED_CONTRACT_INVALID));
    }
    Ok(StoredDecision {
        command_id,
        decision_id,
        finding_id,
        payload_digest,
        revision: decode_revision_recovery(&row)?,
        action: required_text_recovery(&row, "action")?,
        previous_state: required_text_recovery(&row, "previous_state")?,
        new_state: required_text_recovery(&row, "new_state")?,
        actor_kind: required_text_recovery(&row, "actor_kind")?,
        actor_subject: required_text_recovery(&row, "actor_subject")?,
        reason: required_text_recovery(&row, "reason")?,
    })
}

fn decode_revision_recovery(row: &QueryResult) -> Result<u64, IndexDriftRepairRecoveryFailure> {
    let raw = row
        .try_get::<i64>("", "revision")
        .map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))?;
    u64::try_from(raw).map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))
}

fn required_text_recovery(
    row: &QueryResult,
    column: &str,
) -> Result<String, IndexDriftRepairRecoveryFailure> {
    let value = row
        .try_get::<String>("", column)
        .map_err(|_| permanent_recovery_failure(STORED_CONTRACT_INVALID))?;
    if value.is_empty() {
        return Err(permanent_recovery_failure(STORED_CONTRACT_INVALID));
    }
    Ok(value)
}

fn decode_operator_action(
    value: &str,
) -> Result<IndexDriftRepairRecoveryAction, IndexDriftRepairRecoveryFailure> {
    match value {
        "resume" => Ok(IndexDriftRepairRecoveryAction::Resume),
        "pause" => Ok(IndexDriftRepairRecoveryAction::Pause),
        "abandon" => Ok(IndexDriftRepairRecoveryAction::Abandon),
        _ => Err(permanent_recovery_failure(STORED_CONTRACT_INVALID)),
    }
}

fn decode_state(
    value: &str,
) -> Result<IndexDriftRepairRecoveryState, IndexDriftRepairRecoveryFailure> {
    match value {
        "active" => Ok(IndexDriftRepairRecoveryState::Active),
        "paused" => Ok(IndexDriftRepairRecoveryState::Paused),
        "abandoned" => Ok(IndexDriftRepairRecoveryState::Abandoned),
        _ => Err(permanent_recovery_failure(STORED_CONTRACT_INVALID)),
    }
}

fn decode_state_repair(
    value: &str,
) -> Result<IndexDriftRepairRecoveryState, IndexDriftRepairFailure> {
    match value {
        "active" => Ok(IndexDriftRepairRecoveryState::Active),
        "paused" => Ok(IndexDriftRepairRecoveryState::Paused),
        "abandoned" => Ok(IndexDriftRepairRecoveryState::Abandoned),
        _ => Err(permanent_repair_failure(STORED_CONTRACT_INVALID)),
    }
}

fn decode_optional_state(
    value: &str,
) -> Result<Option<IndexDriftRepairRecoveryState>, IndexDriftRepairRecoveryFailure> {
    if value == "unclassified" {
        Ok(None)
    } else {
        decode_state(value).map(Some)
    }
}

fn recovery_action_text(action: IndexDriftRepairRecoveryAction) -> &'static str {
    match action {
        IndexDriftRepairRecoveryAction::Resume => "resume",
        IndexDriftRepairRecoveryAction::Pause => "pause",
        IndexDriftRepairRecoveryAction::Abandon => "abandon",
    }
}

fn recovery_state_text(state: IndexDriftRepairRecoveryState) -> &'static str {
    match state {
        IndexDriftRepairRecoveryState::Active => "active",
        IndexDriftRepairRecoveryState::Paused => "paused",
        IndexDriftRepairRecoveryState::Abandoned => "abandoned",
    }
}

fn optional_recovery_state_text(state: Option<IndexDriftRepairRecoveryState>) -> &'static str {
    state.map_or("unclassified", recovery_state_text)
}

fn command_payload_digest(command: &IndexDriftRepairCommand) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, COMMAND_PAYLOAD_DOMAIN);
    hash_component(&mut hasher, command.tenant_id().as_bytes());
    hash_component(&mut hasher, command.finding_id().as_bytes());
    hash_component(&mut hasher, command.command_id().as_bytes());
    hash_repair_target(&mut hasher, command.target());
    hash_component(&mut hasher, command.actor().kind().as_bytes());
    hash_component(&mut hasher, command.actor().subject().as_bytes());
    hash_component(&mut hasher, command.reason().as_bytes());
    hex::encode(hasher.finalize())
}

fn hash_repair_target(hasher: &mut Sha256, target: &IndexDriftRepairTarget) {
    match target {
        IndexDriftRepairTarget::MissingEntity {
            key,
            indexed_source_version,
            absence_source_version,
        } => {
            hash_component(hasher, b"missing_entity");
            hash_entity_key(hasher, key);
            hash_component(hasher, &indexed_source_version.to_be_bytes());
            hash_component(hasher, &absence_source_version.to_be_bytes());
        }
        IndexDriftRepairTarget::OrphanLink {
            source_key,
            indexed_source_version,
            link_name,
            ordinal,
            target,
            target_absence_source_version,
        } => {
            hash_component(hasher, b"orphan_link");
            hash_entity_key(hasher, source_key);
            hash_component(hasher, &indexed_source_version.to_be_bytes());
            hash_component(hasher, link_name.as_str().as_bytes());
            hash_component(hasher, &ordinal.to_be_bytes());
            hash_linked_key(hasher, target);
            hash_component(hasher, &target_absence_source_version.to_be_bytes());
        }
    }
}

fn hash_entity_key(hasher: &mut Sha256, key: &EntityKey) {
    hash_component(hasher, key.tenant_id.as_bytes());
    hash_schema(hasher, &key.schema);
    hash_component(hasher, key.entity_id.as_bytes());
    hash_locale(hasher, key.locale.as_ref());
}

fn hash_linked_key(hasher: &mut Sha256, key: &LinkedEntityKey) {
    hash_schema(hasher, &key.schema);
    hash_component(hasher, key.entity_id.as_bytes());
    hash_locale(hasher, key.locale.as_ref());
}

fn hash_schema(hasher: &mut Sha256, schema: &SchemaRef) {
    hash_component(hasher, schema.module.as_str().as_bytes());
    hash_component(hasher, schema.entity.as_str().as_bytes());
    hash_component(hasher, &schema.version.get().to_be_bytes());
}

fn hash_locale(hasher: &mut Sha256, locale: Option<&LocaleKey>) {
    match locale {
        Some(locale) => {
            hash_component(hasher, b"locale");
            hash_component(hasher, locale.as_str().as_bytes());
        }
        None => hash_component(hasher, b"no_locale"),
    }
}

fn hash_component(hasher: &mut Sha256, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("bounded repair recovery digest component");
    hasher.update(length.to_be_bytes());
    hasher.update(value);
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn ensure_postgres_recovery(
    db: &DatabaseConnection,
) -> Result<(), IndexDriftRepairRecoveryFailure> {
    if db.get_database_backend() == DbBackend::Postgres {
        Ok(())
    } else {
        Err(permanent_recovery_failure(UNSUPPORTED_BACKEND))
    }
}

fn ensure_postgres_repair(db: &DatabaseConnection) -> Result<(), IndexDriftRepairFailure> {
    if db.get_database_backend() == DbBackend::Postgres {
        Ok(())
    } else {
        Err(permanent_repair_failure(UNSUPPORTED_BACKEND))
    }
}

fn retryable_recovery_failure(code: &str) -> IndexDriftRepairRecoveryFailure {
    IndexDriftRepairRecoveryFailure::retryable(code)
        .expect("static repair recovery failure code is valid")
}

fn permanent_recovery_failure(code: &str) -> IndexDriftRepairRecoveryFailure {
    IndexDriftRepairRecoveryFailure::permanent(code)
        .expect("static repair recovery failure code is valid")
}

fn retryable_repair_failure(code: &str) -> IndexDriftRepairFailure {
    IndexDriftRepairFailure::retryable(code).expect("static repair failure code is valid")
}

fn permanent_repair_failure(code: &str) -> IndexDriftRepairFailure {
    IndexDriftRepairFailure::permanent(code).expect("static repair failure code is valid")
}
