use std::{fmt, sync::Arc};

use async_trait::async_trait;
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend,
    IsolationLevel, QueryResult, Statement, TransactionTrait,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    EntityKey, EntityName, IndexDriftAuthorizedRepairCommand, IndexDriftRepairCommand,
    IndexDriftRepairCompletion, IndexDriftRepairFailure, IndexDriftRepairFinding,
    IndexDriftRepairNotStartedReason, IndexDriftRepairReceipt, IndexDriftRepairReceiptOutcome,
    IndexDriftRepairReservationOutcome, IndexDriftRepairStore,
    IndexDriftRepairStoreCompletionOutcome, IndexDriftRepairTarget, IndexDriftRepairTargetKind,
    IndexDriftRepairTicket, LinkedEntityKey, LocaleKey, ModuleName, SchemaRef, SchemaVersion,
};

const STORAGE_UNAVAILABLE: &str = "index_drift_repair_storage_unavailable";
const STORED_CONTRACT_INVALID: &str = "index_drift_repair_stored_contract_invalid";
const COMMAND_ID_CONFLICT: &str = "index_drift_repair_command_id_conflict";
const RESERVATION_INVALID: &str = "index_drift_repair_reservation_invalid";
const RESERVATION_AMBIGUOUS: &str = "index_drift_repair_reservation_ambiguous";
const UNSUPPORTED_BACKEND: &str = "index_drift_repair_unsupported_backend";
const MISSING_ENTITY_CHECK: &str = "index.confirmed_missing_entity";
const ORPHAN_LINK_CHECK_PREFIX: &str = "index.confirmed_orphan_link.";
const FINDING_DETAILS_CONTRACT: &str = "index_drift_digest_finding_v1";
const MISSING_EVIDENCE_DOMAIN: &[u8] = b"index_confirmed_missing_entity_evidence_v1";
const ORPHAN_EVIDENCE_DOMAIN: &[u8] = b"index_confirmed_orphan_link_evidence_v1";
const ORPHAN_IDENTITY_DOMAIN: &[u8] = b"index_confirmed_orphan_link_identity_v1";
const FINDING_KEY_DOMAIN: &[u8] = b"index_drift_finding_key_v1";
const COMMAND_PAYLOAD_DOMAIN: &[u8] = b"index_drift_repair_command_v1";
const NO_LOCALE_COMPONENT: &[u8] = b"\0";

#[derive(Clone)]
pub struct PostgresIndexDriftRepairStore {
    db: DatabaseConnection,
}

impl PostgresIndexDriftRepairStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn reserve_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        authorized: &IndexDriftAuthorizedRepairCommand,
    ) -> Result<IndexDriftRepairReservationOutcome, IndexDriftRepairFailure> {
        let command = authorized.command();
        let payload_digest = command_payload_digest(command);
        lock_command_id(transaction, command.tenant_id(), command.command_id()).await?;

        if let Some(existing) =
            load_command(transaction, command.tenant_id(), command.command_id()).await?
        {
            if existing.finding_id != command.finding_id()
                || existing.payload_digest != payload_digest
                || existing.target_kind != target_kind_text(command.target().kind())
            {
                return Err(permanent_failure(COMMAND_ID_CONFLICT));
            }
            if existing.state == "completed" {
                return Ok(IndexDriftRepairReservationOutcome::AlreadyCompleted(
                    existing.into_receipt(command.command_id())?,
                ));
            }
            if existing.state != "prepared" {
                return Err(permanent_failure(STORED_CONTRACT_INVALID));
            }
            let finding = match load_and_validate_open_finding(transaction, command).await? {
                FindingLoadOutcome::Open(finding) => finding,
                FindingLoadOutcome::Missing | FindingLoadOutcome::NotOpen => {
                    return Err(permanent_failure(RESERVATION_AMBIGUOUS));
                }
            };
            return Ok(IndexDriftRepairReservationOutcome::Resumed {
                ticket: repair_ticket(command, payload_digest)?,
                finding,
            });
        }

        let finding = match load_and_validate_open_finding(transaction, command).await? {
            FindingLoadOutcome::Open(finding) => finding,
            FindingLoadOutcome::Missing => {
                return Ok(IndexDriftRepairReservationOutcome::NotReserved(
                    IndexDriftRepairNotStartedReason::FindingNotFound,
                ));
            }
            FindingLoadOutcome::NotOpen => {
                return Ok(IndexDriftRepairReservationOutcome::NotReserved(
                    IndexDriftRepairNotStartedReason::FindingNotOpen,
                ));
            }
        };
        if active_finding_command_exists(transaction, command).await? {
            return Ok(IndexDriftRepairReservationOutcome::NotReserved(
                IndexDriftRepairNotStartedReason::FindingBusy,
            ));
        }

        let inserted = transaction
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO index_consistency_finding_repair_commands (tenant_id, command_id, finding_id, payload_digest, target_kind, actor_kind, actor_subject, reason, state) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'prepared') ON CONFLICT (tenant_id, command_id) DO NOTHING",
                vec![
                    command.tenant_id().into(),
                    command.command_id().into(),
                    command.finding_id().into(),
                    payload_digest.clone().into(),
                    target_kind_text(command.target().kind()).into(),
                    command.actor().kind().to_owned().into(),
                    command.actor().subject().to_owned().into(),
                    command.reason().to_owned().into(),
                ],
            ))
            .await
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
        if inserted.rows_affected() != 1 {
            return Err(retryable_failure(STORAGE_UNAVAILABLE));
        }

        Ok(IndexDriftRepairReservationOutcome::Reserved {
            ticket: repair_ticket(command, payload_digest)?,
            finding,
        })
    }

    async fn complete_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        ticket: &IndexDriftRepairTicket,
        completion: &IndexDriftRepairCompletion,
    ) -> Result<IndexDriftRepairStoreCompletionOutcome, IndexDriftRepairFailure> {
        lock_command_id(transaction, ticket.tenant_id(), ticket.command_id()).await?;
        let existing = load_command_by_ticket(transaction, ticket)
            .await?
            .ok_or_else(|| permanent_failure(RESERVATION_INVALID))?;
        if existing.tenant_id != ticket.tenant_id()
            || existing.finding_id != ticket.finding_id()
            || existing.payload_digest != ticket.reservation_digest()
        {
            return Err(permanent_failure(RESERVATION_INVALID));
        }
        if existing.state == "completed" {
            return Ok(IndexDriftRepairStoreCompletionOutcome::AlreadyCompleted(
                existing.into_receipt(ticket.command_id())?,
            ));
        }
        if existing.state != "prepared" {
            return Err(permanent_failure(STORED_CONTRACT_INVALID));
        }

        let finding_open =
            exact_finding_is_open(transaction, ticket.tenant_id(), ticket.finding_id()).await?;
        let effective = if finding_open {
            completion.clone()
        } else {
            IndexDriftRepairCompletion::new(
                completion.owner_name().to_owned(),
                IndexDriftRepairReceiptOutcome::NotRepaired {
                    code: "finding_not_open".to_owned(),
                },
                completion.before_digest().to_owned(),
                completion.after_digest().map(ToOwned::to_owned),
                completion.owner_receipt_digest().map(ToOwned::to_owned),
            )
            .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?
        };

        let (outcome, outcome_code) = match effective.outcome() {
            IndexDriftRepairReceiptOutcome::Repaired => ("repaired", None),
            IndexDriftRepairReceiptOutcome::NotRepaired { code } => {
                ("not_repaired", Some(code.clone()))
            }
        };
        let updated = transaction
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "UPDATE index_consistency_finding_repair_commands SET state = 'completed', outcome = $4, outcome_code = $5, owner_name = $6, before_digest = $7, after_digest = $8, owner_receipt_digest = $9, completed_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND command_id = $2 AND finding_id = $3 AND state = 'prepared' AND payload_digest = $10",
                vec![
                    ticket.tenant_id().into(),
                    ticket.command_id().into(),
                    ticket.finding_id().into(),
                    outcome.into(),
                    outcome_code.into(),
                    effective.owner_name().to_owned().into(),
                    effective.before_digest().to_owned().into(),
                    effective.after_digest().map(ToOwned::to_owned).into(),
                    effective.owner_receipt_digest().map(ToOwned::to_owned).into(),
                    ticket.reservation_digest().to_owned().into(),
                ],
            ))
            .await
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
        if updated.rows_affected() != 1 {
            return Err(retryable_failure(STORAGE_UNAVAILABLE));
        }

        Ok(IndexDriftRepairStoreCompletionOutcome::Completed(
            IndexDriftRepairReceipt::new(
                ticket.command_id(),
                ticket.finding_id(),
                effective.outcome().clone(),
                effective.before_digest().to_owned(),
                effective.after_digest().map(ToOwned::to_owned),
                effective.owner_receipt_digest().map(ToOwned::to_owned),
            )
            .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?,
        ))
    }
}

#[async_trait]
impl IndexDriftRepairStore for PostgresIndexDriftRepairStore {
    async fn reserve(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
    ) -> Result<IndexDriftRepairReservationOutcome, IndexDriftRepairFailure> {
        ensure_postgres(&self.db)?;
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::Serializable),
                Some(AccessMode::ReadWrite),
            )
            .await
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
        let result = self.reserve_in_transaction(&transaction, authorized).await;
        match result {
            Ok(IndexDriftRepairReservationOutcome::NotReserved(reason)) => {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
                Ok(IndexDriftRepairReservationOutcome::NotReserved(reason))
            }
            Ok(outcome) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
                Ok(outcome)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }

    async fn complete(
        &self,
        ticket: &IndexDriftRepairTicket,
        completion: &IndexDriftRepairCompletion,
    ) -> Result<IndexDriftRepairStoreCompletionOutcome, IndexDriftRepairFailure> {
        ensure_postgres(&self.db)?;
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::Serializable),
                Some(AccessMode::ReadWrite),
            )
            .await
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
        let result = self
            .complete_in_transaction(&transaction, ticket, completion)
            .await;
        match result {
            Ok(outcome) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
                Ok(outcome)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

impl fmt::Debug for PostgresIndexDriftRepairStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexDriftRepairStore")
            .finish_non_exhaustive()
    }
}

pub fn materialize_postgres_index_drift_repair_store(
    db: DatabaseConnection,
) -> Result<Arc<dyn IndexDriftRepairStore>, IndexDriftRepairFailure> {
    ensure_postgres(&db)?;
    Ok(Arc::new(PostgresIndexDriftRepairStore::new(db)))
}

#[derive(Clone)]
struct StoredRepairCommand {
    tenant_id: Uuid,
    finding_id: Uuid,
    payload_digest: String,
    target_kind: String,
    state: String,
    outcome: Option<String>,
    outcome_code: Option<String>,
    before_digest: Option<String>,
    after_digest: Option<String>,
    owner_receipt_digest: Option<String>,
}

impl StoredRepairCommand {
    fn into_receipt(
        self,
        command_id: Uuid,
    ) -> Result<IndexDriftRepairReceipt, IndexDriftRepairFailure> {
        if self.state != "completed" {
            return Err(permanent_failure(STORED_CONTRACT_INVALID));
        }
        let outcome = match (self.outcome.as_deref(), self.outcome_code) {
            (Some("repaired"), None) => IndexDriftRepairReceiptOutcome::Repaired,
            (Some("not_repaired"), Some(code)) => {
                IndexDriftRepairReceiptOutcome::NotRepaired { code }
            }
            _ => return Err(permanent_failure(STORED_CONTRACT_INVALID)),
        };
        IndexDriftRepairReceipt::new(
            command_id,
            self.finding_id,
            outcome,
            self.before_digest
                .ok_or_else(|| permanent_failure(STORED_CONTRACT_INVALID))?,
            self.after_digest,
            self.owner_receipt_digest,
        )
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))
    }
}

enum FindingLoadOutcome {
    Open(IndexDriftRepairFinding),
    Missing,
    NotOpen,
}

async fn load_and_validate_open_finding(
    transaction: &DatabaseTransaction,
    command: &IndexDriftRepairCommand,
) -> Result<FindingLoadOutcome, IndexDriftRepairFailure> {
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT finding_key, check_name, state, scope_kind, module_name, entity_name, CAST(schema_version AS BIGINT) AS schema_version_value, entity_id, locale_key, expected_digest, actual_digest, details->>'contract' AS details_contract FROM index_consistency_findings WHERE tenant_id = $1 AND finding_id = $2 FOR UPDATE",
            vec![command.tenant_id().into(), command.finding_id().into()],
        ))
        .await
        .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
    let Some(row) = row else {
        return Ok(FindingLoadOutcome::Missing);
    };
    if required_text(&row, "state")? != "open" {
        return Ok(FindingLoadOutcome::NotOpen);
    }
    if required_text(&row, "details_contract")? != FINDING_DETAILS_CONTRACT {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }

    let finding_key = required_digest(&row, "finding_key")?;
    let check_name = required_text(&row, "check_name")?;
    let expected_digest = required_digest(&row, "expected_digest")?;
    let actual_digest = required_digest(&row, "actual_digest")?;
    if expected_digest == actual_digest {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    let scope_key = decode_entity_scope(&row, command.tenant_id())?;
    validate_target_commitment(
        command.target(),
        &scope_key,
        &finding_key,
        &check_name,
        &expected_digest,
        &actual_digest,
    )?;
    Ok(FindingLoadOutcome::Open(
        IndexDriftRepairFinding::new(
            command.finding_id(),
            finding_key,
            expected_digest,
            actual_digest,
            command.target().clone(),
        )
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?,
    ))
}

fn validate_target_commitment(
    target: &IndexDriftRepairTarget,
    scope_key: &EntityKey,
    finding_key: &str,
    check_name: &str,
    expected_digest: &str,
    actual_digest: &str,
) -> Result<(), IndexDriftRepairFailure> {
    if target.source_key() != scope_key {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    let expected_check = match target {
        IndexDriftRepairTarget::MissingEntity { .. } => MISSING_ENTITY_CHECK.to_owned(),
        IndexDriftRepairTarget::OrphanLink { .. } => {
            format!(
                "{ORPHAN_LINK_CHECK_PREFIX}{}",
                orphan_identity_digest(target)
            )
        }
    };
    if check_name != expected_check
        || finding_key != derive_finding_key(target.tenant_id(), &expected_check, scope_key)
    {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    let (expected, actual) = match target {
        IndexDriftRepairTarget::MissingEntity { .. } => (
            missing_evidence_digest(target, b"owner_absent"),
            missing_evidence_digest(target, b"index_present"),
        ),
        IndexDriftRepairTarget::OrphanLink { .. } => (
            orphan_evidence_digest(target, b"target_absent"),
            orphan_evidence_digest(target, b"source_link_present"),
        ),
    };
    if expected_digest != expected || actual_digest != actual {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    Ok(())
}

fn decode_entity_scope(
    row: &QueryResult,
    tenant_id: Uuid,
) -> Result<EntityKey, IndexDriftRepairFailure> {
    if required_text(row, "scope_kind")? != "entity" {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    let module = ModuleName::new(required_text(row, "module_name")?)
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?;
    let entity = EntityName::new(required_text(row, "entity_name")?)
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?;
    let version = row
        .try_get::<i64>("", "schema_version_value")
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?;
    let version = u32::try_from(version)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| permanent_failure(STORED_CONTRACT_INVALID))?;
    let entity_id = row
        .try_get::<Uuid>("", "entity_id")
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?;
    if entity_id.is_nil() {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    let locale = row
        .try_get::<Option<String>>("", "locale_key")
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?
        .map(|value| LocaleKey::new(value))
        .transpose()
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?;
    Ok(EntityKey {
        tenant_id,
        schema: SchemaRef {
            module,
            entity,
            version: SchemaVersion::new(version),
        },
        entity_id,
        locale,
    })
}

async fn active_finding_command_exists(
    transaction: &DatabaseTransaction,
    command: &IndexDriftRepairCommand,
) -> Result<bool, IndexDriftRepairFailure> {
    transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT command_id FROM index_consistency_finding_repair_commands WHERE tenant_id = $1 AND finding_id = $2 AND state = 'prepared' LIMIT 1",
            vec![command.tenant_id().into(), command.finding_id().into()],
        ))
        .await
        .map(|row| row.is_some())
        .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))
}

async fn exact_finding_is_open(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    finding_id: Uuid,
) -> Result<bool, IndexDriftRepairFailure> {
    transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT finding_id FROM index_consistency_findings WHERE tenant_id = $1 AND finding_id = $2 AND state = 'open' LIMIT 1 FOR SHARE",
            vec![tenant_id.into(), finding_id.into()],
        ))
        .await
        .map(|row| row.is_some())
        .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))
}

async fn lock_command_id(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    command_id: Uuid,
) -> Result<(), IndexDriftRepairFailure> {
    let key = format!("index-drift-repair-command\u{1f}{tenant_id}\u{1f}{command_id}");
    transaction
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![key.into()],
        ))
        .await
        .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
    Ok(())
}

async fn load_command(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    command_id: Uuid,
) -> Result<Option<StoredRepairCommand>, IndexDriftRepairFailure> {
    query_command(
        transaction,
        "SELECT tenant_id, finding_id, payload_digest, target_kind, state, outcome, outcome_code, before_digest, after_digest, owner_receipt_digest FROM index_consistency_finding_repair_commands WHERE tenant_id = $1 AND command_id = $2 FOR UPDATE",
        vec![tenant_id.into(), command_id.into()],
    )
    .await
}

async fn load_command_by_ticket(
    transaction: &DatabaseTransaction,
    ticket: &IndexDriftRepairTicket,
) -> Result<Option<StoredRepairCommand>, IndexDriftRepairFailure> {
    query_command(
        transaction,
        "SELECT tenant_id, finding_id, payload_digest, target_kind, state, outcome, outcome_code, before_digest, after_digest, owner_receipt_digest FROM index_consistency_finding_repair_commands WHERE tenant_id = $1 AND command_id = $2 AND finding_id = $3 AND payload_digest = $4 FOR UPDATE",
        vec![
            ticket.tenant_id().into(),
            ticket.command_id().into(),
            ticket.finding_id().into(),
            ticket.reservation_digest().to_owned().into(),
        ],
    )
    .await
}

async fn query_command(
    transaction: &DatabaseTransaction,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> Result<Option<StoredRepairCommand>, IndexDriftRepairFailure> {
    transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            values,
        ))
        .await
        .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?
        .map(decode_command)
        .transpose()
}

fn decode_command(row: QueryResult) -> Result<StoredRepairCommand, IndexDriftRepairFailure> {
    let tenant_id = row
        .try_get::<Uuid>("", "tenant_id")
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?;
    let finding_id = row
        .try_get::<Uuid>("", "finding_id")
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?;
    if tenant_id.is_nil() || finding_id.is_nil() {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    let target_kind = required_text(&row, "target_kind")?;
    if !matches!(target_kind.as_str(), "missing_entity" | "orphan_link") {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    Ok(StoredRepairCommand {
        tenant_id,
        finding_id,
        payload_digest: required_digest(&row, "payload_digest")?,
        target_kind,
        state: required_text(&row, "state")?,
        outcome: optional_text(&row, "outcome")?,
        outcome_code: optional_text(&row, "outcome_code")?,
        before_digest: optional_digest(&row, "before_digest")?,
        after_digest: optional_digest(&row, "after_digest")?,
        owner_receipt_digest: optional_digest(&row, "owner_receipt_digest")?,
    })
}

fn repair_ticket(
    command: &IndexDriftRepairCommand,
    payload_digest: String,
) -> Result<IndexDriftRepairTicket, IndexDriftRepairFailure> {
    IndexDriftRepairTicket::new(
        command.tenant_id(),
        command.command_id(),
        command.finding_id(),
        payload_digest,
    )
    .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))
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

fn target_kind_text(kind: IndexDriftRepairTargetKind) -> &'static str {
    match kind {
        IndexDriftRepairTargetKind::MissingEntity => "missing_entity",
        IndexDriftRepairTargetKind::OrphanLink => "orphan_link",
    }
}

fn missing_evidence_digest(target: &IndexDriftRepairTarget, state: &[u8]) -> String {
    let IndexDriftRepairTarget::MissingEntity {
        key,
        indexed_source_version,
        absence_source_version,
    } = target
    else {
        return String::new();
    };
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, MISSING_EVIDENCE_DOMAIN);
    hash_component(&mut hasher, state);
    hash_entity_key(&mut hasher, key);
    hash_component(&mut hasher, &indexed_source_version.to_be_bytes());
    hash_component(&mut hasher, &absence_source_version.to_be_bytes());
    hex::encode(hasher.finalize())
}

fn orphan_evidence_digest(target: &IndexDriftRepairTarget, state: &[u8]) -> String {
    let IndexDriftRepairTarget::OrphanLink {
        source_key,
        indexed_source_version,
        link_name,
        ordinal,
        target,
        target_absence_source_version,
    } = target
    else {
        return String::new();
    };
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, ORPHAN_EVIDENCE_DOMAIN);
    hash_component(&mut hasher, state);
    hash_entity_key(&mut hasher, source_key);
    hash_component(&mut hasher, &indexed_source_version.to_be_bytes());
    hash_component(&mut hasher, link_name.as_str().as_bytes());
    hash_component(&mut hasher, &ordinal.to_be_bytes());
    hash_linked_key(&mut hasher, target);
    hash_component(&mut hasher, &target_absence_source_version.to_be_bytes());
    hex::encode(hasher.finalize())
}

fn orphan_identity_digest(target: &IndexDriftRepairTarget) -> String {
    let IndexDriftRepairTarget::OrphanLink {
        link_name,
        ordinal,
        target,
        target_absence_source_version,
        ..
    } = target
    else {
        return String::new();
    };
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, ORPHAN_IDENTITY_DOMAIN);
    hash_component(&mut hasher, link_name.as_str().as_bytes());
    hash_component(&mut hasher, &ordinal.to_be_bytes());
    hash_linked_key(&mut hasher, target);
    hash_component(&mut hasher, &target_absence_source_version.to_be_bytes());
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

fn derive_finding_key(tenant_id: Uuid, check_name: &str, key: &EntityKey) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, FINDING_KEY_DOMAIN);
    hash_component(&mut hasher, tenant_id.as_bytes());
    hash_component(&mut hasher, check_name.as_bytes());
    hash_component(&mut hasher, b"entity");
    hash_schema(&mut hasher, &key.schema);
    hash_component(&mut hasher, key.entity_id.as_bytes());
    match &key.locale {
        Some(locale) => hash_component(&mut hasher, locale.as_str().as_bytes()),
        None => hash_component(&mut hasher, NO_LOCALE_COMPONENT),
    }
    hex::encode(hasher.finalize())
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
    let length = u64::try_from(value.len()).expect("bounded repair digest component length");
    hasher.update(length.to_be_bytes());
    hasher.update(value);
}

fn required_text(row: &QueryResult, column: &str) -> Result<String, IndexDriftRepairFailure> {
    let value = row
        .try_get::<String>("", column)
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?;
    if value.is_empty() {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    Ok(value)
}

fn optional_text(
    row: &QueryResult,
    column: &str,
) -> Result<Option<String>, IndexDriftRepairFailure> {
    row.try_get::<Option<String>>("", column)
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))
}

fn required_digest(row: &QueryResult, column: &str) -> Result<String, IndexDriftRepairFailure> {
    let value = required_text(row, column)?;
    if !valid_digest(&value) {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    Ok(value)
}

fn optional_digest(
    row: &QueryResult,
    column: &str,
) -> Result<Option<String>, IndexDriftRepairFailure> {
    let value = optional_text(row, column)?;
    if value.as_deref().is_some_and(|value| !valid_digest(value)) {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    Ok(value)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn ensure_postgres(db: &DatabaseConnection) -> Result<(), IndexDriftRepairFailure> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(permanent_failure(UNSUPPORTED_BACKEND));
    }
    Ok(())
}

fn retryable_failure(code: &str) -> IndexDriftRepairFailure {
    IndexDriftRepairFailure::retryable(code).expect("static repair retryable code is valid")
}

fn permanent_failure(code: &str) -> IndexDriftRepairFailure {
    IndexDriftRepairFailure::permanent(code).expect("static repair permanent code is valid")
}
