//! Durable global security enforcement for immutable artifact releases.
//!
//! Registry yanking remains a discovery/install concern. Quarantine and
//! revocation are separate owner states that block new execution without
//! mutating tenant enablement intent.

use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, QueryResult, Statement,
    TransactionTrait, Value,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use rustok_events::DomainEvent;

use crate::backend::SupportedBackend;
use crate::{
    ArtifactReleaseRef, ControlPlaneInfrastructure, ModuleCommandContext, SeaOrmSecurityEpochStore,
    installation::valid_digest, promotion::digest_json,
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

/// Host-only registry continuity evidence, not a permission grant or a receipt.
/// A permanent owner revision also distinguishes absent -> listed -> absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactRegistryExecutionAuthority {
    release: ArtifactReleaseRef,
    revision: u64,
    status: ModuleArtifactRegistryReleaseStatus,
}

impl ArtifactRegistryExecutionAuthority {
    pub async fn capture(
        db: &DatabaseConnection,
        release: &ArtifactReleaseRef,
    ) -> Result<Self, ModuleArtifactSecurityError> {
        Self::load_on(db, release).await
    }

    pub(crate) async fn load_on(
        db: &impl ConnectionTrait,
        release: &ArtifactReleaseRef,
    ) -> Result<Self, ModuleArtifactSecurityError> {
        validate_release(release)?;
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(store_error)?;
        // A single statement observes the witness and exact registry status
        // together, including when no matching release has ever been listed.
        let row = db.query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT authority.revision, listed.status, listed.slug AS listed_slug, listed.checksum_sha256
                 FROM (SELECT CAST({} AS TEXT) AS slug, CAST({} AS TEXT) AS version) AS target
                 LEFT JOIN registry_release_authority_revisions AS authority
                   ON authority.slug = target.slug AND authority.version = target.version
                 LEFT JOIN registry_module_releases AS listed
                   ON listed.slug = target.slug AND listed.version = target.version",
                backend.placeholder(1), backend.placeholder(2),
            ),
            vec![release.slug.clone().into(), release.version.clone().into()],
        )).await.map_err(store_error)?
            .ok_or_else(|| ModuleArtifactSecurityError::Store("registry authority query returned no row".into()))?;
        let listed: Option<String> = row.try_get("", "listed_slug").map_err(store_error)?;
        let revision: Option<i64> = row.try_get("", "revision").map_err(store_error)?;
        let revision = match revision {
            Some(value) if value > 0 => value as u64,
            None if listed.is_none() => 0,
            _ => return Err(ModuleArtifactSecurityError::Store("invalid registry release authority revision".into())),
        };
        let checksum: Option<String> = row.try_get("", "checksum_sha256").map_err(store_error)?;
        let status: Option<String> = row.try_get("", "status").map_err(store_error)?;
        let status = match (listed, status.as_deref()) {
            (None, None) => ModuleArtifactRegistryReleaseStatus::Unlisted,
            (Some(_), Some(value)) if checksum.as_deref() == Some(release.digest.trim_start_matches("sha256:")) =>
                registry_release_status(value),
            (Some(_), Some(_)) => ModuleArtifactRegistryReleaseStatus::Unlisted,
            _ => return Err(ModuleArtifactSecurityError::Store("invalid registry release status".into())),
        };
        if status == ModuleArtifactRegistryReleaseStatus::Unavailable {
            return Err(ModuleArtifactSecurityError::AuthorizationDenied(
                "registry release is unavailable".into(),
            ));
        }
        Ok(Self { release: release.clone(), revision, status })
    }

    pub(crate) fn status(&self) -> ModuleArtifactRegistryReleaseStatus {
        self.status
    }

    pub async fn is_current(
        &self,
        db: &DatabaseConnection,
    ) -> Result<bool, ModuleArtifactSecurityError> {
        Ok(Self::load_on(db, &self.release).await? == *self)
    }

    pub async fn require_current_on(
        &self,
        tx: &DatabaseTransaction,
    ) -> Result<(), ModuleArtifactSecurityError> {
        crate::installation::acquire_artifact_activation_lock(
            tx, &crate::ModuleInstallationScope::Platform, &self.release.slug,
        ).await.map_err(store_error)?;
        if Self::load_on(tx, &self.release).await? != *self {
            return Err(ModuleArtifactSecurityError::AuthorizationDenied(
                "registry release authority has changed".into(),
            ));
        }
        Ok(())
    }
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

impl ModuleArtifactSecuritySnapshot {
    /// Revision zero denotes the implicit clear state, not a missing grant.
    /// A cleared quarantine has a new revision and cannot revive an old lease.
    pub fn execution_revision(&self) -> Option<u64> {
        (self.status == ModuleArtifactSecurityStatus::Clear
            && self.registry_status != ModuleArtifactRegistryReleaseStatus::Unavailable)
            .then_some(self.revision)
    }
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
        transaction: &DatabaseTransaction,
        command: &ModuleArtifactSecurityCommand,
    ) -> Result<(), ModuleArtifactSecurityError>;

    async fn authorize_clear_quarantine(
        &self,
        transaction: &DatabaseTransaction,
        command: &ModuleArtifactSecurityCommand,
    ) -> Result<(), ModuleArtifactSecurityError>;

    async fn authorize_revoke(
        &self,
        transaction: &DatabaseTransaction,
        command: &ModuleArtifactSecurityCommand,
    ) -> Result<(), ModuleArtifactSecurityError>;

    /// Recheck time-bounded authority immediately before each successful
    /// commit, after owner lock waits and writes. This also applies to exact
    /// idempotency replays, whose authorization must not expire while waiting.
    async fn ensure_authority_at_commit(
        &self,
        transaction: &DatabaseTransaction,
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

    /// Verify the original release security revision on the effect
    /// transaction. The platform slug fence covers the implicit clear state
    /// even when there is no security row to lock.
    pub async fn require_execution_revision_on(
        tx: &DatabaseTransaction,
        release: &ArtifactReleaseRef,
        expected: u64,
    ) -> Result<(), ModuleArtifactSecurityError> {
        let snapshot = lock_security_snapshot_on(tx, release).await?;
        if snapshot.execution_revision() != Some(expected) {
            return Err(ModuleArtifactSecurityError::AuthorizationDenied(
                "artifact execution security revision is no longer admitted".into(),
            ));
        }
        Ok(())
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
        self.transition(SecurityAction::Quarantine, command).await
    }

    pub async fn clear_quarantine(
        &self,
        command: ModuleArtifactSecurityCommand,
    ) -> Result<ModuleArtifactSecurityReceipt, ModuleArtifactSecurityError> {
        validate_command(&command)?;
        self.transition(SecurityAction::ClearQuarantine, command)
            .await
    }

    pub async fn revoke(
        &self,
        command: ModuleArtifactSecurityCommand,
    ) -> Result<ModuleArtifactSecurityReceipt, ModuleArtifactSecurityError> {
        validate_command(&command)?;
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
        #[derive(Serialize)]
        struct SemanticFingerprint<'a> {
            action: &'a str,
            release: &'a ArtifactReleaseRef,
            expected_revision: u64,
            policy_revision: &'a str,
            reason_code: &'a str,
        }

        // `reason_detail` is free-text audit evidence and the command context
        // carries observability metadata; neither changes the security-state
        // decision. The actor boundary is checked against the stored
        // operation principal separately.
        let request_digest = digest_json(&SemanticFingerprint {
            action: action.as_str(),
            release: &command.release,
            expected_revision: command.expected_revision,
            policy_revision: &command.policy_revision,
            reason_code: &command.reason_code,
        })
        .map_err(digest_error)?;
        let transaction = crate::backend::begin_fenced_transaction(&self.db)
            .await
            .map_err(store_error)?;
        crate::backend::set_platform_scope(&transaction)
            .await
            .map_err(store_error)?;
        // Authorization runs before idempotency replay and before the platform
        // slug lock. The host can acquire credential/RBAC row fences first and
        // retain them through the durable state, receipt and outbox commit.
        match action {
            SecurityAction::Quarantine => self
                .authorizer
                .authorize_quarantine(&transaction, &command)
                .await?,
            SecurityAction::ClearQuarantine => self
                .authorizer
                .authorize_clear_quarantine(&transaction, &command)
                .await?,
            SecurityAction::Revoke => self
                .authorizer
                .authorize_revoke(&transaction, &command)
                .await?,
        }
        if let Some(receipt) =
            reserve_operation(&transaction, &command.context, action, &request_digest).await?
        {
            self.authorizer
                .ensure_authority_at_commit(&transaction, &command)
                .await?;
            transaction.commit().await.map_err(store_error)?;
            return Ok(receipt);
        }
        // Receipt admission is private to this command table. Take the
        // slug and then the global epoch before any security-state row lock.
        crate::installation::acquire_artifact_activation_lock(
            &transaction, &crate::ModuleInstallationScope::Platform, &command.release.slug,
        ).await.map_err(store_error)?;
        if matches!(action, SecurityAction::Quarantine | SecurityAction::Revoke) {
            SeaOrmSecurityEpochStore::lock_for_advance(&transaction).await.map_err(store_error)?;
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
        if matches!(action, SecurityAction::Quarantine | SecurityAction::Revoke) {
            // Preempt every in-flight transition recorded against the
            // previous epoch, atomically with the security decision itself:
            // a quarantine/revocation that committed without advancing the
            // durable epoch would leave stale transitions finalizable.
            SeaOrmSecurityEpochStore::advance(
                &transaction,
                &format!(
                    "Artifact {} for {}@{} ({}): {} — {}",
                    action.as_str(),
                    command.release.slug,
                    command.release.version,
                    command.release.digest,
                    command.reason_code,
                    command.reason_detail,
                ),
                self.infrastructure.now(),
            )
            .await
            .map_err(store_error)?;
        }
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
                        module_slug: command.release.slug.clone(),
                        module_version: command.release.version.clone(),
                        payload_digest: command.release.digest.clone(),
                        security_revision: revision,
                        status: action.target_status().as_str().to_string(),
                        policy_revision: command.policy_revision.clone(),
                        reason_code: command.reason_code.clone(),
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        self.authorizer
            .ensure_authority_at_commit(&transaction, &command)
            .await?;
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
    let backend = SupportedBackend::new(transaction.get_database_backend()).map_err(store_error)?;
    let checksum = command.release.digest.trim_start_matches("sha256:");
    let result = if command.expected_revision == 0 {
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_security_states
                     (module_slug, module_version, payload_digest, revision, status,
                      policy_revision, reason_code, reason_detail, changed_by, changed_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {})
                     ON CONFLICT (module_slug, module_version, payload_digest) DO NOTHING",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.placeholder(6),
                    backend.placeholder(7),
                    backend.placeholder(8),
                    backend.placeholder(9),
                    backend.now_sql(),
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
                    backend.uuid_value(command.context.actor_id),
                ],
            ))
            .await
            .map_err(store_error)?
    } else {
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "UPDATE module_artifact_security_states
                     SET revision = {}, status = {}, policy_revision = {}, reason_code = {},
                         reason_detail = {}, changed_by = {}, changed_at = {}
                     WHERE module_slug = {} AND module_version = {} AND payload_digest = {}
                       AND revision = {} AND status <> 'revoked'",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.placeholder(6),
                    backend.now_sql(),
                    backend.placeholder(7),
                    backend.placeholder(8),
                    backend.placeholder(9),
                    backend.placeholder(10),
                ),
                vec![
                    revision_value(revision)?,
                    status.as_str().into(),
                    command.policy_revision.clone().into(),
                    command.reason_code.clone().into(),
                    command.reason_detail.clone().into(),
                    backend.uuid_value(command.context.actor_id),
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

/// Shared writer/effect serialization point. Never apply the serving
/// mutation journal guard here: emergency security actions must remain
/// possible during a durable lifecycle claim. The slug lock is already
/// acquired before the tenant selection lock in SQL effect admission.
async fn lock_security_snapshot_on(
    tx: &DatabaseTransaction,
    release: &ArtifactReleaseRef,
) -> Result<ModuleArtifactSecuritySnapshot, ModuleArtifactSecurityError> {
    validate_release(release)?;
    crate::installation::acquire_artifact_activation_lock(
        tx,
        &crate::ModuleInstallationScope::Platform,
        &release.slug,
    )
    .await
    .map_err(store_error)?;
    load_snapshot(tx, release, true).await
}

async fn load_snapshot<C: ConnectionTrait>(
    connection: &C,
    release: &ArtifactReleaseRef,
    lock_row: bool,
) -> Result<ModuleArtifactSecuritySnapshot, ModuleArtifactSecurityError> {
    let backend = SupportedBackend::new(connection.get_database_backend()).map_err(store_error)?;
    let lock = if lock_row && backend == SupportedBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT revision, status, policy_revision, reason_code, reason_detail
                 FROM module_artifact_security_states
                 WHERE module_slug = {} AND module_version = {} AND payload_digest = {}{lock}",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
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
    let backend = SupportedBackend::new(connection.get_database_backend()).map_err(store_error)?;
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT status FROM registry_module_releases
                 WHERE slug = {} AND version = {} AND checksum_sha256 = {} LIMIT 1",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
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
    Ok(registry_release_status(
        &row.try_get::<String>("", "status").map_err(store_error)?,
    ))
}

fn registry_release_status(value: &str) -> ModuleArtifactRegistryReleaseStatus {
    match value {
        "active" => ModuleArtifactRegistryReleaseStatus::Active,
        "yanked" => ModuleArtifactRegistryReleaseStatus::Yanked,
        _ => ModuleArtifactRegistryReleaseStatus::Unavailable,
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
    let backend = SupportedBackend::new(transaction.get_database_backend()).map_err(store_error)?;
    let inserted = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "INSERT INTO module_artifact_security_operations
                 (idempotency_key, operation_kind, request_digest, principal_id, trace_id, correlation_id)
                 VALUES ({}, {}, {}, {}, {}, {}) ON CONFLICT (idempotency_key) DO NOTHING",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(4),
                backend.placeholder(5),
                backend.placeholder(6),
            ),
            vec![
                backend.uuid_value(context.idempotency_key),
                action.as_str().into(),
                request_digest.to_owned().into(),
                backend.uuid_value(context.actor_id),
                context.trace_id.clone().into(),
                backend.uuid_value(context.correlation_id),
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
    let backend = SupportedBackend::new(connection.get_database_backend()).map_err(store_error)?;
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT operation_kind, request_digest, principal_id, trace_id, correlation_id,
                        receipt_json
                 FROM module_artifact_security_operations WHERE idempotency_key = {}",
                backend.placeholder(1),
            ),
            vec![backend.uuid_value(context.idempotency_key)],
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
        || stored_context.actor_id != context.actor_id
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
    let backend = SupportedBackend::new(transaction.get_database_backend()).map_err(store_error)?;
    let receipt_json = serde_json::to_string(receipt)
        .map_err(|error| ModuleArtifactSecurityError::Store(error.to_string()))?;
    let updated = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "UPDATE module_artifact_security_operations
                 SET receipt_json = {}, completed_at = {}
                 WHERE idempotency_key = {} AND completed_at IS NULL",
                backend.placeholder(1),
                backend.now_sql(),
                backend.placeholder(2),
            ),
            vec![receipt_json.into(), backend.uuid_value(idempotency_key)],
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
    u64::try_from(value)
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| {
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
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TransactionTrait};
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use uuid::Uuid;

    use super::{
        ModuleArtifactSecurityAuthorizer, ModuleArtifactSecurityCommand,
        ModuleArtifactSecurityError, ModuleArtifactSecurityStatus,
        SeaOrmModuleArtifactSecurityResolver, SeaOrmModuleArtifactSecurityService, SecurityAction,
        lock_security_snapshot_on, persist_state, validate_command, validate_transition,
    };
    use crate::{
        ArtifactReleaseRef, ControlPlaneInfrastructure, ModuleCommandContext, ModulesModule,
    };

    struct AllowSecurityTransitions;

    #[async_trait]
    impl ModuleArtifactSecurityAuthorizer for AllowSecurityTransitions {
        async fn authorize_quarantine(
            &self,
            _transaction: &sea_orm::DatabaseTransaction,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Ok(())
        }

        async fn authorize_clear_quarantine(
            &self,
            _transaction: &sea_orm::DatabaseTransaction,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Ok(())
        }

        async fn authorize_revoke(
            &self,
            _transaction: &sea_orm::DatabaseTransaction,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Ok(())
        }

        async fn ensure_authority_at_commit(
            &self,
            _transaction: &sea_orm::DatabaseTransaction,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Ok(())
        }
    }


    struct DenySecurityTransitions;

    #[async_trait]
    impl ModuleArtifactSecurityAuthorizer for DenySecurityTransitions {
        async fn authorize_quarantine(
            &self,
            _transaction: &sea_orm::DatabaseTransaction,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Err(ModuleArtifactSecurityError::AuthorizationDenied(
                "operator authority was revoked".to_string(),
            ))
        }

        async fn authorize_clear_quarantine(
            &self,
            _transaction: &sea_orm::DatabaseTransaction,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Err(ModuleArtifactSecurityError::AuthorizationDenied(
                "operator authority was revoked".to_string(),
            ))
        }

        async fn authorize_revoke(
            &self,
            _transaction: &sea_orm::DatabaseTransaction,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Err(ModuleArtifactSecurityError::AuthorizationDenied(
                "operator authority was revoked".to_string(),
            ))
        }

        async fn ensure_authority_at_commit(
            &self,
            _transaction: &sea_orm::DatabaseTransaction,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Err(ModuleArtifactSecurityError::AuthorizationDenied(
                "operator authority was revoked".to_string(),
            ))
        }
    }

    struct DenyCommitSecurityTransitions;

    #[async_trait]
    impl ModuleArtifactSecurityAuthorizer for DenyCommitSecurityTransitions {
        async fn authorize_quarantine(
            &self,
            _transaction: &sea_orm::DatabaseTransaction,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Ok(())
        }

        async fn authorize_clear_quarantine(
            &self,
            _transaction: &sea_orm::DatabaseTransaction,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Ok(())
        }

        async fn authorize_revoke(
            &self,
            _transaction: &sea_orm::DatabaseTransaction,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Ok(())
        }

        async fn ensure_authority_at_commit(
            &self,
            _transaction: &sea_orm::DatabaseTransaction,
            _command: &ModuleArtifactSecurityCommand,
        ) -> Result<(), ModuleArtifactSecurityError> {
            Err(ModuleArtifactSecurityError::AuthorizationDenied(
                "credential expired before commit".to_string(),
            ))
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
        // `registry_module_releases` is created by migration 000061 with
        // the full column set the later data migrations read; a minimal
        // hand-made pre-create would shadow it (`IF NOT EXISTS`) and break
        // the chain (000069 backfill).
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

        let tx = crate::backend::begin_fenced_transaction(&database)
            .await
            .unwrap();
        SeaOrmModuleArtifactSecurityResolver::require_execution_revision_on(
            &tx,
            &command.release,
            0,
        )
        .await
        .expect("implicit clear revision admitted");
        tx.rollback().await.unwrap();

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

        // A completed receipt must not bypass current authorization. Replay
        // still enters the owner transaction and asks the host to revalidate.
        let denied_replay_service =
            SeaOrmModuleArtifactSecurityService::with_infrastructure(
                database.clone(),
                DenySecurityTransitions,
                ControlPlaneInfrastructure::for_database(database.clone()),
            );
        assert!(matches!(
            denied_replay_service.quarantine(command.clone()).await,
            Err(ModuleArtifactSecurityError::AuthorizationDenied(_))
        ));

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

        let mut changed_observability_replay = command.clone();
        changed_observability_replay.context.trace_id = "test:changed-trace".to_string();
        changed_observability_replay.context.correlation_id = Uuid::new_v4();
        changed_observability_replay.reason_detail = "retry wording".to_string();
        service
            .quarantine(changed_observability_replay)
            .await
            .expect("changed observability context replays quarantine");

        let tx = crate::backend::begin_fenced_transaction(&database)
            .await
            .unwrap();
        assert!(
            SeaOrmModuleArtifactSecurityResolver::require_execution_revision_on(
                &tx,
                &command.release,
                0,
            )
            .await
            .is_err()
        );
        tx.rollback().await.unwrap();

        let mut clear = command.clone();
        clear.expected_revision = 1;
        clear.context.idempotency_key = Uuid::new_v4();
        let cleared = service
            .clear_quarantine(clear)
            .await
            .expect("clear quarantine");
        assert_eq!(cleared.snapshot.execution_revision(), Some(2));
        let tx = crate::backend::begin_fenced_transaction(&database)
            .await
            .unwrap();
        assert!(
            SeaOrmModuleArtifactSecurityResolver::require_execution_revision_on(
                &tx,
                &command.release,
                0,
            )
            .await
            .is_err(),
            "clearing quarantine must not revive the original lease"
        );
        SeaOrmModuleArtifactSecurityResolver::require_execution_revision_on(
            &tx,
            &command.release,
            2,
        )
        .await
        .expect("new revision admitted");
        tx.rollback().await.unwrap();

        let mut revoke = command.clone();
        revoke.expected_revision = 2;
        revoke.context.idempotency_key = Uuid::new_v4();
        let revoked = service.revoke(revoke).await.expect("revoke");
        assert_eq!(revoked.snapshot.execution_revision(), None);
        let tx = crate::backend::begin_fenced_transaction(&database)
            .await
            .unwrap();
        assert!(
            SeaOrmModuleArtifactSecurityResolver::require_execution_revision_on(
                &tx,
                &command.release,
                2,
            )
            .await
            .is_err()
        );
        tx.rollback().await.unwrap();
    }


    #[tokio::test]
    async fn commit_authority_is_rechecked_for_new_commands_and_exact_replays() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        rustok_outbox::SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("outbox migration");
        for migration in ModulesModule.migrations() {
            migration
                .up(&SchemaManager::new(&database))
                .await
                .expect("module migration");
        }

        let command = command();
        let expired_service = SeaOrmModuleArtifactSecurityService::with_infrastructure(
            database.clone(),
            DenyCommitSecurityTransitions,
            ControlPlaneInfrastructure::for_database(database.clone()),
        );
        assert!(matches!(
            expired_service.quarantine(command.clone()).await,
            Err(ModuleArtifactSecurityError::AuthorizationDenied(_))
        ));

        let resolver = SeaOrmModuleArtifactSecurityResolver::new(database.clone());
        let unchanged = resolver
            .resolve(&command.release)
            .await
            .expect("read rolled-back security state");
        assert_eq!(unchanged.status, ModuleArtifactSecurityStatus::Clear);
        assert_eq!(unchanged.revision, 0);

        let operation_count = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS total FROM module_artifact_security_operations".to_string(),
            ))
            .await
            .expect("query operation count")
            .expect("operation count row")
            .try_get::<i64>("", "total")
            .expect("operation count");
        let event_count = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS total FROM sys_events WHERE event_type = 'module.artifact.security_state_changed'"
                    .to_string(),
            ))
            .await
            .expect("query event count")
            .expect("event count row")
            .try_get::<i64>("", "total")
            .expect("event count");
        assert_eq!(operation_count, 0, "pending receipt must roll back");
        assert_eq!(event_count, 0, "outbox event must roll back");

        let allowed_service = SeaOrmModuleArtifactSecurityService::with_infrastructure(
            database.clone(),
            AllowSecurityTransitions,
            ControlPlaneInfrastructure::for_database(database.clone()),
        );
        allowed_service
            .quarantine(command.clone())
            .await
            .expect("seed completed command receipt");

        assert!(matches!(
            expired_service.quarantine(command.clone()).await,
            Err(ModuleArtifactSecurityError::AuthorizationDenied(_))
        ));

        let replayed_state = resolver
            .resolve(&command.release)
            .await
            .expect("read state after denied replay");
        assert_eq!(replayed_state.status, ModuleArtifactSecurityStatus::Quarantined);
        assert_eq!(replayed_state.revision, 1);

        let operation_count = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS total FROM module_artifact_security_operations".to_string(),
            ))
            .await
            .expect("query operation count")
            .expect("operation count row")
            .try_get::<i64>("", "total")
            .expect("operation count");
        let event_count = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS total FROM sys_events WHERE event_type = 'module.artifact.security_state_changed'"
                    .to_string(),
            ))
            .await
            .expect("query event count")
            .expect("event count row")
            .try_get::<i64>("", "total")
            .expect("event count");
        assert_eq!(operation_count, 1, "denied replay must not duplicate receipts");
        assert_eq!(event_count, 1, "denied replay must not duplicate events");
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_first_quarantine_fences_effect_without_security_row() {
        use std::time::Duration;

        let url = std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").unwrap();
        let db = Database::connect(url).await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let schema = format!("security_fence_test_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await
            .unwrap();
        let setup = db.begin().await.unwrap();
        setup
            .execute_unprepared(&format!("SET LOCAL search_path TO {schema}"))
            .await
            .unwrap();
        setup.execute_unprepared(
            "CREATE TABLE module_artifact_activation_locks (scope_kind TEXT NOT NULL, scope_tenant_key TEXT NOT NULL, slug TEXT NOT NULL, PRIMARY KEY (scope_kind, scope_tenant_key, slug));
             CREATE TABLE registry_module_releases (slug TEXT, version TEXT, checksum_sha256 TEXT, status TEXT);
             CREATE TABLE module_artifact_security_states (module_slug TEXT NOT NULL, module_version TEXT NOT NULL, payload_digest TEXT NOT NULL, revision BIGINT NOT NULL, status TEXT NOT NULL, policy_revision TEXT NOT NULL, reason_code TEXT NOT NULL, reason_detail TEXT NOT NULL, changed_by UUID NOT NULL, changed_at TIMESTAMPTZ NOT NULL, PRIMARY KEY (module_slug, module_version, payload_digest));",
        ).await.unwrap();
        setup.commit().await.unwrap();

        let command = command();
        let writer = crate::backend::begin_fenced_transaction(&db).await.unwrap();
        writer
            .execute_unprepared(&format!("SET LOCAL search_path TO {schema}"))
            .await
            .unwrap();
        let initial = lock_security_snapshot_on(&writer, &command.release)
            .await
            .unwrap();
        assert_eq!(initial.execution_revision(), Some(0));
        // The same owner primitives used by the security command, before
        // its transaction commits the new state and outbox.
        persist_state(
            &writer,
            ModuleArtifactSecurityStatus::Quarantined,
            1,
            &command,
        )
        .await
        .unwrap();

        let worker_db = db.clone();
        let worker_schema = schema.clone();
        let release = command.release.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let mut worker = tokio::spawn(async move {
            let tx = crate::backend::begin_fenced_transaction(&worker_db)
                .await
                .unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {worker_schema}"))
                .await
                .unwrap();
            ready_tx.send(()).unwrap();
            let result = SeaOrmModuleArtifactSecurityResolver::require_execution_revision_on(
                &tx, &release, 0,
            )
            .await;
            tx.rollback().await.unwrap();
            result
        });
        ready_rx.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(150), &mut worker)
                .await
                .is_err()
        );
        writer.commit().await.unwrap();
        assert!(matches!(
            worker.await.unwrap(),
            Err(ModuleArtifactSecurityError::AuthorizationDenied(_))
        ));
        // Reverse order: the effect wins admission first. A writer cannot
        // publish a new security decision until that effect commits.
        let mut clear = command.clone();
        clear.expected_revision = 1;
        let tx = crate::backend::begin_fenced_transaction(&db).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}"))
            .await
            .unwrap();
        assert_eq!(
            lock_security_snapshot_on(&tx, &clear.release)
                .await
                .unwrap()
                .revision,
            1
        );
        persist_state(&tx, ModuleArtifactSecurityStatus::Clear, 2, &clear)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let effect = crate::backend::begin_fenced_transaction(&db).await.unwrap();
        effect
            .execute_unprepared(&format!("SET LOCAL search_path TO {schema}"))
            .await
            .unwrap();
        SeaOrmModuleArtifactSecurityResolver::require_execution_revision_on(
            &effect,
            &command.release,
            2,
        )
        .await
        .unwrap();
        let worker_db = db.clone();
        let worker_schema = schema.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let mut writer = tokio::spawn(async move {
            let tx = crate::backend::begin_fenced_transaction(&worker_db)
                .await
                .unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {worker_schema}"))
                .await
                .unwrap();
            ready_tx.send(()).unwrap();
            let snapshot = lock_security_snapshot_on(&tx, &command.release)
                .await
                .unwrap();
            assert_eq!(snapshot.execution_revision(), Some(2));
            let mut revoke = command;
            revoke.expected_revision = 2;
            persist_state(&tx, ModuleArtifactSecurityStatus::Revoked, 3, &revoke)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        });
        ready_rx.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(150), &mut writer)
                .await
                .is_err()
        );
        effect.commit().await.unwrap();
        writer.await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
            .await
            .unwrap();
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
