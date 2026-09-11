//! Separately signed operations-tool release, protocol matrix, and maintenance operations.
//!
//! Enforces:
//! - Separately signed Ed25519 operations-tool releases containing controller, reconciler,
//!   and node-agent digests plus external protocol revision.
//! - Fleet-level exclusion fence preventing concurrent platform mutations or rollouts.
//! - Exact host/component desired/observed assignments.
//! - Protocol matrix compatibility check between owner and tools.
//! - Idempotent supervisor reports from host executors.
//! - Exactly one predecessor recovery authorization (`recovery_attempts <= 1`).

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ModuleCommandContext,
    conflict_fences::ConflictKey,
    data::{placeholder, uuid_value},
    installation::{sha256_digest, valid_digest},
    promotion::digest_json,
};

pub const OPERATIONS_TOOL_RELEASE_CONTRACT: &str = "rustok.operations_tool_release";
pub const CURRENT_OPERATIONS_TOOL_PROTOCOL: u32 = 1;
const MAX_MAINTENANCE_HOSTS: usize = 1024;
const MAX_HOST_ID_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationsToolComponent {
    Controller,
    Reconciler,
    Agent,
}

/// Terminal observation a separately authenticated host supervisor can report
/// for one exact desired component assignment.
///
/// This is deliberately distinct from an operator command context: the
/// supervisor's transport must authenticate the reporting host, while the
/// report itself proves only the fenced desired/observed assignment outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationsToolSupervisorObservationStatus {
    Converged,
    Failed,
}

impl OperationsToolSupervisorObservationStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Converged => "converged",
            Self::Failed => "failed",
        }
    }
}

impl OperationsToolComponent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Controller => "controller",
            Self::Reconciler => "reconciler",
            Self::Agent => "agent",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "controller" => Some(Self::Controller),
            "reconciler" => Some(Self::Reconciler),
            "agent" => Some(Self::Agent),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationsToolReleasePayload {
    pub contract: String,
    pub release_id: Uuid,
    pub version: String,
    pub protocol_revision: u32,
    pub package_digest: String,
    pub controller_digest: String,
    pub reconciler_digest: String,
    pub agent_digest: String,
    pub signer_key_digest: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationsToolRelease {
    pub payload: OperationsToolReleasePayload,
    pub signature: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedOperationsToolRelease {
    release: OperationsToolRelease,
}

impl VerifiedOperationsToolRelease {
    pub fn release(&self) -> &OperationsToolRelease {
        &self.release
    }

    pub fn payload(&self) -> &OperationsToolReleasePayload {
        &self.release.payload
    }

    pub fn into_release(self) -> OperationsToolRelease {
        self.release
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum OperationsToolError {
    #[error("Database error: {0}")]
    Storage(String),
    #[error("Invalid digest `{0}`")]
    InvalidDigest(String),
    #[error("Release signature is invalid or rejected")]
    SignatureRejected,
    #[error("Invalid public key")]
    InvalidPublicKey,
    #[error("Release is not yet valid")]
    NotYetValid,
    #[error("Release has expired")]
    Expired,
    #[error("Operations-tool release `{0}` not found")]
    NotFound(Uuid),
    #[error(
        "Protocol mismatch: owner requires {owner_protocol}, but tool supplies {tool_protocol}"
    )]
    ProtocolIncompatible {
        owner_protocol: u32,
        tool_protocol: u32,
    },
    #[error("Predecessor recovery already exhausted for operation `{0}` (max 1 attempt)")]
    RecoveryExhausted(Uuid),
    #[error("Operation `{0}` has no predecessor release to recover to")]
    NoPredecessor(Uuid),
    #[error("Operation `{0}` not found")]
    OperationNotFound(Uuid),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Fleet conflict fence `{0:?}` is currently held")]
    Conflict(ConflictKey),
    #[error("Operations-tool maintenance command is invalid")]
    InvalidCommand,
    #[error("Operations-tool maintenance idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("Maintenance operation `{0}` already represents a different command")]
    OperationConflict(Uuid),
    #[error("Maintenance operation `{operation_id}` cannot recover from status `{status}`")]
    OperationNotRecoverable { operation_id: Uuid, status: String },
    #[error("Operations-tool supervisor report is invalid")]
    InvalidSupervisorReport,
    #[error(
        "Operations-tool assignment `{operation_id}` / `{host_id}` / `{component:?}` was not found"
    )]
    AssignmentNotFound {
        operation_id: Uuid,
        host_id: String,
        component: OperationsToolComponent,
    },
    #[error("Operations-tool supervisor report is stale for the current desired assignment")]
    SupervisorReportStale,
    #[error("Operations-tool supervisor report conflicts with a terminal or prior observation")]
    SupervisorReportConflict,
    #[error("A converged supervisor report must observe the exact desired digest")]
    SupervisorReportDigestMismatch,
}

fn decode_fixed<const N: usize>(encoded: &str) -> Result<[u8; N], OperationsToolError> {
    let bytes = STANDARD
        .decode(encoded.trim())
        .map_err(|_| OperationsToolError::InvalidPublicKey)?;
    if bytes.len() != N {
        return Err(OperationsToolError::InvalidPublicKey);
    }
    let mut fixed = [0u8; N];
    fixed.copy_from_slice(&bytes);
    Ok(fixed)
}

impl OperationsToolRelease {
    pub fn verify(
        &self,
        public_key_base64: &str,
        now: DateTime<Utc>,
    ) -> Result<VerifiedOperationsToolRelease, OperationsToolError> {
        if self.payload.contract != OPERATIONS_TOOL_RELEASE_CONTRACT {
            return Err(OperationsToolError::SignatureRejected);
        }
        if !valid_digest(&self.payload.package_digest)
            || !valid_digest(&self.payload.controller_digest)
            || !valid_digest(&self.payload.reconciler_digest)
            || !valid_digest(&self.payload.agent_digest)
        {
            return Err(OperationsToolError::InvalidDigest(
                "digest validation failed".to_string(),
            ));
        }
        if self.payload.issued_at > now {
            return Err(OperationsToolError::NotYetValid);
        }
        if self.payload.expires_at <= now {
            return Err(OperationsToolError::Expired);
        }

        let public_key = decode_fixed::<32>(public_key_base64)?;
        if sha256_digest(&public_key) != self.payload.signer_key_digest {
            return Err(OperationsToolError::InvalidPublicKey);
        }

        let verifying_key = VerifyingKey::from_bytes(&public_key)
            .map_err(|_| OperationsToolError::InvalidPublicKey)?;
        let signature_bytes = decode_fixed::<64>(&self.signature)
            .map_err(|_| OperationsToolError::SignatureRejected)?;
        let signature = Signature::from_bytes(&signature_bytes);

        let canonical_bytes = rustok_api::manifest_hash::canonical_json_bytes(&self.payload)
            .map_err(|e| OperationsToolError::Serialization(e.to_string()))?;

        verifying_key
            .verify_strict(&canonical_bytes, &signature)
            .map_err(|_| OperationsToolError::SignatureRejected)?;

        Ok(VerifiedOperationsToolRelease {
            release: self.clone(),
        })
    }
}

/// Protocol compatibility matrix between owner control plane and operations tools.
#[derive(Clone, Debug)]
pub struct OperationsToolProtocolMatrix {
    pub supported_protocols: Vec<u32>,
}

impl Default for OperationsToolProtocolMatrix {
    fn default() -> Self {
        Self {
            supported_protocols: vec![CURRENT_OPERATIONS_TOOL_PROTOCOL],
        }
    }
}

impl OperationsToolProtocolMatrix {
    pub fn is_compatible(&self, owner_protocol: u32, tool_protocol: u32) -> bool {
        self.supported_protocols.contains(&tool_protocol) && owner_protocol == tool_protocol
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationsToolMaintenanceOperation {
    pub operation_id: Uuid,
    pub target_release_id: Uuid,
    pub predecessor_release_id: Option<Uuid>,
    pub status: String,
    pub recovery_attempts: u32,
    pub context: ModuleCommandContext,
    pub recovery_context: Option<ModuleCommandContext>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationsToolAssignment {
    pub assignment_id: Uuid,
    pub operation_id: Uuid,
    pub host_id: String,
    pub component: OperationsToolComponent,
    pub desired_digest: String,
    pub observed_digest: Option<String>,
    pub status: String,
    pub reported_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartOperationsToolMaintenanceCommand {
    pub operation_id: Uuid,
    pub target_release_id: Uuid,
    pub predecessor_release_id: Option<Uuid>,
    pub host_ids: Vec<String>,
    pub context: ModuleCommandContext,
}

/// Operator command that authorizes the one bounded predecessor recovery attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizeOperationsToolPredecessorRecoveryCommand {
    pub operation_id: Uuid,
    pub context: ModuleCommandContext,
}

/// Narrow executor evidence for one host/component assignment. A supervisor
/// must echo the owner-issued desired digest so a late report from a prior
/// maintenance or recovery generation cannot mutate the current assignment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationsToolSupervisorReport {
    pub operation_id: Uuid,
    pub host_id: String,
    pub component: OperationsToolComponent,
    pub expected_desired_digest: String,
    pub observed_digest: String,
    pub status: OperationsToolSupervisorObservationStatus,
}

#[derive(Clone)]
pub struct OperationsToolService {
    db: DatabaseConnection,
    public_key_base64: String,
    protocol_matrix: OperationsToolProtocolMatrix,
}

impl OperationsToolService {
    pub fn new(db: DatabaseConnection, public_key_base64: String) -> Self {
        Self {
            db,
            public_key_base64,
            protocol_matrix: OperationsToolProtocolMatrix::default(),
        }
    }

    /// Publishes and verifies a separately signed operations-tool release.
    pub async fn publish_release(
        &self,
        release: OperationsToolRelease,
        now: DateTime<Utc>,
    ) -> Result<VerifiedOperationsToolRelease, OperationsToolError> {
        let verified = release.verify(&self.public_key_base64, now)?;
        let payload = verified.payload();
        let backend = self.db.get_database_backend();

        let insert_sql = format!(
            "INSERT INTO module_operations_tool_releases (\
                release_id, version, protocol_revision, package_digest, controller_digest, \
                reconciler_digest, agent_digest, signer_key_digest, signature, issued_at, \
                expires_at, created_at\
             ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})\
             ON CONFLICT (release_id) DO NOTHING",
            placeholder(backend, 1),
            placeholder(backend, 2),
            placeholder(backend, 3),
            placeholder(backend, 4),
            placeholder(backend, 5),
            placeholder(backend, 6),
            placeholder(backend, 7),
            placeholder(backend, 8),
            placeholder(backend, 9),
            placeholder(backend, 10),
            placeholder(backend, 11),
            placeholder(backend, 12),
        );

        let values = vec![
            uuid_value(payload.release_id, backend),
            payload.version.clone().into(),
            (payload.protocol_revision as i32).into(),
            payload.package_digest.clone().into(),
            payload.controller_digest.clone().into(),
            payload.reconciler_digest.clone().into(),
            payload.agent_digest.clone().into(),
            payload.signer_key_digest.clone().into(),
            release.signature.clone().into(),
            payload.issued_at.to_rfc3339().into(),
            payload.expires_at.to_rfc3339().into(),
            now.to_rfc3339().into(),
        ];

        self.db
            .execute_raw(Statement::from_sql_and_values(backend, insert_sql, values))
            .await
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?;

        Ok(verified)
    }

    /// Preflight verification for an operations-tool release against protocol and signature constraints.
    pub async fn verify_preflight(
        &self,
        target_release_id: Uuid,
        current_protocol: u32,
        now: DateTime<Utc>,
    ) -> Result<VerifiedOperationsToolRelease, OperationsToolError> {
        self.verify_preflight_in(&self.db, target_release_id, current_protocol, now)
            .await
    }

    async fn verify_preflight_in<C: ConnectionTrait>(
        &self,
        connection: &C,
        target_release_id: Uuid,
        current_protocol: u32,
        now: DateTime<Utc>,
    ) -> Result<VerifiedOperationsToolRelease, OperationsToolError> {
        let backend = connection.get_database_backend();
        let query_sql = format!(
            "SELECT release_id, version, protocol_revision, package_digest, controller_digest, \
                    reconciler_digest, agent_digest, signer_key_digest, signature, issued_at, expires_at \
             FROM module_operations_tool_releases WHERE release_id = {}",
            placeholder(backend, 1)
        );

        let row = connection
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                query_sql,
                vec![uuid_value(target_release_id, backend)],
            ))
            .await
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?
            .ok_or(OperationsToolError::NotFound(target_release_id))?;

        let release = self.parse_release_row(row, backend)?;
        if !self
            .protocol_matrix
            .is_compatible(current_protocol, release.payload.protocol_revision)
        {
            return Err(OperationsToolError::ProtocolIncompatible {
                owner_protocol: current_protocol,
                tool_protocol: release.payload.protocol_revision,
            });
        }

        release.verify(&self.public_key_base64, now)
    }

    /// Starts an `operations_tool_maintenance` operation in the canonical ledger,
    /// acquiring the fleet-level exclusion fence and generating host component assignments.
    pub async fn start_maintenance(
        &self,
        command: StartOperationsToolMaintenanceCommand,
        now: DateTime<Utc>,
    ) -> Result<OperationsToolMaintenanceOperation, OperationsToolError> {
        validate_start_maintenance_command(&command)?;
        let request_digest = digest_json(&command)
            .map_err(|error| OperationsToolError::Serialization(error.to_string()))?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        if let Some(replay) = self
            .load_start_replay(&transaction, &command, &request_digest)
            .await?
        {
            transaction
                .commit()
                .await
                .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
            return Ok(replay);
        }
        if self
            .find_active_maintenance_operation(&transaction)
            .await?
            .is_some()
        {
            return Err(OperationsToolError::Conflict(
                ConflictKey::fleet_operations_tool(),
            ));
        }

        let target_release = self
            .verify_preflight_in(
                &transaction,
                command.target_release_id,
                CURRENT_OPERATIONS_TOOL_PROTOCOL,
                now,
            )
            .await?;
        if let Some(predecessor_release_id) = command.predecessor_release_id {
            self.verify_preflight_in(
                &transaction,
                predecessor_release_id,
                CURRENT_OPERATIONS_TOOL_PROTOCOL,
                now,
            )
            .await?;
        }

        let backend = transaction.get_database_backend();
        let op_sql = format!(
            "INSERT INTO module_operations_tool_maintenance_operations (\
                operation_id, target_release_id, predecessor_release_id, status, recovery_attempts, \
                request_digest, actor_id, idempotency_key, trace_id, correlation_id, created_at, updated_at\
             ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})\
             ON CONFLICT DO NOTHING",
            placeholder(backend, 1),
            placeholder(backend, 2),
            placeholder(backend, 3),
            placeholder(backend, 4),
            placeholder(backend, 5),
            placeholder(backend, 6),
            placeholder(backend, 7),
            placeholder(backend, 8),
            placeholder(backend, 9),
            placeholder(backend, 10),
            placeholder(backend, 11),
            placeholder(backend, 12),
        );
        let inserted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                op_sql,
                vec![
                    uuid_value(command.operation_id, backend),
                    uuid_value(command.target_release_id, backend),
                    optional_uuid_value(command.predecessor_release_id, backend),
                    "in_progress".into(),
                    0i32.into(),
                    request_digest.clone().into(),
                    uuid_value(command.context.actor_id, backend),
                    uuid_value(command.context.idempotency_key, backend),
                    command.context.trace_id.clone().into(),
                    uuid_value(command.context.correlation_id, backend),
                    now.to_rfc3339().into(),
                    now.to_rfc3339().into(),
                ],
            ))
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        if inserted.rows_affected() != 1 {
            if let Some(replay) = self
                .load_start_replay(&transaction, &command, &request_digest)
                .await?
            {
                transaction
                    .commit()
                    .await
                    .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
                return Ok(replay);
            }
            if self
                .find_operation_by_id(&transaction, command.operation_id)
                .await?
                .is_some()
            {
                return Err(OperationsToolError::OperationConflict(command.operation_id));
            }
            if self
                .find_active_maintenance_operation(&transaction)
                .await?
                .is_some()
            {
                return Err(OperationsToolError::Conflict(
                    ConflictKey::fleet_operations_tool(),
                ));
            }
            return Err(OperationsToolError::Storage(
                "maintenance operation reservation disappeared during insert".to_string(),
            ));
        }

        let assignment_sql = format!(
            "INSERT INTO module_operations_tool_assignments (\
                assignment_id, operation_id, host_id, component, desired_digest, status, updated_at\
             ) VALUES ({}, {}, {}, {}, {}, {}, {})",
            placeholder(backend, 1),
            placeholder(backend, 2),
            placeholder(backend, 3),
            placeholder(backend, 4),
            placeholder(backend, 5),
            placeholder(backend, 6),
            placeholder(backend, 7),
        );
        let target_payload = target_release.payload();
        for host_id in &command.host_ids {
            for (component, digest) in [
                (
                    OperationsToolComponent::Controller,
                    &target_payload.controller_digest,
                ),
                (
                    OperationsToolComponent::Reconciler,
                    &target_payload.reconciler_digest,
                ),
                (OperationsToolComponent::Agent, &target_payload.agent_digest),
            ] {
                transaction
                    .execute_raw(Statement::from_sql_and_values(
                        backend,
                        assignment_sql.clone(),
                        vec![
                            uuid_value(Uuid::new_v4(), backend),
                            uuid_value(command.operation_id, backend),
                            host_id.clone().into(),
                            component.as_str().into(),
                            digest.clone().into(),
                            "staged".into(),
                            now.to_rfc3339().into(),
                        ],
                    ))
                    .await
                    .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
            }
        }

        let operation = OperationsToolMaintenanceOperation {
            operation_id: command.operation_id,
            target_release_id: command.target_release_id,
            predecessor_release_id: command.predecessor_release_id,
            status: "in_progress".to_string(),
            recovery_attempts: 0,
            context: command.context,
            recovery_context: None,
            created_at: now,
            updated_at: now,
        };
        transaction
            .commit()
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        Ok(operation)
    }

    /// Records one authenticated supervisor observation against the exact current
    /// desired digest. A stale desired generation, a divergent claimed
    /// convergence, or a mutation after terminal completion fails closed.
    pub async fn report_supervisor_observation(
        &self,
        report: OperationsToolSupervisorReport,
        now: DateTime<Utc>,
    ) -> Result<OperationsToolAssignment, OperationsToolError> {
        validate_supervisor_report(&report)?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        let operation = self
            .find_operation_by_id_for_update(&transaction, report.operation_id)
            .await?
            .ok_or(OperationsToolError::OperationNotFound(report.operation_id))?;
        let assignment = self
            .find_assignment(
                &transaction,
                report.operation_id,
                &report.host_id,
                report.component,
            )
            .await?
            .ok_or_else(|| OperationsToolError::AssignmentNotFound {
                operation_id: report.operation_id,
                host_id: report.host_id.clone(),
                component: report.component,
            })?;

        if assignment.desired_digest != report.expected_desired_digest {
            return Err(OperationsToolError::SupervisorReportStale);
        }

        let report_status = report.status.as_str();
        let exact_replay = assignment.status == report_status
            && assignment.observed_digest.as_deref() == Some(report.observed_digest.as_str());
        if exact_replay {
            transaction
                .commit()
                .await
                .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
            return Ok(assignment);
        }

        if !active_maintenance_status(&operation.status)
            || !staged_assignment_status(&assignment.status)
        {
            return Err(OperationsToolError::SupervisorReportConflict);
        }

        let backend = transaction.get_database_backend();
        let updated = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_operations_tool_assignments \
                     SET observed_digest = {}, status = {}, reported_at = {}, updated_at = {} \
                     WHERE assignment_id = {} AND desired_digest = {} \
                       AND status = 'staged'",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                ),
                vec![
                    report.observed_digest.clone().into(),
                    report_status.into(),
                    now.to_rfc3339().into(),
                    now.to_rfc3339().into(),
                    uuid_value(assignment.assignment_id, backend),
                    report.expected_desired_digest.clone().into(),
                ],
            ))
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        if updated.rows_affected() != 1 {
            return Err(OperationsToolError::SupervisorReportConflict);
        }

        match report.status {
            OperationsToolSupervisorObservationStatus::Converged => {
                self.check_and_converge_operation_in(&transaction, report.operation_id, now)
                    .await?;
            }
            OperationsToolSupervisorObservationStatus::Failed => {
                let transitioned = transaction
                    .execute_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "UPDATE module_operations_tool_maintenance_operations \
                             SET status = 'recovery_required', updated_at = {} \
                             WHERE operation_id = {} AND status IN ('in_progress', 'rolling_back')",
                            placeholder(backend, 1),
                            placeholder(backend, 2),
                        ),
                        vec![
                            now.to_rfc3339().into(),
                            uuid_value(report.operation_id, backend),
                        ],
                    ))
                    .await
                    .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
                if transitioned.rows_affected() != 1 {
                    return Err(OperationsToolError::SupervisorReportConflict);
                }
            }
        }

        let assignment = self
            .find_assignment(
                &transaction,
                report.operation_id,
                &report.host_id,
                report.component,
            )
            .await?
            .ok_or_else(|| OperationsToolError::AssignmentNotFound {
                operation_id: report.operation_id,
                host_id: report.host_id.clone(),
                component: report.component,
            })?;
        transaction
            .commit()
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        Ok(assignment)
    }

    /// Authorizes exactly one predecessor recovery attempt for an operations-tool maintenance operation.
    pub async fn authorize_predecessor_recovery(
        &self,
        command: AuthorizeOperationsToolPredecessorRecoveryCommand,
        now: DateTime<Utc>,
    ) -> Result<OperationsToolMaintenanceOperation, OperationsToolError> {
        validate_recovery_command(&command)?;
        let request_digest = digest_json(&command)
            .map_err(|error| OperationsToolError::Serialization(error.to_string()))?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        if let Some(replay) = self
            .load_recovery_replay(&transaction, &command, &request_digest)
            .await?
        {
            transaction
                .commit()
                .await
                .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
            return Ok(replay);
        }
        let op = self
            .find_operation_by_id_for_update(&transaction, command.operation_id)
            .await?
            .ok_or(OperationsToolError::OperationNotFound(command.operation_id))?;
        if let Some(replay) = self
            .load_recovery_replay(&transaction, &command, &request_digest)
            .await?
        {
            transaction
                .commit()
                .await
                .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
            return Ok(replay);
        }
        if op.recovery_attempts >= 1 {
            return Err(OperationsToolError::RecoveryExhausted(command.operation_id));
        }
        if op.status != "recovery_required" {
            return Err(OperationsToolError::OperationNotRecoverable {
                operation_id: command.operation_id,
                status: op.status,
            });
        }

        let pred_id = op
            .predecessor_release_id
            .ok_or(OperationsToolError::NoPredecessor(command.operation_id))?;

        let pred_release = self
            .verify_preflight_in(&transaction, pred_id, CURRENT_OPERATIONS_TOOL_PROTOCOL, now)
            .await?;
        let pred_payload = pred_release.payload();

        let backend = transaction.get_database_backend();

        let update_op_sql = format!(
            "UPDATE module_operations_tool_maintenance_operations \
             SET recovery_attempts = 1, status = 'rolling_back', recovery_request_digest = {}, \
                 recovery_actor_id = {}, recovery_idempotency_key = {}, recovery_trace_id = {}, \
                 recovery_correlation_id = {}, updated_at = {} \
             WHERE operation_id = {} AND status = 'recovery_required' AND recovery_attempts = 0 \
               AND recovery_idempotency_key IS NULL",
            placeholder(backend, 1),
            placeholder(backend, 2),
            placeholder(backend, 3),
            placeholder(backend, 4),
            placeholder(backend, 5),
            placeholder(backend, 6),
            placeholder(backend, 7),
        );

        let updated = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                update_op_sql,
                vec![
                    request_digest.clone().into(),
                    uuid_value(command.context.actor_id, backend),
                    uuid_value(command.context.idempotency_key, backend),
                    command.context.trace_id.clone().into(),
                    uuid_value(command.context.correlation_id, backend),
                    now.to_rfc3339().into(),
                    uuid_value(command.operation_id, backend),
                ],
            ))
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        if updated.rows_affected() != 1 {
            if let Some(replay) = self
                .load_recovery_replay(&transaction, &command, &request_digest)
                .await?
            {
                transaction
                    .commit()
                    .await
                    .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
                return Ok(replay);
            }
            let current = self
                .find_operation_by_id(&transaction, command.operation_id)
                .await?
                .ok_or(OperationsToolError::OperationNotFound(command.operation_id))?;
            if current.recovery_attempts >= 1 {
                return Err(OperationsToolError::RecoveryExhausted(command.operation_id));
            }
            if current.predecessor_release_id.is_none() {
                return Err(OperationsToolError::NoPredecessor(command.operation_id));
            }
            return Err(OperationsToolError::OperationNotRecoverable {
                operation_id: command.operation_id,
                status: current.status,
            });
        }

        let update_assignment_sql = format!(
            "UPDATE module_operations_tool_assignments \
             SET desired_digest = {}, observed_digest = NULL, status = 'staged', \
                 reported_at = NULL, updated_at = {} \
             WHERE operation_id = {} AND component = {}",
            placeholder(backend, 1),
            placeholder(backend, 2),
            placeholder(backend, 3),
            placeholder(backend, 4),
        );
        for (comp, digest) in [
            (
                OperationsToolComponent::Controller,
                &pred_payload.controller_digest,
            ),
            (
                OperationsToolComponent::Reconciler,
                &pred_payload.reconciler_digest,
            ),
            (OperationsToolComponent::Agent, &pred_payload.agent_digest),
        ] {
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    update_assignment_sql.clone(),
                    vec![
                        digest.clone().into(),
                        now.to_rfc3339().into(),
                        uuid_value(command.operation_id, backend),
                        comp.as_str().into(),
                    ],
                ))
                .await
                .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        }

        let operation = self
            .find_operation_by_id(&transaction, command.operation_id)
            .await?
            .ok_or(OperationsToolError::OperationNotFound(command.operation_id))?;
        transaction
            .commit()
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        Ok(operation)
    }

    async fn check_and_converge_operation_in<C: ConnectionTrait>(
        &self,
        connection: &C,
        operation_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), OperationsToolError> {
        let backend = connection.get_database_backend();
        let update_sql = format!(
            "UPDATE module_operations_tool_maintenance_operations \
             SET status = CASE status \
                    WHEN 'in_progress' THEN 'converged' \
                    WHEN 'rolling_back' THEN 'rolled_back' \
                    ELSE status END, \
                 updated_at = {} \
             WHERE operation_id = {} AND status IN ('in_progress', 'rolling_back') \
               AND NOT EXISTS (\
                    SELECT 1 FROM module_operations_tool_assignments \
                    WHERE operation_id = {} AND status != 'converged'\
               )",
            placeholder(backend, 1),
            placeholder(backend, 2),
            placeholder(backend, 3),
        );
        connection
            .execute_raw(Statement::from_sql_and_values(
                backend,
                update_sql,
                vec![
                    now.to_rfc3339().into(),
                    uuid_value(operation_id, backend),
                    uuid_value(operation_id, backend),
                ],
            ))
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        Ok(())
    }

    pub async fn get_operation(
        &self,
        operation_id: Uuid,
    ) -> Result<OperationsToolMaintenanceOperation, OperationsToolError> {
        self.find_operation_by_id(&self.db, operation_id)
            .await?
            .ok_or(OperationsToolError::OperationNotFound(operation_id))
    }

    async fn find_operation_by_id<C: ConnectionTrait>(
        &self,
        connection: &C,
        operation_id: Uuid,
    ) -> Result<Option<OperationsToolMaintenanceOperation>, OperationsToolError> {
        self.find_operation_by_id_with_lock(connection, operation_id, false)
            .await
    }

    async fn find_operation_by_id_for_update<C: ConnectionTrait>(
        &self,
        connection: &C,
        operation_id: Uuid,
    ) -> Result<Option<OperationsToolMaintenanceOperation>, OperationsToolError> {
        self.find_operation_by_id_with_lock(connection, operation_id, true)
            .await
    }

    async fn find_operation_by_id_with_lock<C: ConnectionTrait>(
        &self,
        connection: &C,
        operation_id: Uuid,
        lock_for_update: bool,
    ) -> Result<Option<OperationsToolMaintenanceOperation>, OperationsToolError> {
        let backend = connection.get_database_backend();
        let lock_clause = if lock_for_update && backend == DbBackend::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        let row = connection
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT operation_id, target_release_id, predecessor_release_id, status, \
                            recovery_attempts, actor_id, idempotency_key, trace_id, correlation_id, \
                            recovery_actor_id, recovery_idempotency_key, recovery_trace_id, \
                            recovery_correlation_id, created_at, updated_at \
                     FROM module_operations_tool_maintenance_operations \
                     WHERE operation_id = {}{}",
                    placeholder(backend, 1),
                    lock_clause,
                ),
                vec![uuid_value(operation_id, backend)],
            ))
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        row.map(|row| self.parse_operation_row(row, backend))
            .transpose()
    }

    async fn find_active_maintenance_operation<C: ConnectionTrait>(
        &self,
        connection: &C,
    ) -> Result<Option<Uuid>, OperationsToolError> {
        let backend = connection.get_database_backend();
        let row = connection
            .query_one_raw(Statement::from_string(
                backend,
                "SELECT operation_id FROM module_operations_tool_maintenance_operations \
                 WHERE status IN ('in_progress', 'rolling_back', 'recovery_required') LIMIT 1"
                    .to_string(),
            ))
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        row.map(|row| self.get_uuid_from_row(&row, "operation_id", backend))
            .transpose()
    }

    async fn load_start_replay<C: ConnectionTrait>(
        &self,
        connection: &C,
        command: &StartOperationsToolMaintenanceCommand,
        request_digest: &str,
    ) -> Result<Option<OperationsToolMaintenanceOperation>, OperationsToolError> {
        let backend = connection.get_database_backend();
        let Some(row) = connection
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT request_digest, operation_id, target_release_id, predecessor_release_id, status, \
                            recovery_attempts, actor_id, idempotency_key, trace_id, correlation_id, \
                            recovery_actor_id, recovery_idempotency_key, recovery_trace_id, \
                            recovery_correlation_id, created_at, updated_at \
                     FROM module_operations_tool_maintenance_operations WHERE idempotency_key = {}",
                    placeholder(backend, 1),
                ),
                vec![uuid_value(command.context.idempotency_key, backend)],
            ))
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?
        else {
            return Ok(None);
        };
        let stored_request_digest: String = row
            .try_get("", "request_digest")
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        let operation = self.parse_operation_row(row, backend)?;
        if stored_request_digest != request_digest
            || operation.operation_id != command.operation_id
            || operation.context != command.context
        {
            return Err(OperationsToolError::IdempotencyConflict);
        }
        Ok(Some(operation))
    }

    async fn load_recovery_replay<C: ConnectionTrait>(
        &self,
        connection: &C,
        command: &AuthorizeOperationsToolPredecessorRecoveryCommand,
        request_digest: &str,
    ) -> Result<Option<OperationsToolMaintenanceOperation>, OperationsToolError> {
        let backend = connection.get_database_backend();
        let Some(row) = connection
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT recovery_request_digest, operation_id, target_release_id, \
                            predecessor_release_id, status, recovery_attempts, actor_id, \
                            idempotency_key, trace_id, correlation_id, recovery_actor_id, \
                            recovery_idempotency_key, recovery_trace_id, recovery_correlation_id, \
                            created_at, updated_at \
                     FROM module_operations_tool_maintenance_operations \
                     WHERE recovery_idempotency_key = {}",
                    placeholder(backend, 1),
                ),
                vec![uuid_value(command.context.idempotency_key, backend)],
            ))
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?
        else {
            return Ok(None);
        };
        let stored_request_digest: String = row
            .try_get("", "recovery_request_digest")
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        let operation = self.parse_operation_row(row, backend)?;
        if stored_request_digest != request_digest
            || operation.operation_id != command.operation_id
            || operation.recovery_context.as_ref() != Some(&command.context)
        {
            return Err(OperationsToolError::IdempotencyConflict);
        }
        Ok(Some(operation))
    }

    pub async fn get_assignment(
        &self,
        operation_id: Uuid,
        host_id: &str,
        component: OperationsToolComponent,
    ) -> Result<OperationsToolAssignment, OperationsToolError> {
        self.find_assignment(&self.db, operation_id, host_id, component)
            .await?
            .ok_or_else(|| OperationsToolError::AssignmentNotFound {
                operation_id,
                host_id: host_id.to_string(),
                component,
            })
    }

    async fn find_assignment<C: ConnectionTrait>(
        &self,
        connection: &C,
        operation_id: Uuid,
        host_id: &str,
        component: OperationsToolComponent,
    ) -> Result<Option<OperationsToolAssignment>, OperationsToolError> {
        let backend = connection.get_database_backend();
        let query_sql = format!(
            "SELECT assignment_id, operation_id, host_id, component, desired_digest, \
                    observed_digest, status, reported_at, updated_at \
             FROM module_operations_tool_assignments \
             WHERE operation_id = {} AND host_id = {} AND component = {}",
            placeholder(backend, 1),
            placeholder(backend, 2),
            placeholder(backend, 3),
        );

        let row = connection
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                query_sql,
                vec![
                    uuid_value(operation_id, backend),
                    host_id.into(),
                    component.as_str().into(),
                ],
            ))
            .await
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;

        row.map(|row| self.parse_assignment_row(row, backend))
            .transpose()
    }

    fn parse_assignment_row(
        &self,
        row: sea_orm::QueryResult,
        backend: DbBackend,
    ) -> Result<OperationsToolAssignment, OperationsToolError> {
        let assignment_id = self.get_uuid_from_row(&row, "assignment_id", backend)?;
        let operation_id = self.get_uuid_from_row(&row, "operation_id", backend)?;
        let host_id: String = row
            .try_get("", "host_id")
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        let component: String = row
            .try_get("", "component")
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        let component = OperationsToolComponent::parse(&component).ok_or_else(|| {
            OperationsToolError::Storage(
                "operations-tool assignment contains an invalid component".to_string(),
            )
        })?;
        let desired_digest: String = row
            .try_get("", "desired_digest")
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        let observed_digest: Option<String> = row
            .try_get("", "observed_digest")
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        let status: String = row
            .try_get("", "status")
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        let reported_at = self.get_optional_datetime_from_row(&row, "reported_at", backend)?;
        let updated_at = self.get_datetime_from_row(&row, "updated_at", backend)?;

        if !valid_host_id(&host_id)
            || !valid_digest(&desired_digest)
            || observed_digest
                .as_deref()
                .is_some_and(|digest| !valid_digest(digest))
            || !valid_assignment_status(&status)
            || !valid_assignment_observation(
                &status,
                observed_digest.as_deref(),
                reported_at.as_ref(),
            )
        {
            return Err(OperationsToolError::Storage(
                "operations-tool assignment contains invalid state".to_string(),
            ));
        }

        Ok(OperationsToolAssignment {
            assignment_id,
            operation_id,
            host_id,
            component,
            desired_digest,
            observed_digest,
            status,
            reported_at,
            updated_at,
        })
    }

    fn parse_release_row(
        &self,
        row: sea_orm::QueryResult,
        backend: DbBackend,
    ) -> Result<OperationsToolRelease, OperationsToolError> {
        let release_id: Uuid = self.get_uuid_from_row(&row, "release_id", backend)?;
        let version: String = row
            .try_get("", "version")
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let protocol_revision: i32 = row
            .try_get("", "protocol_revision")
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let package_digest: String = row
            .try_get("", "package_digest")
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let controller_digest: String = row
            .try_get("", "controller_digest")
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let reconciler_digest: String = row
            .try_get("", "reconciler_digest")
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let agent_digest: String = row
            .try_get("", "agent_digest")
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let signer_key_digest: String = row
            .try_get("", "signer_key_digest")
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let signature: String = row
            .try_get("", "signature")
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let issued_at: DateTime<Utc> = self.get_datetime_from_row(&row, "issued_at", backend)?;
        let expires_at: DateTime<Utc> = self.get_datetime_from_row(&row, "expires_at", backend)?;

        Ok(OperationsToolRelease {
            payload: OperationsToolReleasePayload {
                contract: OPERATIONS_TOOL_RELEASE_CONTRACT.to_string(),
                release_id,
                version,
                protocol_revision: protocol_revision as u32,
                package_digest,
                controller_digest,
                reconciler_digest,
                agent_digest,
                signer_key_digest,
                issued_at,
                expires_at,
            },
            signature,
        })
    }

    fn parse_operation_row(
        &self,
        row: sea_orm::QueryResult,
        backend: DbBackend,
    ) -> Result<OperationsToolMaintenanceOperation, OperationsToolError> {
        let recovery_attempts: i32 = row
            .try_get("", "recovery_attempts")
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        let recovery_attempts = u32::try_from(recovery_attempts)
            .ok()
            .filter(|value| *value <= 1)
            .ok_or_else(|| {
                OperationsToolError::Storage(
                    "maintenance operation recovery_attempts is invalid".to_string(),
                )
            })?;
        let status: String = row
            .try_get("", "status")
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        if !valid_maintenance_status(&status) {
            return Err(OperationsToolError::Storage(
                "maintenance operation contains an invalid status".to_string(),
            ));
        }
        let context = ModuleCommandContext {
            actor_id: self.get_uuid_from_row(&row, "actor_id", backend)?,
            tenant_id: None,
            trace_id: row
                .try_get("", "trace_id")
                .map_err(|error| OperationsToolError::Storage(error.to_string()))?,
            correlation_id: self.get_uuid_from_row(&row, "correlation_id", backend)?,
            idempotency_key: self.get_uuid_from_row(&row, "idempotency_key", backend)?,
        };
        validate_platform_context(&context)?;

        let recovery_actor_id =
            self.get_optional_uuid_from_row(&row, "recovery_actor_id", backend)?;
        let recovery_idempotency_key =
            self.get_optional_uuid_from_row(&row, "recovery_idempotency_key", backend)?;
        let recovery_trace_id: Option<String> = row
            .try_get("", "recovery_trace_id")
            .map_err(|error| OperationsToolError::Storage(error.to_string()))?;
        let recovery_correlation_id =
            self.get_optional_uuid_from_row(&row, "recovery_correlation_id", backend)?;
        let recovery_context = match (
            recovery_actor_id,
            recovery_trace_id,
            recovery_correlation_id,
            recovery_idempotency_key,
        ) {
            (None, None, None, None) => None,
            (Some(actor_id), Some(trace_id), Some(correlation_id), Some(idempotency_key)) => {
                let context = ModuleCommandContext {
                    actor_id,
                    tenant_id: None,
                    trace_id,
                    correlation_id,
                    idempotency_key,
                };
                validate_platform_context(&context)?;
                Some(context)
            }
            _ => {
                return Err(OperationsToolError::Storage(
                    "maintenance recovery evidence is incomplete".to_string(),
                ));
            }
        };
        if !valid_maintenance_recovery_state(&status, recovery_attempts, recovery_context.is_some())
        {
            return Err(OperationsToolError::Storage(
                "maintenance operation contains inconsistent recovery state".to_string(),
            ));
        }

        Ok(OperationsToolMaintenanceOperation {
            operation_id: self.get_uuid_from_row(&row, "operation_id", backend)?,
            target_release_id: self.get_uuid_from_row(&row, "target_release_id", backend)?,
            predecessor_release_id: self.get_optional_uuid_from_row(
                &row,
                "predecessor_release_id",
                backend,
            )?,
            status,
            recovery_attempts,
            context,
            recovery_context,
            created_at: self.get_datetime_from_row(&row, "created_at", backend)?,
            updated_at: self.get_datetime_from_row(&row, "updated_at", backend)?,
        })
    }

    fn get_uuid_from_row(
        &self,
        row: &sea_orm::QueryResult,
        col: &str,
        backend: DbBackend,
    ) -> Result<Uuid, OperationsToolError> {
        match backend {
            DbBackend::Sqlite => {
                let s: String = row
                    .try_get("", col)
                    .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
                Uuid::parse_str(&s).map_err(|e| OperationsToolError::Storage(e.to_string()))
            }
            _ => row
                .try_get("", col)
                .map_err(|e| OperationsToolError::Storage(e.to_string())),
        }
    }

    fn get_optional_uuid_from_row(
        &self,
        row: &sea_orm::QueryResult,
        col: &str,
        backend: DbBackend,
    ) -> Result<Option<Uuid>, OperationsToolError> {
        match backend {
            DbBackend::Sqlite => {
                let s: Option<String> = row
                    .try_get("", col)
                    .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
                match s {
                    Some(val) => Uuid::parse_str(&val)
                        .map(Some)
                        .map_err(|e| OperationsToolError::Storage(e.to_string())),
                    None => Ok(None),
                }
            }
            _ => row
                .try_get("", col)
                .map_err(|e| OperationsToolError::Storage(e.to_string())),
        }
    }

    fn get_datetime_from_row(
        &self,
        row: &sea_orm::QueryResult,
        col: &str,
        backend: DbBackend,
    ) -> Result<DateTime<Utc>, OperationsToolError> {
        match backend {
            DbBackend::Sqlite => {
                let s: String = row
                    .try_get("", col)
                    .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| OperationsToolError::Storage(e.to_string()))
            }
            _ => row
                .try_get("", col)
                .map_err(|e| OperationsToolError::Storage(e.to_string())),
        }
    }

    fn get_optional_datetime_from_row(
        &self,
        row: &sea_orm::QueryResult,
        col: &str,
        backend: DbBackend,
    ) -> Result<Option<DateTime<Utc>>, OperationsToolError> {
        match backend {
            DbBackend::Sqlite => {
                let s: Option<String> = row
                    .try_get("", col)
                    .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
                match s {
                    Some(val) => DateTime::parse_from_rfc3339(&val)
                        .map(|dt| Some(dt.with_timezone(&Utc)))
                        .map_err(|e| OperationsToolError::Storage(e.to_string())),
                    None => Ok(None),
                }
            }
            _ => row
                .try_get("", col)
                .map_err(|e| OperationsToolError::Storage(e.to_string())),
        }
    }
}

fn validate_start_maintenance_command(
    command: &StartOperationsToolMaintenanceCommand,
) -> Result<(), OperationsToolError> {
    if command.operation_id.is_nil()
        || command.target_release_id.is_nil()
        || command
            .predecessor_release_id
            .is_some_and(|release_id| release_id.is_nil())
        || command.predecessor_release_id == Some(command.target_release_id)
        || command.host_ids.is_empty()
        || command.host_ids.len() > MAX_MAINTENANCE_HOSTS
        || !valid_platform_context(&command.context)
    {
        return Err(OperationsToolError::InvalidCommand);
    }

    let mut host_ids = BTreeSet::new();
    if command
        .host_ids
        .iter()
        .any(|host_id| !valid_host_id(host_id) || !host_ids.insert(host_id.as_str()))
    {
        return Err(OperationsToolError::InvalidCommand);
    }
    Ok(())
}

fn validate_recovery_command(
    command: &AuthorizeOperationsToolPredecessorRecoveryCommand,
) -> Result<(), OperationsToolError> {
    if command.operation_id.is_nil() || !valid_platform_context(&command.context) {
        return Err(OperationsToolError::InvalidCommand);
    }
    Ok(())
}

fn validate_supervisor_report(
    report: &OperationsToolSupervisorReport,
) -> Result<(), OperationsToolError> {
    if report.operation_id.is_nil()
        || !valid_host_id(&report.host_id)
        || !valid_digest(&report.expected_desired_digest)
        || !valid_digest(&report.observed_digest)
    {
        return Err(OperationsToolError::InvalidSupervisorReport);
    }
    if report.status == OperationsToolSupervisorObservationStatus::Converged
        && report.expected_desired_digest != report.observed_digest
    {
        return Err(OperationsToolError::SupervisorReportDigestMismatch);
    }
    Ok(())
}

fn valid_platform_context(context: &ModuleCommandContext) -> bool {
    context.tenant_id.is_none() && context.validate().is_ok()
}

fn validate_platform_context(context: &ModuleCommandContext) -> Result<(), OperationsToolError> {
    if valid_platform_context(context) {
        Ok(())
    } else {
        Err(OperationsToolError::Storage(
            "maintenance operation contains an invalid platform command context".to_string(),
        ))
    }
}

fn valid_host_id(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= MAX_HOST_ID_BYTES
        && !value.chars().any(char::is_control)
}

fn active_maintenance_status(status: &str) -> bool {
    matches!(status, "in_progress" | "rolling_back")
}

fn valid_maintenance_status(status: &str) -> bool {
    matches!(
        status,
        "in_progress" | "rolling_back" | "recovery_required" | "converged" | "rolled_back"
    )
}

fn valid_maintenance_recovery_state(
    status: &str,
    recovery_attempts: u32,
    has_recovery_context: bool,
) -> bool {
    matches!(
        (recovery_attempts, has_recovery_context, status),
        (0, false, "in_progress" | "converged" | "recovery_required")
            | (
                1,
                true,
                "rolling_back" | "rolled_back" | "recovery_required"
            )
    )
}

fn staged_assignment_status(status: &str) -> bool {
    status == "staged"
}

fn valid_assignment_status(status: &str) -> bool {
    matches!(status, "staged" | "converged" | "failed")
}

fn valid_assignment_observation(
    status: &str,
    observed_digest: Option<&str>,
    reported_at: Option<&DateTime<Utc>>,
) -> bool {
    match status {
        "staged" => observed_digest.is_none() && reported_at.is_none(),
        "converged" | "failed" => observed_digest.is_some() && reported_at.is_some(),
        _ => false,
    }
}

fn optional_uuid_value(value: Option<Uuid>, backend: DbBackend) -> Value {
    match backend {
        DbBackend::Postgres => Value::Uuid(value),
        _ => Value::String(value.map(|value| value.to_string())),
    }
}
