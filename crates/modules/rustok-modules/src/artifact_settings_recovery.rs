//! Protected recovery, purge, and restore ownership for dynamic artifact settings.
//!
//! The artifact settings table is deliberately distinct from structured artifact
//! data. A settings purge can proceed only after a host policy has authorized
//! the exact retired installation and an encrypted recovery point exists. The
//! service never receives a KMS key or a secret value resolver: the host owns
//! that boundary through [`ArtifactSettingsRecoveryCipher`].

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, TransactionTrait,
    Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use rustok_api::manifest_hash::{
    canonical_json_bytes, canonical_manifest_snapshot_json, hash_manifest_snapshot,
};
use rustok_events::DomainEvent;

use crate::{
    ControlPlaneInfrastructure, ModuleArtifactDescriptor, ModuleCommandContext,
    artifact_schema::{ArtifactSchemaValidationError, ArtifactSchemaValidatorCache},
    canonical_artifact_descriptor_digest,
    data::configure_tenant_scope,
    installation::{ModuleInstallationScope, acquire_artifact_activation_lock},
};

const MAX_REASON_BYTES: usize = 2_000;
const MAX_POLICY_SNAPSHOT_ID_BYTES: usize = 128;
const MAX_KEY_VERSION_BYTES: usize = 256;
const MAX_CIPHERTEXT_BYTES: usize = 128 * 1024;
const MAX_COLLECTION_BATCH: u32 = 100;
const MAX_COLLECTION_SCAN: u32 = 1_000;

/// Exact tenant-scoped command for materializing an encrypted settings
/// recovery point. The source installation must already be inactive and
/// uninstalled; a caller cannot snapshot a serving settings instance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryPointCreateRequest {
    pub tenant_id: Uuid,
    pub installation_id: Uuid,
    pub expected_installation_revision: u64,
    pub expected_settings_revision: u64,
    /// Mandatory authenticated evidence. Its tenant must match this recovery
    /// operation's tenant scope.
    pub context: ModuleCommandContext,
    pub reason: String,
}

/// Host-owned retention evidence returned only after policy evaluation.
/// Callers cannot set expiry or holds on a recovery point directly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryRetention {
    pub policy_snapshot_id: String,
    /// Digest of the unresolved logical secret-handle set. Raw secret values,
    /// resolver identities, and external secret bytes never enter this owner.
    pub secret_handle_digest: String,
    pub retain_until: DateTime<Utc>,
    pub legal_hold: bool,
    pub audit_hold: bool,
    pub incident_hold: bool,
}

/// Host-owned resulting policy for a retention update. The secret-handle
/// digest is deliberately absent: it is immutable authenticated context for
/// the existing ciphertext, not mutable retention policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryRetentionUpdate {
    pub policy_snapshot_id: String,
    pub retain_until: DateTime<Utc>,
    pub legal_hold: bool,
    pub audit_hold: bool,
    pub incident_hold: bool,
}

/// Metadata for a protected settings recovery point. Ciphertext, canonical
/// settings, grants, and external secret bytes are intentionally absent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryPoint {
    pub recovery_point_id: Uuid,
    pub tenant_id: Uuid,
    pub installation_id: Uuid,
    pub data_owner_id: Uuid,
    pub settings_instance_id: Uuid,
    pub settings_revision: u64,
    pub schema_digest: String,
    pub descriptor_digest: String,
    pub value_digest: String,
    /// Immutable KMS key version that protects this ciphertext. A mutable key
    /// alias is insufficient because recovery must remain independently
    /// auditable and rewrappable.
    pub key_version: String,
    pub policy_snapshot_id: String,
    pub secret_handle_digest: String,
    pub retention_revision: u64,
    pub retain_until: DateTime<Utc>,
    pub legal_hold: bool,
    pub audit_hold: bool,
    pub incident_hold: bool,
}

/// A revision-guarded recovery-retention command. The host authorizer returns
/// the resulting policy snapshot and hold state; callers cannot shorten
/// retention, release a hold, or bypass an outstanding hold directly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryRetentionUpdateRequest {
    pub tenant_id: Uuid,
    pub recovery_point_id: Uuid,
    pub expected_retention_revision: u64,
    pub extend_retain_until: Option<DateTime<Utc>>,
    pub legal_hold: Option<bool>,
    pub audit_hold: Option<bool>,
    pub incident_hold: Option<bool>,
    pub context: ModuleCommandContext,
    pub reason: String,
}

/// Immutable receipt for one revision-guarded retention update. It deliberately
/// excludes mutable recovery-point fields such as the KMS key version so an
/// idempotent replay returns the original operation outcome after a later
/// rewrap or collection transition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryRetentionUpdateResult {
    pub recovery_point_id: Uuid,
    pub retention_revision: u64,
    pub policy_snapshot_id: String,
    pub retain_until: DateTime<Utc>,
    pub legal_hold: bool,
    pub audit_hold: bool,
    pub incident_hold: bool,
}

/// Separately authorized destructive settings-owner operation. It cannot be
/// combined with structured artifact-data deletion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsPurgeRequest {
    pub tenant_id: Uuid,
    pub installation_id: Uuid,
    pub recovery_point_id: Uuid,
    pub expected_installation_revision: u64,
    pub expected_settings_revision: u64,
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsPurgeResult {
    pub purge_operation_id: Uuid,
    pub recovery_point_id: Uuid,
    pub tombstone_revision: u64,
}

/// A restore creates a fresh non-serving settings instance. A target is
/// optional: an absent target intentionally leaves the instance unbound for a
/// later continuity-authorized reinstall.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRestoreRequest {
    pub tenant_id: Uuid,
    pub recovery_point_id: Uuid,
    /// Required exactly when a target installation is selected. An unbound
    /// restore intentionally carries no target revision and must later use the
    /// separate continuity bind command.
    pub target_installation_id: Option<Uuid>,
    pub expected_target_installation_revision: Option<u64>,
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRestoreResult {
    pub restore_operation_id: Uuid,
    pub recovery_point_id: Uuid,
    pub settings_instance_id: Uuid,
    pub target_installation_id: Option<Uuid>,
}

/// An owner-authorized rewrap command. The selected KMS key is never supplied
/// by the caller: the host cipher chooses the current approved key version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryRewrapRequest {
    pub tenant_id: Uuid,
    pub recovery_point_id: Uuid,
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryRewrapResult {
    pub rewrap_operation_id: Uuid,
    pub recovery_point_id: Uuid,
    pub previous_key_version: String,
    pub key_version: String,
}

/// A bounded owner-worker command for terminal recovery-point collection.
/// Collection is a separate retention action and never reuses purge authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryCollectionRequest {
    pub tenant_id: Uuid,
    pub context: ModuleCommandContext,
    pub reason: String,
    pub policy_snapshot_id: String,
    pub limit: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryCollectionResult {
    pub collected: u64,
    pub retained: u64,
    pub resumed: u64,
}

/// Binds an intentionally unbound restored settings instance to the exact
/// continuity-authorized successor installation. It is not a generic
/// attachment operation and cannot clear the original tombstone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryBindRequest {
    pub tenant_id: Uuid,
    pub recovery_point_id: Uuid,
    pub target_installation_id: Uuid,
    pub expected_target_installation_revision: u64,
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryBindResult {
    pub bind_operation_id: Uuid,
    pub recovery_point_id: Uuid,
    pub target_installation_id: Uuid,
    pub settings_instance_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryCollectionCandidate {
    pub recovery_point_id: Uuid,
    pub tenant_id: Uuid,
    pub data_owner_id: Uuid,
    pub settings_instance_id: Uuid,
    pub retention_revision: u64,
    pub retain_until: DateTime<Utc>,
    pub legal_hold: bool,
    pub audit_hold: bool,
    pub incident_hold: bool,
}

#[async_trait]
pub trait ArtifactSettingsRecoveryCollectionPolicy: Send + Sync {
    fn snapshot_id(&self) -> &str;

    async fn may_collect(
        &self,
        candidate: &ArtifactSettingsRecoveryCollectionCandidate,
    ) -> Result<bool, ArtifactSettingsRecoveryError>;
}

/// Associated data passed to the host KMS/envelope implementation. A cipher
/// must bind all fields while encrypting and decrypting to prevent ciphertext
/// substitution across tenant, owner, instance, schema, or recovery point.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryCipherContext {
    pub tenant_id: Uuid,
    pub recovery_point_id: Uuid,
    pub data_owner_id: Uuid,
    pub settings_instance_id: Uuid,
    pub schema_digest: String,
    pub descriptor_digest: String,
    pub value_digest: String,
    pub secret_handle_digest: String,
}

/// Opaque authenticated ciphertext from the host-owned KMS/envelope boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryCiphertext {
    pub key_version: String,
    pub bytes: Vec<u8>,
}

/// Immutable recovery metadata supplied to host policy for a destructive
/// purge or a restore. The policy port receives no ciphertext, plaintext,
/// secret values, grants, or external-resolver identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSettingsRecoveryAuthorizationContext {
    pub recovery_point_id: Uuid,
    pub tenant_id: Uuid,
    pub installation_id: Uuid,
    pub data_owner_id: Uuid,
    pub settings_instance_id: Uuid,
    pub settings_revision: u64,
    pub schema_digest: String,
    pub descriptor_digest: String,
    pub value_digest: String,
    pub key_version: String,
    pub retention_revision: u64,
    pub policy_snapshot_id: String,
    pub secret_handle_digest: String,
    pub retain_until: DateTime<Utc>,
    pub legal_hold: bool,
    pub audit_hold: bool,
    pub incident_hold: bool,
    pub state: String,
    /// The exact fresh instance created by a prior restore, if any. This lets
    /// the host authorize a one-time continuity bind without querying mutable
    /// owner tables outside the recovery transaction.
    pub restored_installation_id: Option<Uuid>,
    pub restored_settings_instance_id: Option<Uuid>,
}

/// The host must provide authenticated encryption and decryption backed by a
/// durable KMS or equivalent key-management boundary. This module never
/// accepts raw keys and fails closed if this port refuses either operation.
#[async_trait]
pub trait ArtifactSettingsRecoveryCipher: Send + Sync {
    async fn encrypt(
        &self,
        context: &ArtifactSettingsRecoveryCipherContext,
        canonical_settings: &[u8],
    ) -> Result<ArtifactSettingsRecoveryCiphertext, ArtifactSettingsRecoveryError>;

    async fn decrypt(
        &self,
        context: &ArtifactSettingsRecoveryCipherContext,
        ciphertext: &ArtifactSettingsRecoveryCiphertext,
    ) -> Result<Vec<u8>, ArtifactSettingsRecoveryError>;

    /// Rewraps an existing authenticated ciphertext under the host's currently
    /// approved key version without exposing a raw KMS key to this module.
    async fn rewrap(
        &self,
        context: &ArtifactSettingsRecoveryCipherContext,
        ciphertext: &ArtifactSettingsRecoveryCiphertext,
    ) -> Result<ArtifactSettingsRecoveryCiphertext, ArtifactSettingsRecoveryError>;
}

/// Host policy boundary for separately privileged recovery-point, purge, and
/// restore operations. Implementations must prove actor authority, lifecycle,
/// work/traffic fences, retention, legal/audit/incident holds, and secret
/// handle continuity without returning secret values to this owner service.
#[async_trait]
pub trait ArtifactSettingsRecoveryAuthorizer: Send + Sync {
    async fn authorize_recovery_point(
        &self,
        request: &ArtifactSettingsRecoveryPointCreateRequest,
    ) -> Result<ArtifactSettingsRecoveryRetention, ArtifactSettingsRecoveryError>;

    async fn authorize_purge(
        &self,
        request: &ArtifactSettingsPurgeRequest,
        recovery: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<(), ArtifactSettingsRecoveryError>;

    async fn authorize_restore(
        &self,
        request: &ArtifactSettingsRestoreRequest,
        recovery: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<(), ArtifactSettingsRecoveryError>;

    async fn authorize_retention_update(
        &self,
        request: &ArtifactSettingsRecoveryRetentionUpdateRequest,
        recovery: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<ArtifactSettingsRecoveryRetentionUpdate, ArtifactSettingsRecoveryError>;

    async fn authorize_rewrap(
        &self,
        request: &ArtifactSettingsRecoveryRewrapRequest,
        recovery: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<(), ArtifactSettingsRecoveryError>;

    async fn authorize_collection(
        &self,
        request: &ArtifactSettingsRecoveryCollectionRequest,
    ) -> Result<(), ArtifactSettingsRecoveryError>;

    async fn authorize_bind(
        &self,
        request: &ArtifactSettingsRecoveryBindRequest,
        recovery: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<(), ArtifactSettingsRecoveryError>;
}

/// Durable owner service for settings recovery points and destructive
/// lifecycle. It is intentionally not a sandbox capability and can only be
/// constructed by host composition with explicit policy and encryption ports.
#[derive(Clone)]
pub struct SeaOrmArtifactSettingsRecoveryService<A, C> {
    db: DatabaseConnection,
    authorizer: A,
    cipher: C,
    validators: Arc<ArtifactSchemaValidatorCache>,
    infrastructure: ControlPlaneInfrastructure,
}

impl<A, C> SeaOrmArtifactSettingsRecoveryService<A, C>
where
    A: ArtifactSettingsRecoveryAuthorizer,
    C: ArtifactSettingsRecoveryCipher,
{
    pub fn new(db: DatabaseConnection, authorizer: A, cipher: C) -> Self {
        let infrastructure = ControlPlaneInfrastructure::for_database(db.clone());
        Self::with_infrastructure(
            db,
            authorizer,
            cipher,
            Arc::new(ArtifactSchemaValidatorCache::default()),
            infrastructure,
        )
    }

    pub(crate) fn with_infrastructure(
        db: DatabaseConnection,
        authorizer: A,
        cipher: C,
        validators: Arc<ArtifactSchemaValidatorCache>,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            authorizer,
            cipher,
            validators,
            infrastructure,
        }
    }

    pub async fn create_recovery_point(
        &self,
        request: ArtifactSettingsRecoveryPointCreateRequest,
    ) -> Result<ArtifactSettingsRecoveryPoint, ArtifactSettingsRecoveryError> {
        validate_recovery_point_request(&request)?;
        let retention = self.authorizer.authorize_recovery_point(&request).await?;
        validate_retention(&retention, self.infrastructure.now())?;

        if let Some(existing) = self.find_recovery_operation(&request).await? {
            return Ok(existing);
        }

        let source = self.load_retired_source(&request).await?;
        let recovery_point_id = self.infrastructure.new_id();
        let context = source.cipher_context(recovery_point_id, &retention);
        let canonical_settings = canonical_settings_bytes(&source.settings)?;
        let ciphertext = self.cipher.encrypt(&context, &canonical_settings).await?;
        validate_ciphertext(&ciphertext)?;

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        if let Some(existing) =
            find_recovery_operation_in(&transaction, &request, transaction.get_database_backend())
                .await?
        {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(existing);
        }

        let locked_source = load_retired_source_in(
            &transaction,
            request.tenant_id,
            request.installation_id,
            request.expected_installation_revision,
            request.expected_settings_revision,
            &self.validators,
        )
        .await?;
        if locked_source.identity() != source.identity()
            || locked_source.value_digest != source.value_digest
        {
            return Err(ArtifactSettingsRecoveryError::RecoveryPrecondition);
        }

        let backend = transaction.get_database_backend();
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_settings_recovery_points (recovery_point_id, tenant_id, installation_id, data_owner_id, settings_instance_id, settings_revision, schema_digest, descriptor_digest, value_digest, key_version, ciphertext, retention_revision, policy_snapshot_id, secret_handle_digest, retain_until, legal_hold, audit_hold, incident_hold, state, created_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 1, {}, {}, {}, {}, {}, {}, 'ready', {})",
                    placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
                    placeholder(backend, 4), placeholder(backend, 5), placeholder(backend, 6),
                    placeholder(backend, 7), placeholder(backend, 8), placeholder(backend, 9),
                    placeholder(backend, 10), placeholder(backend, 11), placeholder(backend, 12),
                    placeholder(backend, 13), placeholder(backend, 14), placeholder(backend, 15),
                    placeholder(backend, 16), placeholder(backend, 17), now_expression(backend),
                ),
                vec![
                    uuid_value(recovery_point_id, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(source.installation_id, backend),
                    uuid_value(source.data_owner_id, backend),
                    uuid_value(source.settings_instance_id, backend),
                    revision_value(source.settings_revision)?,
                    source.schema_digest.clone().into(),
                    source.descriptor_digest.clone().into(),
                    source.value_digest.clone().into(),
                    ciphertext.key_version.clone().into(),
                    bytes_value(ciphertext.bytes.clone()),
                    retention.policy_snapshot_id.clone().into(),
                    retention.secret_handle_digest.clone().into(),
                    datetime_value(retention.retain_until, backend),
                    bool_value(retention.legal_hold, backend),
                    bool_value(retention.audit_hold, backend),
                    bool_value(retention.incident_hold, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_settings_recovery_operations (operation_id, tenant_id, installation_id, expected_installation_revision, expected_settings_revision, recovery_point_id, actor_id, trace_id, correlation_id, reason, idempotency_key, committed_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                    placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
                    placeholder(backend, 4), placeholder(backend, 5), placeholder(backend, 6),
                    placeholder(backend, 7), placeholder(backend, 8), placeholder(backend, 9),
                    placeholder(backend, 10), placeholder(backend, 11), now_expression(backend),
                ),
                vec![
                    uuid_value(self.infrastructure.new_id(), backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.installation_id, backend),
                    revision_value(request.expected_installation_revision)?,
                    revision_value(request.expected_settings_revision)?,
                    uuid_value(recovery_point_id, backend),
                    uuid_value(request.context.actor_id, backend),
                    request.context.trace_id.clone().into(),
                    uuid_value(request.context.correlation_id, backend),
                    request.reason.clone().into(),
                    uuid_value(request.context.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
                    DomainEvent::ModuleArtifactSettingsRecoveryPointCreated {
                        recovery_point_id,
                        tenant_id: request.tenant_id,
                        installation_id: request.installation_id,
                        settings_instance_id: source.settings_instance_id,
                        settings_revision: source.settings_revision,
                    },
                ),
            )
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(recovery_point_from_source(
            recovery_point_id,
            &source,
            ciphertext.key_version,
            retention,
        ))
    }

    pub async fn purge(
        &self,
        request: ArtifactSettingsPurgeRequest,
    ) -> Result<ArtifactSettingsPurgeResult, ArtifactSettingsRecoveryError> {
        validate_purge_request(&request)?;
        let recovery = self
            .load_recovery_material(request.tenant_id, request.recovery_point_id)
            .await?;
        self.authorizer
            .authorize_purge(&request, &recovery.authorization_context())
            .await?;
        if let Some(existing) = self.find_purge_operation(&request).await? {
            return Ok(existing);
        }
        if recovery.installation_id != request.installation_id
            || recovery.settings_revision != request.expected_settings_revision
        {
            return Err(ArtifactSettingsRecoveryError::PurgePrecondition);
        }
        let ciphertext = recovery.ciphertext()?;
        let plaintext = self
            .cipher
            .decrypt(&recovery.cipher_context(), &ciphertext)
            .await?;
        let recovered_settings = parse_canonical_settings(&plaintext)?;
        if settings_digest(&recovered_settings)? != recovery.value_digest {
            return Err(ArtifactSettingsRecoveryError::CiphertextIntegrity);
        }

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let backend = transaction.get_database_backend();
        if let Some(existing) = find_purge_operation_in(&transaction, &request, backend).await? {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(existing);
        }
        let locked_recovery = load_recovery_material_in(
            &transaction,
            request.tenant_id,
            request.recovery_point_id,
            true,
        )
        .await?;
        if !locked_recovery.matches_authorized_recovery(&recovery) {
            return Err(ArtifactSettingsRecoveryError::PurgePrecondition);
        }
        if locked_recovery.state != "ready"
            || locked_recovery.restored_at.is_some()
            || locked_recovery.retain_until <= self.infrastructure.now()
        {
            return Err(ArtifactSettingsRecoveryError::RecoveryUnavailable);
        }
        let source = load_retired_source_in(
            &transaction,
            request.tenant_id,
            request.installation_id,
            request.expected_installation_revision,
            request.expected_settings_revision,
            &self.validators,
        )
        .await?;
        if source.identity() != locked_recovery.source_identity()
            || source.value_digest != locked_recovery.value_digest
            || source.settings != recovered_settings
        {
            return Err(ArtifactSettingsRecoveryError::PurgePrecondition);
        }
        ensure_no_active_owner_binding(&transaction, request.tenant_id, source.data_owner_id)
            .await?;
        let tombstone_revision = next_tombstone_revision(
            &transaction,
            request.tenant_id,
            source.data_owner_id,
            source.settings_instance_id,
        )
        .await?;
        let deleted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_settings_instances WHERE tenant_id = {} AND data_owner_id = {} AND settings_instance_id = {} AND revision = {}",
                    placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3), placeholder(backend, 4),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(source.data_owner_id, backend),
                    uuid_value(source.settings_instance_id, backend),
                    revision_value(request.expected_settings_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if deleted.rows_affected() != 1 {
            return Err(ArtifactSettingsRecoveryError::PurgePrecondition);
        }
        let purge_operation_id = self.infrastructure.new_id();
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_settings_purge_operations (operation_id, tenant_id, installation_id, recovery_point_id, expected_installation_revision, expected_settings_revision, tombstone_revision, actor_id, trace_id, correlation_id, reason, idempotency_key, committed_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                    placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3), placeholder(backend, 4),
                    placeholder(backend, 5), placeholder(backend, 6), placeholder(backend, 7), placeholder(backend, 8),
                    placeholder(backend, 9), placeholder(backend, 10), placeholder(backend, 11),
                    placeholder(backend, 12), now_expression(backend),
                ),
                vec![
                    uuid_value(purge_operation_id, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.installation_id, backend),
                    uuid_value(request.recovery_point_id, backend),
                    revision_value(request.expected_installation_revision)?,
                    revision_value(request.expected_settings_revision)?,
                    revision_value(tombstone_revision)?,
                    uuid_value(request.context.actor_id, backend),
                    request.context.trace_id.clone().into(),
                    uuid_value(request.context.correlation_id, backend),
                    request.reason.clone().into(),
                    uuid_value(request.context.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_settings_tombstones (tenant_id, data_owner_id, settings_instance_id, tombstone_revision, recovery_point_id, purge_operation_id, purged_at) VALUES ({}, {}, {}, {}, {}, {}, {})",
                    placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
                    placeholder(backend, 4), placeholder(backend, 5), placeholder(backend, 6), now_expression(backend),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(source.data_owner_id, backend),
                    uuid_value(source.settings_instance_id, backend),
                    revision_value(tombstone_revision)?,
                    uuid_value(request.recovery_point_id, backend),
                    uuid_value(purge_operation_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
                    DomainEvent::ModuleArtifactSettingsPurged {
                        recovery_point_id: request.recovery_point_id,
                        tenant_id: request.tenant_id,
                        installation_id: request.installation_id,
                        settings_instance_id: source.settings_instance_id,
                        tombstone_revision,
                    },
                ),
            )
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactSettingsPurgeResult {
            purge_operation_id,
            recovery_point_id: request.recovery_point_id,
            tombstone_revision,
        })
    }

    pub async fn restore(
        &self,
        request: ArtifactSettingsRestoreRequest,
    ) -> Result<ArtifactSettingsRestoreResult, ArtifactSettingsRecoveryError> {
        validate_restore_request(&request)?;
        let recovery = self
            .load_recovery_material(request.tenant_id, request.recovery_point_id)
            .await?;
        self.authorizer
            .authorize_restore(&request, &recovery.authorization_context())
            .await?;
        if let Some(existing) = self.find_restore_operation(&request).await? {
            return Ok(existing);
        }
        let ciphertext = recovery.ciphertext()?;
        let plaintext = self
            .cipher
            .decrypt(&recovery.cipher_context(), &ciphertext)
            .await?;
        let recovered_settings = parse_canonical_settings(&plaintext)?;
        if settings_digest(&recovered_settings)? != recovery.value_digest {
            return Err(ArtifactSettingsRecoveryError::CiphertextIntegrity);
        }

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let backend = transaction.get_database_backend();
        if let Some(existing) = find_restore_operation_in(&transaction, &request, backend).await? {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(existing);
        }
        let locked_recovery = load_recovery_material_in(
            &transaction,
            request.tenant_id,
            request.recovery_point_id,
            true,
        )
        .await?;
        if !locked_recovery.matches_authorized_recovery(&recovery)
            || locked_recovery.state != "ready"
            || locked_recovery.restored_at.is_some()
            || locked_recovery.retain_until <= self.infrastructure.now()
        {
            return Err(ArtifactSettingsRecoveryError::RecoveryUnavailable);
        }
        ensure_tombstone(
            &transaction,
            request.tenant_id,
            locked_recovery.data_owner_id,
            locked_recovery.settings_instance_id,
            request.recovery_point_id,
        )
        .await?;
        let source =
            load_source_for_recovery(&transaction, request.tenant_id, &locked_recovery).await?;
        validate_settings(&self.validators, &source, &recovered_settings)?;
        let settings_instance_id = self.infrastructure.new_id();
        if settings_instance_id == locked_recovery.settings_instance_id {
            return Err(ArtifactSettingsRecoveryError::RestorePrecondition);
        }
        if let Some(target_installation_id) = request.target_installation_id {
            let expected_target_installation_revision = request
                .expected_target_installation_revision
                .ok_or(ArtifactSettingsRecoveryError::RestorePrecondition)?;
            let target = load_restore_target(
                &transaction,
                request.tenant_id,
                target_installation_id,
                locked_recovery.data_owner_id,
                &source,
                &self.validators,
                &recovered_settings,
            )
            .await?;
            if target.schema_digest != locked_recovery.schema_digest
                || target.admission_revision != expected_target_installation_revision
            {
                return Err(ArtifactSettingsRecoveryError::RestorePrecondition);
            }
            let updated = transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE module_artifact_installations SET settings_instance_id = {} WHERE installation_id = {} AND data_owner_id = {} AND settings_instance_id = {}",
                        placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3), placeholder(backend, 4),
                    ),
                    vec![
                        uuid_value(settings_instance_id, backend),
                        uuid_value(target_installation_id, backend),
                        uuid_value(locked_recovery.data_owner_id, backend),
                        uuid_value(target.settings_instance_id, backend),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if updated.rows_affected() != 1 {
                return Err(ArtifactSettingsRecoveryError::RestorePrecondition);
            }
        }
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_settings_instances (tenant_id, data_owner_id, settings_instance_id, schema_digest, settings, revision, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, 1, {}, {})",
                    placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3), placeholder(backend, 4),
                    placeholder(backend, 5), now_expression(backend), now_expression(backend),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(locked_recovery.data_owner_id, backend),
                    uuid_value(settings_instance_id, backend),
                    locked_recovery.schema_digest.clone().into(),
                    SqlValue::Json(Some(Box::new(recovered_settings))),
                ],
            ))
            .await
            .map_err(storage_error)?;
        let restore_operation_id = self.infrastructure.new_id();
        let recovery_marked_restored = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_settings_recovery_points SET restored_at = {}, restored_installation_id = {}, restored_settings_instance_id = {} WHERE recovery_point_id = {} AND tenant_id = {} AND restored_at IS NULL",
                    now_expression(backend), placeholder(backend, 1), placeholder(backend, 2),
                    placeholder(backend, 3), placeholder(backend, 4),
                ),
                vec![
                    optional_uuid_value(request.target_installation_id, backend),
                    uuid_value(settings_instance_id, backend),
                    uuid_value(request.recovery_point_id, backend),
                    uuid_value(request.tenant_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if recovery_marked_restored.rows_affected() != 1 {
            return Err(ArtifactSettingsRecoveryError::RestorePrecondition);
        }
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_settings_restore_operations (operation_id, tenant_id, recovery_point_id, target_installation_id, expected_target_installation_revision, settings_instance_id, actor_id, trace_id, correlation_id, reason, idempotency_key, committed_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                    placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3), placeholder(backend, 4),
                    placeholder(backend, 5), placeholder(backend, 6), placeholder(backend, 7), placeholder(backend, 8),
                    placeholder(backend, 9), placeholder(backend, 10), placeholder(backend, 11), now_expression(backend),
                ),
                vec![
                    uuid_value(restore_operation_id, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.recovery_point_id, backend),
                    optional_uuid_value(request.target_installation_id, backend),
                    optional_revision_value(request.expected_target_installation_revision)?,
                    uuid_value(settings_instance_id, backend),
                    uuid_value(request.context.actor_id, backend),
                    request.context.trace_id.clone().into(),
                    uuid_value(request.context.correlation_id, backend),
                    request.reason.clone().into(),
                    uuid_value(request.context.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
                    DomainEvent::ModuleArtifactSettingsRestored {
                        recovery_point_id: request.recovery_point_id,
                        tenant_id: request.tenant_id,
                        target_installation_id: request.target_installation_id,
                        settings_instance_id,
                    },
                ),
            )
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactSettingsRestoreResult {
            restore_operation_id,
            recovery_point_id: request.recovery_point_id,
            settings_instance_id,
            target_installation_id: request.target_installation_id,
        })
    }

    pub async fn update_retention(
        &self,
        request: ArtifactSettingsRecoveryRetentionUpdateRequest,
    ) -> Result<ArtifactSettingsRecoveryRetentionUpdateResult, ArtifactSettingsRecoveryError> {
        validate_retention_update_request(&request)?;
        let request_digest = request_digest(&request)?;
        let recovery = self
            .load_recovery_material(request.tenant_id, request.recovery_point_id)
            .await?;
        let retention = self
            .authorizer
            .authorize_retention_update(&request, &recovery.authorization_context())
            .await?;
        validate_retention_update(&retention)?;

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let backend = transaction.get_database_backend();
        if let Some(existing) =
            find_retention_operation_in(&transaction, &request, &request_digest, backend).await?
        {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(existing);
        }
        let locked = load_recovery_material_in(
            &transaction,
            request.tenant_id,
            request.recovery_point_id,
            true,
        )
        .await?;
        if !locked.matches_authorized_recovery(&recovery)
            || locked.state != "ready"
            || locked.retention_revision != request.expected_retention_revision
        {
            return Err(ArtifactSettingsRecoveryError::RetentionPrecondition);
        }
        if retention.retain_until < locked.retain_until {
            return Err(ArtifactSettingsRecoveryError::RetentionPrecondition);
        }
        if (locked.legal_hold && !retention.legal_hold)
            || (locked.audit_hold && !retention.audit_hold)
            || (locked.incident_hold && !retention.incident_hold)
        {
            return Err(ArtifactSettingsRecoveryError::RetentionPrecondition);
        }
        let retention_revision = locked
            .retention_revision
            .checked_add(1)
            .ok_or(ArtifactSettingsRecoveryError::RetentionPrecondition)?;
        let updated = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_settings_recovery_points SET retention_revision = {}, policy_snapshot_id = {}, retain_until = {}, legal_hold = {}, audit_hold = {}, incident_hold = {} WHERE tenant_id = {} AND recovery_point_id = {} AND retention_revision = {} AND state = 'ready'",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                    placeholder(backend, 9),
                ),
                vec![
                    revision_value(retention_revision)?,
                    retention.policy_snapshot_id.clone().into(),
                    datetime_value(retention.retain_until, backend),
                    bool_value(retention.legal_hold, backend),
                    bool_value(retention.audit_hold, backend),
                    bool_value(retention.incident_hold, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.recovery_point_id, backend),
                    revision_value(request.expected_retention_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(ArtifactSettingsRecoveryError::RetentionPrecondition);
        }
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_settings_recovery_retention_operations (operation_id, tenant_id, recovery_point_id, idempotency_key, request_digest, expected_retention_revision, retention_revision, retain_until, legal_hold, audit_hold, incident_hold, policy_snapshot_id, actor_id, trace_id, correlation_id, reason, committed_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
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
                    placeholder(backend, 13),
                    placeholder(backend, 14),
                    placeholder(backend, 15),
                    placeholder(backend, 16),
                    now_expression(backend),
                ),
                vec![
                    uuid_value(self.infrastructure.new_id(), backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.recovery_point_id, backend),
                    uuid_value(request.context.idempotency_key, backend),
                    request_digest.into(),
                    revision_value(request.expected_retention_revision)?,
                    revision_value(retention_revision)?,
                    datetime_value(retention.retain_until, backend),
                    bool_value(retention.legal_hold, backend),
                    bool_value(retention.audit_hold, backend),
                    bool_value(retention.incident_hold, backend),
                    retention.policy_snapshot_id.clone().into(),
                    uuid_value(request.context.actor_id, backend),
                    request.context.trace_id.clone().into(),
                    uuid_value(request.context.correlation_id, backend),
                    request.reason.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
                    DomainEvent::ModuleArtifactSettingsRecoveryRetentionUpdated {
                        recovery_point_id: request.recovery_point_id,
                        tenant_id: request.tenant_id,
                        retention_revision,
                        retain_until: retention.retain_until,
                        legal_hold: retention.legal_hold,
                        audit_hold: retention.audit_hold,
                        incident_hold: retention.incident_hold,
                    },
                ),
            )
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(recovery.retention_update_result(retention_revision, retention))
    }

    pub async fn rewrap(
        &self,
        request: ArtifactSettingsRecoveryRewrapRequest,
    ) -> Result<ArtifactSettingsRecoveryRewrapResult, ArtifactSettingsRecoveryError> {
        validate_rewrap_request(&request)?;
        let recovery = self
            .load_recovery_material(request.tenant_id, request.recovery_point_id)
            .await?;
        self.authorizer
            .authorize_rewrap(&request, &recovery.authorization_context())
            .await?;
        if let Some(existing) = self.find_rewrap_operation(&request).await? {
            return Ok(existing);
        }
        let ciphertext = recovery.ciphertext()?;
        let rewrapped = self
            .cipher
            .rewrap(&recovery.cipher_context(), &ciphertext)
            .await?;
        validate_ciphertext(&rewrapped)?;
        let rewrapped_plaintext = self
            .cipher
            .decrypt(&recovery.cipher_context(), &rewrapped)
            .await?;
        let rewrapped_settings = parse_canonical_settings(&rewrapped_plaintext)?;
        if settings_digest(&rewrapped_settings)? != recovery.value_digest {
            return Err(ArtifactSettingsRecoveryError::CiphertextIntegrity);
        }

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let backend = transaction.get_database_backend();
        if let Some(existing) = find_rewrap_operation_in(&transaction, &request, backend).await? {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(existing);
        }
        let locked = load_recovery_material_in(
            &transaction,
            request.tenant_id,
            request.recovery_point_id,
            true,
        )
        .await?;
        if !locked.matches_authorized_recovery(&recovery) || locked.state != "ready" {
            return Err(ArtifactSettingsRecoveryError::RewrapPrecondition);
        }
        let updated = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_settings_recovery_points SET key_version = {}, ciphertext = {} WHERE tenant_id = {} AND recovery_point_id = {} AND key_version = {} AND ciphertext = {} AND state = 'ready'",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                ),
                vec![
                    rewrapped.key_version.clone().into(),
                    bytes_value(rewrapped.bytes.clone()),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.recovery_point_id, backend),
                    recovery.key_version.clone().into(),
                    bytes_value(ciphertext.bytes),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(ArtifactSettingsRecoveryError::RewrapPrecondition);
        }
        let rewrap_operation_id = self.infrastructure.new_id();
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_settings_recovery_rewrap_operations (operation_id, tenant_id, recovery_point_id, idempotency_key, previous_key_version, key_version, actor_id, trace_id, correlation_id, reason, committed_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
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
                    now_expression(backend),
                ),
                vec![
                    uuid_value(rewrap_operation_id, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.recovery_point_id, backend),
                    uuid_value(request.context.idempotency_key, backend),
                    recovery.key_version.clone().into(),
                    rewrapped.key_version.clone().into(),
                    uuid_value(request.context.actor_id, backend),
                    request.context.trace_id.clone().into(),
                    uuid_value(request.context.correlation_id, backend),
                    request.reason.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
                    DomainEvent::ModuleArtifactSettingsRecoveryRewrapped {
                        recovery_point_id: request.recovery_point_id,
                        tenant_id: request.tenant_id,
                        previous_key_version: recovery.key_version.clone(),
                        key_version: rewrapped.key_version.clone(),
                    },
                ),
            )
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactSettingsRecoveryRewrapResult {
            rewrap_operation_id,
            recovery_point_id: request.recovery_point_id,
            previous_key_version: recovery.key_version,
            key_version: rewrapped.key_version,
        })
    }

    pub async fn collect(
        &self,
        request: ArtifactSettingsRecoveryCollectionRequest,
        policy: &dyn ArtifactSettingsRecoveryCollectionPolicy,
    ) -> Result<ArtifactSettingsRecoveryCollectionResult, ArtifactSettingsRecoveryError> {
        validate_collection_request(&request)?;
        self.authorizer.authorize_collection(&request).await?;
        if policy.snapshot_id() != request.policy_snapshot_id {
            return Err(ArtifactSettingsRecoveryError::CollectionPrecondition);
        }

        let candidates = self.load_collection_candidates(request.tenant_id).await?;
        let now = self.infrastructure.now();
        let mut result = ArtifactSettingsRecoveryCollectionResult::default();
        for candidate in candidates {
            if result.collected >= u64::from(request.limit) {
                break;
            }
            if candidate.status == "collecting" {
                // The authorization decision was durably recorded when this
                // candidate entered `collecting`. Resuming the terminal
                // ciphertext deletion must not depend on a mutable policy
                // evaluation, otherwise a crash between the two phases could
                // leave a recovery point permanently uncollectable.
                result.resumed = result
                    .resumed
                    .checked_add(1)
                    .ok_or(ArtifactSettingsRecoveryError::CollectionPrecondition)?;
            } else if candidate.candidate.legal_hold
                || candidate.candidate.audit_hold
                || candidate.candidate.incident_hold
                || candidate.candidate.retain_until > now
                || !policy.may_collect(&candidate.candidate).await?
            {
                result.retained = result
                    .retained
                    .checked_add(1)
                    .ok_or(ArtifactSettingsRecoveryError::CollectionPrecondition)?;
                continue;
            }

            let work = self
                .start_collection(&request, &candidate.candidate, now)
                .await?;
            self.finish_collection(&work).await?;
            result.collected = result
                .collected
                .checked_add(1)
                .ok_or(ArtifactSettingsRecoveryError::CollectionPrecondition)?;
        }
        Ok(result)
    }

    pub async fn bind(
        &self,
        request: ArtifactSettingsRecoveryBindRequest,
    ) -> Result<ArtifactSettingsRecoveryBindResult, ArtifactSettingsRecoveryError> {
        validate_bind_request(&request)?;
        let recovery = self
            .load_recovery_material(request.tenant_id, request.recovery_point_id)
            .await?;
        self.authorizer
            .authorize_bind(&request, &recovery.authorization_context())
            .await?;
        if let Some(existing) = self.find_bind_operation(&request).await? {
            return Ok(existing);
        }

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let backend = transaction.get_database_backend();
        if let Some(existing) = find_bind_operation_in(&transaction, &request, backend).await? {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(existing);
        }
        let locked = load_recovery_material_in(
            &transaction,
            request.tenant_id,
            request.recovery_point_id,
            true,
        )
        .await?;
        if !locked.matches_authorized_recovery(&recovery)
            || locked.state != "ready"
            || locked.restored_at.is_none()
            || locked.restored_installation_id.is_some()
            || locked.restored_settings_instance_id.is_none()
        {
            return Err(ArtifactSettingsRecoveryError::BindPrecondition);
        }
        let settings_instance_id = locked
            .restored_settings_instance_id
            .ok_or(ArtifactSettingsRecoveryError::BindPrecondition)?;
        let source = load_source_for_recovery(&transaction, request.tenant_id, &locked).await?;
        let restored_settings = load_settings_instance(
            &transaction,
            request.tenant_id,
            locked.data_owner_id,
            settings_instance_id,
        )
        .await?;
        if restored_settings.schema_digest != locked.schema_digest {
            return Err(ArtifactSettingsRecoveryError::BindPrecondition);
        }
        let target = load_restore_target(
            &transaction,
            request.tenant_id,
            request.target_installation_id,
            locked.data_owner_id,
            &source,
            &self.validators,
            &restored_settings.value,
        )
        .await?;
        if target.schema_digest != locked.schema_digest
            || target.admission_revision != request.expected_target_installation_revision
        {
            return Err(ArtifactSettingsRecoveryError::BindPrecondition);
        }
        let updated = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_installations SET settings_instance_id = {} WHERE installation_id = {} AND data_owner_id = {} AND settings_instance_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    uuid_value(settings_instance_id, backend),
                    uuid_value(request.target_installation_id, backend),
                    uuid_value(locked.data_owner_id, backend),
                    uuid_value(target.settings_instance_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(ArtifactSettingsRecoveryError::BindPrecondition);
        }
        let bound = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_settings_recovery_points SET restored_installation_id = {} WHERE tenant_id = {} AND recovery_point_id = {} AND restored_at IS NOT NULL AND restored_installation_id IS NULL AND restored_settings_instance_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    uuid_value(request.target_installation_id, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.recovery_point_id, backend),
                    uuid_value(settings_instance_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if bound.rows_affected() != 1 {
            return Err(ArtifactSettingsRecoveryError::BindPrecondition);
        }
        let bind_operation_id = self.infrastructure.new_id();
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_settings_recovery_bind_operations (operation_id, tenant_id, recovery_point_id, target_installation_id, expected_target_installation_revision, settings_instance_id, actor_id, trace_id, correlation_id, reason, idempotency_key, committed_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
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
                    now_expression(backend),
                ),
                vec![
                    uuid_value(bind_operation_id, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.recovery_point_id, backend),
                    uuid_value(request.target_installation_id, backend),
                    revision_value(request.expected_target_installation_revision)?,
                    uuid_value(settings_instance_id, backend),
                    uuid_value(request.context.actor_id, backend),
                    request.context.trace_id.clone().into(),
                    uuid_value(request.context.correlation_id, backend),
                    request.reason.clone().into(),
                    uuid_value(request.context.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
                    DomainEvent::ModuleArtifactSettingsRecoveryBound {
                        recovery_point_id: request.recovery_point_id,
                        tenant_id: request.tenant_id,
                        target_installation_id: request.target_installation_id,
                        settings_instance_id,
                    },
                ),
            )
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactSettingsRecoveryBindResult {
            bind_operation_id,
            recovery_point_id: request.recovery_point_id,
            target_installation_id: request.target_installation_id,
            settings_instance_id,
        })
    }

    async fn load_collection_candidates(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<SettingsRecoveryCollectionCandidate>, ArtifactSettingsRecoveryError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let backend = transaction.get_database_backend();
        let rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT recovery_point_id, tenant_id, data_owner_id, settings_instance_id, retention_revision, retain_until, legal_hold, audit_hold, incident_hold, state FROM module_artifact_settings_recovery_points WHERE tenant_id = {} AND state IN ('ready', 'collecting') ORDER BY CASE WHEN state = 'collecting' THEN 0 ELSE 1 END, created_at ASC, recovery_point_id ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![
                    uuid_value(tenant_id, backend),
                    i64::from(MAX_COLLECTION_SCAN).into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        let candidates = rows
            .into_iter()
            .map(|row| settings_recovery_collection_candidate_from_row(row, backend))
            .collect::<Result<Vec<_>, _>>()?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(candidates)
    }

    async fn start_collection(
        &self,
        request: &ArtifactSettingsRecoveryCollectionRequest,
        candidate: &ArtifactSettingsRecoveryCollectionCandidate,
        now: DateTime<Utc>,
    ) -> Result<SettingsRecoveryCollectionWork, ArtifactSettingsRecoveryError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let backend = transaction.get_database_backend();
        let locked = load_recovery_material_in(
            &transaction,
            request.tenant_id,
            candidate.recovery_point_id,
            true,
        )
        .await?;
        if locked.state == "collecting" {
            let work = settings_recovery_collection_work_in(
                &transaction,
                request.tenant_id,
                candidate.recovery_point_id,
            )
            .await?;
            transaction.commit().await.map_err(storage_error)?;
            return Ok(work);
        }
        if locked.state != "ready"
            || locked.retention_revision != candidate.retention_revision
            || locked.legal_hold
            || locked.audit_hold
            || locked.incident_hold
            || locked.retain_until > now
        {
            return Err(ArtifactSettingsRecoveryError::CollectionPrecondition);
        }
        let collection_id = self.infrastructure.new_id();
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_settings_recovery_collections (collection_id, tenant_id, recovery_point_id, policy_snapshot_id, actor_id, trace_id, correlation_id, idempotency_key, reason, collecting_at, completed_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, NULL)",
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
                    uuid_value(collection_id, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(candidate.recovery_point_id, backend),
                    request.policy_snapshot_id.clone().into(),
                    uuid_value(request.context.actor_id, backend),
                    request.context.trace_id.clone().into(),
                    uuid_value(request.context.correlation_id, backend),
                    uuid_value(request.context.idempotency_key, backend),
                    request.reason.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        let updated = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_settings_recovery_points SET state = 'collecting' WHERE tenant_id = {} AND recovery_point_id = {} AND state = 'ready' AND retention_revision = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(candidate.recovery_point_id, backend),
                    revision_value(candidate.retention_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(ArtifactSettingsRecoveryError::CollectionPrecondition);
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(SettingsRecoveryCollectionWork {
            collection_id,
            tenant_id: request.tenant_id,
            recovery_point_id: candidate.recovery_point_id,
            context: request.context.clone(),
        })
    }

    async fn finish_collection(
        &self,
        work: &SettingsRecoveryCollectionWork,
    ) -> Result<(), ArtifactSettingsRecoveryError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, work.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let backend = transaction.get_database_backend();
        let locked =
            load_recovery_material_in(&transaction, work.tenant_id, work.recovery_point_id, true)
                .await?;
        if locked.state != "collecting"
            || settings_recovery_collection_work_in(
                &transaction,
                work.tenant_id,
                work.recovery_point_id,
            )
            .await?
                != *work
        {
            return Err(ArtifactSettingsRecoveryError::CollectionPrecondition);
        }
        let completed = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_settings_recovery_collections SET completed_at = {} WHERE tenant_id = {} AND recovery_point_id = {} AND collection_id = {} AND completed_at IS NULL",
                    now_expression(backend),
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    uuid_value(work.tenant_id, backend),
                    uuid_value(work.recovery_point_id, backend),
                    uuid_value(work.collection_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if completed.rows_affected() != 1 {
            return Err(ArtifactSettingsRecoveryError::CollectionPrecondition);
        }
        let collected = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_settings_recovery_points SET state = 'collected', ciphertext = {}, collected_at = {} WHERE tenant_id = {} AND recovery_point_id = {} AND state = 'collecting'",
                    null_bytes_value(),
                    now_expression(backend),
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![
                    uuid_value(work.tenant_id, backend),
                    uuid_value(work.recovery_point_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if collected.rows_affected() != 1 {
            return Err(ArtifactSettingsRecoveryError::CollectionPrecondition);
        }
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &work.context,
                    DomainEvent::ModuleArtifactSettingsRecoveryCollected {
                        collection_id: work.collection_id,
                        recovery_point_id: work.recovery_point_id,
                        tenant_id: work.tenant_id,
                    },
                ),
            )
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)
    }

    async fn find_recovery_operation(
        &self,
        request: &ArtifactSettingsRecoveryPointCreateRequest,
    ) -> Result<Option<ArtifactSettingsRecoveryPoint>, ArtifactSettingsRecoveryError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let result =
            find_recovery_operation_in(&transaction, request, transaction.get_database_backend())
                .await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(result)
    }

    async fn find_purge_operation(
        &self,
        request: &ArtifactSettingsPurgeRequest,
    ) -> Result<Option<ArtifactSettingsPurgeResult>, ArtifactSettingsRecoveryError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let result =
            find_purge_operation_in(&transaction, request, transaction.get_database_backend())
                .await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(result)
    }

    async fn find_restore_operation(
        &self,
        request: &ArtifactSettingsRestoreRequest,
    ) -> Result<Option<ArtifactSettingsRestoreResult>, ArtifactSettingsRecoveryError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let result =
            find_restore_operation_in(&transaction, request, transaction.get_database_backend())
                .await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(result)
    }

    async fn find_rewrap_operation(
        &self,
        request: &ArtifactSettingsRecoveryRewrapRequest,
    ) -> Result<Option<ArtifactSettingsRecoveryRewrapResult>, ArtifactSettingsRecoveryError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let result =
            find_rewrap_operation_in(&transaction, request, transaction.get_database_backend())
                .await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(result)
    }

    async fn find_bind_operation(
        &self,
        request: &ArtifactSettingsRecoveryBindRequest,
    ) -> Result<Option<ArtifactSettingsRecoveryBindResult>, ArtifactSettingsRecoveryError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let result =
            find_bind_operation_in(&transaction, request, transaction.get_database_backend())
                .await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(result)
    }

    async fn load_retired_source(
        &self,
        request: &ArtifactSettingsRecoveryPointCreateRequest,
    ) -> Result<SettingsSource, ArtifactSettingsRecoveryError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let source = load_retired_source_in(
            &transaction,
            request.tenant_id,
            request.installation_id,
            request.expected_installation_revision,
            request.expected_settings_revision,
            &self.validators,
        )
        .await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(source)
    }

    async fn load_recovery_material(
        &self,
        tenant_id: Uuid,
        recovery_point_id: Uuid,
    ) -> Result<RecoveryMaterial, ArtifactSettingsRecoveryError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
        let recovery =
            load_recovery_material_in(&transaction, tenant_id, recovery_point_id, false).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(recovery)
    }
}

#[derive(Clone, Debug)]
struct SettingsRecoveryCollectionCandidate {
    candidate: ArtifactSettingsRecoveryCollectionCandidate,
    status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SettingsRecoveryCollectionWork {
    collection_id: Uuid,
    tenant_id: Uuid,
    recovery_point_id: Uuid,
    context: ModuleCommandContext,
}

#[derive(Clone, Debug)]
struct SettingsSource {
    tenant_id: Uuid,
    installation_id: Uuid,
    data_owner_id: Uuid,
    registry: String,
    repository: String,
    settings_instance_id: Uuid,
    settings_revision: u64,
    schema_digest: String,
    descriptor_digest: String,
    descriptor: ModuleArtifactDescriptor,
    settings: Value,
    value_digest: String,
}

impl SettingsSource {
    fn identity(&self) -> (Uuid, Uuid, Uuid, u64, &str, &str) {
        (
            self.installation_id,
            self.data_owner_id,
            self.settings_instance_id,
            self.settings_revision,
            &self.schema_digest,
            &self.descriptor_digest,
        )
    }

    fn cipher_context(
        &self,
        recovery_point_id: Uuid,
        retention: &ArtifactSettingsRecoveryRetention,
    ) -> ArtifactSettingsRecoveryCipherContext {
        ArtifactSettingsRecoveryCipherContext {
            tenant_id: self.tenant_id,
            recovery_point_id,
            data_owner_id: self.data_owner_id,
            settings_instance_id: self.settings_instance_id,
            schema_digest: self.schema_digest.clone(),
            descriptor_digest: self.descriptor_digest.clone(),
            value_digest: self.value_digest.clone(),
            secret_handle_digest: retention.secret_handle_digest.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct RecoveryMaterial {
    tenant_id: Uuid,
    recovery_point_id: Uuid,
    installation_id: Uuid,
    data_owner_id: Uuid,
    settings_instance_id: Uuid,
    settings_revision: u64,
    schema_digest: String,
    descriptor_digest: String,
    value_digest: String,
    secret_handle_digest: String,
    key_version: String,
    ciphertext: Option<Vec<u8>>,
    retention_revision: u64,
    policy_snapshot_id: String,
    retain_until: DateTime<Utc>,
    legal_hold: bool,
    audit_hold: bool,
    incident_hold: bool,
    state: String,
    restored_at: Option<DateTime<Utc>>,
    restored_installation_id: Option<Uuid>,
    restored_settings_instance_id: Option<Uuid>,
}

impl RecoveryMaterial {
    fn identity(&self) -> (Uuid, Uuid, Uuid, Uuid, u64, &str, &str, &str, &str) {
        (
            self.recovery_point_id,
            self.installation_id,
            self.data_owner_id,
            self.settings_instance_id,
            self.settings_revision,
            &self.schema_digest,
            &self.descriptor_digest,
            &self.value_digest,
            &self.secret_handle_digest,
        )
    }

    /// Revalidates every policy-visible and ciphertext-visible fact that was
    /// authorized before the transaction began. This closes a TOCTOU window
    /// between host authorization/KMS work and the owner row lock.
    fn matches_authorized_recovery(&self, authorized: &Self) -> bool {
        self.identity() == authorized.identity()
            && self.key_version == authorized.key_version
            && self.ciphertext == authorized.ciphertext
            && self.retention_revision == authorized.retention_revision
            && self.policy_snapshot_id == authorized.policy_snapshot_id
            && self.retain_until == authorized.retain_until
            && self.legal_hold == authorized.legal_hold
            && self.audit_hold == authorized.audit_hold
            && self.incident_hold == authorized.incident_hold
            && self.state == authorized.state
            && self.restored_at == authorized.restored_at
            && self.restored_installation_id == authorized.restored_installation_id
            && self.restored_settings_instance_id == authorized.restored_settings_instance_id
    }

    fn source_identity(&self) -> (Uuid, Uuid, Uuid, u64, &str, &str) {
        (
            self.installation_id,
            self.data_owner_id,
            self.settings_instance_id,
            self.settings_revision,
            &self.schema_digest,
            &self.descriptor_digest,
        )
    }

    fn cipher_context(&self) -> ArtifactSettingsRecoveryCipherContext {
        ArtifactSettingsRecoveryCipherContext {
            tenant_id: self.tenant_id,
            recovery_point_id: self.recovery_point_id,
            data_owner_id: self.data_owner_id,
            settings_instance_id: self.settings_instance_id,
            schema_digest: self.schema_digest.clone(),
            descriptor_digest: self.descriptor_digest.clone(),
            value_digest: self.value_digest.clone(),
            secret_handle_digest: self.secret_handle_digest.clone(),
        }
    }

    fn ciphertext(
        &self,
    ) -> Result<ArtifactSettingsRecoveryCiphertext, ArtifactSettingsRecoveryError> {
        Ok(ArtifactSettingsRecoveryCiphertext {
            key_version: self.key_version.clone(),
            bytes: self
                .ciphertext
                .clone()
                .ok_or(ArtifactSettingsRecoveryError::RecoveryUnavailable)?,
        })
    }

    fn authorization_context(&self) -> ArtifactSettingsRecoveryAuthorizationContext {
        ArtifactSettingsRecoveryAuthorizationContext {
            recovery_point_id: self.recovery_point_id,
            tenant_id: self.tenant_id,
            installation_id: self.installation_id,
            data_owner_id: self.data_owner_id,
            settings_instance_id: self.settings_instance_id,
            settings_revision: self.settings_revision,
            schema_digest: self.schema_digest.clone(),
            descriptor_digest: self.descriptor_digest.clone(),
            value_digest: self.value_digest.clone(),
            key_version: self.key_version.clone(),
            retention_revision: self.retention_revision,
            policy_snapshot_id: self.policy_snapshot_id.clone(),
            secret_handle_digest: self.secret_handle_digest.clone(),
            retain_until: self.retain_until,
            legal_hold: self.legal_hold,
            audit_hold: self.audit_hold,
            incident_hold: self.incident_hold,
            state: self.state.clone(),
            restored_installation_id: self.restored_installation_id,
            restored_settings_instance_id: self.restored_settings_instance_id,
        }
    }

    fn retention_update_result(
        &self,
        retention_revision: u64,
        retention: ArtifactSettingsRecoveryRetentionUpdate,
    ) -> ArtifactSettingsRecoveryRetentionUpdateResult {
        ArtifactSettingsRecoveryRetentionUpdateResult {
            recovery_point_id: self.recovery_point_id,
            policy_snapshot_id: retention.policy_snapshot_id,
            retention_revision,
            retain_until: retention.retain_until,
            legal_hold: retention.legal_hold,
            audit_hold: retention.audit_hold,
            incident_hold: retention.incident_hold,
        }
    }
}

fn recovery_point_from_source(
    recovery_point_id: Uuid,
    source: &SettingsSource,
    key_version: String,
    retention: ArtifactSettingsRecoveryRetention,
) -> ArtifactSettingsRecoveryPoint {
    ArtifactSettingsRecoveryPoint {
        recovery_point_id,
        tenant_id: source.tenant_id,
        installation_id: source.installation_id,
        data_owner_id: source.data_owner_id,
        settings_instance_id: source.settings_instance_id,
        settings_revision: source.settings_revision,
        schema_digest: source.schema_digest.clone(),
        descriptor_digest: source.descriptor_digest.clone(),
        value_digest: source.value_digest.clone(),
        key_version,
        policy_snapshot_id: retention.policy_snapshot_id.clone(),
        secret_handle_digest: retention.secret_handle_digest.clone(),
        retention_revision: 1,
        retain_until: retention.retain_until,
        legal_hold: retention.legal_hold,
        audit_hold: retention.audit_hold,
        incident_hold: retention.incident_hold,
    }
}

async fn load_retired_source_in<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    installation_id: Uuid,
    expected_installation_revision: u64,
    expected_settings_revision: u64,
    validators: &ArtifactSchemaValidatorCache,
) -> Result<SettingsSource, ArtifactSettingsRecoveryError> {
    let installation = load_installation(connection, tenant_id, installation_id, true).await?;
    acquire_artifact_activation_lock(
        connection,
        &installation.scope,
        &installation.descriptor.slug,
    )
    .await
    .map_err(storage_error)?;
    if installation.status != "inactive"
        || !installation.uninstalled
        || installation.admission_revision != expected_installation_revision
    {
        return Err(ArtifactSettingsRecoveryError::RecoveryPrecondition);
    }
    let settings = load_settings_instance(
        connection,
        tenant_id,
        installation.data_owner_id,
        installation.settings_instance_id,
    )
    .await?;
    if settings.revision != expected_settings_revision
        || settings.schema_digest != installation.schema_digest
    {
        return Err(ArtifactSettingsRecoveryError::RecoveryPrecondition);
    }
    validate_settings_descriptor(
        validators,
        &installation.descriptor,
        &installation.schema_digest,
        &settings.value,
    )?;
    Ok(SettingsSource {
        tenant_id,
        installation_id,
        data_owner_id: installation.data_owner_id,
        registry: installation.registry,
        repository: installation.repository,
        settings_instance_id: installation.settings_instance_id,
        settings_revision: settings.revision,
        schema_digest: installation.schema_digest,
        descriptor_digest: installation.descriptor_digest,
        descriptor: installation.descriptor,
        value_digest: settings_digest(&settings.value)?,
        settings: settings.value,
    })
}

async fn load_source_for_recovery<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    recovery: &RecoveryMaterial,
) -> Result<SettingsSource, ArtifactSettingsRecoveryError> {
    let installation =
        load_installation(connection, tenant_id, recovery.installation_id, true).await?;
    if installation.data_owner_id != recovery.data_owner_id
        || installation.settings_instance_id != recovery.settings_instance_id
        || installation.schema_digest != recovery.schema_digest
        || installation.descriptor_digest != recovery.descriptor_digest
    {
        return Err(ArtifactSettingsRecoveryError::RestorePrecondition);
    }
    Ok(SettingsSource {
        tenant_id,
        installation_id: recovery.installation_id,
        data_owner_id: recovery.data_owner_id,
        registry: installation.registry,
        repository: installation.repository,
        settings_instance_id: recovery.settings_instance_id,
        settings_revision: recovery.settings_revision,
        schema_digest: recovery.schema_digest.clone(),
        descriptor_digest: recovery.descriptor_digest.clone(),
        descriptor: installation.descriptor,
        settings: Value::Null,
        value_digest: recovery.value_digest.clone(),
    })
}

#[derive(Clone, Debug)]
struct InstallationRow {
    scope: ModuleInstallationScope,
    data_owner_id: Uuid,
    registry: String,
    repository: String,
    settings_instance_id: Uuid,
    admission_revision: u64,
    status: String,
    uninstalled: bool,
    descriptor: ModuleArtifactDescriptor,
    descriptor_digest: String,
    schema_digest: String,
}

async fn load_installation<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    installation_id: Uuid,
    lock: bool,
) -> Result<InstallationRow, ArtifactSettingsRecoveryError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT installation.scope_kind, installation.tenant_id, installation.slug, installation.registry, installation.repository, installation.data_owner_id, installation.settings_instance_id, admission.revision AS admission_revision, admission.status, CAST(installation.descriptor AS TEXT) AS descriptor, CASE WHEN uninstall.operation_id IS NULL THEN 0 ELSE 1 END AS uninstalled FROM module_artifact_installations installation JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id LEFT JOIN module_artifact_uninstall_operations uninstall ON uninstall.installation_id = installation.installation_id WHERE installation.installation_id = {} AND ((installation.scope_kind = 'platform' AND installation.tenant_id IS NULL) OR (installation.scope_kind = 'tenant' AND installation.tenant_id = {})){}",
                placeholder(backend, 1), placeholder(backend, 2), lock_clause(backend, lock),
            ),
            vec![uuid_value(installation_id, backend), uuid_value(tenant_id, backend)],
        ))
        .await
        .map_err(storage_error)?
        .ok_or(ArtifactSettingsRecoveryError::InstallationUnavailable)?;
    let data_owner_id = uuid_from_row(&row, "data_owner_id", backend)?;
    let settings_instance_id = uuid_from_row(&row, "settings_instance_id", backend)?;
    let scope_kind: String = row.try_get("", "scope_kind").map_err(storage_error)?;
    let scope = match scope_kind.as_str() {
        "platform" => ModuleInstallationScope::Platform,
        "tenant" => ModuleInstallationScope::Tenant {
            tenant_id: uuid_from_row(&row, "tenant_id", backend)?,
        },
        _ => return Err(ArtifactSettingsRecoveryError::InvalidInstallation),
    };
    let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
        &row.try_get::<String>("", "descriptor")
            .map_err(storage_error)?,
    )
    .map_err(|_| ArtifactSettingsRecoveryError::InvalidInstallation)?;
    descriptor
        .validate()
        .map_err(|_| ArtifactSettingsRecoveryError::InvalidInstallation)?;
    let schema_digest = descriptor
        .settings_schema_digest
        .clone()
        .ok_or(ArtifactSettingsRecoveryError::MissingSchema)?;
    descriptor
        .settings_schema()
        .ok_or(ArtifactSettingsRecoveryError::InvalidInstallation)?;
    let admission_revision: i64 = row
        .try_get("", "admission_revision")
        .map_err(storage_error)?;
    Ok(InstallationRow {
        scope,
        data_owner_id,
        registry: row.try_get("", "registry").map_err(storage_error)?,
        repository: row.try_get("", "repository").map_err(storage_error)?,
        settings_instance_id,
        admission_revision: positive_u64(admission_revision)?,
        status: row.try_get("", "status").map_err(storage_error)?,
        uninstalled: bool_from_row(&row, "uninstalled", backend)?,
        descriptor_digest: canonical_artifact_descriptor_digest(&descriptor),
        descriptor,
        schema_digest,
    })
}

#[derive(Clone, Debug)]
struct SettingsInstanceRow {
    schema_digest: String,
    value: Value,
    revision: u64,
}

async fn load_settings_instance<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    data_owner_id: Uuid,
    settings_instance_id: Uuid,
) -> Result<SettingsInstanceRow, ArtifactSettingsRecoveryError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT schema_digest, settings, revision FROM module_artifact_settings_instances WHERE tenant_id = {} AND data_owner_id = {} AND settings_instance_id = {}{}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                lock_clause(backend, true),
            ),
            vec![
                uuid_value(tenant_id, backend),
                uuid_value(data_owner_id, backend),
                uuid_value(settings_instance_id, backend),
            ],
        ))
        .await
        .map_err(storage_error)?
        .ok_or(ArtifactSettingsRecoveryError::SettingsUnavailable)?;
    let revision: i64 = row.try_get("", "revision").map_err(storage_error)?;
    Ok(SettingsInstanceRow {
        schema_digest: row.try_get("", "schema_digest").map_err(storage_error)?,
        value: row.try_get("", "settings").map_err(storage_error)?,
        revision: positive_u64(revision)?,
    })
}

fn validate_settings(
    validators: &ArtifactSchemaValidatorCache,
    source: &SettingsSource,
    value: &Value,
) -> Result<(), ArtifactSettingsRecoveryError> {
    if !value.is_object() {
        return Err(ArtifactSettingsRecoveryError::InvalidSettings);
    }
    validate_settings_descriptor(validators, &source.descriptor, &source.schema_digest, value)
}

fn validate_settings_descriptor(
    validators: &ArtifactSchemaValidatorCache,
    descriptor: &ModuleArtifactDescriptor,
    schema_digest: &str,
    value: &Value,
) -> Result<(), ArtifactSettingsRecoveryError> {
    if !value.is_object() {
        return Err(ArtifactSettingsRecoveryError::InvalidSettings);
    }
    let schema = descriptor
        .settings_schema()
        .ok_or(ArtifactSettingsRecoveryError::InvalidInstallation)?;
    validators
        .validate(schema_digest, schema, value)
        .map_err(map_validation_error)
}

async fn load_restore_target<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    target_installation_id: Uuid,
    data_owner_id: Uuid,
    source: &SettingsSource,
    validators: &ArtifactSchemaValidatorCache,
    settings: &Value,
) -> Result<InstallationRow, ArtifactSettingsRecoveryError> {
    let target = load_installation(connection, tenant_id, target_installation_id, true).await?;
    acquire_artifact_activation_lock(connection, &target.scope, &target.descriptor.slug)
        .await
        .map_err(storage_error)?;
    if target.data_owner_id != data_owner_id
        || target.status != "inactive"
        || target.uninstalled
        || target.registry != source.registry
        || target.repository != source.repository
        || target.descriptor.slug != source.descriptor.slug
    {
        return Err(ArtifactSettingsRecoveryError::RestorePrecondition);
    }
    let source = SettingsSource {
        tenant_id,
        installation_id: target_installation_id,
        data_owner_id: target.data_owner_id,
        registry: target.registry.clone(),
        repository: target.repository.clone(),
        settings_instance_id: target.settings_instance_id,
        settings_revision: 1,
        schema_digest: target.schema_digest.clone(),
        descriptor_digest: target.descriptor_digest.clone(),
        descriptor: target.descriptor.clone(),
        settings: Value::Null,
        value_digest: String::new(),
    };
    validate_settings(validators, &source, settings)?;
    Ok(target)
}

async fn ensure_no_active_owner_binding<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    data_owner_id: Uuid,
) -> Result<(), ArtifactSettingsRecoveryError> {
    let backend = connection.get_database_backend();
    let active = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT 1 FROM module_artifact_installations installation JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id LEFT JOIN module_artifact_uninstall_operations uninstall ON uninstall.installation_id = installation.installation_id WHERE installation.data_owner_id = {} AND admission.status = 'active' AND uninstall.operation_id IS NULL AND ((installation.scope_kind = 'platform' AND installation.tenant_id IS NULL) OR (installation.scope_kind = 'tenant' AND installation.tenant_id = {})) LIMIT 1",
                placeholder(backend, 1), placeholder(backend, 2),
            ),
            vec![uuid_value(data_owner_id, backend), uuid_value(tenant_id, backend)],
        ))
        .await
        .map_err(storage_error)?;
    if active.is_some() {
        return Err(ArtifactSettingsRecoveryError::PurgePrecondition);
    }
    Ok(())
}

async fn next_tombstone_revision<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    data_owner_id: Uuid,
    settings_instance_id: Uuid,
) -> Result<u64, ArtifactSettingsRecoveryError> {
    let backend = connection.get_database_backend();
    let prior = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT tombstone_revision FROM module_artifact_settings_tombstones WHERE tenant_id = {} AND data_owner_id = {} AND settings_instance_id = {}{}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                lock_clause(backend, true),
            ),
            vec![
                uuid_value(tenant_id, backend),
                uuid_value(data_owner_id, backend),
                uuid_value(settings_instance_id, backend),
            ],
        ))
        .await
        .map_err(storage_error)?;
    if prior.is_some() {
        return Err(ArtifactSettingsRecoveryError::PurgePrecondition);
    }
    Ok(1)
}

async fn ensure_tombstone<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    data_owner_id: Uuid,
    settings_instance_id: Uuid,
    recovery_point_id: Uuid,
) -> Result<(), ArtifactSettingsRecoveryError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT 1 FROM module_artifact_settings_tombstones WHERE tenant_id = {} AND data_owner_id = {} AND settings_instance_id = {} AND recovery_point_id = {}{}",
                placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3), placeholder(backend, 4), lock_clause(backend, true),
            ),
            vec![
                uuid_value(tenant_id, backend),
                uuid_value(data_owner_id, backend),
                uuid_value(settings_instance_id, backend),
                uuid_value(recovery_point_id, backend),
            ],
        ))
        .await
        .map_err(storage_error)?;
    row.is_some()
        .then_some(())
        .ok_or(ArtifactSettingsRecoveryError::RestorePrecondition)
}

async fn find_recovery_operation_in<C: ConnectionTrait>(
    connection: &C,
    request: &ArtifactSettingsRecoveryPointCreateRequest,
    backend: DbBackend,
) -> Result<Option<ArtifactSettingsRecoveryPoint>, ArtifactSettingsRecoveryError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT operation.installation_id, operation.expected_installation_revision, operation.expected_settings_revision, operation.actor_id, operation.trace_id, operation.correlation_id, operation.idempotency_key, operation.reason, point.* FROM module_artifact_settings_recovery_operations operation JOIN module_artifact_settings_recovery_points point ON point.recovery_point_id = operation.recovery_point_id WHERE operation.tenant_id = {} AND operation.idempotency_key = {}",
                placeholder(backend, 1), placeholder(backend, 2),
            ),
            vec![
                uuid_value(request.tenant_id, backend),
                uuid_value(request.context.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(storage_error)?;
    let Some(row) = row else { return Ok(None) };
    let installation_id = uuid_from_row(&row, "installation_id", backend)?;
    let expected_installation_revision: i64 = row
        .try_get("", "expected_installation_revision")
        .map_err(storage_error)?;
    let expected_settings_revision: i64 = row
        .try_get("", "expected_settings_revision")
        .map_err(storage_error)?;
    let reason: String = row.try_get("", "reason").map_err(storage_error)?;
    if installation_id != request.installation_id
        || positive_u64(expected_installation_revision)? != request.expected_installation_revision
        || positive_u64(expected_settings_revision)? != request.expected_settings_revision
        || command_context_from_receipt_row(&row, request.tenant_id, backend)? != request.context
        || reason != request.reason
    {
        return Err(ArtifactSettingsRecoveryError::IdempotencyConflict);
    }
    Ok(Some(recovery_point_from_row(&row, backend)?))
}

fn command_context_from_receipt_row(
    row: &QueryResult,
    tenant_id: Uuid,
    backend: DbBackend,
) -> Result<ModuleCommandContext, ArtifactSettingsRecoveryError> {
    let context = ModuleCommandContext {
        actor_id: uuid_from_row(row, "actor_id", backend)?,
        tenant_id: Some(tenant_id),
        trace_id: row.try_get("", "trace_id").map_err(storage_error)?,
        correlation_id: uuid_from_row(row, "correlation_id", backend)?,
        idempotency_key: uuid_from_row(row, "idempotency_key", backend)?,
    };
    context
        .validate()
        .map_err(|error| ArtifactSettingsRecoveryError::Storage(error.to_string()))?;
    Ok(context)
}

async fn find_purge_operation_in<C: ConnectionTrait>(
    connection: &C,
    request: &ArtifactSettingsPurgeRequest,
    backend: DbBackend,
) -> Result<Option<ArtifactSettingsPurgeResult>, ArtifactSettingsRecoveryError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT operation_id, installation_id, recovery_point_id, expected_installation_revision, expected_settings_revision, tombstone_revision, actor_id, trace_id, correlation_id, idempotency_key, reason FROM module_artifact_settings_purge_operations WHERE tenant_id = {} AND idempotency_key = {}",
                placeholder(backend, 1), placeholder(backend, 2),
            ),
            vec![
                uuid_value(request.tenant_id, backend),
                uuid_value(request.context.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(storage_error)?;
    let Some(row) = row else { return Ok(None) };
    let installation_id = uuid_from_row(&row, "installation_id", backend)?;
    let recovery_point_id = uuid_from_row(&row, "recovery_point_id", backend)?;
    let expected_installation_revision: i64 = row
        .try_get("", "expected_installation_revision")
        .map_err(storage_error)?;
    let expected_settings_revision: i64 = row
        .try_get("", "expected_settings_revision")
        .map_err(storage_error)?;
    let reason: String = row.try_get("", "reason").map_err(storage_error)?;
    if installation_id != request.installation_id
        || recovery_point_id != request.recovery_point_id
        || positive_u64(expected_installation_revision)? != request.expected_installation_revision
        || positive_u64(expected_settings_revision)? != request.expected_settings_revision
        || command_context_from_receipt_row(&row, request.tenant_id, backend)? != request.context
        || reason != request.reason
    {
        return Err(ArtifactSettingsRecoveryError::IdempotencyConflict);
    }
    let operation_id = uuid_from_row(&row, "operation_id", backend)?;
    let tombstone_revision: i64 = row
        .try_get("", "tombstone_revision")
        .map_err(storage_error)?;
    Ok(Some(ArtifactSettingsPurgeResult {
        purge_operation_id: operation_id,
        recovery_point_id,
        tombstone_revision: positive_u64(tombstone_revision)?,
    }))
}

async fn find_restore_operation_in<C: ConnectionTrait>(
    connection: &C,
    request: &ArtifactSettingsRestoreRequest,
    backend: DbBackend,
) -> Result<Option<ArtifactSettingsRestoreResult>, ArtifactSettingsRecoveryError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT operation_id, recovery_point_id, target_installation_id, expected_target_installation_revision, settings_instance_id, actor_id, trace_id, correlation_id, idempotency_key, reason FROM module_artifact_settings_restore_operations WHERE tenant_id = {} AND idempotency_key = {}",
                placeholder(backend, 1), placeholder(backend, 2),
            ),
            vec![
                uuid_value(request.tenant_id, backend),
                uuid_value(request.context.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(storage_error)?;
    let Some(row) = row else { return Ok(None) };
    let recovery_point_id = uuid_from_row(&row, "recovery_point_id", backend)?;
    let target_installation_id = optional_uuid_from_row(&row, "target_installation_id", backend)?;
    let expected_target_installation_revision =
        optional_positive_u64_from_row(&row, "expected_target_installation_revision")?;
    let reason: String = row.try_get("", "reason").map_err(storage_error)?;
    if recovery_point_id != request.recovery_point_id
        || target_installation_id != request.target_installation_id
        || expected_target_installation_revision != request.expected_target_installation_revision
        || command_context_from_receipt_row(&row, request.tenant_id, backend)? != request.context
        || reason != request.reason
    {
        return Err(ArtifactSettingsRecoveryError::IdempotencyConflict);
    }
    Ok(Some(ArtifactSettingsRestoreResult {
        restore_operation_id: uuid_from_row(&row, "operation_id", backend)?,
        recovery_point_id,
        settings_instance_id: uuid_from_row(&row, "settings_instance_id", backend)?,
        target_installation_id,
    }))
}

async fn find_retention_operation_in<C: ConnectionTrait>(
    connection: &C,
    request: &ArtifactSettingsRecoveryRetentionUpdateRequest,
    request_digest: &str,
    backend: DbBackend,
) -> Result<Option<ArtifactSettingsRecoveryRetentionUpdateResult>, ArtifactSettingsRecoveryError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT recovery_point_id, request_digest, retention_revision, retain_until, legal_hold, audit_hold, incident_hold, policy_snapshot_id, actor_id, trace_id, correlation_id, idempotency_key FROM module_artifact_settings_recovery_retention_operations WHERE tenant_id = {} AND idempotency_key = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
            ),
            vec![
                uuid_value(request.tenant_id, backend),
                uuid_value(request.context.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let stored_digest: String = row.try_get("", "request_digest").map_err(storage_error)?;
    let recovery_point_id = uuid_from_row(&row, "recovery_point_id", backend)?;
    if recovery_point_id != request.recovery_point_id
        || stored_digest != request_digest
        || command_context_from_receipt_row(&row, request.tenant_id, backend)? != request.context
    {
        return Err(ArtifactSettingsRecoveryError::IdempotencyConflict);
    }
    Ok(Some(ArtifactSettingsRecoveryRetentionUpdateResult {
        recovery_point_id,
        retention_revision: positive_u64(
            row.try_get::<i64>("", "retention_revision")
                .map_err(storage_error)?,
        )?,
        policy_snapshot_id: row
            .try_get("", "policy_snapshot_id")
            .map_err(storage_error)?,
        retain_until: datetime_from_row(&row, "retain_until", backend)?,
        legal_hold: bool_from_row(&row, "legal_hold", backend)?,
        audit_hold: bool_from_row(&row, "audit_hold", backend)?,
        incident_hold: bool_from_row(&row, "incident_hold", backend)?,
    }))
}

async fn find_rewrap_operation_in<C: ConnectionTrait>(
    connection: &C,
    request: &ArtifactSettingsRecoveryRewrapRequest,
    backend: DbBackend,
) -> Result<Option<ArtifactSettingsRecoveryRewrapResult>, ArtifactSettingsRecoveryError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT operation_id, recovery_point_id, previous_key_version, key_version, actor_id, trace_id, correlation_id, idempotency_key, reason FROM module_artifact_settings_recovery_rewrap_operations WHERE tenant_id = {} AND idempotency_key = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
            ),
            vec![
                uuid_value(request.tenant_id, backend),
                uuid_value(request.context.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let recovery_point_id = uuid_from_row(&row, "recovery_point_id", backend)?;
    let reason: String = row.try_get("", "reason").map_err(storage_error)?;
    if recovery_point_id != request.recovery_point_id
        || command_context_from_receipt_row(&row, request.tenant_id, backend)? != request.context
        || reason != request.reason
    {
        return Err(ArtifactSettingsRecoveryError::IdempotencyConflict);
    }
    Ok(Some(ArtifactSettingsRecoveryRewrapResult {
        rewrap_operation_id: uuid_from_row(&row, "operation_id", backend)?,
        recovery_point_id,
        previous_key_version: row
            .try_get("", "previous_key_version")
            .map_err(storage_error)?,
        key_version: row.try_get("", "key_version").map_err(storage_error)?,
    }))
}

async fn find_bind_operation_in<C: ConnectionTrait>(
    connection: &C,
    request: &ArtifactSettingsRecoveryBindRequest,
    backend: DbBackend,
) -> Result<Option<ArtifactSettingsRecoveryBindResult>, ArtifactSettingsRecoveryError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT operation_id, recovery_point_id, target_installation_id, expected_target_installation_revision, settings_instance_id, actor_id, trace_id, correlation_id, idempotency_key, reason FROM module_artifact_settings_recovery_bind_operations WHERE tenant_id = {} AND idempotency_key = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
            ),
            vec![
                uuid_value(request.tenant_id, backend),
                uuid_value(request.context.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let recovery_point_id = uuid_from_row(&row, "recovery_point_id", backend)?;
    let target_installation_id = uuid_from_row(&row, "target_installation_id", backend)?;
    let expected_target_installation_revision: i64 = row
        .try_get("", "expected_target_installation_revision")
        .map_err(storage_error)?;
    let reason: String = row.try_get("", "reason").map_err(storage_error)?;
    if recovery_point_id != request.recovery_point_id
        || target_installation_id != request.target_installation_id
        || positive_u64(expected_target_installation_revision)?
            != request.expected_target_installation_revision
        || command_context_from_receipt_row(&row, request.tenant_id, backend)? != request.context
        || reason != request.reason
    {
        return Err(ArtifactSettingsRecoveryError::IdempotencyConflict);
    }
    Ok(Some(ArtifactSettingsRecoveryBindResult {
        bind_operation_id: uuid_from_row(&row, "operation_id", backend)?,
        recovery_point_id,
        target_installation_id,
        settings_instance_id: uuid_from_row(&row, "settings_instance_id", backend)?,
    }))
}

fn settings_recovery_collection_candidate_from_row(
    row: QueryResult,
    backend: DbBackend,
) -> Result<SettingsRecoveryCollectionCandidate, ArtifactSettingsRecoveryError> {
    Ok(SettingsRecoveryCollectionCandidate {
        candidate: ArtifactSettingsRecoveryCollectionCandidate {
            recovery_point_id: uuid_from_row(&row, "recovery_point_id", backend)?,
            tenant_id: uuid_from_row(&row, "tenant_id", backend)?,
            data_owner_id: uuid_from_row(&row, "data_owner_id", backend)?,
            settings_instance_id: uuid_from_row(&row, "settings_instance_id", backend)?,
            retention_revision: positive_u64(
                row.try_get::<i64>("", "retention_revision")
                    .map_err(storage_error)?,
            )?,
            retain_until: datetime_from_row(&row, "retain_until", backend)?,
            legal_hold: bool_from_row(&row, "legal_hold", backend)?,
            audit_hold: bool_from_row(&row, "audit_hold", backend)?,
            incident_hold: bool_from_row(&row, "incident_hold", backend)?,
        },
        status: row.try_get("", "state").map_err(storage_error)?,
    })
}

async fn settings_recovery_collection_work_in<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    recovery_point_id: Uuid,
) -> Result<SettingsRecoveryCollectionWork, ArtifactSettingsRecoveryError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT collection_id, tenant_id, recovery_point_id, actor_id, trace_id, correlation_id, idempotency_key FROM module_artifact_settings_recovery_collections WHERE tenant_id = {} AND recovery_point_id = {} AND completed_at IS NULL",
                placeholder(backend, 1),
                placeholder(backend, 2),
            ),
            vec![
                uuid_value(tenant_id, backend),
                uuid_value(recovery_point_id, backend),
            ],
        ))
        .await
        .map_err(storage_error)?
        .ok_or(ArtifactSettingsRecoveryError::CollectionPrecondition)?;
    let stored_tenant_id = uuid_from_row(&row, "tenant_id", backend)?;
    let context = ModuleCommandContext {
        actor_id: uuid_from_row(&row, "actor_id", backend)?,
        tenant_id: Some(stored_tenant_id),
        trace_id: row.try_get("", "trace_id").map_err(storage_error)?,
        correlation_id: uuid_from_row(&row, "correlation_id", backend)?,
        idempotency_key: uuid_from_row(&row, "idempotency_key", backend)?,
    };
    if stored_tenant_id != tenant_id || !valid_command_context(stored_tenant_id, &context) {
        return Err(ArtifactSettingsRecoveryError::CollectionPrecondition);
    }
    Ok(SettingsRecoveryCollectionWork {
        collection_id: uuid_from_row(&row, "collection_id", backend)?,
        tenant_id: stored_tenant_id,
        recovery_point_id: uuid_from_row(&row, "recovery_point_id", backend)?,
        context,
    })
}

async fn load_recovery_material_in<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    recovery_point_id: Uuid,
    lock: bool,
) -> Result<RecoveryMaterial, ArtifactSettingsRecoveryError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT tenant_id, recovery_point_id, installation_id, data_owner_id, settings_instance_id, settings_revision, schema_digest, descriptor_digest, value_digest, key_version, ciphertext, retention_revision, policy_snapshot_id, secret_handle_digest, retain_until, legal_hold, audit_hold, incident_hold, state, restored_at, restored_installation_id, restored_settings_instance_id FROM module_artifact_settings_recovery_points WHERE tenant_id = {} AND recovery_point_id = {}{}",
                placeholder(backend, 1), placeholder(backend, 2), lock_clause(backend, lock),
            ),
            vec![uuid_value(tenant_id, backend), uuid_value(recovery_point_id, backend)],
        ))
        .await
        .map_err(storage_error)?
        .ok_or(ArtifactSettingsRecoveryError::RecoveryUnavailable)?;
    recovery_material_from_row(&row, backend)
}

fn recovery_point_from_row(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<ArtifactSettingsRecoveryPoint, ArtifactSettingsRecoveryError> {
    let settings_revision: i64 = row
        .try_get("", "settings_revision")
        .map_err(storage_error)?;
    let retention_revision: i64 = row
        .try_get("", "retention_revision")
        .map_err(storage_error)?;
    Ok(ArtifactSettingsRecoveryPoint {
        recovery_point_id: uuid_from_row(row, "recovery_point_id", backend)?,
        tenant_id: uuid_from_row(row, "tenant_id", backend)?,
        installation_id: uuid_from_row(row, "installation_id", backend)?,
        data_owner_id: uuid_from_row(row, "data_owner_id", backend)?,
        settings_instance_id: uuid_from_row(row, "settings_instance_id", backend)?,
        settings_revision: positive_u64(settings_revision)?,
        schema_digest: row.try_get("", "schema_digest").map_err(storage_error)?,
        descriptor_digest: row
            .try_get("", "descriptor_digest")
            .map_err(storage_error)?,
        value_digest: row.try_get("", "value_digest").map_err(storage_error)?,
        key_version: row.try_get("", "key_version").map_err(storage_error)?,
        policy_snapshot_id: row
            .try_get("", "policy_snapshot_id")
            .map_err(storage_error)?,
        secret_handle_digest: row
            .try_get("", "secret_handle_digest")
            .map_err(storage_error)?,
        retention_revision: positive_u64(retention_revision)?,
        retain_until: datetime_from_row(row, "retain_until", backend)?,
        legal_hold: bool_from_row(row, "legal_hold", backend)?,
        audit_hold: bool_from_row(row, "audit_hold", backend)?,
        incident_hold: bool_from_row(row, "incident_hold", backend)?,
    })
}

fn recovery_material_from_row(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<RecoveryMaterial, ArtifactSettingsRecoveryError> {
    let settings_revision: i64 = row
        .try_get("", "settings_revision")
        .map_err(storage_error)?;
    Ok(RecoveryMaterial {
        tenant_id: uuid_from_row(row, "tenant_id", backend)?,
        recovery_point_id: uuid_from_row(row, "recovery_point_id", backend)?,
        installation_id: uuid_from_row(row, "installation_id", backend)?,
        data_owner_id: uuid_from_row(row, "data_owner_id", backend)?,
        settings_instance_id: uuid_from_row(row, "settings_instance_id", backend)?,
        settings_revision: positive_u64(settings_revision)?,
        schema_digest: row.try_get("", "schema_digest").map_err(storage_error)?,
        descriptor_digest: row
            .try_get("", "descriptor_digest")
            .map_err(storage_error)?,
        value_digest: row.try_get("", "value_digest").map_err(storage_error)?,
        key_version: row.try_get("", "key_version").map_err(storage_error)?,
        ciphertext: row.try_get("", "ciphertext").map_err(storage_error)?,
        retention_revision: positive_u64(
            row.try_get::<i64>("", "retention_revision")
                .map_err(storage_error)?,
        )?,
        policy_snapshot_id: row
            .try_get("", "policy_snapshot_id")
            .map_err(storage_error)?,
        secret_handle_digest: row
            .try_get("", "secret_handle_digest")
            .map_err(storage_error)?,
        retain_until: datetime_from_row(row, "retain_until", backend)?,
        legal_hold: bool_from_row(row, "legal_hold", backend)?,
        audit_hold: bool_from_row(row, "audit_hold", backend)?,
        incident_hold: bool_from_row(row, "incident_hold", backend)?,
        state: row.try_get("", "state").map_err(storage_error)?,
        restored_at: optional_datetime_from_row(row, "restored_at", backend)?,
        restored_installation_id: optional_uuid_from_row(row, "restored_installation_id", backend)?,
        restored_settings_instance_id: optional_uuid_from_row(
            row,
            "restored_settings_instance_id",
            backend,
        )?,
    })
}

fn validate_recovery_point_request(
    request: &ArtifactSettingsRecoveryPointCreateRequest,
) -> Result<(), ArtifactSettingsRecoveryError> {
    validate_command(
        request.tenant_id,
        request.installation_id,
        request.expected_installation_revision,
        request.expected_settings_revision,
        &request.context,
        &request.reason,
    )
}

fn validate_purge_request(
    request: &ArtifactSettingsPurgeRequest,
) -> Result<(), ArtifactSettingsRecoveryError> {
    validate_command(
        request.tenant_id,
        request.installation_id,
        request.expected_installation_revision,
        request.expected_settings_revision,
        &request.context,
        &request.reason,
    )?;
    if request.recovery_point_id.is_nil() {
        return Err(ArtifactSettingsRecoveryError::InvalidRequest);
    }
    Ok(())
}

fn validate_restore_request(
    request: &ArtifactSettingsRestoreRequest,
) -> Result<(), ArtifactSettingsRecoveryError> {
    if request.tenant_id.is_nil()
        || request.recovery_point_id.is_nil()
        || request.target_installation_id.is_some_and(|id| id.is_nil())
        || (request.target_installation_id.is_some()
            != request.expected_target_installation_revision.is_some())
        || request
            .expected_target_installation_revision
            .is_some_and(|revision| revision == 0)
        || !valid_command_context(request.tenant_id, &request.context)
        || !valid_reason(&request.reason)
    {
        return Err(ArtifactSettingsRecoveryError::InvalidRequest);
    }
    Ok(())
}

fn validate_retention_update_request(
    request: &ArtifactSettingsRecoveryRetentionUpdateRequest,
) -> Result<(), ArtifactSettingsRecoveryError> {
    if request.tenant_id.is_nil()
        || request.recovery_point_id.is_nil()
        || request.expected_retention_revision == 0
        || !valid_command_context(request.tenant_id, &request.context)
        || !valid_reason(&request.reason)
        || (request.extend_retain_until.is_none()
            && request.legal_hold.is_none()
            && request.audit_hold.is_none()
            && request.incident_hold.is_none())
    {
        return Err(ArtifactSettingsRecoveryError::InvalidRequest);
    }
    Ok(())
}

fn validate_rewrap_request(
    request: &ArtifactSettingsRecoveryRewrapRequest,
) -> Result<(), ArtifactSettingsRecoveryError> {
    if request.tenant_id.is_nil()
        || request.recovery_point_id.is_nil()
        || !valid_command_context(request.tenant_id, &request.context)
        || !valid_reason(&request.reason)
    {
        return Err(ArtifactSettingsRecoveryError::InvalidRequest);
    }
    Ok(())
}

fn validate_collection_request(
    request: &ArtifactSettingsRecoveryCollectionRequest,
) -> Result<(), ArtifactSettingsRecoveryError> {
    if request.tenant_id.is_nil()
        || !valid_command_context(request.tenant_id, &request.context)
        || !valid_reason(&request.reason)
        || !valid_policy_snapshot_id(&request.policy_snapshot_id)
        || request.limit == 0
        || request.limit > MAX_COLLECTION_BATCH
    {
        return Err(ArtifactSettingsRecoveryError::InvalidRequest);
    }
    Ok(())
}

fn validate_bind_request(
    request: &ArtifactSettingsRecoveryBindRequest,
) -> Result<(), ArtifactSettingsRecoveryError> {
    if request.tenant_id.is_nil()
        || request.recovery_point_id.is_nil()
        || request.target_installation_id.is_nil()
        || request.expected_target_installation_revision == 0
        || !valid_command_context(request.tenant_id, &request.context)
        || !valid_reason(&request.reason)
    {
        return Err(ArtifactSettingsRecoveryError::InvalidRequest);
    }
    Ok(())
}

fn validate_command(
    tenant_id: Uuid,
    installation_id: Uuid,
    expected_installation_revision: u64,
    expected_settings_revision: u64,
    context: &ModuleCommandContext,
    reason: &str,
) -> Result<(), ArtifactSettingsRecoveryError> {
    if tenant_id.is_nil()
        || installation_id.is_nil()
        || expected_installation_revision == 0
        || expected_settings_revision == 0
        || !valid_command_context(tenant_id, context)
        || !valid_reason(reason)
    {
        return Err(ArtifactSettingsRecoveryError::InvalidRequest);
    }
    Ok(())
}

fn valid_command_context(tenant_id: Uuid, context: &ModuleCommandContext) -> bool {
    context.tenant_id == Some(tenant_id) && context.validate().is_ok()
}

fn validate_retention(
    retention: &ArtifactSettingsRecoveryRetention,
    now: DateTime<Utc>,
) -> Result<(), ArtifactSettingsRecoveryError> {
    if !valid_policy_snapshot_id(&retention.policy_snapshot_id)
        || !valid_sha256_digest(&retention.secret_handle_digest)
        || retention.retain_until <= now
    {
        return Err(ArtifactSettingsRecoveryError::PolicyDenied);
    }
    Ok(())
}

fn validate_retention_update(
    retention: &ArtifactSettingsRecoveryRetentionUpdate,
) -> Result<(), ArtifactSettingsRecoveryError> {
    if !valid_policy_snapshot_id(&retention.policy_snapshot_id) {
        return Err(ArtifactSettingsRecoveryError::PolicyDenied);
    }
    Ok(())
}

fn valid_policy_snapshot_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_POLICY_SNAPSHOT_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn validate_ciphertext(
    ciphertext: &ArtifactSettingsRecoveryCiphertext,
) -> Result<(), ArtifactSettingsRecoveryError> {
    if ciphertext.key_version.trim().is_empty()
        || ciphertext.key_version.trim() != ciphertext.key_version
        || ciphertext.key_version.len() > MAX_KEY_VERSION_BYTES
        || ciphertext.bytes.is_empty()
        || ciphertext.bytes.len() > MAX_CIPHERTEXT_BYTES
    {
        return Err(ArtifactSettingsRecoveryError::CiphertextIntegrity);
    }
    Ok(())
}

fn valid_reason(reason: &str) -> bool {
    !reason.trim().is_empty() && reason.trim() == reason && reason.len() <= MAX_REASON_BYTES
}

fn canonical_settings_bytes(settings: &Value) -> Result<Vec<u8>, ArtifactSettingsRecoveryError> {
    canonical_json_bytes(settings).map_err(storage_error)
}

fn parse_canonical_settings(bytes: &[u8]) -> Result<Value, ArtifactSettingsRecoveryError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| ArtifactSettingsRecoveryError::CiphertextIntegrity)?;
    if !value.is_object() || canonical_settings_bytes(&value)? != bytes {
        return Err(ArtifactSettingsRecoveryError::CiphertextIntegrity);
    }
    Ok(value)
}

fn settings_digest(settings: &Value) -> Result<String, ArtifactSettingsRecoveryError> {
    let canonical = canonical_manifest_snapshot_json(settings).map_err(storage_error)?;
    Ok(format!("sha256:{}", hash_manifest_snapshot(&canonical)))
}

fn request_digest<T: Serialize>(request: &T) -> Result<String, ArtifactSettingsRecoveryError> {
    let value = serde_json::to_value(request).map_err(storage_error)?;
    settings_digest(&value)
}

fn valid_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn map_validation_error(error: ArtifactSchemaValidationError) -> ArtifactSettingsRecoveryError {
    match error {
        ArtifactSchemaValidationError::Compilation => {
            ArtifactSettingsRecoveryError::InvalidInstallation
        }
        ArtifactSchemaValidationError::Violation => ArtifactSettingsRecoveryError::SchemaViolation,
        ArtifactSchemaValidationError::CachePoisoned => {
            ArtifactSettingsRecoveryError::ValidatorUnavailable
        }
    }
}

fn positive_u64(value: i64) -> Result<u64, ArtifactSettingsRecoveryError> {
    u64::try_from(value).ok().filter(|value| *value > 0).ok_or(
        ArtifactSettingsRecoveryError::Storage("stored revision is invalid".to_string()),
    )
}

fn revision_value(value: u64) -> Result<SqlValue, ArtifactSettingsRecoveryError> {
    i64::try_from(value)
        .map(Into::into)
        .map_err(|_| ArtifactSettingsRecoveryError::InvalidRequest)
}

fn optional_revision_value(value: Option<u64>) -> Result<SqlValue, ArtifactSettingsRecoveryError> {
    match value {
        Some(value) => revision_value(value),
        None => Ok(SqlValue::BigInt(None)),
    }
}

fn optional_positive_u64_from_row(
    row: &QueryResult,
    column: &str,
) -> Result<Option<u64>, ArtifactSettingsRecoveryError> {
    row.try_get::<Option<i64>>("", column)
        .map_err(storage_error)?
        .map(positive_u64)
        .transpose()
}

fn placeholder(backend: DbBackend, index: usize) -> String {
    match backend {
        DbBackend::Postgres => format!("${index}"),
        _ => format!("?{index}"),
    }
}

fn lock_clause(backend: DbBackend, lock: bool) -> &'static str {
    match (backend, lock) {
        (DbBackend::Postgres, true) => " FOR UPDATE",
        _ => "",
    }
}

fn now_expression(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "NOW()",
        _ => "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    }
}

fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => SqlValue::Uuid(Some(value)),
        _ => value.to_string().into(),
    }
}

fn optional_uuid_value(value: Option<Uuid>, backend: DbBackend) -> SqlValue {
    match (backend, value) {
        (DbBackend::Postgres, Some(value)) => SqlValue::Uuid(Some(value)),
        (DbBackend::Postgres, None) => SqlValue::Uuid(None),
        (_, Some(value)) => value.to_string().into(),
        (_, None) => SqlValue::String(None),
    }
}

fn bytes_value(value: Vec<u8>) -> SqlValue {
    SqlValue::Bytes(Some(value))
}

fn null_bytes_value() -> SqlValue {
    SqlValue::Bytes(None)
}

fn bool_value(value: bool, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => value.into(),
        _ => i64::from(value).into(),
    }
}

fn datetime_value(value: DateTime<Utc>, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => SqlValue::ChronoDateTimeUtc(Some(value)),
        _ => value.to_rfc3339().into(),
    }
}

fn uuid_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, ArtifactSettingsRecoveryError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        _ => row
            .try_get::<String>("", column)
            .map_err(storage_error)?
            .parse()
            .map_err(|_| {
                ArtifactSettingsRecoveryError::Storage("stored UUID is invalid".to_string())
            }),
    }
}

fn optional_uuid_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Option<Uuid>, ArtifactSettingsRecoveryError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        _ => row
            .try_get::<Option<String>>("", column)
            .map_err(storage_error)?
            .map(|value| {
                value.parse().map_err(|_| {
                    ArtifactSettingsRecoveryError::Storage("stored UUID is invalid".to_string())
                })
            })
            .transpose(),
    }
}

fn bool_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<bool, ArtifactSettingsRecoveryError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        _ => Ok(row.try_get::<i64>("", column).map_err(storage_error)? != 0),
    }
}

fn datetime_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<DateTime<Utc>, ArtifactSettingsRecoveryError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        _ => row
            .try_get::<String>("", column)
            .map_err(storage_error)
            .and_then(|value| {
                DateTime::parse_from_rfc3339(&value)
                    .map(|value| value.with_timezone(&Utc))
                    .map_err(storage_error)
            }),
    }
}

fn optional_datetime_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Option<DateTime<Utc>>, ArtifactSettingsRecoveryError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        _ => row
            .try_get::<Option<String>>("", column)
            .map_err(storage_error)?
            .map(|value| {
                DateTime::parse_from_rfc3339(&value)
                    .map(|value| value.with_timezone(&Utc))
                    .map_err(storage_error)
            })
            .transpose(),
    }
}

fn storage_error(error: impl std::fmt::Display) -> ArtifactSettingsRecoveryError {
    ArtifactSettingsRecoveryError::Storage(error.to_string())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactSettingsRecoveryError {
    #[error("artifact settings recovery request is invalid")]
    InvalidRequest,
    #[error("artifact settings recovery policy denied the operation")]
    PolicyDenied,
    #[error("artifact settings installation is unavailable")]
    InstallationUnavailable,
    #[error("artifact settings installation metadata is invalid")]
    InvalidInstallation,
    #[error("artifact does not declare a settings schema")]
    MissingSchema,
    #[error("artifact settings are unavailable")]
    SettingsUnavailable,
    #[error("artifact settings are not a JSON object")]
    InvalidSettings,
    #[error("artifact settings violate their admitted schema")]
    SchemaViolation,
    #[error("artifact settings schema validator is unavailable")]
    ValidatorUnavailable,
    #[error("artifact settings recovery precondition failed")]
    RecoveryPrecondition,
    #[error("artifact settings purge precondition failed")]
    PurgePrecondition,
    #[error("artifact settings restore precondition failed")]
    RestorePrecondition,
    #[error("artifact settings recovery retention precondition failed")]
    RetentionPrecondition,
    #[error("artifact settings recovery rewrap precondition failed")]
    RewrapPrecondition,
    #[error("artifact settings recovery collection precondition failed")]
    CollectionPrecondition,
    #[error("artifact settings recovery continuity-bind precondition failed")]
    BindPrecondition,
    #[error("artifact settings recovery point is unavailable")]
    RecoveryUnavailable,
    #[error("artifact settings recovery ciphertext is invalid or could not be verified")]
    CiphertextIntegrity,
    #[error("artifact settings idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("artifact settings recovery storage failed: {0}")]
    Storage(String),
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use chrono::Duration;
    use rustok_core::MigrationSource;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::{
        ArtifactModuleKind, ArtifactPayloadKind, ArtifactSchemaDocument, ModulesModule,
        canonical_schema_digest,
    };

    #[derive(Clone)]
    struct AllowRecoveryAuthorizer;

    struct AllowRecoveryCollectionPolicy;

    fn command_context(tenant_id: Uuid, actor_id: Uuid) -> ModuleCommandContext {
        ModuleCommandContext {
            actor_id,
            tenant_id: Some(tenant_id),
            trace_id: "test:artifact-settings-recovery".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        }
    }

    #[test]
    fn settings_recovery_rejects_a_foreign_command_context() {
        let tenant_id = Uuid::new_v4();
        let foreign_tenant_id = Uuid::new_v4();
        let context = command_context(foreign_tenant_id, Uuid::new_v4());

        assert_eq!(
            validate_purge_request(&ArtifactSettingsPurgeRequest {
                tenant_id,
                installation_id: Uuid::new_v4(),
                recovery_point_id: Uuid::new_v4(),
                expected_installation_revision: 1,
                expected_settings_revision: 1,
                context: context.clone(),
                reason: "reject foreign settings purge context".to_string(),
            }),
            Err(ArtifactSettingsRecoveryError::InvalidRequest)
        );
        assert_eq!(
            validate_collection_request(&ArtifactSettingsRecoveryCollectionRequest {
                tenant_id,
                context,
                reason: "reject foreign collection context".to_string(),
                policy_snapshot_id: "test-policy".to_string(),
                limit: 1,
            }),
            Err(ArtifactSettingsRecoveryError::InvalidRequest)
        );
    }

    #[async_trait]
    impl ArtifactSettingsRecoveryCollectionPolicy for AllowRecoveryCollectionPolicy {
        fn snapshot_id(&self) -> &str {
            "retention-policy-2026-08"
        }

        async fn may_collect(
            &self,
            _: &ArtifactSettingsRecoveryCollectionCandidate,
        ) -> Result<bool, ArtifactSettingsRecoveryError> {
            Ok(true)
        }
    }

    #[async_trait]
    impl ArtifactSettingsRecoveryAuthorizer for AllowRecoveryAuthorizer {
        async fn authorize_recovery_point(
            &self,
            _: &ArtifactSettingsRecoveryPointCreateRequest,
        ) -> Result<ArtifactSettingsRecoveryRetention, ArtifactSettingsRecoveryError> {
            Ok(ArtifactSettingsRecoveryRetention {
                policy_snapshot_id: "retention-policy-2026-08".to_string(),
                secret_handle_digest: format!("sha256:{}", "c".repeat(64)),
                retain_until: Utc::now() + Duration::hours(1),
                legal_hold: false,
                audit_hold: false,
                incident_hold: false,
            })
        }

        async fn authorize_purge(
            &self,
            _: &ArtifactSettingsPurgeRequest,
            _: &ArtifactSettingsRecoveryAuthorizationContext,
        ) -> Result<(), ArtifactSettingsRecoveryError> {
            Ok(())
        }

        async fn authorize_restore(
            &self,
            _: &ArtifactSettingsRestoreRequest,
            _: &ArtifactSettingsRecoveryAuthorizationContext,
        ) -> Result<(), ArtifactSettingsRecoveryError> {
            Ok(())
        }

        async fn authorize_retention_update(
            &self,
            request: &ArtifactSettingsRecoveryRetentionUpdateRequest,
            recovery: &ArtifactSettingsRecoveryAuthorizationContext,
        ) -> Result<ArtifactSettingsRecoveryRetentionUpdate, ArtifactSettingsRecoveryError>
        {
            Ok(ArtifactSettingsRecoveryRetentionUpdate {
                policy_snapshot_id: "retention-policy-2026-08".to_string(),
                retain_until: request.extend_retain_until.unwrap_or(recovery.retain_until),
                legal_hold: request.legal_hold.unwrap_or(recovery.legal_hold),
                audit_hold: request.audit_hold.unwrap_or(recovery.audit_hold),
                incident_hold: request.incident_hold.unwrap_or(recovery.incident_hold),
            })
        }

        async fn authorize_rewrap(
            &self,
            _: &ArtifactSettingsRecoveryRewrapRequest,
            _: &ArtifactSettingsRecoveryAuthorizationContext,
        ) -> Result<(), ArtifactSettingsRecoveryError> {
            Ok(())
        }

        async fn authorize_collection(
            &self,
            _: &ArtifactSettingsRecoveryCollectionRequest,
        ) -> Result<(), ArtifactSettingsRecoveryError> {
            Ok(())
        }

        async fn authorize_bind(
            &self,
            _: &ArtifactSettingsRecoveryBindRequest,
            _: &ArtifactSettingsRecoveryAuthorizationContext,
        ) -> Result<(), ArtifactSettingsRecoveryError> {
            Ok(())
        }
    }

    /// Test-only authenticated envelope. Production callers must inject their
    /// own KMS-backed cipher through the public port.
    #[derive(Clone)]
    struct ContextBoundCipher;

    #[async_trait]
    impl ArtifactSettingsRecoveryCipher for ContextBoundCipher {
        async fn encrypt(
            &self,
            context: &ArtifactSettingsRecoveryCipherContext,
            canonical_settings: &[u8],
        ) -> Result<ArtifactSettingsRecoveryCiphertext, ArtifactSettingsRecoveryError> {
            let mut ciphertext = context_tag(context, canonical_settings)?;
            ciphertext.extend_from_slice(canonical_settings);
            Ok(ArtifactSettingsRecoveryCiphertext {
                key_version: "test-kms-key-2026-08".to_string(),
                bytes: ciphertext,
            })
        }

        async fn decrypt(
            &self,
            context: &ArtifactSettingsRecoveryCipherContext,
            ciphertext: &ArtifactSettingsRecoveryCiphertext,
        ) -> Result<Vec<u8>, ArtifactSettingsRecoveryError> {
            if !matches!(
                ciphertext.key_version.as_str(),
                "test-kms-key-2026-08" | "test-kms-key-2026-09"
            ) || ciphertext.bytes.len() < 32
            {
                return Err(ArtifactSettingsRecoveryError::CiphertextIntegrity);
            }
            let (tag, settings) = ciphertext.bytes.split_at(32);
            (tag == context_tag(context, settings)?.as_slice())
                .then(|| settings.to_vec())
                .ok_or(ArtifactSettingsRecoveryError::CiphertextIntegrity)
        }

        async fn rewrap(
            &self,
            context: &ArtifactSettingsRecoveryCipherContext,
            ciphertext: &ArtifactSettingsRecoveryCiphertext,
        ) -> Result<ArtifactSettingsRecoveryCiphertext, ArtifactSettingsRecoveryError> {
            let settings = self.decrypt(context, ciphertext).await?;
            let mut rewrapped = context_tag(context, &settings)?;
            rewrapped.extend_from_slice(&settings);
            Ok(ArtifactSettingsRecoveryCiphertext {
                key_version: "test-kms-key-2026-09".to_string(),
                bytes: rewrapped,
            })
        }
    }

    fn context_tag(
        context: &ArtifactSettingsRecoveryCipherContext,
        settings: &[u8],
    ) -> Result<Vec<u8>, ArtifactSettingsRecoveryError> {
        let mut hasher = Sha256::new();
        hasher.update(canonical_json_bytes(context).map_err(storage_error)?);
        hasher.update(settings);
        Ok(hasher.finalize().to_vec())
    }

    #[tokio::test]
    async fn recovery_purge_and_restore_are_tombstoned_idempotent_and_outboxed() {
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

        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let source_installation_id = Uuid::new_v4();
        let target_installation_id = Uuid::new_v4();
        let data_owner_id = Uuid::new_v4();
        let source_settings_instance_id = Uuid::new_v4();
        let target_settings_instance_id = Uuid::new_v4();
        let schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": { "theme": { "type": "string" } },
            "required": ["theme"],
            "additionalProperties": false,
        });
        let schema_digest = canonical_schema_digest(&schema);
        let source_descriptor = descriptor("1.0.0", "a", schema_digest.clone(), schema.clone());
        let target_descriptor = descriptor("1.0.1", "b", schema_digest.clone(), schema);
        insert_installation(
            &database,
            source_installation_id,
            tenant_id,
            data_owner_id,
            source_settings_instance_id,
            &source_descriptor,
        )
        .await;
        insert_installation(
            &database,
            target_installation_id,
            tenant_id,
            data_owner_id,
            target_settings_instance_id,
            &target_descriptor,
        )
        .await;
        insert_admission(&database, source_installation_id, "inactive", 3).await;
        insert_admission(&database, target_installation_id, "inactive", 1).await;
        insert_uninstall_evidence(&database, source_installation_id, actor_id).await;
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_settings_instances (tenant_id, data_owner_id, settings_instance_id, schema_digest, settings, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 5, '2026-08-13T00:00:00Z', '2026-08-13T00:00:00Z')"
                    .to_string(),
                vec![
                    tenant_id.to_string().into(),
                    data_owner_id.to_string().into(),
                    source_settings_instance_id.to_string().into(),
                    schema_digest.clone().into(),
                    SqlValue::Json(Some(Box::new(serde_json::json!({ "theme": "dark" })))),
                ],
            ))
            .await
            .expect("source settings");

        let service = SeaOrmArtifactSettingsRecoveryService::new(
            database.clone(),
            AllowRecoveryAuthorizer,
            ContextBoundCipher,
        );
        let recovery_request = ArtifactSettingsRecoveryPointCreateRequest {
            tenant_id,
            installation_id: source_installation_id,
            expected_installation_revision: 3,
            expected_settings_revision: 5,
            context: command_context(tenant_id, actor_id),
            reason: "retain settings before purge".to_string(),
        };
        let recovery = service
            .create_recovery_point(recovery_request.clone())
            .await
            .expect("create recovery point");
        assert_eq!(recovery.tenant_id, tenant_id);
        assert_eq!(recovery.settings_instance_id, source_settings_instance_id);
        assert_eq!(
            recovery.secret_handle_digest,
            format!("sha256:{}", "c".repeat(64))
        );
        assert_eq!(
            service
                .create_recovery_point(recovery_request)
                .await
                .expect("replay recovery point"),
            recovery
        );

        let retention_request = ArtifactSettingsRecoveryRetentionUpdateRequest {
            tenant_id,
            recovery_point_id: recovery.recovery_point_id,
            expected_retention_revision: recovery.retention_revision,
            extend_retain_until: Some(recovery.retain_until + Duration::hours(1)),
            legal_hold: Some(false),
            audit_hold: Some(false),
            incident_hold: Some(false),
            context: command_context(tenant_id, actor_id),
            reason: "extend protected settings retention".to_string(),
        };
        let retained = service
            .update_retention(retention_request.clone())
            .await
            .expect("update recovery retention");
        assert_eq!(retained.retention_revision, 2);
        assert_eq!(
            service
                .update_retention(retention_request.clone())
                .await
                .expect("replay recovery retention"),
            retained
        );

        let rewrap_request = ArtifactSettingsRecoveryRewrapRequest {
            tenant_id,
            recovery_point_id: recovery.recovery_point_id,
            context: command_context(tenant_id, actor_id),
            reason: "rotate approved KMS key".to_string(),
        };
        let rewrapped = service
            .rewrap(rewrap_request.clone())
            .await
            .expect("rewrap recovery ciphertext");
        assert_eq!(rewrapped.previous_key_version, "test-kms-key-2026-08");
        assert_eq!(rewrapped.key_version, "test-kms-key-2026-09");
        assert_eq!(
            service
                .rewrap(rewrap_request)
                .await
                .expect("replay recovery rewrap"),
            rewrapped
        );
        assert_eq!(
            service
                .update_retention(retention_request)
                .await
                .expect("replay retention after rewrap"),
            retained
        );

        let purge_request = ArtifactSettingsPurgeRequest {
            tenant_id,
            installation_id: source_installation_id,
            recovery_point_id: recovery.recovery_point_id,
            expected_installation_revision: 3,
            expected_settings_revision: 5,
            context: command_context(tenant_id, actor_id),
            reason: "purge retired settings".to_string(),
        };
        let purge = service
            .purge(purge_request.clone())
            .await
            .expect("purge settings");
        assert_eq!(purge.tombstone_revision, 1);
        assert_eq!(
            service.purge(purge_request).await.expect("replay purge"),
            purge
        );
        assert_eq!(
            count_rows(
                &database,
                "module_artifact_settings_instances",
                "tenant_id",
                tenant_id,
            )
            .await,
            0
        );
        assert_eq!(
            count_rows(
                &database,
                "module_artifact_settings_tombstones",
                "tenant_id",
                tenant_id,
            )
            .await,
            1
        );

        let restore_request = ArtifactSettingsRestoreRequest {
            tenant_id,
            recovery_point_id: recovery.recovery_point_id,
            target_installation_id: None,
            expected_target_installation_revision: None,
            context: command_context(tenant_id, actor_id),
            reason: "restore settings before continuity selection".to_string(),
        };
        let restored = service
            .restore(restore_request.clone())
            .await
            .expect("restore settings");
        assert_ne!(restored.settings_instance_id, source_settings_instance_id);
        assert_eq!(restored.target_installation_id, None);
        assert_eq!(
            service
                .restore(restore_request)
                .await
                .expect("replay restore"),
            restored
        );
        let target_binding = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT settings_instance_id FROM module_artifact_installations WHERE installation_id = ?1"
                    .to_string(),
                vec![target_installation_id.to_string().into()],
            ))
            .await
            .expect("target binding")
            .expect("target row");
        assert_eq!(
            target_binding
                .try_get::<String>("", "settings_instance_id")
                .expect("target settings instance"),
            target_settings_instance_id.to_string()
        );
        let restored_settings = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT settings, revision FROM module_artifact_settings_instances WHERE tenant_id = ?1 AND data_owner_id = ?2 AND settings_instance_id = ?3"
                    .to_string(),
                vec![
                    tenant_id.to_string().into(),
                    data_owner_id.to_string().into(),
                    restored.settings_instance_id.to_string().into(),
                ],
            ))
            .await
            .expect("restored settings")
            .expect("restored settings row");
        assert_eq!(
            restored_settings
                .try_get::<Value>("", "settings")
                .expect("settings value"),
            serde_json::json!({ "theme": "dark" })
        );
        assert_eq!(
            restored_settings
                .try_get::<i64>("", "revision")
                .expect("settings revision"),
            1
        );
        let bind_request = ArtifactSettingsRecoveryBindRequest {
            tenant_id,
            recovery_point_id: recovery.recovery_point_id,
            target_installation_id,
            expected_target_installation_revision: 1,
            context: command_context(tenant_id, actor_id),
            reason: "bind settings to continuity-approved successor".to_string(),
        };
        let bound = service
            .bind(bind_request.clone())
            .await
            .expect("bind restored settings");
        assert_eq!(bound.settings_instance_id, restored.settings_instance_id);
        assert_eq!(
            service
                .bind(bind_request)
                .await
                .expect("replay settings bind"),
            bound
        );
        let target_binding = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT settings_instance_id FROM module_artifact_installations WHERE installation_id = ?1"
                    .to_string(),
                vec![target_installation_id.to_string().into()],
            ))
            .await
            .expect("bound target settings")
            .expect("bound target row");
        assert_eq!(
            target_binding
                .try_get::<String>("", "settings_instance_id")
                .expect("bound target settings instance"),
            restored.settings_instance_id.to_string()
        );

        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_artifact_settings_recovery_points SET retain_until = '2026-08-12T00:00:00Z' WHERE recovery_point_id = ?1"
                    .to_string(),
                vec![recovery.recovery_point_id.to_string().into()],
            ))
            .await
            .expect("expire recovery point for collection");
        let collection_context = command_context(tenant_id, actor_id);
        let collected = service
            .collect(
                ArtifactSettingsRecoveryCollectionRequest {
                    tenant_id,
                    context: collection_context.clone(),
                    reason: "collect expired encrypted recovery point".to_string(),
                    policy_snapshot_id: "retention-policy-2026-08".to_string(),
                    limit: 1,
                },
                &AllowRecoveryCollectionPolicy,
            )
            .await
            .expect("collect expired recovery point");
        assert_eq!(collected.collected, 1);
        let collection_state = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT state, ciphertext, collected_at FROM module_artifact_settings_recovery_points WHERE recovery_point_id = ?1"
                    .to_string(),
                vec![recovery.recovery_point_id.to_string().into()],
            ))
            .await
            .expect("collection state")
            .expect("collection row");
        assert_eq!(
            collection_state
                .try_get::<String>("", "state")
                .expect("collection state value"),
            "collected"
        );
        assert!(
            collection_state
                .try_get::<Option<Vec<u8>>>("", "ciphertext")
                .expect("collected ciphertext")
                .is_none()
        );
        assert!(
            collection_state
                .try_get::<Option<String>>("", "collected_at")
                .expect("collection timestamp")
                .is_some()
        );
        let collection_receipt = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT actor_id, trace_id, correlation_id, idempotency_key FROM module_artifact_settings_recovery_collections WHERE recovery_point_id = ?1"
                    .to_string(),
                vec![recovery.recovery_point_id.to_string().into()],
            ))
            .await
            .expect("collection receipt query")
            .expect("collection receipt");
        assert_eq!(
            collection_receipt
                .try_get::<String>("", "actor_id")
                .expect("collection actor"),
            collection_context.actor_id.to_string()
        );
        assert_eq!(
            collection_receipt
                .try_get::<String>("", "trace_id")
                .expect("collection trace"),
            collection_context.trace_id
        );
        assert_eq!(
            collection_receipt
                .try_get::<String>("", "correlation_id")
                .expect("collection correlation"),
            collection_context.correlation_id.to_string()
        );
        assert_eq!(
            collection_receipt
                .try_get::<String>("", "idempotency_key")
                .expect("collection idempotency"),
            collection_context.idempotency_key.to_string()
        );
        let collection_event = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT payload FROM sys_events WHERE event_type = 'module.artifact.settings_recovery_collected'"
                    .to_string(),
            ))
            .await
            .expect("collection event query")
            .expect("collection event");
        let collection_payload: Value = collection_event
            .try_get("", "payload")
            .expect("collection event payload");
        let collection_envelope: rustok_events::EventEnvelope =
            serde_json::from_value(collection_payload).expect("collection event envelope");
        assert_eq!(
            collection_envelope.actor_id,
            Some(collection_context.actor_id)
        );
        assert_eq!(collection_envelope.tenant_id, tenant_id);
        assert_eq!(
            collection_envelope.correlation_id,
            collection_context.correlation_id
        );
        assert_eq!(
            collection_envelope.trace_id.as_deref(),
            Some(collection_context.trace_id.as_str())
        );
        let event_types = database
            .query_all_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT event_type FROM sys_events ORDER BY event_type".to_string(),
            ))
            .await
            .expect("outbox events")
            .into_iter()
            .map(|row| row.try_get::<String>("", "event_type").expect("event type"))
            .collect::<Vec<_>>();
        assert_eq!(
            event_types,
            vec![
                "module.artifact.settings_purged".to_string(),
                "module.artifact.settings_recovery_bound".to_string(),
                "module.artifact.settings_recovery_collected".to_string(),
                "module.artifact.settings_recovery_point_created".to_string(),
                "module.artifact.settings_recovery_retention_updated".to_string(),
                "module.artifact.settings_recovery_rewrapped".to_string(),
                "module.artifact.settings_restored".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn direct_restore_requires_the_selected_target_admission_revision() {
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

        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let source_installation_id = Uuid::new_v4();
        let target_installation_id = Uuid::new_v4();
        let data_owner_id = Uuid::new_v4();
        let source_settings_instance_id = Uuid::new_v4();
        let target_settings_instance_id = Uuid::new_v4();
        let schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": { "theme": { "type": "string" } },
            "required": ["theme"],
            "additionalProperties": false,
        });
        let schema_digest = canonical_schema_digest(&schema);
        let source_descriptor = descriptor("1.0.0", "a", schema_digest.clone(), schema.clone());
        let target_descriptor = descriptor("1.0.1", "b", schema_digest.clone(), schema);
        insert_installation(
            &database,
            source_installation_id,
            tenant_id,
            data_owner_id,
            source_settings_instance_id,
            &source_descriptor,
        )
        .await;
        insert_installation(
            &database,
            target_installation_id,
            tenant_id,
            data_owner_id,
            target_settings_instance_id,
            &target_descriptor,
        )
        .await;
        insert_admission(&database, source_installation_id, "inactive", 3).await;
        insert_admission(&database, target_installation_id, "inactive", 1).await;
        insert_uninstall_evidence(&database, source_installation_id, actor_id).await;
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_settings_instances (tenant_id, data_owner_id, settings_instance_id, schema_digest, settings, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 5, '2026-08-13T00:00:00Z', '2026-08-13T00:00:00Z')"
                    .to_string(),
                vec![
                    tenant_id.to_string().into(),
                    data_owner_id.to_string().into(),
                    source_settings_instance_id.to_string().into(),
                    schema_digest.into(),
                    SqlValue::Json(Some(Box::new(serde_json::json!({ "theme": "dark" })))),
                ],
            ))
            .await
            .expect("source settings");

        let service = SeaOrmArtifactSettingsRecoveryService::new(
            database.clone(),
            AllowRecoveryAuthorizer,
            ContextBoundCipher,
        );
        let recovery = service
            .create_recovery_point(ArtifactSettingsRecoveryPointCreateRequest {
                tenant_id,
                installation_id: source_installation_id,
                expected_installation_revision: 3,
                expected_settings_revision: 5,
                context: command_context(tenant_id, actor_id),
                reason: "retain settings before direct restore".to_string(),
            })
            .await
            .expect("create recovery point");
        service
            .purge(ArtifactSettingsPurgeRequest {
                tenant_id,
                installation_id: source_installation_id,
                recovery_point_id: recovery.recovery_point_id,
                expected_installation_revision: 3,
                expected_settings_revision: 5,
                context: command_context(tenant_id, actor_id),
                reason: "purge retained source settings".to_string(),
            })
            .await
            .expect("purge source settings");

        let stale = service
            .restore(ArtifactSettingsRestoreRequest {
                tenant_id,
                recovery_point_id: recovery.recovery_point_id,
                target_installation_id: Some(target_installation_id),
                expected_target_installation_revision: Some(2),
                context: command_context(tenant_id, actor_id),
                reason: "attempt stale direct restore".to_string(),
            })
            .await;
        assert_eq!(
            stale,
            Err(ArtifactSettingsRecoveryError::RestorePrecondition)
        );

        let request = ArtifactSettingsRestoreRequest {
            tenant_id,
            recovery_point_id: recovery.recovery_point_id,
            target_installation_id: Some(target_installation_id),
            expected_target_installation_revision: Some(1),
            context: command_context(tenant_id, actor_id),
            reason: "restore retained settings into successor".to_string(),
        };
        let restored = service
            .restore(request.clone())
            .await
            .expect("direct restore");
        assert_eq!(
            restored.target_installation_id,
            Some(target_installation_id)
        );
        assert_ne!(restored.settings_instance_id, source_settings_instance_id);
        assert_ne!(restored.settings_instance_id, target_settings_instance_id);
        assert_eq!(
            service
                .restore(request)
                .await
                .expect("replay direct restore"),
            restored
        );

        let target_binding = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT settings_instance_id FROM module_artifact_installations WHERE installation_id = ?1"
                    .to_string(),
                vec![target_installation_id.to_string().into()],
            ))
            .await
            .expect("target settings binding")
            .expect("target installation");
        assert_eq!(
            target_binding
                .try_get::<String>("", "settings_instance_id")
                .expect("target settings instance"),
            restored.settings_instance_id.to_string()
        );
    }

    fn descriptor(
        version: &str,
        digest_character: &str,
        schema_digest: String,
        schema: Value,
    ) -> ModuleArtifactDescriptor {
        ModuleArtifactDescriptor {
            schema_version: crate::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            slug: "recovery_module".to_string(),
            version: version.to_string(),
            payload_kind: ArtifactPayloadKind::Rhai,
            module_kind: ArtifactModuleKind::Optional,
            runtime_abi: "rustok:module/runtime@1".to_string(),
            platform_compatibility: "^0.1".to_string(),
            required_features: Vec::new(),
            artifact_digest: format!("sha256:{}", digest_character.repeat(64)),
            entrypoint: "main".to_string(),
            capabilities: Vec::new(),
            bindings: Vec::new(),
            dependencies: Vec::new(),
            permissions: Vec::new(),
            schema_documents: vec![ArtifactSchemaDocument {
                digest: schema_digest.clone(),
                document: schema,
            }],
            settings_schema_digest: Some(schema_digest),
            data_schema_digest: None,
            localization_catalogs: Vec::new(),
            ui_contributions: Vec::new(),
            persistence_contract: None,
        }
    }

    async fn insert_installation(
        database: &DatabaseConnection,
        installation_id: Uuid,
        tenant_id: Uuid,
        data_owner_id: Uuid,
        settings_instance_id: Uuid,
        descriptor: &ModuleArtifactDescriptor,
    ) {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_installations (\
                    installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, slug, version, payload_kind, \
                    runtime_abi, payload_digest, entrypoint, descriptor, data_owner_id, settings_instance_id, dependency_graph_revision, \
                    dependency_graph_digest, dependency_lock, installed_at\
                 ) VALUES (?1, 'tenant', ?2, 'registry.example', 'modules/recovery', ?3, 'recovery_module', ?4, 'rhai', \
                    'rustok:module/runtime@1', ?5, 'main', ?6, ?7, ?8, 1, ?9, '{}', '2026-08-13T00:00:00Z')"
                    .to_string(),
                vec![
                    installation_id.to_string().into(),
                    tenant_id.to_string().into(),
                    format!("sha256:{}", "d".repeat(64)).into(),
                    descriptor.version.clone().into(),
                    descriptor.artifact_digest.clone().into(),
                    SqlValue::Json(Some(Box::new(
                        serde_json::to_value(descriptor).expect("descriptor JSON"),
                    ))),
                    data_owner_id.to_string().into(),
                    settings_instance_id.to_string().into(),
                    format!("sha256:{}", "e".repeat(64)).into(),
                ],
            ))
            .await
            .expect("installation");
    }

    async fn insert_admission(
        database: &DatabaseConnection,
        installation_id: Uuid,
        status: &str,
        revision: i64,
    ) {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_admissions (stage_id, installation_id, payload_digest, media_type, size_bytes, verification_evidence, status, revision, committed_at) VALUES (?1, ?2, ?3, 'application/vnd.rustok.rhai', 1, '{}', ?4, ?5, '2026-08-13T00:00:00Z')"
                    .to_string(),
                vec![
                    Uuid::new_v4().to_string().into(),
                    installation_id.to_string().into(),
                    format!("sha256:{}", "f".repeat(64)).into(),
                    status.to_string().into(),
                    revision.into(),
                ],
            ))
            .await
            .expect("admission");
    }

    async fn insert_uninstall_evidence(
        database: &DatabaseConnection,
        installation_id: Uuid,
        actor_id: Uuid,
    ) {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_uninstall_operations \
                 (operation_id, installation_id, expected_revision, actor_id, trace_id, correlation_id, reason, idempotency_key, committed_at) \
                 VALUES (?1, ?2, 2, ?3, 'test:artifact-settings-recovery', ?4, 'retired source', ?5, '2026-08-13T00:00:00Z')"
                    .to_string(),
                vec![
                    Uuid::new_v4().to_string().into(),
                    installation_id.to_string().into(),
                    actor_id.to_string().into(),
                    Uuid::new_v4().to_string().into(),
                    Uuid::new_v4().to_string().into(),
                ],
            ))
            .await
            .expect("source uninstall evidence");
    }

    async fn count_rows(
        database: &DatabaseConnection,
        table: &str,
        tenant_column: &str,
        tenant_id: Uuid,
    ) -> i64 {
        database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                format!("SELECT COUNT(*) AS count FROM {table} WHERE {tenant_column} = ?1"),
                vec![tenant_id.to_string().into()],
            ))
            .await
            .expect("count query")
            .expect("count row")
            .try_get("", "count")
            .expect("count")
    }
}
