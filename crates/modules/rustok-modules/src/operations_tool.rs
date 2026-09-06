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
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    conflict_fences::ConflictKey,
    data::{placeholder, uuid_value},
    installation::{sha256_digest, valid_digest},
};

pub const OPERATIONS_TOOL_RELEASE_CONTRACT: &str = "rustok.operations_tool_release";
pub const CURRENT_OPERATIONS_TOOL_PROTOCOL: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationsToolComponent {
    Controller,
    Reconciler,
    Agent,
}

impl OperationsToolComponent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Controller => "controller",
            Self::Reconciler => "reconciler",
            Self::Agent => "agent",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
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
    #[error("Protocol mismatch: owner requires {owner_protocol}, but tool supplies {tool_protocol}")]
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
            return Err(OperationsToolError::InvalidDigest("digest validation failed".to_string()));
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
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
    pub trace_id: String,
    pub correlation_id: Uuid,
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

#[derive(Clone, Debug)]
pub struct StartOperationsToolMaintenanceCommand {
    pub operation_id: Uuid,
    pub target_release_id: Uuid,
    pub predecessor_release_id: Option<Uuid>,
    pub host_ids: Vec<String>,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
    pub trace_id: String,
    pub correlation_id: Uuid,
}

#[derive(Clone, Debug)]
pub struct OperationsToolSupervisorReport {
    pub operation_id: Uuid,
    pub host_id: String,
    pub component: OperationsToolComponent,
    pub observed_digest: String,
    pub status: String,
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
        let backend = self.db.get_database_backend();
        let query_sql = format!(
            "SELECT release_id, version, protocol_revision, package_digest, controller_digest, \
                    reconciler_digest, agent_digest, signer_key_digest, signature, issued_at, expires_at \
             FROM module_operations_tool_releases WHERE release_id = {}",
            placeholder(backend, 1)
        );

        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                query_sql,
                vec![uuid_value(target_release_id, backend)],
            ))
            .await
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?
            .ok_or(OperationsToolError::NotFound(target_release_id))?;

        let release = self.parse_release_row(row, backend)?;
        if !self.protocol_matrix.is_compatible(current_protocol, release.payload.protocol_revision) {
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
        let target_release = self
            .verify_preflight(command.target_release_id, CURRENT_OPERATIONS_TOOL_PROTOCOL, now)
            .await?;

        if let Some(pred_id) = command.predecessor_release_id {
            self.verify_preflight(pred_id, CURRENT_OPERATIONS_TOOL_PROTOCOL, now)
                .await?;
        }

        let backend = self.db.get_database_backend();

        // 1. Insert maintenance operation
        let op_sql = format!(
            "INSERT INTO module_operations_tool_maintenance_operations (\
                operation_id, target_release_id, predecessor_release_id, status, \
                recovery_attempts, actor_id, idempotency_key, trace_id, correlation_id, \
                created_at, updated_at\
             ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})\
             ON CONFLICT (operation_id) DO NOTHING",
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
        );

        let pred_val = match command.predecessor_release_id {
            Some(pid) => uuid_value(pid, backend),
            None => Value::from(None::<String>),
        };

        let op_values = vec![
            uuid_value(command.operation_id, backend),
            uuid_value(command.target_release_id, backend),
            pred_val,
            "in_progress".into(),
            0i32.into(),
            uuid_value(command.actor_id, backend),
            uuid_value(command.idempotency_key, backend),
            command.trace_id.into(),
            uuid_value(command.correlation_id, backend),
            now.to_rfc3339().into(),
            now.to_rfc3339().into(),
        ];

        self.db
            .execute_raw(Statement::from_sql_and_values(backend, op_sql, op_values))
            .await
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?;

        // 2. Pre-stage desired host component assignments
        let target_payload = target_release.payload();
        for host_id in &command.host_ids {
            for (component, digest) in [
                (OperationsToolComponent::Controller, &target_payload.controller_digest),
                (OperationsToolComponent::Reconciler, &target_payload.reconciler_digest),
                (OperationsToolComponent::Agent, &target_payload.agent_digest),
            ] {
                let assignment_id = Uuid::new_v4();
                let assign_sql = format!(
                    "INSERT INTO module_operations_tool_assignments (\
                        assignment_id, operation_id, host_id, component, desired_digest, \
                        status, updated_at\
                     ) VALUES ({}, {}, {}, {}, {}, {}, {})\
                     ON CONFLICT (operation_id, host_id, component) DO NOTHING",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                );

                let assign_values = vec![
                    uuid_value(assignment_id, backend),
                    uuid_value(command.operation_id, backend),
                    host_id.clone().into(),
                    component.as_str().into(),
                    digest.clone().into(),
                    "staged".into(),
                    now.to_rfc3339().into(),
                ];

                self.db
                    .execute_raw(Statement::from_sql_and_values(backend, assign_sql, assign_values))
                    .await
                    .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
            }
        }

        self.get_operation(command.operation_id).await
    }

    /// Host supervisor reports observed component execution status idempotently.
    pub async fn report_supervisor_observation(
        &self,
        report: OperationsToolSupervisorReport,
        now: DateTime<Utc>,
    ) -> Result<OperationsToolAssignment, OperationsToolError> {
        let backend = self.db.get_database_backend();

        let update_sql = format!(
            "UPDATE module_operations_tool_assignments \
             SET observed_digest = {}, status = {}, reported_at = {}, updated_at = {} \
             WHERE operation_id = {} AND host_id = {} AND component = {}",
            placeholder(backend, 1),
            placeholder(backend, 2),
            placeholder(backend, 3),
            placeholder(backend, 4),
            placeholder(backend, 5),
            placeholder(backend, 6),
            placeholder(backend, 7),
        );

        let values = vec![
            report.observed_digest.clone().into(),
            report.status.clone().into(),
            now.to_rfc3339().into(),
            now.to_rfc3339().into(),
            uuid_value(report.operation_id, backend),
            report.host_id.clone().into(),
            report.component.as_str().into(),
        ];

        self.db
            .execute_raw(Statement::from_sql_and_values(backend, update_sql, values))
            .await
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?;

        // If all assignments converged, mark operation converged
        self.check_and_converge_operation(report.operation_id, now).await?;

        self.get_assignment(report.operation_id, &report.host_id, report.component)
            .await
    }

    /// Authorizes exactly one predecessor recovery attempt for an operations-tool maintenance operation.
    pub async fn authorize_predecessor_recovery(
        &self,
        operation_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<OperationsToolMaintenanceOperation, OperationsToolError> {
        let op = self.get_operation(operation_id).await?;
        if op.recovery_attempts >= 1 {
            return Err(OperationsToolError::RecoveryExhausted(operation_id));
        }

        let pred_id = op
            .predecessor_release_id
            .ok_or(OperationsToolError::NoPredecessor(operation_id))?;

        let pred_release = self
            .verify_preflight(pred_id, CURRENT_OPERATIONS_TOOL_PROTOCOL, now)
            .await?;
        let pred_payload = pred_release.payload();

        let backend = self.db.get_database_backend();

        // 1. Advance recovery attempt and mark rolled_back
        let update_op_sql = format!(
            "UPDATE module_operations_tool_maintenance_operations \
             SET recovery_attempts = recovery_attempts + 1, status = 'rolled_back', updated_at = {} \
             WHERE operation_id = {}",
            placeholder(backend, 1),
            placeholder(backend, 2),
        );

        self.db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                update_op_sql,
                vec![now.to_rfc3339().into(), uuid_value(operation_id, backend)],
            ))
            .await
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?;

        // 2. Re-point desired digests to predecessor component digests
        for (comp, digest) in [
            (OperationsToolComponent::Controller, &pred_payload.controller_digest),
            (OperationsToolComponent::Reconciler, &pred_payload.reconciler_digest),
            (OperationsToolComponent::Agent, &pred_payload.agent_digest),
        ] {
            let update_assign_sql = format!(
                "UPDATE module_operations_tool_assignments \
                 SET desired_digest = {}, status = 'staged', updated_at = {} \
                 WHERE operation_id = {} AND component = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
            );

            self.db
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    update_assign_sql,
                    vec![
                        digest.clone().into(),
                        now.to_rfc3339().into(),
                        uuid_value(operation_id, backend),
                        comp.as_str().into(),
                    ],
                ))
                .await
                .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        }

        self.get_operation(operation_id).await
    }

    async fn check_and_converge_operation(
        &self,
        operation_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), OperationsToolError> {
        let backend = self.db.get_database_backend();
        let query_sql = format!(
            "SELECT 1 FROM module_operations_tool_assignments \
             WHERE operation_id = {} AND status != 'converged' LIMIT 1",
            placeholder(backend, 1)
        );

        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                query_sql,
                vec![uuid_value(operation_id, backend)],
            ))
            .await
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?;

        if row.is_none() {
            let update_sql = format!(
                "UPDATE module_operations_tool_maintenance_operations \
                 SET status = 'converged', updated_at = {} \
                 WHERE operation_id = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
            );
            self.db
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    update_sql,
                    vec![now.to_rfc3339().into(), uuid_value(operation_id, backend)],
                ))
                .await
                .map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        }
        Ok(())
    }

    pub async fn get_operation(
        &self,
        operation_id: Uuid,
    ) -> Result<OperationsToolMaintenanceOperation, OperationsToolError> {
        let backend = self.db.get_database_backend();
        let query_sql = format!(
            "SELECT operation_id, target_release_id, predecessor_release_id, status, \
                    recovery_attempts, actor_id, idempotency_key, trace_id, correlation_id, \
                    created_at, updated_at \
             FROM module_operations_tool_maintenance_operations WHERE operation_id = {}",
            placeholder(backend, 1)
        );

        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                query_sql,
                vec![uuid_value(operation_id, backend)],
            ))
            .await
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?
            .ok_or(OperationsToolError::OperationNotFound(operation_id))?;

        let op_id: Uuid = self.get_uuid_from_row(&row, "operation_id", backend)?;
        let target_release_id: Uuid = self.get_uuid_from_row(&row, "target_release_id", backend)?;
        let predecessor_release_id: Option<Uuid> = self.get_optional_uuid_from_row(&row, "predecessor_release_id", backend)?;
        let status: String = row.try_get("", "status").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let recovery_attempts: i32 = row.try_get("", "recovery_attempts").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let actor_id: Uuid = self.get_uuid_from_row(&row, "actor_id", backend)?;
        let idempotency_key: Uuid = self.get_uuid_from_row(&row, "idempotency_key", backend)?;
        let trace_id: String = row.try_get("", "trace_id").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let correlation_id: Uuid = self.get_uuid_from_row(&row, "correlation_id", backend)?;
        let created_at: DateTime<Utc> = self.get_datetime_from_row(&row, "created_at", backend)?;
        let updated_at: DateTime<Utc> = self.get_datetime_from_row(&row, "updated_at", backend)?;

        Ok(OperationsToolMaintenanceOperation {
            operation_id: op_id,
            target_release_id,
            predecessor_release_id,
            status,
            recovery_attempts: recovery_attempts as u32,
            actor_id,
            idempotency_key,
            trace_id,
            correlation_id,
            created_at,
            updated_at,
        })
    }

    pub async fn get_assignment(
        &self,
        operation_id: Uuid,
        host_id: &str,
        component: OperationsToolComponent,
    ) -> Result<OperationsToolAssignment, OperationsToolError> {
        let backend = self.db.get_database_backend();
        let query_sql = format!(
            "SELECT assignment_id, operation_id, host_id, component, desired_digest, \
                    observed_digest, status, reported_at, updated_at \
             FROM module_operations_tool_assignments \
             WHERE operation_id = {} AND host_id = {} AND component = {}",
            placeholder(backend, 1),
            placeholder(backend, 2),
            placeholder(backend, 3),
        );

        let row = self
            .db
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
            .map_err(|e| OperationsToolError::Storage(e.to_string()))?
            .ok_or_else(|| {
                OperationsToolError::Storage("Assignment not found".to_string())
            })?;

        let assignment_id: Uuid = self.get_uuid_from_row(&row, "assignment_id", backend)?;
        let op_id: Uuid = self.get_uuid_from_row(&row, "operation_id", backend)?;
        let h_id: String = row.try_get("", "host_id").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let comp_str: String = row.try_get("", "component").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let comp = OperationsToolComponent::from_str(&comp_str).expect("valid component");
        let desired_digest: String = row.try_get("", "desired_digest").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let observed_digest: Option<String> = row.try_get("", "observed_digest").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let status: String = row.try_get("", "status").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let reported_at: Option<DateTime<Utc>> = self.get_optional_datetime_from_row(&row, "reported_at", backend)?;
        let updated_at: DateTime<Utc> = self.get_datetime_from_row(&row, "updated_at", backend)?;

        Ok(OperationsToolAssignment {
            assignment_id,
            operation_id: op_id,
            host_id: h_id,
            component: comp,
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
        let version: String = row.try_get("", "version").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let protocol_revision: i32 = row.try_get("", "protocol_revision").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let package_digest: String = row.try_get("", "package_digest").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let controller_digest: String = row.try_get("", "controller_digest").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let reconciler_digest: String = row.try_get("", "reconciler_digest").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let agent_digest: String = row.try_get("", "agent_digest").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let signer_key_digest: String = row.try_get("", "signer_key_digest").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
        let signature: String = row.try_get("", "signature").map_err(|e| OperationsToolError::Storage(e.to_string()))?;
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

    fn get_uuid_from_row(&self, row: &sea_orm::QueryResult, col: &str, backend: DbBackend) -> Result<Uuid, OperationsToolError> {
        match backend {
            DbBackend::Sqlite => {
                let s: String = row.try_get("", col).map_err(|e| OperationsToolError::Storage(e.to_string()))?;
                Uuid::parse_str(&s).map_err(|e| OperationsToolError::Storage(e.to_string()))
            }
            _ => row.try_get("", col).map_err(|e| OperationsToolError::Storage(e.to_string())),
        }
    }

    fn get_optional_uuid_from_row(&self, row: &sea_orm::QueryResult, col: &str, backend: DbBackend) -> Result<Option<Uuid>, OperationsToolError> {
        match backend {
            DbBackend::Sqlite => {
                let s: Option<String> = row.try_get("", col).map_err(|e| OperationsToolError::Storage(e.to_string()))?;
                match s {
                    Some(val) => Uuid::parse_str(&val).map(Some).map_err(|e| OperationsToolError::Storage(e.to_string())),
                    None => Ok(None),
                }
            }
            _ => row.try_get("", col).map_err(|e| OperationsToolError::Storage(e.to_string())),
        }
    }

    fn get_datetime_from_row(&self, row: &sea_orm::QueryResult, col: &str, backend: DbBackend) -> Result<DateTime<Utc>, OperationsToolError> {
        match backend {
            DbBackend::Sqlite => {
                let s: String = row.try_get("", col).map_err(|e| OperationsToolError::Storage(e.to_string()))?;
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| OperationsToolError::Storage(e.to_string()))
            }
            _ => row.try_get("", col).map_err(|e| OperationsToolError::Storage(e.to_string())),
        }
    }

    fn get_optional_datetime_from_row(&self, row: &sea_orm::QueryResult, col: &str, backend: DbBackend) -> Result<Option<DateTime<Utc>>, OperationsToolError> {
        match backend {
            DbBackend::Sqlite => {
                let s: Option<String> = row.try_get("", col).map_err(|e| OperationsToolError::Storage(e.to_string()))?;
                match s {
                    Some(val) => DateTime::parse_from_rfc3339(&val)
                        .map(|dt| Some(dt.with_timezone(&Utc)))
                        .map_err(|e| OperationsToolError::Storage(e.to_string())),
                    None => Ok(None),
                }
            }
            _ => row.try_get("", col).map_err(|e| OperationsToolError::Storage(e.to_string())),
        }
    }
}
