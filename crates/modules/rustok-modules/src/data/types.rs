//! Artifact data models, scopes, namespaces, quotas, and DTOs.

use async_trait::async_trait;
use rustok_sandbox::SandboxError;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::*;
use super::constants::*;
use super::error::*;

/// Host-owned namespace for untrusted artifact data. Guests never supply a
/// physical table, bucket, database schema, or secret-store location.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataScope {
    pub tenant_id: Uuid,
    pub data_owner_id: Uuid,
    pub namespace_instance_id: Uuid,
    pub data_contract_digest: String,
    pub module_slug: String,
    pub data_contract_revision: u64,
    pub policy_revision: u64,
}

/// Actual namespace facts shared by owner maintenance commands. This contains
/// no caller grant; authority must be evaluated on the owner's transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDataNamespace {
    pub tenant_id: Uuid,
    pub data_owner_id: Uuid,
    pub namespace_instance_id: Uuid,
    pub module_slug: String,
    pub data_contract_revision: u64,
    pub data_contract_digest: String,
    pub namespace_revision: u64,
}

/// Call after root locking. Only the record-copy owner may hand off its own
/// target hold after validating the exact pending request or continuation.
pub(crate) async fn ensure_namespace_not_migration_held_on<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    data_owner_id: Uuid,
    namespace_instance_id: Uuid,
    authorized_record_copy_target: Option<Uuid>,
) -> Result<(), ArtifactDataError> {
    let backend = db.get_database_backend();
    let mut values = vec![
        uuid_value(tenant_id, backend),
        uuid_value(data_owner_id, backend),
        uuid_value(namespace_instance_id, backend),
    ];
    let excluded = authorized_record_copy_target.map(|target| {
        values.push(uuid_value(target, backend));
        placeholder(backend, 4)
    });
    let record =
        crate::migrations::m20260903_000047_artifact_data_copy_operations::record_copy_hold(
            backend,
            "namespace",
            false,
            excluded.as_deref(),
        );
    let held=db.query_one_raw(Statement::from_sql_and_values(backend,format!(
        "SELECT 1 FROM module_artifact_data_namespaces namespace
         WHERE namespace.tenant_id={} AND namespace.data_owner_id={} AND namespace.namespace_instance_id={}
          AND(EXISTS(SELECT 1 FROM module_artifact_data_object_migration_operations operation
              WHERE operation.tenant_id=namespace.tenant_id AND operation.data_owner_id=namespace.data_owner_id
               AND operation.status IN ('preparing','committing')
               AND(operation.source_namespace_instance_id=namespace.namespace_instance_id OR operation.target_namespace_instance_id=namespace.namespace_instance_id))
           OR {record})",placeholder(backend,1),placeholder(backend,2),placeholder(backend,3)),values))
        .await.map_err(storage_error)?;
    if held.is_some() {
        return Err(ArtifactDataError::NamespaceHeld);
    }
    Ok(())
}

/// Serialize root mutation before reading namespace facts. The unchanged
/// logical revision still creates a PostgreSQL row version, so older
/// repeatable-read writers cannot miss a hold committed behind this lock.
pub(crate) async fn lock_artifact_data_namespace_on<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    data_owner_id: Uuid,
    namespace_instance_id: Uuid,
) -> Result<Option<(ArtifactDataNamespace, String)>, ArtifactDataError> {
    let backend = db.get_database_backend();
    let values = vec![
        uuid_value(tenant_id, backend),
        uuid_value(data_owner_id, backend),
        uuid_value(namespace_instance_id, backend),
    ];
    db.execute_raw(Statement::from_sql_and_values(
        backend,
        format!(
            "UPDATE module_artifact_data_namespaces SET namespace_revision=namespace_revision
         WHERE tenant_id={} AND data_owner_id={} AND namespace_instance_id={}",
            placeholder(backend, 1),
            placeholder(backend, 2),
            placeholder(backend, 3)
        ),
        values.clone(),
    ))
    .await
    .map_err(storage_error)?;
    let row = db.query_one_raw(Statement::from_sql_and_values(backend, format!(
        "SELECT namespace.*, CASE WHEN purged_at IS NULL THEN state ELSE 'purged' END AS migration_state
         FROM module_artifact_data_namespaces namespace
         WHERE tenant_id={} AND data_owner_id={} AND namespace_instance_id={}{}",
        placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
        namespace_lock_clause(backend)), values)).await.map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let namespace = ArtifactDataNamespace {
        tenant_id: uuid_from_row(&row, "tenant_id", backend)?,
        data_owner_id: uuid_from_row(&row, "data_owner_id", backend)?,
        namespace_instance_id: uuid_from_row(&row, "namespace_instance_id", backend)?,
        module_slug: row.try_get("", "module_slug").map_err(storage_error)?,
        data_contract_revision: u64::try_from(
            row.try_get::<i64>("", "data_contract_revision")
                .map_err(storage_error)?,
        )
        .map_err(storage_error)?,
        data_contract_digest: row
            .try_get("", "data_contract_digest")
            .map_err(storage_error)?,
        namespace_revision: u64::try_from(
            row.try_get::<i64>("", "namespace_revision")
                .map_err(storage_error)?,
        )
        .map_err(storage_error)?,
    };
    if namespace.tenant_id.is_nil()
        || namespace.data_owner_id.is_nil()
        || namespace.namespace_instance_id.is_nil()
        || !valid_module_slug(&namespace.module_slug)
        || namespace.data_contract_revision == 0
        || namespace.namespace_revision == 0
        || !crate::promotion::valid_digest(&namespace.data_contract_digest)
    {
        return Err(ArtifactDataError::InvalidScope);
    }
    Ok(Some((
        namespace,
        row.try_get("", "migration_state").map_err(storage_error)?,
    )))
}

/// Host-selected limits for one exact artifact-data policy decision. An
/// artifact never supplies these values. Production composition may tighten
/// them, but cannot exceed the platform hard ceilings validated here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataQuota {
    pub max_structured_records: u64,
    pub max_structured_bytes: u64,
    pub max_objects: u64,
    pub max_object_bytes: u64,
    pub max_upload_sessions: u64,
    pub max_staging_bytes: u64,
}

impl Default for ArtifactDataQuota {
    fn default() -> Self {
        Self {
            max_structured_records: MAX_ARTIFACT_DATA_NAMESPACE_RECORDS,
            max_structured_bytes: MAX_ARTIFACT_DATA_NAMESPACE_VALUE_BYTES,
            max_objects: MAX_ARTIFACT_DATA_NAMESPACE_OBJECTS,
            max_object_bytes: MAX_ARTIFACT_DATA_NAMESPACE_OBJECT_BYTES,
            max_upload_sessions: MAX_ARTIFACT_DATA_NAMESPACE_UPLOAD_SESSIONS,
            max_staging_bytes: MAX_ARTIFACT_DATA_NAMESPACE_STAGING_BYTES,
        }
    }
}

impl ArtifactDataQuota {
    pub(crate) fn validate(self) -> Result<Self, ArtifactDataError> {
        if self.max_structured_records == 0
            || self.max_structured_records > MAX_ARTIFACT_DATA_NAMESPACE_RECORDS
            || self.max_structured_bytes == 0
            || self.max_structured_bytes > MAX_ARTIFACT_DATA_NAMESPACE_VALUE_BYTES
            || self.max_objects == 0
            || self.max_objects > MAX_ARTIFACT_DATA_NAMESPACE_OBJECTS
            || self.max_object_bytes == 0
            || self.max_object_bytes > MAX_ARTIFACT_DATA_NAMESPACE_OBJECT_BYTES
            || self.max_upload_sessions == 0
            || self.max_upload_sessions > MAX_ARTIFACT_DATA_NAMESPACE_UPLOAD_SESSIONS
            || self.max_staging_bytes == 0
            || self.max_staging_bytes > MAX_ARTIFACT_DATA_NAMESPACE_STAGING_BYTES
        {
            return Err(ArtifactDataError::InvalidQuota);
        }
        Ok(self)
    }
}

/// Deployment-owned quota lookup for an exact tenant/module/data-contract and
/// policy revision. It is resolved only after installation identity and
/// capability admission have succeeded.
#[async_trait]
pub trait ArtifactDataQuotaPolicy: Send + Sync {
    async fn quota_for(
        &self,
        scope: &ArtifactDataScope,
    ) -> Result<ArtifactDataQuota, ArtifactDataError>;
}

/// Standalone policy used by the default control-plane composition and tests.
/// A deployment can inject a stricter durable policy resolver without changing
/// the broker or giving an artifact access to policy storage.
#[derive(Clone, Copy, Debug, Default)]
pub struct FixedArtifactDataQuotaPolicy {
    quota: ArtifactDataQuota,
}

impl FixedArtifactDataQuotaPolicy {
    pub fn new(quota: ArtifactDataQuota) -> Result<Self, ArtifactDataError> {
        Ok(Self {
            quota: quota.validate()?,
        })
    }
}

#[async_trait]
impl ArtifactDataQuotaPolicy for FixedArtifactDataQuotaPolicy {
    async fn quota_for(
        &self,
        scope: &ArtifactDataScope,
    ) -> Result<ArtifactDataQuota, ArtifactDataError> {
        scope.validate()?;
        self.quota.validate()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataWrite {
    pub key: String,
    pub value: Value,
    pub expected_revision: Option<u64>,
    /// Owner-only create-if-absent guard. Sandbox capability decoding always
    /// sets this to false; upgrade application uses it with a deterministic
    /// idempotency key so it can never overwrite target-contract data.
    #[serde(default)]
    pub create_only: bool,
    pub idempotency_key: Uuid,
}

/// A bounded, atomic group of structured-value writes. The guest supplies
/// logical keys and values only; the owner validates the whole batch before it
/// opens its transaction and commits every accepted write together.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataBatchWrite {
    pub writes: Vec<ArtifactDataWrite>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataRecord {
    pub key: String,
    pub value: Value,
    pub revision: u64,
}

/// Owner command for deleting one logical structured-data record. The exact
/// revision prevents a stale execution from deleting a newer value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataDeleteRequest {
    pub key: String,
    pub expected_revision: u64,
    pub idempotency_key: Uuid,
}

/// Durable structured-data deletion receipt. It contains no schema, table, or
/// other physical persistence identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataDeleteResult {
    pub key: String,
    pub deleted_revision: u64,
}

/// Immutable logical metadata for one brokered artifact object. It deliberately
/// excludes the driver storage key, URL, bucket, and any host credential.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataObject {
    pub name: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub digest_sha256: String,
    pub revision: u64,
}

/// Owner command for replacing one bounded private object. The payload is not
/// serializable guest state and is never persisted in an operation record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactDataObjectUpload {
    pub name: String,
    pub content_type: String,
    pub data: Bytes,
    pub expected_revision: Option<u64>,
    pub idempotency_key: Uuid,
}

/// Owner command for deleting one logical object. The exact revision is
/// mandatory so a stale artifact execution cannot remove a newer value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataObjectDeleteRequest {
    pub name: String,
    pub expected_revision: u64,
    pub idempotency_key: Uuid,
}

/// Durable deletion receipt. Physical object identity stays private and the
/// unreachable bytes remain subject to the retention-aware GC policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataObjectDeleteResult {
    pub name: String,
    pub deleted_revision: u64,
}

/// A durable, resumable owner upload. It contains no physical storage identity
/// and is safe to return to an admitted artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataObjectUploadSession {
    pub session_id: Uuid,
    pub name: String,
    pub content_type: String,
    pub expected_revision: Option<u64>,
    /// Owner-generated timestamp rendered as a portable string so the
    /// capability does not expose a database-specific temporal value.
    pub expires_at: String,
    pub completed_object: Option<ArtifactDataObject>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataObjectUploadSessionRequest {
    pub name: String,
    pub content_type: String,
    pub expected_revision: Option<u64>,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactDataObjectUploadChunk {
    pub session_id: Uuid,
    pub sequence: u64,
    pub data: Bytes,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataObjectUploadCompleteRequest {
    pub session_id: Uuid,
    pub size_bytes: u64,
    pub digest_sha256: String,
}

/// A verified private object returned only after the owner re-hashes bytes
/// read from its private storage key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactDataObjectContent {
    pub object: ArtifactDataObject,
    pub data: Bytes,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataObjectPage {
    pub objects: Vec<ArtifactDataObject>,
    pub next_after_name: Option<String>,
}

/// Durable retention facts for a no-longer-referenced private object. Missing
/// rules are deliberately retained: physical deletion is never age-only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataObjectRetentionRule {
    pub delete_after: chrono::DateTime<chrono::Utc>,
    pub legal_hold: bool,
    pub audit_hold: bool,
    pub rollback_hold: bool,
}

#[async_trait]
pub trait ArtifactDataObjectRetentionPolicy: Send + Sync {
    async fn may_delete(
        &self,
        scope: &ArtifactDataScope,
        storage_key: &str,
    ) -> Result<bool, ArtifactDataError>;
}

/// Snapshot-backed policy for a bounded GC pass. A production caller supplies
/// this from its legal-hold, audit, rollback, and expiry projections.
pub struct SnapshotArtifactDataObjectRetentionPolicy {
    now: chrono::DateTime<chrono::Utc>,
    rules: HashMap<String, ArtifactDataObjectRetentionRule>,
}

impl SnapshotArtifactDataObjectRetentionPolicy {
    pub fn new(
        now: chrono::DateTime<chrono::Utc>,
        rules: HashMap<String, ArtifactDataObjectRetentionRule>,
    ) -> Self {
        Self { now, rules }
    }
}

#[async_trait]
impl ArtifactDataObjectRetentionPolicy for SnapshotArtifactDataObjectRetentionPolicy {
    async fn may_delete(
        &self,
        _scope: &ArtifactDataScope,
        storage_key: &str,
    ) -> Result<bool, ArtifactDataError> {
        Ok(self.rules.get(storage_key).is_some_and(|rule| {
            !rule.legal_hold
                && !rule.audit_hold
                && !rule.rollback_hold
                && rule.delete_after <= self.now
        }))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataObjectGcResult {
    pub deleted: u64,
    pub retained: u64,
}

/// Result of retiring expired resumable upload sessions. The reaper only
/// queues private chunk bytes for retention-aware GC; it never deletes them
/// directly.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataObjectUploadReapResult {
    pub abandoned_sessions: u64,
    pub queued_chunks: u64,
}

impl ArtifactDataObject {
    pub fn validate(&self) -> Result<(), ArtifactDataError> {
        if validate_artifact_data_key(&self.name).is_err()
            || self.content_type.trim().is_empty()
            || self.content_type.trim() != self.content_type
            || self.content_type.len() > MAX_ARTIFACT_OBJECT_CONTENT_TYPE_BYTES
            || self.content_type.chars().any(char::is_control)
            || self.size_bytes == 0
            || self.size_bytes > MAX_ARTIFACT_OBJECT_BYTES
            || !prefixed_sha256_digest(&self.digest_sha256)
            || self.revision == 0
        {
            return Err(ArtifactDataError::InvalidObject);
        }
        Ok(())
    }
}

/// A bounded keyset page. The continuation is a validated logical key; guests
/// never receive a database offset or query plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataPageRequest {
    pub prefix: String,
    pub after_key: Option<String>,
    pub limit: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataPage {
    pub records: Vec<ArtifactDataRecord>,
    pub next_after_key: Option<String>,
}

/// A bounded equality lookup over one descriptor-declared logical index. The
/// request contains no physical database identity, expression, sort order, or
/// offset; pagination remains keyset-only on the logical data key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataIndexQuery {
    pub index: String,
    pub value: Value,
    pub page: ArtifactDataPageRequest,
}

/// A read/transform-only request for advancing one bounded page of structured
/// artifact data to a newer admitted data-contract revision. Persisting the
/// resulting plan is deliberately a separate owner command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataUpgradeRequest {
    /// Stable owner-generated identity for retrying one bounded upgrade page.
    pub plan_id: Uuid,
    /// The exact host-selected installation whose target contract is being
    /// prepared and later checkpointed.
    pub target_installation_id: Uuid,
    pub source: ArtifactDataScope,
    pub target: ArtifactDataScope,
    /// Identifies a pre-bound, admitted sandbox hook. It is never a guest
    /// command line, module path, or executable reference.
    pub hook_binding_id: String,
    pub page: ArtifactDataPageRequest,
}

/// The only input passed to an admitted data-contract upgrade hook.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataUpgradeInput {
    pub source: ArtifactDataScope,
    pub target: ArtifactDataScope,
    pub record: ArtifactDataRecord,
}

/// A transformed value paired with the source revision it was planned from.
/// The revision lets a later owner command reject a stale plan through its
/// normal optimistic-write contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataUpgradeRecord {
    pub key: String,
    pub value: Value,
    pub source_revision: u64,
}

/// A bounded, non-durable upgrade result. It carries no database transaction,
/// write authority, checkpoint, or lifecycle transition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataUpgradePlan {
    pub plan_id: Uuid,
    pub target_installation_id: Uuid,
    pub source: ArtifactDataScope,
    pub target: ArtifactDataScope,
    pub hook_binding_id: String,
    pub records: Vec<ArtifactDataUpgradeRecord>,
    pub next_after_key: Option<String>,
}

/// Owner command for applying a previously validated bounded plan. It cannot
/// supply arbitrary checkpoint contents: the applier derives redacted owner
/// metadata from the immutable plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataUpgradeApplyRequest {
    pub plan: ArtifactDataUpgradePlan,
    pub installation_scope: ModuleInstallationScope,
    pub expected_installation_revision: u64,
    pub has_irreversible_migration: bool,
    /// Mandatory owner-command evidence for the durable migration checkpoint.
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataUpgradeApplyResult {
    pub records: Vec<ArtifactDataRecord>,
    pub installation_revision: u64,
}

/// The operation being authorized by the host. Values are intentionally absent:
/// policy evaluation receives namespace and logical-key context, never an
/// unbounded guest payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactDataAccess {
    Read { key: String },
    Write { key: String },
    Delete { key: String },
    List,
    Query { index: String },
    ObjectRead { name: String },
    ObjectWrite { name: String },
    ObjectDelete { name: String },
    ObjectList,
}

/// An explicit destructive command for one exact retired installation.
///
/// The caller cannot choose a module slug, data-contract revision, or policy
/// revision. The owner derives that namespace from the installation's admitted
/// descriptor and current capability grant after it has proved that the
/// installation is retired.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataPurgeRequest {
    pub installation_id: Uuid,
    pub expected_namespace_revision: u64,
    /// Mandatory authenticated evidence for this destructive owner command.
    /// The tenant identity must match the selected installation's visible data
    /// namespace exactly.
    pub context: ModuleCommandContext,
    pub reason: String,
}

/// Owner-derived authority facts for one artifact-data purge.
///
/// This is intentionally narrower than an installation projection: it gives
/// the host policy port the exact namespace and stable data-owner identity
/// without exposing descriptor, registry, or storage details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataPurgeAuthorizationContext {
    pub installation_id: Uuid,
    pub data_owner_id: Uuid,
    pub installation_revision: u64,
    pub scope: ArtifactDataScope,
}

/// Redacted, non-authoritative preview for one artifact-data purge.
///
/// The apply path always reloads and locks the same owner facts; this view is
/// advisory only and never supplies an executable namespace selector.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataPurgePreview {
    pub installation_id: Uuid,
    pub namespace_revision: u64,
    pub records_to_purge: u64,
    pub can_purge: bool,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataPurgeResult {
    pub namespace_revision: u64,
    pub purged_records: u64,
}

/// Owner-only, bounded export of structured artifact data. This is an audited
/// keyset page, not a durable backup snapshot: callers that need a complete
/// consistent backup must compose one through a separate snapshot/export job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataExportRequest {
    pub scope: ArtifactDataScope,
    /// Prevents an export that was authorized against a namespace state that
    /// has since been purged or otherwise lifecycle-revised.
    pub expected_namespace_revision: u64,
    pub page: ArtifactDataPageRequest,
    /// Authenticated command evidence for this owner-only export. Its tenant
    /// must match the retained namespace exactly.
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataExportResult {
    pub export_id: Uuid,
    pub namespace_revision: u64,
    pub page: ArtifactDataPage,
}

impl ArtifactDataScope {
    pub fn validate(&self) -> Result<(), ArtifactDataError> {
        if self.tenant_id.is_nil()
            || self.data_owner_id.is_nil()
            || self.namespace_instance_id.is_nil()
            || !crate::promotion::valid_digest(&self.data_contract_digest)
            || !valid_module_slug(&self.module_slug)
            || self.data_contract_revision == 0
            || self.policy_revision == 0
        {
            return Err(ArtifactDataError::InvalidScope);
        }
        Ok(())
    }
}

/// Derives the namespace used by data-adjacent capabilities from an exact
/// installation selected by the shared capability resolver. The artifact call
/// never supplies a tenant, data revision, or policy revision for this scope.
pub(crate) fn artifact_data_scope_for_execution(
    installation: &InstalledModuleArtifact,
    execution: &ArtifactCapabilityExecution,
    capability: &rustok_sandbox::CapabilityName,
) -> SandboxResult<ArtifactDataScope> {
    let authority = crate::artifact_capability_router::artifact_capability_scope_for_execution(
        installation,
        execution,
        capability,
    )?;
    let contract = installation
        .descriptor
        .persistence_contract
        .as_ref()
        .ok_or_else(|| SandboxError::CapabilityDenied(capability.clone()))?;
    let scope = ArtifactDataScope {
        tenant_id: authority.tenant_id,
        data_owner_id: authority.data_owner_id,
        namespace_instance_id: installation
            .namespace_instance_id
            .ok_or_else(|| SandboxError::CapabilityDenied(capability.clone()))?,
        data_contract_digest: crate::promotion::digest_json(contract)
            .map_err(|_| SandboxError::CapabilityDenied(capability.clone()))?,
        module_slug: authority.release.slug,
        data_contract_revision: contract.revision,
        policy_revision: authority.policy_revision,
    };
    scope
        .validate()
        .map_err(|_| SandboxError::CapabilityDenied(capability.clone()))?;
    Ok(scope)
}

pub fn validate_artifact_data_key(key: &str) -> Result<(), ArtifactDataError> {
    if key.is_empty()
        || key.len() > MAX_ARTIFACT_DATA_KEY_BYTES
        || key.starts_with('/')
        || key.split('/').any(|segment| {
            segment.is_empty() || segment == "." || segment == ".." || segment.contains('\\')
        })
    {
        return Err(ArtifactDataError::InvalidKey);
    }
    Ok(())
}

pub fn validate_artifact_data_prefix(prefix: &str) -> Result<(), ArtifactDataError> {
    let key = prefix
        .strip_suffix('/')
        .filter(|key| !key.ends_with('/'))
        .ok_or(ArtifactDataError::InvalidKey)?;
    validate_artifact_data_key(key)
}
