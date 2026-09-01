//! Durable global security enforcement for immutable artifact releases.
//!
//! Registry yanking remains a discovery/install concern. Quarantine and
//! revocation are separate owner states that block new execution without
//! mutating tenant enablement intent.

use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use rustok_events::DomainEvent;

use crate::{
    ArtifactReleaseRef, ControlPlaneInfrastructure, ModuleCommandContext,
    data::{now_expression, placeholder, uuid_value},
    promotion::{digest_json, valid_digest},
};

const MAX_POLICY_REVISION_BYTES: usize = 128;
const MAX_REASON_CODE_BYTES: usize = 128;
const MAX_REASON_DETAIL_BYTES: usize = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleArtifactSecurityStatus {
    Clear,
    Quarantined,
    Revoked,
}

impl ModuleArtifactSecurityStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Quarantined => "quarantined",
            Self::Revoked => "revoked",
        }
    }

    fn parse(value: &str) -> Result<Self, ModuleArtifactSecurityError> {
        match value {
            "clear" => Ok(Self::Clear),
            "quarantined" => Ok(Self::Quarantined),
            "revoked" => Ok(Self::Revoked),
            _ => Err(ModuleArtifactSecurityError::Store(
                "artifact security status is invalid".to_string(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleArtifactRegistryReleaseStatus {
    Unlisted,
    Active,
    Yanked,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactSecuritySnapshot {
    pub release: ArtifactReleaseRef,
    pub revision: u64,
    pub status: ModuleArtifactSecurityStatus,
    pub registry_status: ModuleArtifactRegistryReleaseStatus,
    pub policy_revision: Option<String>,
    pub reason_code: Option<String>,
    pub reason_detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactSecurityCommand {
    pub release: ArtifactReleaseRef,
    pub expected_revision: u64,
    pub policy_revision: String,
    pub reason_code: String,
    pub reason_detail: String,
    pub context: ModuleCommandContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactSecurityReceipt {
    pub snapshot: ModuleArtifactSecuritySnapshot,
    pub created: bool,
}

#[async_trait]
pub trait ModuleArtifactSecurityAuthorizer: Send + Sync {
    async fn authorize_quarantine(
        &self,
        command: &ModuleArtifactSecurityCommand,
    ) -> Result<(), ModuleArtifactSecurityError>;

    async fn authorize_clear_quarantine(
        &self,
        command: &ModuleArtifactSecurityCommand,
    ) -> Result<(), ModuleArtifactSecurityError>;

    async fn authorize_revoke(
        &self,
        command: &ModuleArtifactSecurityCommand,
    ) -> Result<(), ModuleArtifactSecurityError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SecurityAction {
    Quarantine,
    ClearQuarantine,
    Revoke,
}

impl SecurityAction {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Quarantine => "quarantine",
            Self::ClearQuarantine => "clear_quarantine",
            Self::Revoke => "revoke",
        }
    }

    const fn target_status(self) -> ModuleArtifactSecurityStatus {
        match self {
            Self::Quarantine => ModuleArtifactSecurityStatus::Quarantined,
            Self::ClearQuarantine => ModuleArtifactSecurityStatus::Clear,
            Self::Revoke => ModuleArtifactSecurityStatus::Revoked,
        }
    }
}

#[derive(Clone)]
pub struct SeaOrmModuleArtifactSecurityService<A> {
    db: DatabaseConnection,
    authorizer: A,
    infrastructure: ControlPlaneInfrastructure,
}

/// Read-only owner resolver consumed by effective policy. It exposes only the
/// redacted release status/revision snapshot and performs no authorization or
/// state transition.
#[derive(Clone)]
pub struct SeaOrmModuleArtifactSecurityResolver {
    db: DatabaseConnection,
}

impl SeaOrmModuleArtifactSecurityResolver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn resolve(
        &self,
        release: &ArtifactReleaseRef,
    ) -> Result<ModuleArtifactSecuritySnapshot, ModuleArtifactSecurityError> {
        validate_release(release)?;
        load_snapshot(&self.db, release, false).await
    }
}

impl<A> SeaOrmModuleArtifactSecurityService<A>
where
    A: ModuleArtifactSecurityAuthorizer,
{
    pub(crate) fn with_infrastructure(
        db: DatabaseConnection,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            authorizer,
            infrastructure,
        }
    }

    pub async fn quarantine(
        &self,
        command: ModuleArtifactSecurityCommand,
    ) -> Result<ModuleArtifactSecurityReceipt, ModuleArtifactSecurityError> {
        validate_command(&command)?;
        self.authorizer.authorize_quarantine(&command).await?;
        self.transition(SecurityAction::Quarantine, command).await
    }

    pub async fn clear_quarantine(
        &self,
        command: ModuleArtifactSecurityCommand,
    ) -> Result<ModuleArtifactSecurityReceipt, ModuleArtifactSecurityError> {
        validate_command(&command)?;
        self.authorizer.authorize_clear_quarantine(&command).await?;
        self.transition(SecurityAction::ClearQuarantine, command)
            .await
    }

    pub async fn revoke(
        &self,
        command: ModuleArtifactSecurityCommand,
    ) -> Result<ModuleArtifactSecurityReceipt, ModuleArtifactSecurityError> {
        validate_command(&command)?;
        self.authorizer.authorize_revoke(&command).await?;
        self.transition(SecurityAction::Revoke, command).await
    }

    pub async fn snapshot(
        &self,
        release: &ArtifactReleaseRef,
    ) -> Result<ModuleArtifactSecuritySnapshot, ModuleArtifactSecurityError> {
        validate_release(release)?;
        load_snapshot(&self.db, release, false).await
    }

    async fn transition(
        &self,
        action: SecurityAction,
        command: ModuleArtifactSecurityCommand,
    ) -> Result<ModuleArtifactSecurityReceipt, ModuleArtifactSecurityError> {
        let request_digest = digest_json(&(action.as_str(), &command)).map_err(digest_error)?;
        if let Some(receipt) =
            load_operation(&self.db, &command.context, action, &request_digest).await?
        {
            return Ok(receipt);
        }

        let transaction = self.db.begin().await.map_err(store_error)?;
        if let Some(receipt) =
            reserve_operation(&transaction, &command.context, action, &request_digest).await?
        {
            transaction.commit().await.map_err(store_error)?;
            return Ok(receipt);
        }
        let current = load_snapshot(&transaction, &command.release, true).await?;
        if current.revision != command.expected_revision {
            return Err(ModuleArtifactSecurityError::RevisionConflict {
                expected: command.expected_revision,
                current: current.revision,
            });
        }
        validate_transition(action, current.status)?;
        let revision = current
            .revision
            .checked_add(1)
            .ok_or(ModuleArtifactSecurityError::RevisionOverflow)?;
        persist_state(&transaction, action.target_status(), revision, &command).await?;
        let mut snapshot = load_snapshot(&transaction, &command.release, false).await?;
        snapshot.registry_status = current.registry_status;
        let receipt = ModuleArtifactSecurityReceipt {
            snapshot,
            created: true,
        };
        complete_operation(&transaction, command.context.idempotency_key, &receipt).await?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &command.context,
                    DomainEvent::ModuleArtifactSecurityStateChanged {
                        module_slug: command.release.slug,
                        module_version: command.release.version,
                        payload_digest: command.release.digest,
                        security_revision: revision,
                        status: action.target_status().as_str().to_string(),
                        policy_revision: command.policy_revision,
                        reason_code: command.reason_code,
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(receipt)
    }
}

#[derive(Debug, Error)]
pub enum ModuleArtifactSecurityError {
    #[error("artifact security command is invalid")]
    InvalidCommand,
    #[error("artifact security revision conflict: expected {expected}, current {current}")]
    RevisionConflict { expected: u64, current: u64 },
    #[error("artifact release is already quarantined")]
    AlreadyQuarantined,
    #[error("artifact release is not quarantined")]
    NotQuarantined,
    #[error("artifact release is permanently revoked")]
    Revoked,
    #[error("artifact security idempotency key conflicts with another command")]
    IdempotencyConflict,
    #[error("artifact security operation is still in progress")]
    OperationInProgress,
    #[error("artifact security revision overflow")]
    RevisionOverflow,
    #[error("artifact security authorization denied: {0}")]
    AuthorizationDenied(String),
    #[error("artifact security store failed: {0}")]
    Store(String),
}

fn validate_command(
    command: &ModuleArtifactSecurityCommand,
) -> Result<(), ModuleArtifactSecurityError> {
    validate_release(&command.release)?;
    if !valid_platform_command_context(&command.context)
        || !valid_text(&command.policy_revision, MAX_POLICY_REVISION_BYTES)
        || !valid_text(&command.reason_code, MAX_REASON_CODE_BYTES)
        || !valid_text(&command.reason_detail, MAX_REASON_DETAIL_BYTES)
    {
        return Err(ModuleArtifactSecurityError::InvalidCommand);
    }
    Ok(())
}

fn valid_platform_command_context(context: &ModuleCommandContext) -> bool {
    context.tenant_id.is_none() && context.validate().is_ok()
}

fn validate_release(release: &ArtifactReleaseRef) -> Result<(), ModuleArtifactSecurityError> {
    if !valid_text(&release.slug, 128)
        || Version::parse(&release.version).is_err()
        || !valid_digest(&release.digest)
    {
        return Err(ModuleArtifactSecurityError::InvalidCommand);
    }
    Ok(())
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn validate_transition(
    action: SecurityAction,
    current: ModuleArtifactSecurityStatus,
) -> Result<(), ModuleArtifactSecurityError> {
    match (action, current) {
        (_, ModuleArtifactSecurityStatus::Revoked) => Err(ModuleArtifactSecurityError::Revoked),
        (SecurityAction::Quarantine, ModuleArtifactSecurityStatus::Clear) => Ok(()),
        (SecurityAction::Quarantine, ModuleArtifactSecurityStatus::Quarantined) => {
            Err(ModuleArtifactSecurityError::AlreadyQuarantined)
        }
        (SecurityAction::ClearQuarantine, ModuleArtifactSecurityStatus::Quarantined) => Ok(()),
        (SecurityAction::ClearQuarantine, ModuleArtifactSecurityStatus::Clear) => {
            Err(ModuleArtifactSecurityError::NotQuarantined)
        }
        (SecurityAction::Revoke, _) => Ok(()),
    }
}

async fn persist_state(
    transaction: &DatabaseTransaction,
    status: ModuleArtifactSecurityStatus,
    revision: u64,
    command: &ModuleArtifactSecurityCommand,
) -> Result<(), ModuleArtifactSecurityError> {
    let backend = transaction.get_database_backend();
    let checksum = command.release.digest.trim_start_matches("sha256:");
    let result = if command.expected_revision == 0 {
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_security_states
                     (module_slug, module_version, payload_digest, revision, status,
                      policy_revision, reason_code, reason_detail, changed_by, changed_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {})
                     ON CONFLICT (module_slug, module_version, payload_digest) DO NOTHING",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                    placeholder(backend, 9),
                    now_expression(backend),
                ),
                vec![
                    command.release.slug.clone().into(),
                    command.release.version.clone().into(),
                    format!("sha256:{checksum}").into(),
                    revision_value(revision)?,
                    status.as_str().into(),
                    command.policy_revision.clone().into(),
                    command.reason_code.clone().into(),
                    command.reason_detail.clone().into(),
                    uuid_value(command.context.actor_id, backend),
                ],
            ))
            .await
            .map_err(store_error)?
    } else {
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_security_states
                     SET revision = {}, status = {}, policy_revision = {}, reason_code = {},
                         reason_detail = {}, changed_by = {}, changed_at = {}
                     WHERE module_slug = {} AND module_version = {} AND payload_digest = {}
                       AND revision = {} AND status <> 'revoked'",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    now_expression(backend),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                    placeholder(backend, 9),
                    placeholder(backend, 10),
                ),
                vec![
                    revision_value(revision)?,
                    status.as_str().into(),
                    command.policy_revision.clone().into(),
                    command.reason_code.clone().into(),
                    command.reason_detail.clone().into(),
                    uuid_value(command.context.actor_id, backend),
                    command.release.slug.clone().into(),
                    command.release.version.clone().into(),
                    command.release.digest.clone().into(),
                    revision_value(command.expected_revision)?,
                ],
            ))
            .await
            .map_err(store_error)?
    };
    if result.rows_affected() != 1 {
        let current = load_snapshot(transaction, &command.release, false).await?;
        return Err(ModuleArtifactSecurityError::RevisionConflict {
            expected: command.expected_revision,
            current: current.revision,
        });
    }
    Ok(())
}

async fn load_snapshot<C: ConnectionTrait>(
    connection: &C,
    release: &ArtifactReleaseRef,
    lock_row: bool,
) -> Result<ModuleArtifactSecuritySnapshot, ModuleArtifactSecurityError> {
    let backend = connection.get_database_backend();
    let lock = if lock_row && backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT revision, status, policy_revision, reason_code, reason_detail
                 FROM module_artifact_security_states
                 WHERE module_slug = {} AND module_version = {} AND payload_digest = {}{lock}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            vec![
                release.slug.clone().into(),
                release.version.clone().into(),
                release.digest.clone().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    let registry_status = load_registry_status(connection, release).await?;
    match row {
        Some(row) => snapshot_from_row(release, registry_status, &row),
        None => Ok(ModuleArtifactSecuritySnapshot {
            release: release.clone(),
            revision: 0,
            status: ModuleArtifactSecurityStatus::Clear,
            registry_status,
            policy_revision: None,
            reason_code: None,
            reason_detail: None,
        }),
    }
}

async fn load_registry_status<C: ConnectionTrait>(
    connection: &C,
    release: &ArtifactReleaseRef,
) -> Result<ModuleArtifactRegistryReleaseStatus, ModuleArtifactSecurityError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT status FROM registry_module_releases
                 WHERE slug = {} AND version = {} AND checksum_sha256 = {} LIMIT 1",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            vec![
                release.slug.clone().into(),
                release.version.clone().into(),
                release
                    .digest
                    .trim_start_matches("sha256:")
                    .to_owned()
                    .into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    let Some(row) = row else {
        return Ok(ModuleArtifactRegistryReleaseStatus::Unlisted);
    };
    match row
        .try_get::<String>("", "status")
        .map_err(store_error)?
        .as_str()
    {
        "active" => Ok(ModuleArtifactRegistryReleaseStatus::Active),
        "yanked" => Ok(ModuleArtifactRegistryReleaseStatus::Yanked),
        _ => Ok(ModuleArtifactRegistryReleaseStatus::Unavailable),
    }
}

fn snapshot_from_row(
    release: &ArtifactReleaseRef,
    registry_status: ModuleArtifactRegistryReleaseStatus,
    row: &QueryResult,
) -> Result<ModuleArtifactSecuritySnapshot, ModuleArtifactSecurityError> {
    Ok(ModuleArtifactSecuritySnapshot {
        release: release.clone(),
        revision: revision_from_row(row, "revision")?,
        status: ModuleArtifactSecurityStatus::parse(
            &row.try_get::<String>("", "status").map_err(store_error)?,
        )?,
        registry_status,
        policy_revision: row.try_get("", "policy_revision").map_err(store_error)?,
        reason_code: row.try_get("", "reason_code").map_err(store_error)?,
        reason_detail: row.try_get("", "reason_detail").map_err(store_error)?,
    })
}

async fn reserve_operation(
    transaction: &DatabaseTransaction,
    context: &ModuleCommandContext,
    action: SecurityAction,
    request_digest: &str,
) -> Result<Option<ModuleArtifactSecurityReceipt>, ModuleArtifactSecurityError> {
    let backend = transaction.get_database_backend();
    let inserted = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_security_operations
                 (idempotency_key, operation_kind, request_digest, principal_id, trace_id, correlation_id)
                 VALUES ({}, {}, {}, {}, {}, {}) ON CONFLICT (idempotency_key) DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
            ),
            vec![
                uuid_value(context.idempotency_key, backend),
                action.as_str().into(),
                request_digest.to_owned().into(),
                uuid_value(context.actor_id, backend),
                context.trace_id.clone().into(),
                uuid_value(context.correlation_id, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if inserted.rows_affected() == 1 {
        return Ok(None);
    }
    load_operation(transaction, context, action, request_digest).await
}

async fn load_operation<C: ConnectionTrait>(
    connection: &C,
    context: &ModuleCommandContext,
    action: SecurityAction,
    request_digest: &str,
) -> Result<Option<ModuleArtifactSecurityReceipt>, ModuleArtifactSecurityError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT operation_kind, request_digest, principal_id, trace_id, correlation_id,
                        receipt_json
                 FROM module_artifact_security_operations WHERE idempotency_key = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(context.idempotency_key, backend)],
        ))
        .await
        .map_err(store_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let stored_action: String = row.try_get("", "operation_kind").map_err(store_error)?;
    let stored_digest: String = row.try_get("", "request_digest").map_err(store_error)?;
    let stored_actor =
        crate::data::uuid_from_row(&row, "principal_id", backend).map_err(store_error)?;
    let stored_context = ModuleCommandContext {
        actor_id: stored_actor,
        tenant_id: None,
        trace_id: row.try_get("", "trace_id").map_err(store_error)?,
        correlation_id: crate::data::uuid_from_row(&row, "correlation_id", backend)
            .map_err(store_error)?,
        idempotency_key: context.idempotency_key,
    };
    if stored_action != action.as_str()
        || stored_digest != request_digest
        || !valid_platform_command_context(&stored_context)
        || stored_context != *context
    {
        return Err(ModuleArtifactSecurityError::IdempotencyConflict);
    }
    let receipt_json: Option<String> = row.try_get("", "receipt_json").map_err(store_error)?;
    if receipt_json.is_none() {
        return Err(ModuleArtifactSecurityError::OperationInProgress);
    }
    let mut receipt: ModuleArtifactSecurityReceipt =
        serde_json::from_str(receipt_json.as_deref().ok_or_else(|| {
            ModuleArtifactSecurityError::Store(
                "completed artifact security operation has no receipt".to_string(),
            )
        })?)
        .map_err(|error| ModuleArtifactSecurityError::Store(error.to_string()))?;
    receipt.created = false;
    Ok(Some(receipt))
}

async fn complete_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    receipt: &ModuleArtifactSecurityReceipt,
) -> Result<(), ModuleArtifactSecurityError> {
    let backend = transaction.get_database_backend();
    let receipt_json = serde_json::to_string(receipt)
        .map_err(|error| ModuleArtifactSecurityError::Store(error.to_string()))?;
    let updated = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_security_operations
                 SET receipt_json = {}, completed_at = {}
                 WHERE idempotency_key = {} AND completed_at IS NULL",
                placeholder(backend, 1),
                now_expression(backend),
                placeholder(backend, 2),
            ),
            vec![receipt_json.into(), uuid_value(idempotency_key, backend)],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleArtifactSecurityError::OperationInProgress);
    }
    Ok(())
}

fn revision_value(revision: u64) -> Result<Value, ModuleArtifactSecurityError> {
    i64::try_from(revision)
        .map(Into::into)
        .map_err(|_| ModuleArtifactSecurityError::RevisionOverflow)
}

fn revision_from_row(row: &QueryResult, column: &str) -> Result<u64, ModuleArtifactSecurityError> {
    let value: i64 = row.try_get("", column).map_err(store_error)?;
    u64::try_from(value).map_err(|_| {
        ModuleArtifactSecurityError::Store("artifact security revision is invalid".to_string())
    })
}

fn digest_error(error: impl std::fmt::Display) -> ModuleArtifactSecurityError {
    ModuleArtifactSecurityError::Store(error.to_string())
}

fn store_error(error: impl std::fmt::Display) -> ModuleArtifactSecurityError {
    ModuleArtifactSecurityError::Store(error.to_string())
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use rustok_core::MigrationSource;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use uuid::Uuid;

    use super::{
        ModuleArtifactSecurityAuthorizer, ModuleArtifactSecurityCommand,
        ModuleArtifactSecurityError, ModuleArtifactSecurityStatus,
        SeaOrmModuleArtifactSecurityService, SecurityAction, validate_command, validate_transition,
    };
    use crate::{
        ArtifactReleaseRef, ControlPlaneInfrastructure, ModuleCommandContext, ModulesModule,
    };

    struct AllowSecurityTransitions;

    #[async_trait]
    impl ModuleArtifactSecurityAuthorizer for AllowSecurityTransitions {
        async fn authorize_quarantine(
            &self,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Ok(())
        }

        async fn authorize_clear_quarantine(
            &self,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Ok(())
        }

        async fn authorize_revoke(
            &self,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Ok(())
        }
    }

    fn command() -> ModuleArtifactSecurityCommand {
        ModuleArtifactSecurityCommand {
            release: ArtifactReleaseRef {
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: format!("sha256:{}", "a".repeat(64)),
            },
            expected_revision: 0,
            policy_revision: "security-policy-1".to_string(),
            reason_code: "malware_detected".to_string(),
            reason_detail: "Independent verification found a prohibited payload.".to_string(),
            context: ModuleCommandContext {
                actor_id: Uuid::new_v4(),
                tenant_id: None,
                trace_id: "test:artifact-security".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
        }
    }

    #[tokio::test]
    async fn security_state_persists_and_replays_platform_command_context() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        rustok_outbox::SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("outbox migration");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE registry_module_releases (
                    id TEXT PRIMARY KEY,
                    slug TEXT NOT NULL,
                    version TEXT NOT NULL,
                    checksum_sha256 TEXT NOT NULL,
                    status TEXT NOT NULL
                 )"
                .to_string(),
            ))
            .await
            .expect("registry release fixture");
        for migration in ModulesModule.migrations() {
            migration
                .up(&SchemaManager::new(&database))
                .await
                .expect("module migration");
        }
        let service = SeaOrmModuleArtifactSecurityService::with_infrastructure(
            database.clone(),
            AllowSecurityTransitions,
            ControlPlaneInfrastructure::for_database(database.clone()),
        );
        let command = command();

        let initial = service
            .quarantine(command.clone())
            .await
            .expect("initial quarantine");
        assert!(initial.created);
        assert_eq!(
            initial.snapshot.status,
            ModuleArtifactSecurityStatus::Quarantined
        );

        let replay = service
            .quarantine(command.clone())
            .await
            .expect("exact quarantine replay");
        assert!(!replay.created);
        assert_eq!(replay.snapshot, initial.snapshot);

        let receipt = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT principal_id, trace_id, correlation_id
                 FROM module_artifact_security_operations WHERE idempotency_key = ?1",
                vec![command.context.idempotency_key.to_string().into()],
            ))
            .await
            .expect("security receipt query")
            .expect("security receipt");
        assert_eq!(
            receipt
                .try_get::<String>("", "principal_id")
                .expect("receipt actor"),
            command.context.actor_id.to_string()
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "trace_id")
                .expect("receipt trace"),
            command.context.trace_id
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "correlation_id")
                .expect("receipt correlation"),
            command.context.correlation_id.to_string()
        );

        let event = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT payload FROM sys_events WHERE event_type = 'module.artifact.security_state_changed'"
                    .to_string(),
            ))
            .await
            .expect("security event query")
            .expect("security event");
        let payload: serde_json::Value = event
            .try_get("", "payload")
            .expect("security event payload");
        let envelope: rustok_events::EventEnvelope =
            serde_json::from_value(payload).expect("security event envelope");
        assert_eq!(envelope.tenant_id, Uuid::nil());
        assert_eq!(envelope.actor_id, Some(command.context.actor_id));
        assert_eq!(envelope.correlation_id, command.context.correlation_id);
        assert_eq!(
            envelope.trace_id.as_deref(),
            Some(command.context.trace_id.as_str())
        );

        let mut conflicting_replay = command;
        conflicting_replay.context.trace_id = "test:changed-trace".to_string();
        assert!(matches!(
            service.quarantine(conflicting_replay).await,
            Err(ModuleArtifactSecurityError::IdempotencyConflict)
        ));
    }

    #[test]
    fn security_state_rejects_a_tenant_scoped_command_context() {
        let mut command = command();
        command.context.tenant_id = Some(Uuid::new_v4());
        assert!(matches!(
            validate_command(&command),
            Err(ModuleArtifactSecurityError::InvalidCommand)
        ));
    }

    #[test]
    fn quarantine_and_revoke_transitions_are_distinct_and_revocation_is_terminal() {
        assert!(
            validate_transition(
                SecurityAction::Quarantine,
                ModuleArtifactSecurityStatus::Clear
            )
            .is_ok()
        );
        assert!(matches!(
            validate_transition(
                SecurityAction::ClearQuarantine,
                ModuleArtifactSecurityStatus::Clear
            ),
            Err(ModuleArtifactSecurityError::NotQuarantined)
        ));
        assert!(matches!(
            validate_transition(
                SecurityAction::Revoke,
                ModuleArtifactSecurityStatus::Quarantined
            ),
            Ok(())
        ));
        assert!(matches!(
            validate_transition(
                SecurityAction::ClearQuarantine,
                ModuleArtifactSecurityStatus::Revoked
            ),
            Err(ModuleArtifactSecurityError::Revoked)
        ));
    }
}
