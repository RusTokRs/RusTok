use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use bytes::Bytes;
use jsonschema::{Draft, PatternOptions};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, sync::Arc};
use thiserror::Error;
use uuid::Uuid;

use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::OutboxTransport;
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityResponse, ExecutionPhase,
    SandboxError, SandboxResult, SandboxSubject,
};
use rustok_storage::StorageService;

use crate::{
    resolve_granted_artifact_capability, ArtifactBindingDispatch, ArtifactBindingExecutor,
    ArtifactCapabilityBrokerResolver, ArtifactCapabilityExecution, ArtifactInstallationTarget,
    ArtifactMigrationCheckpointRequest, ArtifactReleaseRef, InstalledModuleArtifact,
    ModuleArtifactDescriptor, ModuleInstallationScope, ModuleRuntimeBinding,
    ModuleRuntimeBindingKind,
};

const MAX_ARTIFACT_DATA_KEY_BYTES: usize = 256;
const MAX_ARTIFACT_DATA_VALUE_BYTES: usize = 64 * 1024;
const MAX_ARTIFACT_DATA_PAGE_SIZE: u32 = 100;
const MAX_ARTIFACT_DATA_BATCH_SIZE: usize = 32;
const MAX_DATA_SCHEMA_REGEX_BYTES: usize = 64 * 1024;
const MAX_ARTIFACT_OBJECT_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ARTIFACT_OBJECT_CONTENT_TYPE_BYTES: usize = 128;
const MAX_SANDBOX_ARTIFACT_OBJECT_BYTES: usize = 44 * 1024;

/// Host-owned namespace for untrusted artifact data. Guests never supply a
/// physical table, bucket, database schema, or secret-store location.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataScope {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub data_contract_revision: u64,
    pub policy_revision: u64,
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

impl ArtifactDataObject {
    pub fn validate(&self) -> Result<(), ArtifactDataError> {
        if validate_artifact_data_key(&self.name).is_err()
            || self.content_type.trim().is_empty()
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
    List,
    ObjectRead { name: String },
    ObjectWrite { name: String },
    ObjectList,
}

/// An explicit destructive command. The authorizer receives this exact request
/// and is responsible for lifecycle, retention, legal-hold, and actor policy
/// checks before the database transaction begins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataPurgeRequest {
    pub scope: ArtifactDataScope,
    pub expected_namespace_revision: u64,
    pub actor_id: Uuid,
    pub reason: String,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataPurgeResult {
    pub namespace_revision: u64,
    pub purged_records: u64,
}

impl ArtifactDataScope {
    pub fn validate(&self) -> Result<(), ArtifactDataError> {
        if self.tenant_id.is_nil()
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
    if installation.installation_id != execution.installation_id
        || installation.release.slug != execution.slug
        || installation.release.version != execution.version
        || installation.release.digest != execution.digest
        || installation.descriptor.slug != execution.slug
        || installation.descriptor.version != execution.version
        || installation.descriptor.artifact_digest != execution.digest
    {
        return Err(SandboxError::CapabilityDenied(capability.clone()));
    }
    let contract = installation
        .descriptor
        .persistence_contract
        .as_ref()
        .ok_or_else(|| SandboxError::CapabilityDenied(capability.clone()))?;
    let scope = ArtifactDataScope {
        tenant_id: execution.tenant_id,
        module_slug: installation.descriptor.slug.clone(),
        data_contract_revision: contract.revision,
        policy_revision: installation.capability_grant_revision,
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

fn validate_artifact_data_value(value: &Value) -> Result<(), ArtifactDataError> {
    let encoded =
        serde_json::to_vec(value).map_err(|error| ArtifactDataError::Storage(error.to_string()))?;
    if encoded.len() > MAX_ARTIFACT_DATA_VALUE_BYTES {
        return Err(ArtifactDataError::ValueTooLarge {
            limit: MAX_ARTIFACT_DATA_VALUE_BYTES,
            actual: encoded.len(),
        });
    }
    Ok(())
}

fn validate_artifact_data_batch(batch: &ArtifactDataBatchWrite) -> Result<(), ArtifactDataError> {
    if batch.writes.is_empty() || batch.writes.len() > MAX_ARTIFACT_DATA_BATCH_SIZE {
        return Err(ArtifactDataError::InvalidBatch);
    }
    let mut keys = HashSet::with_capacity(batch.writes.len());
    let mut idempotency_keys = HashSet::with_capacity(batch.writes.len());
    for write in &batch.writes {
        validate_artifact_data_key(&write.key)?;
        validate_artifact_data_value(&write.value)?;
        if write.idempotency_key.is_nil() {
            return Err(ArtifactDataError::InvalidIdempotencyKey);
        }
        if !keys.insert(&write.key) || !idempotency_keys.insert(write.idempotency_key) {
            return Err(ArtifactDataError::InvalidBatch);
        }
    }
    Ok(())
}

fn validate_page_request(page: &ArtifactDataPageRequest) -> Result<(), ArtifactDataError> {
    validate_artifact_data_prefix(&page.prefix)?;
    if page.limit == 0 || page.limit > MAX_ARTIFACT_DATA_PAGE_SIZE {
        return Err(ArtifactDataError::InvalidPage);
    }
    if let Some(after_key) = &page.after_key {
        validate_artifact_data_key(after_key)?;
        if !after_key.starts_with(&page.prefix) {
            return Err(ArtifactDataError::InvalidPage);
        }
    }
    Ok(())
}

fn valid_module_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && !value.starts_with('_')
        && !value.ends_with('_')
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn valid_upgrade_hook_binding_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-' | '.')
        })
}

fn prefixed_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn object_for_upload(
    upload: &ArtifactDataObjectUpload,
) -> Result<ArtifactDataObject, ArtifactDataError> {
    if upload.idempotency_key.is_nil() {
        return Err(ArtifactDataError::InvalidIdempotencyKey);
    }
    let size_bytes =
        u64::try_from(upload.data.len()).map_err(|_| ArtifactDataError::InvalidObject)?;
    let object = ArtifactDataObject {
        name: upload.name.clone(),
        content_type: upload.content_type.clone(),
        size_bytes,
        digest_sha256: format!("sha256:{}", hex::encode(Sha256::digest(&upload.data))),
        revision: 1,
    };
    object.validate()?;
    if upload.expected_revision == Some(0) {
        return Err(ArtifactDataError::RevisionConflict);
    }
    Ok(object)
}

/// Read and write calls are self-contained owner operations. An implementation
/// must finish any storage transaction before returning; it must not expose a
/// live transaction to a caller that may invoke untrusted code next.
#[async_trait]
pub trait ArtifactDataBroker: Send + Sync {
    async fn get(
        &self,
        scope: &ArtifactDataScope,
        key: &str,
    ) -> Result<Option<ArtifactDataRecord>, ArtifactDataError>;

    async fn put(
        &self,
        scope: &ArtifactDataScope,
        write: ArtifactDataWrite,
    ) -> Result<ArtifactDataRecord, ArtifactDataError>;

    async fn put_batch(
        &self,
        scope: &ArtifactDataScope,
        batch: ArtifactDataBatchWrite,
    ) -> Result<Vec<ArtifactDataRecord>, ArtifactDataError>;

    async fn list(
        &self,
        scope: &ArtifactDataScope,
        page: ArtifactDataPageRequest,
    ) -> Result<ArtifactDataPage, ArtifactDataError>;
}

/// Owner-owned broker for bounded binary artifact data. Its public contract
/// contains logical names and verified bytes only; storage keys are private
/// implementation details and never cross this boundary.
#[async_trait]
pub trait ArtifactDataObjectBroker: Send + Sync {
    async fn get_object(
        &self,
        scope: &ArtifactDataScope,
        name: &str,
    ) -> Result<Option<ArtifactDataObject>, ArtifactDataError>;

    async fn read_object(
        &self,
        scope: &ArtifactDataScope,
        name: &str,
    ) -> Result<Option<ArtifactDataObjectContent>, ArtifactDataError>;

    async fn put_object(
        &self,
        scope: &ArtifactDataScope,
        upload: ArtifactDataObjectUpload,
    ) -> Result<ArtifactDataObject, ArtifactDataError>;

    async fn list_objects(
        &self,
        scope: &ArtifactDataScope,
        page: ArtifactDataPageRequest,
    ) -> Result<ArtifactDataObjectPage, ArtifactDataError>;
}

/// Host-owned invocation of a pre-bound sandbox transformation. The hook has
/// no storage handle and receives one record at a time.
#[async_trait]
pub trait ArtifactDataUpgradeHook: Send + Sync {
    async fn transform_data(
        &self,
        hook_binding_id: &str,
        input: ArtifactDataUpgradeInput,
    ) -> Result<Value, ArtifactDataError>;
}

/// Production bridge from a dedicated admitted `data_upgrade` binding to the
/// data-contract planner. The binding is deliberately unavailable through the
/// generic dispatcher: only this owner-owned adapter may invoke it.
pub struct ArtifactBindingDataUpgradeHook<E> {
    executor: E,
    release: ArtifactReleaseRef,
    binding: ModuleRuntimeBinding,
}

impl<E> ArtifactBindingDataUpgradeHook<E> {
    pub fn new(
        executor: E,
        release: ArtifactReleaseRef,
        binding: ModuleRuntimeBinding,
    ) -> Result<Self, ArtifactDataError> {
        if binding.kind != ModuleRuntimeBindingKind::DataUpgrade || binding.id.is_empty() {
            return Err(ArtifactDataError::InvalidUpgrade);
        }
        Ok(Self {
            executor,
            release,
            binding,
        })
    }
}

#[async_trait]
impl<E> ArtifactDataUpgradeHook for ArtifactBindingDataUpgradeHook<E>
where
    E: ArtifactBindingExecutor,
{
    async fn transform_data(
        &self,
        hook_binding_id: &str,
        input: ArtifactDataUpgradeInput,
    ) -> Result<Value, ArtifactDataError> {
        if hook_binding_id != self.binding.id
            || input.source.module_slug != self.release.slug
            || input.target.module_slug != self.release.slug
            || input.source.tenant_id != input.target.tenant_id
        {
            return Err(ArtifactDataError::InvalidUpgrade);
        }
        self.executor
            .dispatch_binding(ArtifactBindingDispatch {
                release: &self.release,
                binding: &self.binding,
                target: ArtifactInstallationTarget::CurrentRelease,
                tenant_id: input.source.tenant_id,
                input: json!({
                    "source": input.source,
                    "target": input.target,
                    "record": input.record,
                }),
                // `data_upgrade` is intentionally omitted from the public
                // generic dispatcher. This internal owner bridge uses the
                // neutral sandbox phase while the binding kind carries the
                // admission and authorization distinction.
                phase: ExecutionPhase::Manual,
                context: crate::ArtifactBindingExecutionContext::default(),
            })
            .await
            .map_err(ArtifactDataError::UpgradeHook)
    }
}

/// Policy evaluation is host-owned and request-scoped. The implementation can
/// bind actor, grants, quotas, and the admitted policy revision without giving
/// an artifact a direct handle to any of those systems.
#[async_trait]
pub trait ArtifactDataAuthorizer: Send + Sync {
    async fn authorize_data(
        &self,
        scope: &ArtifactDataScope,
        access: ArtifactDataAccess,
    ) -> Result<(), ArtifactDataError>;
}

/// The host resolves the admitted data-contract schema and validates a bounded
/// structured value before it becomes durable. Production adapters must use the
/// maintained `jsonschema` validator with bounded regular-expression settings;
/// artifacts never supply an executable validator or schema location.
#[async_trait]
pub trait ArtifactDataSchemaValidator: Send + Sync {
    async fn validate_data_value(
        &self,
        scope: &ArtifactDataScope,
        value: &Value,
    ) -> Result<(), ArtifactDataError>;
}

/// Resolves a data-contract schema only from the descriptor persisted with the
/// exact injected installation. It accepts an admitted lifecycle state so an
/// owner can validate a target contract before activation; the separate data
/// authorizer remains responsible for operation lifecycle policy.
#[derive(Clone)]
pub struct SeaOrmArtifactDataSchemaValidator {
    db: DatabaseConnection,
    installation_id: Uuid,
}

impl SeaOrmArtifactDataSchemaValidator {
    /// The host injects the exact immutable installation selected for this
    /// data broker. It is never derived from the module slug at validation
    /// time and never crosses the artifact capability boundary.
    pub fn new(db: DatabaseConnection, installation_id: Uuid) -> Self {
        Self {
            db,
            installation_id,
        }
    }

    async fn data_contract_schema(
        &self,
        scope: &ArtifactDataScope,
    ) -> Result<Value, ArtifactDataError> {
        scope.validate()?;
        if self.installation_id.is_nil() {
            return Err(ArtifactDataError::DataContractUnavailable);
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        let placeholders = match backend {
            DbBackend::Postgres => ("$1", "$2", "$3"),
            _ => ("?1", "?2", "?3"),
        };
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT installation.descriptor \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission \
                       ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id = {} \
                       AND installation.slug = {} \
                       AND (installation.scope_kind = 'platform' OR installation.tenant_id = {}) \
                       AND admission.status IN ('admitted', 'installed', 'active', 'inactive')",
                    placeholders.0, placeholders.1, placeholders.2,
                ),
                vec![
                    uuid_value(self.installation_id, backend),
                    scope.module_slug.clone().into(),
                    uuid_value(scope.tenant_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?
            .ok_or(ArtifactDataError::DataContractUnavailable)?;
        transaction.commit().await.map_err(storage_error)?;

        let descriptor_value: Value = row.try_get("", "descriptor").map_err(storage_error)?;
        let descriptor =
            serde_json::from_value::<crate::ModuleArtifactDescriptor>(descriptor_value)
                .map_err(|_| ArtifactDataError::DataContractUnavailable)?;
        descriptor
            .validate()
            .map_err(|_| ArtifactDataError::DataContractUnavailable)?;
        let contract = descriptor
            .persistence_contract
            .as_ref()
            .filter(|contract| contract.revision == scope.data_contract_revision)
            .ok_or(ArtifactDataError::DataContractUnavailable)?;
        descriptor
            .schema_document(&contract.schema_digest)
            .cloned()
            .ok_or(ArtifactDataError::DataContractUnavailable)
    }
}

#[async_trait]
impl ArtifactDataSchemaValidator for SeaOrmArtifactDataSchemaValidator {
    async fn validate_data_value(
        &self,
        scope: &ArtifactDataScope,
        value: &Value,
    ) -> Result<(), ArtifactDataError> {
        let schema = self.data_contract_schema(scope).await?;
        let validator = jsonschema::options()
            .with_draft(Draft::Draft202012)
            .should_validate_formats(true)
            .should_ignore_unknown_formats(false)
            .with_pattern_options(
                PatternOptions::fancy_regex()
                    .backtrack_limit(MAX_DATA_SCHEMA_REGEX_BYTES)
                    .dfa_size_limit(MAX_DATA_SCHEMA_REGEX_BYTES),
            )
            .build(&schema)
            .map_err(|_| ArtifactDataError::DataContractSchemaInvalid)?;
        validator
            .validate(value)
            .map_err(|_| ArtifactDataError::DataContractSchemaViolation)
    }
}

/// The host owns destructive-data authority. An artifact cannot supply an
/// implementation or replace this check through its broker capability.
#[async_trait]
pub trait ArtifactDataPurgeAuthorizer: Send + Sync {
    async fn authorize_purge(
        &self,
        request: &ArtifactDataPurgeRequest,
    ) -> Result<(), ArtifactDataError>;
}

/// Produces non-durable data-contract upgrade plans. The data broker call is
/// complete before any sandbox hook begins, so an untrusted transformation can
/// never run while a control-plane or storage transaction is held open.
#[derive(Clone)]
pub struct ArtifactDataUpgradePlanner<B, H, V> {
    data: B,
    hook: H,
    schema_validator: V,
}

impl<B, H, V> ArtifactDataUpgradePlanner<B, H, V>
where
    B: ArtifactDataBroker,
    H: ArtifactDataUpgradeHook,
    V: ArtifactDataSchemaValidator,
{
    pub fn new(data: B, hook: H, schema_validator: V) -> Self {
        Self {
            data,
            hook,
            schema_validator,
        }
    }

    pub async fn plan(
        &self,
        request: ArtifactDataUpgradeRequest,
    ) -> Result<ArtifactDataUpgradePlan, ArtifactDataError> {
        validate_upgrade_request(&request)?;

        // `list` finishes its bounded read before this await resolves. Do not
        // move transformation or a later write into a broker transaction.
        let page = self.data.list(&request.source, request.page).await?;
        let mut records = Vec::with_capacity(page.records.len());
        for record in page.records {
            let source_revision = record.revision;
            let key = record.key.clone();
            let value = self
                .hook
                .transform_data(
                    &request.hook_binding_id,
                    ArtifactDataUpgradeInput {
                        source: request.source.clone(),
                        target: request.target.clone(),
                        record,
                    },
                )
                .await?;
            validate_artifact_data_value(&value)?;
            self.schema_validator
                .validate_data_value(&request.target, &value)
                .await?;
            records.push(ArtifactDataUpgradeRecord {
                key,
                value,
                source_revision,
            });
        }

        Ok(ArtifactDataUpgradePlan {
            plan_id: request.plan_id,
            target_installation_id: request.target_installation_id,
            source: request.source,
            target: request.target,
            hook_binding_id: request.hook_binding_id,
            records,
            next_after_key: page.next_after_key,
        })
    }
}

fn validate_upgrade_request(request: &ArtifactDataUpgradeRequest) -> Result<(), ArtifactDataError> {
    request.source.validate()?;
    request.target.validate()?;
    validate_page_request(&request.page)?;
    if request.plan_id.is_nil()
        || request.target_installation_id.is_nil()
        || !valid_upgrade_hook_binding_id(&request.hook_binding_id)
        || request.source.tenant_id != request.target.tenant_id
        || request.source.module_slug != request.target.module_slug
        || request.target.data_contract_revision <= request.source.data_contract_revision
    {
        return Err(ArtifactDataError::InvalidUpgrade);
    }
    Ok(())
}

/// Owner-owned installation checkpoint boundary. Implementations must retain
/// the installation revision CAS and transactional outbox semantics.
#[async_trait]
pub trait ArtifactDataMigrationCheckpointStore: Send + Sync {
    async fn record_data_upgrade_checkpoint(
        &self,
        request: ArtifactMigrationCheckpointRequest,
    ) -> Result<u64, ArtifactDataError>;
}

/// Applies a bounded plan without opening a control-plane transaction across
/// source reads, target writes, or checkpointing. Repeating the same plan ID
/// produces the same per-record idempotency keys, so a retry resumes a partial
/// attempt before creating the installation checkpoint.
#[derive(Clone)]
pub struct ArtifactDataUpgradeApplier<B, C> {
    data: B,
    checkpoints: C,
}

impl<B, C> ArtifactDataUpgradeApplier<B, C>
where
    B: ArtifactDataBroker,
    C: ArtifactDataMigrationCheckpointStore,
{
    pub fn new(data: B, checkpoints: C) -> Self {
        Self { data, checkpoints }
    }

    pub async fn apply(
        &self,
        request: ArtifactDataUpgradeApplyRequest,
    ) -> Result<ArtifactDataUpgradeApplyResult, ArtifactDataError> {
        validate_upgrade_apply_request(&request)?;
        let mut records = Vec::with_capacity(request.plan.records.len());
        for record in &request.plan.records {
            let current = self.data.get(&request.plan.source, &record.key).await?;
            if !matches!(current, Some(ref current) if current.revision == record.source_revision) {
                return Err(ArtifactDataError::StaleUpgradePlan);
            }
            let persisted = self
                .data
                .put(
                    &request.plan.target,
                    ArtifactDataWrite {
                        key: record.key.clone(),
                        value: record.value.clone(),
                        expected_revision: None,
                        create_only: true,
                        idempotency_key: upgrade_record_idempotency_key(
                            request.plan.plan_id,
                            &request.plan.target,
                            record,
                        ),
                    },
                )
                .await?;
            records.push(persisted);
        }
        let checkpoint = json!({
            "kind": "artifact_data_upgrade",
            "plan_id": request.plan.plan_id,
            "hook_binding_id": request.plan.hook_binding_id,
            "source": {
                "module_slug": request.plan.source.module_slug,
                "data_contract_revision": request.plan.source.data_contract_revision,
            },
            "target": {
                "module_slug": request.plan.target.module_slug,
                "data_contract_revision": request.plan.target.data_contract_revision,
            },
            "records_applied": records.len(),
            "next_after_key": request.plan.next_after_key,
        });
        let installation_revision = self
            .checkpoints
            .record_data_upgrade_checkpoint(ArtifactMigrationCheckpointRequest {
                installation_id: request.plan.target_installation_id,
                scope: request.installation_scope,
                expected_revision: request.expected_installation_revision,
                checkpoint,
                has_irreversible_migration: request.has_irreversible_migration,
            })
            .await?;
        Ok(ArtifactDataUpgradeApplyResult {
            records,
            installation_revision,
        })
    }
}

fn validate_upgrade_apply_request(
    request: &ArtifactDataUpgradeApplyRequest,
) -> Result<(), ArtifactDataError> {
    validate_upgrade_plan(&request.plan)?;
    if request.expected_installation_revision == 0 {
        return Err(ArtifactDataError::InvalidUpgrade);
    }
    Ok(())
}

fn validate_upgrade_plan(plan: &ArtifactDataUpgradePlan) -> Result<(), ArtifactDataError> {
    if plan.plan_id.is_nil()
        || plan.target_installation_id.is_nil()
        || !valid_upgrade_hook_binding_id(&plan.hook_binding_id)
        || plan.source.tenant_id != plan.target.tenant_id
        || plan.source.module_slug != plan.target.module_slug
        || plan.target.data_contract_revision <= plan.source.data_contract_revision
    {
        return Err(ArtifactDataError::InvalidUpgrade);
    }
    plan.source.validate()?;
    plan.target.validate()?;
    for (index, record) in plan.records.iter().enumerate() {
        validate_artifact_data_key(&record.key)?;
        validate_artifact_data_value(&record.value)?;
        if record.source_revision == 0
            || plan.records[..index]
                .iter()
                .any(|previous| previous.key == record.key)
        {
            return Err(ArtifactDataError::InvalidUpgrade);
        }
    }
    Ok(())
}

fn upgrade_record_idempotency_key(
    plan_id: Uuid,
    target: &ArtifactDataScope,
    record: &ArtifactDataUpgradeRecord,
) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(plan_id.as_bytes());
    hasher.update(target.tenant_id.as_bytes());
    hasher.update(target.module_slug.as_bytes());
    hasher.update(target.data_contract_revision.to_be_bytes());
    hasher.update(record.key.as_bytes());
    hasher.update(record.source_revision.to_be_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    Uuid::from_bytes(bytes)
}

/// SeaORM adapter for the host-owned structured-value namespace. It never
/// accepts a guest-selected database object, SQL fragment, or storage path.
#[derive(Clone)]
pub struct SeaOrmArtifactDataBroker<A, V> {
    db: DatabaseConnection,
    authorizer: A,
    schema_validator: V,
}

impl<A, V> SeaOrmArtifactDataBroker<A, V>
where
    A: ArtifactDataAuthorizer,
    V: ArtifactDataSchemaValidator,
{
    pub fn new(db: DatabaseConnection, authorizer: A, schema_validator: V) -> Self {
        Self {
            db,
            authorizer,
            schema_validator,
        }
    }
}

#[async_trait]
impl<A, V> ArtifactDataBroker for SeaOrmArtifactDataBroker<A, V>
where
    A: ArtifactDataAuthorizer,
    V: ArtifactDataSchemaValidator,
{
    async fn get(
        &self,
        scope: &ArtifactDataScope,
        key: &str,
    ) -> Result<Option<ArtifactDataRecord>, ArtifactDataError> {
        scope.validate()?;
        validate_artifact_data_key(key)?;
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::Read {
                    key: key.to_owned(),
                },
            )
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT data_key, value, revision FROM module_artifact_data
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND data_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                scope_values(scope, backend, key)?,
            ))
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        row.map(record_from_row).transpose()
    }

    async fn put(
        &self,
        scope: &ArtifactDataScope,
        write: ArtifactDataWrite,
    ) -> Result<ArtifactDataRecord, ArtifactDataError> {
        scope.validate()?;
        validate_artifact_data_key(&write.key)?;
        validate_artifact_data_value(&write.value)?;
        if write.idempotency_key.is_nil() {
            return Err(ArtifactDataError::InvalidIdempotencyKey);
        }
        self.schema_validator
            .validate_data_value(scope, &write.value)
            .await?;
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::Write {
                    key: write.key.clone(),
                },
            )
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let record = persist_artifact_data_write(&transaction, scope, write).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(record)
    }

    async fn put_batch(
        &self,
        scope: &ArtifactDataScope,
        batch: ArtifactDataBatchWrite,
    ) -> Result<Vec<ArtifactDataRecord>, ArtifactDataError> {
        scope.validate()?;
        validate_artifact_data_batch(&batch)?;
        for write in &batch.writes {
            self.schema_validator
                .validate_data_value(scope, &write.value)
                .await?;
            self.authorizer
                .authorize_data(
                    scope,
                    ArtifactDataAccess::Write {
                        key: write.key.clone(),
                    },
                )
                .await?;
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let mut records = Vec::with_capacity(batch.writes.len());
        for write in batch.writes {
            records.push(persist_artifact_data_write(&transaction, scope, write).await?);
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(records)
    }

    async fn list(
        &self,
        scope: &ArtifactDataScope,
        page: ArtifactDataPageRequest,
    ) -> Result<ArtifactDataPage, ArtifactDataError> {
        scope.validate()?;
        validate_page_request(&page)?;
        self.authorizer
            .authorize_data(scope, ArtifactDataAccess::List)
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        let query_limit = i64::from(page.limit) + 1;
        let prefix_pattern = format!("{}%", escape_like_prefix(&page.prefix));
        let (query, values) = match page.after_key {
            Some(after_key) => (
                format!(
                    "SELECT data_key, value, revision FROM module_artifact_data
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                     AND data_key LIKE {} ESCAPE '\\' AND data_key > {}
                     ORDER BY data_key ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    scope.module_slug.clone().into(),
                    revision_value(scope.data_contract_revision)?,
                    prefix_pattern.clone().into(),
                    after_key.into(),
                    query_limit.into(),
                ],
            ),
            None => (
                format!(
                    "SELECT data_key, value, revision FROM module_artifact_data
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                     AND data_key LIKE {} ESCAPE '\\'
                     ORDER BY data_key ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    scope.module_slug.clone().into(),
                    revision_value(scope.data_contract_revision)?,
                    prefix_pattern.into(),
                    query_limit.into(),
                ],
            ),
        };
        let mut records = transaction
            .query_all(Statement::from_sql_and_values(backend, query, values))
            .await
            .map_err(storage_error)?
            .into_iter()
            .map(record_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        transaction.commit().await.map_err(storage_error)?;
        let next_after_key = if records.len() > page.limit as usize {
            records.truncate(page.limit as usize);
            records.last().map(|record| record.key.clone())
        } else {
            None
        };
        Ok(ArtifactDataPage {
            records,
            next_after_key,
        })
    }
}

/// SeaORM and storage implementation of the private artifact object broker.
/// The generated path is intentionally not configurable through any artifact
/// command, metadata field, or capability payload.
#[derive(Clone)]
pub struct SeaOrmArtifactDataObjectBroker<A> {
    db: DatabaseConnection,
    storage: StorageService,
    authorizer: A,
}

impl<A> SeaOrmArtifactDataObjectBroker<A>
where
    A: ArtifactDataAuthorizer,
{
    pub fn new(db: DatabaseConnection, storage: StorageService, authorizer: A) -> Self {
        Self {
            db,
            storage,
            authorizer,
        }
    }
}

#[async_trait]
impl<A> ArtifactDataObjectBroker for SeaOrmArtifactDataObjectBroker<A>
where
    A: ArtifactDataAuthorizer,
{
    async fn get_object(
        &self,
        scope: &ArtifactDataScope,
        name: &str,
    ) -> Result<Option<ArtifactDataObject>, ArtifactDataError> {
        scope.validate()?;
        validate_artifact_data_key(name)?;
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::ObjectRead {
                    name: name.to_owned(),
                },
            )
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let object = find_artifact_data_object(&transaction, scope, name).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(object.map(|stored| stored.object))
    }

    async fn read_object(
        &self,
        scope: &ArtifactDataScope,
        name: &str,
    ) -> Result<Option<ArtifactDataObjectContent>, ArtifactDataError> {
        scope.validate()?;
        validate_artifact_data_key(name)?;
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::ObjectRead {
                    name: name.to_owned(),
                },
            )
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let stored = find_artifact_data_object(&transaction, scope, name).await?;
        transaction.commit().await.map_err(storage_error)?;
        let Some(stored) = stored else {
            return Ok(None);
        };
        let data = self
            .storage
            .read(&stored.storage_key)
            .await
            .map_err(storage_error)?;
        if u64::try_from(data.len()).ok() != Some(stored.object.size_bytes)
            || format!("sha256:{}", hex::encode(Sha256::digest(&data)))
                != stored.object.digest_sha256
        {
            return Err(ArtifactDataError::ObjectIntegrity);
        }
        Ok(Some(ArtifactDataObjectContent {
            object: stored.object,
            data,
        }))
    }

    async fn put_object(
        &self,
        scope: &ArtifactDataScope,
        upload: ArtifactDataObjectUpload,
    ) -> Result<ArtifactDataObject, ArtifactDataError> {
        scope.validate()?;
        let requested = object_for_upload(&upload)?;
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::ObjectWrite {
                    name: requested.name.clone(),
                },
            )
            .await?;

        if let Some(existing) = self
            .find_object_operation(scope, &upload, &requested)
            .await?
        {
            return Ok(existing);
        }

        let generated_key = private_artifact_data_object_key(scope);
        let uploaded = self
            .storage
            .store(&generated_key, upload.data.clone(), &requested.content_type)
            .await
            .map_err(storage_error)?;
        if uploaded.size != requested.size_bytes {
            let _ = self.storage.delete(&uploaded.path).await;
            return Err(ArtifactDataError::ObjectIntegrity);
        }

        let transaction = match self.db.begin().await.map_err(storage_error) {
            Ok(transaction) => transaction,
            Err(error) => {
                let _ = self.storage.delete(&uploaded.path).await;
                return Err(error);
            }
        };
        if let Err(error) = configure_tenant_scope(&transaction, scope.tenant_id).await {
            let _ = self.storage.delete(&uploaded.path).await;
            return Err(error);
        }
        let stored = match persist_artifact_data_object(
            &transaction,
            scope,
            &upload,
            &requested,
            &uploaded.path,
        )
        .await
        {
            Ok(stored) => stored,
            Err(error) => {
                let _ = self.storage.delete(&uploaded.path).await;
                return Err(error);
            }
        };
        if let Err(error) = transaction.commit().await.map_err(storage_error) {
            // A commit error can represent an unknown outcome. Retain this
            // owner-generated object for retention/GC reconciliation instead
            // of risking a delete of a now-committed object.
            return Err(error);
        }
        if stored.storage_key != uploaded.path {
            let _ = self.storage.delete(&uploaded.path).await;
        }
        Ok(stored.object)
    }

    async fn list_objects(
        &self,
        scope: &ArtifactDataScope,
        page: ArtifactDataPageRequest,
    ) -> Result<ArtifactDataObjectPage, ArtifactDataError> {
        scope.validate()?;
        validate_page_request(&page)?;
        self.authorizer
            .authorize_data(scope, ArtifactDataAccess::ObjectList)
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        let limit = i64::from(page.limit) + 1;
        let prefix = format!("{}%", escape_like_prefix(&page.prefix));
        let (query, values) = match page.after_key {
            Some(after_name) => (
                format!(
                    "SELECT object_name, content_type, size_bytes, digest_sha256, revision, storage_key
                     FROM module_artifact_data_objects
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                     AND object_name LIKE {} ESCAPE '\\' AND object_name > {}
                     ORDER BY object_name ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    scope.module_slug.clone().into(),
                    revision_value(scope.data_contract_revision)?,
                    prefix.clone().into(),
                    after_name.into(),
                    limit.into(),
                ],
            ),
            None => (
                format!(
                    "SELECT object_name, content_type, size_bytes, digest_sha256, revision, storage_key
                     FROM module_artifact_data_objects
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                     AND object_name LIKE {} ESCAPE '\\'
                     ORDER BY object_name ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    scope.module_slug.clone().into(),
                    revision_value(scope.data_contract_revision)?,
                    prefix.into(),
                    limit.into(),
                ],
            ),
        };
        let mut objects = transaction
            .query_all(Statement::from_sql_and_values(backend, query, values))
            .await
            .map_err(storage_error)?
            .into_iter()
            .map(stored_artifact_data_object_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        transaction.commit().await.map_err(storage_error)?;
        let next_after_name = if objects.len() > page.limit as usize {
            objects.truncate(page.limit as usize);
            objects.last().map(|object| object.object.name.clone())
        } else {
            None
        };
        Ok(ArtifactDataObjectPage {
            objects: objects.into_iter().map(|stored| stored.object).collect(),
            next_after_name,
        })
    }
}

impl<A> SeaOrmArtifactDataObjectBroker<A>
where
    A: ArtifactDataAuthorizer,
{
    async fn find_object_operation(
        &self,
        scope: &ArtifactDataScope,
        upload: &ArtifactDataObjectUpload,
        requested: &ArtifactDataObject,
    ) -> Result<Option<ArtifactDataObject>, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        ensure_active_namespace(&transaction, scope, backend).await?;
        let operation =
            find_artifact_data_object_operation(&transaction, scope, upload.idempotency_key)
                .await?;
        transaction.commit().await.map_err(storage_error)?;
        let Some((stored, expected_revision)) = operation else {
            return Ok(None);
        };
        validate_object_operation(&stored, upload, requested, expected_revision)?;
        Ok(Some(stored.object))
    }
}

#[derive(Clone)]
struct StoredArtifactDataObject {
    object: ArtifactDataObject,
    storage_key: String,
}

async fn find_artifact_data_object<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    name: &str,
) -> Result<Option<StoredArtifactDataObject>, ArtifactDataError> {
    let backend = connection.get_database_backend();
    connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT object_name, content_type, size_bytes, digest_sha256, revision, storage_key
                 FROM module_artifact_data_objects
                 WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND object_name = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
            ),
            scope_values(scope, backend, name)?,
        ))
        .await
        .map_err(storage_error)?
        .map(stored_artifact_data_object_from_row)
        .transpose()
}

async fn persist_artifact_data_object(
    transaction: &DatabaseTransaction,
    scope: &ArtifactDataScope,
    upload: &ArtifactDataObjectUpload,
    requested: &ArtifactDataObject,
    storage_key: &str,
) -> Result<StoredArtifactDataObject, ArtifactDataError> {
    let backend = transaction.get_database_backend();
    ensure_active_namespace(transaction, scope, backend).await?;
    if let Some((existing, expected_revision)) =
        find_artifact_data_object_operation(transaction, scope, upload.idempotency_key).await?
    {
        validate_object_operation(&existing, upload, requested, expected_revision)?;
        return Ok(existing);
    }
    let current = find_artifact_data_object(transaction, scope, &requested.name).await?;
    let revision = match current {
        Some(current) => {
            if upload.expected_revision != Some(current.object.revision) {
                return Err(ArtifactDataError::RevisionConflict);
            }
            let revision = current
                .object
                .revision
                .checked_add(1)
                .ok_or(ArtifactDataError::RevisionConflict)?;
            let result = transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE module_artifact_data_objects
                         SET storage_key = {}, content_type = {}, size_bytes = {}, digest_sha256 = {}, revision = {}, updated_at = {}
                         WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND object_name = {} AND revision = {}",
                        placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
                        placeholder(backend, 4), placeholder(backend, 5), now_expression(backend),
                        placeholder(backend, 6), placeholder(backend, 7), placeholder(backend, 8),
                        placeholder(backend, 9), placeholder(backend, 10),
                    ),
                    vec![
                        storage_key.to_owned().into(), requested.content_type.clone().into(),
                        revision_value(requested.size_bytes)?, requested.digest_sha256.clone().into(),
                        revision_value(revision)?, uuid_value(scope.tenant_id, backend),
                        scope.module_slug.clone().into(), revision_value(scope.data_contract_revision)?,
                        requested.name.clone().into(), revision_value(current.object.revision)?,
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if result.rows_affected() != 1 {
                return Err(ArtifactDataError::RevisionConflict);
            }
            revision
        }
        None => {
            if upload.expected_revision.is_some() {
                return Err(ArtifactDataError::RevisionConflict);
            }
            let result = transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "INSERT INTO module_artifact_data_objects
                         (tenant_id, module_slug, data_contract_revision, object_name, storage_key, content_type, size_bytes, digest_sha256, revision, created_at, updated_at)
                         VALUES ({}, {}, {}, {}, {}, {}, {}, {}, 1, {}, {}) ON CONFLICT DO NOTHING",
                        placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
                        placeholder(backend, 4), placeholder(backend, 5), placeholder(backend, 6),
                        placeholder(backend, 7), placeholder(backend, 8), now_expression(backend), now_expression(backend),
                    ),
                    vec![
                        uuid_value(scope.tenant_id, backend), scope.module_slug.clone().into(),
                        revision_value(scope.data_contract_revision)?, requested.name.clone().into(),
                        storage_key.to_owned().into(), requested.content_type.clone().into(),
                        revision_value(requested.size_bytes)?, requested.digest_sha256.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if result.rows_affected() != 1 {
                return Err(ArtifactDataError::RevisionConflict);
            }
            1
        }
    };
    let stored = StoredArtifactDataObject {
        object: ArtifactDataObject {
            name: requested.name.clone(),
            content_type: requested.content_type.clone(),
            size_bytes: requested.size_bytes,
            digest_sha256: requested.digest_sha256.clone(),
            revision,
        },
        storage_key: storage_key.to_owned(),
    };
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_data_object_operations
                 (tenant_id, module_slug, data_contract_revision, idempotency_key, object_name, storage_key, content_type, size_bytes, digest_sha256, expected_revision, revision, completed_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3), placeholder(backend, 4),
                placeholder(backend, 5), placeholder(backend, 6), placeholder(backend, 7), placeholder(backend, 8),
                placeholder(backend, 9), placeholder(backend, 10), placeholder(backend, 11), now_expression(backend),
            ),
            vec![
                uuid_value(scope.tenant_id, backend), scope.module_slug.clone().into(),
                revision_value(scope.data_contract_revision)?, uuid_value(upload.idempotency_key, backend),
                stored.object.name.clone().into(), stored.storage_key.clone().into(),
                stored.object.content_type.clone().into(), revision_value(stored.object.size_bytes)?,
                stored.object.digest_sha256.clone().into(), optional_revision_value(upload.expected_revision)?,
                revision_value(stored.object.revision)?,
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(stored)
}

async fn find_artifact_data_object_operation<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    idempotency_key: Uuid,
) -> Result<Option<(StoredArtifactDataObject, Option<u64>)>, ArtifactDataError> {
    let backend = connection.get_database_backend();
    connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT object_name, content_type, size_bytes, digest_sha256, revision, storage_key, expected_revision
                 FROM module_artifact_data_object_operations
                 WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND idempotency_key = {}",
                placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3), placeholder(backend, 4),
            ),
            vec![
                uuid_value(scope.tenant_id, backend), scope.module_slug.clone().into(),
                revision_value(scope.data_contract_revision)?, uuid_value(idempotency_key, backend),
            ],
        ))
        .await
        .map_err(storage_error)?
        .map(|row| {
            let expected_revision: Option<i64> = row
                .try_get("", "expected_revision")
                .map_err(storage_error)?;
            let expected_revision = expected_revision
                .map(u64::try_from)
                .transpose()
                .map_err(|_| ArtifactDataError::IdempotencyConflict)?;
            Ok((stored_artifact_data_object_from_row(row)?, expected_revision))
        })
        .transpose()
}

fn validate_object_operation(
    stored: &StoredArtifactDataObject,
    upload: &ArtifactDataObjectUpload,
    requested: &ArtifactDataObject,
    expected_revision: Option<u64>,
) -> Result<(), ArtifactDataError> {
    if stored.object.name != requested.name
        || stored.object.content_type != requested.content_type
        || stored.object.size_bytes != requested.size_bytes
        || stored.object.digest_sha256 != requested.digest_sha256
        || expected_revision != upload.expected_revision
    {
        return Err(ArtifactDataError::IdempotencyConflict);
    }
    Ok(())
}

fn stored_artifact_data_object_from_row(
    row: sea_orm::QueryResult,
) -> Result<StoredArtifactDataObject, ArtifactDataError> {
    let revision: i64 = row.try_get("", "revision").map_err(storage_error)?;
    let size_bytes: i64 = row.try_get("", "size_bytes").map_err(storage_error)?;
    let object = ArtifactDataObject {
        name: row.try_get("", "object_name").map_err(storage_error)?,
        content_type: row.try_get("", "content_type").map_err(storage_error)?,
        size_bytes: u64::try_from(size_bytes).map_err(|_| ArtifactDataError::ObjectIntegrity)?,
        digest_sha256: row.try_get("", "digest_sha256").map_err(storage_error)?,
        revision: u64::try_from(revision).map_err(|_| ArtifactDataError::ObjectIntegrity)?,
    };
    object.validate()?;
    Ok(StoredArtifactDataObject {
        object,
        storage_key: row.try_get("", "storage_key").map_err(storage_error)?,
    })
}

fn private_artifact_data_object_key(scope: &ArtifactDataScope) -> String {
    format!(
        "module-artifact-data/{}/{}/{}/{}",
        scope.tenant_id,
        scope.module_slug,
        scope.data_contract_revision,
        Uuid::new_v4(),
    )
}

async fn persist_artifact_data_write(
    transaction: &DatabaseTransaction,
    scope: &ArtifactDataScope,
    write: ArtifactDataWrite,
) -> Result<ArtifactDataRecord, ArtifactDataError> {
    let backend = transaction.get_database_backend();
    ensure_active_namespace(transaction, scope, backend).await?;
    if let Some(row) = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT data_key, value, revision, expected_revision FROM module_artifact_data_operations
                 WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND idempotency_key = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
            ),
            vec![
                uuid_value(scope.tenant_id, backend),
                scope.module_slug.clone().into(),
                revision_value(scope.data_contract_revision)?,
                uuid_value(write.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(storage_error)?
    {
        let expected_revision: Option<i64> = row
            .try_get("", "expected_revision")
            .map_err(storage_error)?;
        let record = record_from_row(row)?;
        if record.key != write.key
            || record.value != write.value
            || expected_revision
                .map(u64::try_from)
                .transpose()
                .map_err(|_| ArtifactDataError::IdempotencyConflict)?
                != write.expected_revision
        {
            return Err(ArtifactDataError::IdempotencyConflict);
        }
        return Ok(record);
    }

    let current = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT data_key, value, revision FROM module_artifact_data
                 WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND data_key = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
            ),
            scope_values(scope, backend, &write.key)?,
        ))
        .await
        .map_err(storage_error)?;
    let revision = if let Some(row) = current {
        let current = record_from_row(row)?;
        if write.create_only || write.expected_revision != Some(current.revision) {
            return Err(ArtifactDataError::RevisionConflict);
        }
        let next_revision = current
            .revision
            .checked_add(1)
            .ok_or(ArtifactDataError::RevisionConflict)?;
        let result = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data SET value = {}, revision = {}, updated_at = {}
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                     AND data_key = {} AND revision = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    now_expression(backend),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                ),
                vec![
                    SqlValue::Json(Some(Box::new(write.value.clone()))),
                    revision_value(next_revision)?,
                    uuid_value(scope.tenant_id, backend),
                    scope.module_slug.clone().into(),
                    revision_value(scope.data_contract_revision)?,
                    write.key.clone().into(),
                    revision_value(current.revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if result.rows_affected() != 1 {
            return Err(ArtifactDataError::RevisionConflict);
        }
        next_revision
    } else {
        if write.expected_revision.is_some() {
            return Err(ArtifactDataError::RevisionConflict);
        }
        let result = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_data
                     (tenant_id, module_slug, data_contract_revision, data_key, value, revision, updated_at)
                     VALUES ({}, {}, {}, {}, {}, 1, {}) ON CONFLICT DO NOTHING",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    now_expression(backend),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    scope.module_slug.clone().into(),
                    revision_value(scope.data_contract_revision)?,
                    write.key.clone().into(),
                    SqlValue::Json(Some(Box::new(write.value.clone()))),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if result.rows_affected() != 1 {
            return Err(ArtifactDataError::RevisionConflict);
        }
        1
    };
    let record = ArtifactDataRecord {
        key: write.key,
        value: write.value,
        revision,
    };
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_data_operations
                 (tenant_id, module_slug, data_contract_revision, idempotency_key, data_key, value, expected_revision, revision, completed_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {})",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
                now_expression(backend),
            ),
            vec![
                uuid_value(scope.tenant_id, backend),
                scope.module_slug.clone().into(),
                revision_value(scope.data_contract_revision)?,
                uuid_value(write.idempotency_key, backend),
                record.key.clone().into(),
                SqlValue::Json(Some(Box::new(record.value.clone()))),
                optional_revision_value(write.expected_revision)?,
                revision_value(record.revision)?,
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(record)
}

/// The `platform.data` adapter for one admitted artifact namespace. It is
/// injected into the neutral sandbox runtime and delegates all persistence,
/// policy, schema, and RLS enforcement to the owner data broker.
#[derive(Clone)]
pub struct SeaOrmArtifactDataCapabilityBroker<A, V> {
    data: SeaOrmArtifactDataBroker<A, V>,
    scope: ArtifactDataScope,
}

impl<A, V> SeaOrmArtifactDataCapabilityBroker<A, V>
where
    A: ArtifactDataAuthorizer,
    V: ArtifactDataSchemaValidator,
{
    pub fn new(
        db: DatabaseConnection,
        authorizer: A,
        schema_validator: V,
        scope: ArtifactDataScope,
    ) -> Self {
        Self {
            data: SeaOrmArtifactDataBroker::new(db, authorizer, schema_validator),
            scope,
        }
    }
}

#[async_trait]
impl<A, V> CapabilityBroker for SeaOrmArtifactDataCapabilityBroker<A, V>
where
    A: ArtifactDataAuthorizer,
    V: ArtifactDataSchemaValidator,
{
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        if call.capability.as_str() != "platform.data"
            || call.context.tenant_id != Some(self.scope.tenant_id)
            || !matches!(
                &call.subject,
                SandboxSubject::ModuleArtifact { slug, .. } if slug == &self.scope.module_slug
            )
        {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }
        match decode_data_capability_call(call)? {
            DataCapabilityCall::Get { key } => {
                let record = self
                    .data
                    .get(&self.scope, &key)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "record": record }),
                })
            }
            DataCapabilityCall::Put { write } => {
                let record = self
                    .data
                    .put(&self.scope, write)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "record": record }),
                })
            }
            DataCapabilityCall::PutBatch { batch } => {
                let records = self
                    .data
                    .put_batch(&self.scope, batch)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "records": records }),
                })
            }
            DataCapabilityCall::List { page } => {
                let page = self
                    .data
                    .list(&self.scope, page)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({
                        "records": page.records,
                        "next_after_key": page.next_after_key,
                    }),
                })
            }
        }
    }
}

/// Production resolver for the `platform.data` owner. It derives the complete
/// namespace from the exact sandbox installation identity and never accepts
/// tenant, module, revision, or schema information from an artifact call.
#[derive(Clone)]
pub struct SeaOrmArtifactDataCapabilityBrokerResolver {
    db: DatabaseConnection,
}

impl SeaOrmArtifactDataCapabilityBrokerResolver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[derive(Clone)]
struct ExactArtifactDataAuthorizer {
    scope: ArtifactDataScope,
}

#[async_trait]
impl ArtifactDataAuthorizer for ExactArtifactDataAuthorizer {
    async fn authorize_data(
        &self,
        scope: &ArtifactDataScope,
        _access: ArtifactDataAccess,
    ) -> Result<(), ArtifactDataError> {
        if scope == &self.scope {
            Ok(())
        } else {
            Err(ArtifactDataError::PolicyDenied)
        }
    }
}

#[async_trait]
impl ArtifactCapabilityBrokerResolver for SeaOrmArtifactDataCapabilityBrokerResolver {
    async fn resolve_broker(
        &self,
        execution: &ArtifactCapabilityExecution,
        capability: &rustok_sandbox::CapabilityName,
    ) -> SandboxResult<Arc<dyn CapabilityBroker>> {
        if capability.as_str() != "platform.data" {
            return Err(SandboxError::CapabilityDenied(capability.clone()));
        }
        let installation =
            resolve_granted_artifact_capability(&self.db, execution, capability).await?;
        let scope = artifact_data_scope_for_execution(&installation, execution, capability)?;
        let authorizer = ExactArtifactDataAuthorizer {
            scope: scope.clone(),
        };
        let schema_validator =
            SeaOrmArtifactDataSchemaValidator::new(self.db.clone(), execution.installation_id);
        Ok(Arc::new(SeaOrmArtifactDataCapabilityBroker::new(
            self.db.clone(),
            authorizer,
            schema_validator,
            scope,
        )))
    }
}

/// The `platform.data.objects` adapter for bounded binary object calls. It is
/// deliberately a distinct capability from structured `platform.data`, so
/// policies can grant object prefixes and operations without broadening JSON
/// value access.
#[derive(Clone)]
pub struct SeaOrmArtifactDataObjectCapabilityBroker<A> {
    objects: SeaOrmArtifactDataObjectBroker<A>,
    scope: ArtifactDataScope,
}

impl<A> SeaOrmArtifactDataObjectCapabilityBroker<A>
where
    A: ArtifactDataAuthorizer,
{
    pub fn new(
        db: DatabaseConnection,
        storage: StorageService,
        authorizer: A,
        scope: ArtifactDataScope,
    ) -> Self {
        Self {
            objects: SeaOrmArtifactDataObjectBroker::new(db, storage, authorizer),
            scope,
        }
    }
}

#[async_trait]
impl<A> CapabilityBroker for SeaOrmArtifactDataObjectCapabilityBroker<A>
where
    A: ArtifactDataAuthorizer,
{
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        if call.capability.as_str() != "platform.data.objects"
            || call.context.tenant_id != Some(self.scope.tenant_id)
            || !matches!(
                &call.subject,
                SandboxSubject::ModuleArtifact { slug, .. } if slug == &self.scope.module_slug
            )
        {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }
        match decode_object_data_capability_call(call)? {
            ObjectDataCapabilityCall::GetMetadata { name } => {
                let object = self
                    .objects
                    .get_object(&self.scope, &name)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "object": object }),
                })
            }
            ObjectDataCapabilityCall::Read { name } => {
                let object = self
                    .objects
                    .read_object(&self.scope, &name)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                let output = match object {
                    Some(content) if content.data.len() <= MAX_SANDBOX_ARTIFACT_OBJECT_BYTES => {
                        json!({
                            "object": content.object,
                            "data_base64": BASE64_STANDARD.encode(content.data),
                        })
                    }
                    Some(_) => {
                        return Err(data_capability_constraint(
                            call,
                            "object exceeds the sandbox transfer limit",
                        ));
                    }
                    None => json!({ "object": Value::Null }),
                };
                Ok(CapabilityResponse { output })
            }
            ObjectDataCapabilityCall::Put { upload } => {
                let object = self
                    .objects
                    .put_object(&self.scope, upload)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "object": object }),
                })
            }
            ObjectDataCapabilityCall::List { page } => {
                let page = self
                    .objects
                    .list_objects(&self.scope, page)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({
                        "objects": page.objects,
                        "next_after_name": page.next_after_name,
                    }),
                })
            }
        }
    }
}

/// Production resolver for the bounded `platform.data.objects` owner. The
/// storage service is deployment-provided, but artifact scope and every
/// physical object key remain owner-controlled.
#[derive(Clone)]
pub struct SeaOrmArtifactDataObjectCapabilityBrokerResolver {
    db: DatabaseConnection,
    storage: StorageService,
}

impl SeaOrmArtifactDataObjectCapabilityBrokerResolver {
    pub fn new(db: DatabaseConnection, storage: StorageService) -> Self {
        Self { db, storage }
    }
}

#[async_trait]
impl ArtifactCapabilityBrokerResolver for SeaOrmArtifactDataObjectCapabilityBrokerResolver {
    async fn resolve_broker(
        &self,
        execution: &ArtifactCapabilityExecution,
        capability: &rustok_sandbox::CapabilityName,
    ) -> SandboxResult<Arc<dyn CapabilityBroker>> {
        if capability.as_str() != "platform.data.objects" {
            return Err(SandboxError::CapabilityDenied(capability.clone()));
        }
        let installation =
            resolve_granted_artifact_capability(&self.db, execution, capability).await?;
        let scope = artifact_data_scope_for_execution(&installation, execution, capability)?;
        let authorizer = ExactArtifactDataAuthorizer {
            scope: scope.clone(),
        };
        Ok(Arc::new(SeaOrmArtifactDataObjectCapabilityBroker::new(
            self.db.clone(),
            self.storage.clone(),
            authorizer,
            scope,
        )))
    }
}

enum ObjectDataCapabilityCall {
    GetMetadata { name: String },
    Read { name: String },
    Put { upload: ArtifactDataObjectUpload },
    List { page: ArtifactDataPageRequest },
}

fn decode_object_data_capability_call(
    call: &CapabilityCall,
) -> SandboxResult<ObjectDataCapabilityCall> {
    let input = call
        .input
        .as_object()
        .ok_or_else(|| data_capability_constraint(call, "object-data input must be an object"))?;
    match call.operation.as_str() {
        "get_metadata" => {
            reject_data_capability_fields(call, input, &["name"])?;
            Ok(ObjectDataCapabilityCall::GetMetadata {
                name: required_data_capability_string(call, input, "name")?.to_string(),
            })
        }
        "read" => {
            reject_data_capability_fields(call, input, &["name"])?;
            Ok(ObjectDataCapabilityCall::Read {
                name: required_data_capability_string(call, input, "name")?.to_string(),
            })
        }
        "put" => {
            reject_data_capability_fields(
                call,
                input,
                &[
                    "name",
                    "content_type",
                    "data_base64",
                    "expected_revision",
                    "idempotency_key",
                ],
            )?;
            let data = BASE64_STANDARD
                .decode(required_data_capability_string(call, input, "data_base64")?)
                .map_err(|_| data_capability_constraint(call, "object data_base64 is invalid"))?;
            if data.is_empty() || data.len() > MAX_SANDBOX_ARTIFACT_OBJECT_BYTES {
                return Err(data_capability_constraint(
                    call,
                    "object exceeds the sandbox transfer limit",
                ));
            }
            let expected_revision = input
                .get("expected_revision")
                .map(|value| {
                    value
                        .as_u64()
                        .filter(|revision| *revision > 0)
                        .ok_or_else(|| {
                            data_capability_constraint(
                                call,
                                "object expected_revision must be a positive integer",
                            )
                        })
                })
                .transpose()?;
            let idempotency_key = Uuid::parse_str(required_data_capability_string(
                call,
                input,
                "idempotency_key",
            )?)
            .map_err(|_| {
                data_capability_constraint(call, "object idempotency_key must be a UUID")
            })?;
            Ok(ObjectDataCapabilityCall::Put {
                upload: ArtifactDataObjectUpload {
                    name: required_data_capability_string(call, input, "name")?.to_string(),
                    content_type: required_data_capability_string(call, input, "content_type")?
                        .to_string(),
                    data: Bytes::from(data),
                    expected_revision,
                    idempotency_key,
                },
            })
        }
        "list" => {
            reject_data_capability_fields(call, input, &["prefix", "after_name", "limit"])?;
            let after_key = input
                .get("after_name")
                .map(|value| {
                    value.as_str().map(str::to_string).ok_or_else(|| {
                        data_capability_constraint(call, "object after_name must be a string")
                    })
                })
                .transpose()?;
            let limit = input
                .get("limit")
                .and_then(Value::as_u64)
                .filter(|limit| (1..=100).contains(limit))
                .ok_or_else(|| {
                    data_capability_constraint(call, "object list limit must be between 1 and 100")
                })?;
            Ok(ObjectDataCapabilityCall::List {
                page: ArtifactDataPageRequest {
                    prefix: required_data_capability_string(call, input, "prefix")?.to_string(),
                    after_key,
                    limit: u32::try_from(limit).map_err(|_| {
                        data_capability_constraint(call, "object list limit must fit u32")
                    })?,
                },
            })
        }
        _ => Err(data_capability_constraint(
            call,
            "object-data operation is unsupported",
        )),
    }
}

enum DataCapabilityCall {
    Get { key: String },
    Put { write: ArtifactDataWrite },
    PutBatch { batch: ArtifactDataBatchWrite },
    List { page: ArtifactDataPageRequest },
}

fn decode_data_capability_call(call: &CapabilityCall) -> SandboxResult<DataCapabilityCall> {
    let input = call
        .input
        .as_object()
        .ok_or_else(|| data_capability_constraint(call, "data input must be an object"))?;
    match call.operation.as_str() {
        "get" => {
            reject_data_capability_fields(call, input, &["key"])?;
            Ok(DataCapabilityCall::Get {
                key: required_data_capability_string(call, input, "key")?.to_string(),
            })
        }
        "put" => Ok(DataCapabilityCall::Put {
            write: decode_data_capability_write(call, input)?,
        }),
        "put_batch" => {
            reject_data_capability_fields(call, input, &["writes"])?;
            let writes = input
                .get("writes")
                .and_then(Value::as_array)
                .ok_or_else(|| data_capability_constraint(call, "data writes must be an array"))?
                .iter()
                .map(|value| {
                    value
                        .as_object()
                        .ok_or_else(|| {
                            data_capability_constraint(call, "data batch entry must be an object")
                        })
                        .and_then(|write| decode_data_capability_write(call, write))
                })
                .collect::<SandboxResult<Vec<_>>>()?;
            let batch = ArtifactDataBatchWrite { writes };
            validate_artifact_data_batch(&batch)
                .map_err(|_| data_capability_constraint(call, "data batch is invalid"))?;
            Ok(DataCapabilityCall::PutBatch { batch })
        }
        "list" => {
            reject_data_capability_fields(call, input, &["prefix", "after_key", "limit"])?;
            let prefix = required_data_capability_string(call, input, "prefix")?.to_string();
            let after_key = input
                .get("after_key")
                .map(|value| {
                    value.as_str().map(str::to_string).ok_or_else(|| {
                        data_capability_constraint(call, "data after_key must be a string")
                    })
                })
                .transpose()?;
            let limit = input
                .get("limit")
                .and_then(Value::as_u64)
                .filter(|limit| (1..=100).contains(limit))
                .ok_or_else(|| {
                    data_capability_constraint(call, "data list limit must be between 1 and 100")
                })?;
            Ok(DataCapabilityCall::List {
                page: ArtifactDataPageRequest {
                    prefix,
                    after_key,
                    limit: u32::try_from(limit).map_err(|_| {
                        data_capability_constraint(call, "data list limit must fit u32")
                    })?,
                },
            })
        }
        _ => Err(data_capability_constraint(
            call,
            "data operation is unsupported",
        )),
    }
}

fn decode_data_capability_write(
    call: &CapabilityCall,
    input: &serde_json::Map<String, Value>,
) -> SandboxResult<ArtifactDataWrite> {
    reject_data_capability_fields(
        call,
        input,
        &["key", "value", "expected_revision", "idempotency_key"],
    )?;
    let value = input
        .get("value")
        .cloned()
        .ok_or_else(|| data_capability_constraint(call, "data put input requires value"))?;
    let expected_revision = input
        .get("expected_revision")
        .map(|value| {
            value
                .as_u64()
                .filter(|revision| *revision > 0)
                .ok_or_else(|| {
                    data_capability_constraint(
                        call,
                        "data expected_revision must be a positive integer",
                    )
                })
        })
        .transpose()?;
    let idempotency_key = Uuid::parse_str(required_data_capability_string(
        call,
        input,
        "idempotency_key",
    )?)
    .map_err(|_| data_capability_constraint(call, "data idempotency_key must be a UUID"))?;
    Ok(ArtifactDataWrite {
        key: required_data_capability_string(call, input, "key")?.to_string(),
        value,
        expected_revision,
        create_only: false,
        idempotency_key,
    })
}

fn data_capability_constraint(call: &CapabilityCall, reason: &str) -> SandboxError {
    SandboxError::CapabilityConstraintDenied {
        capability: call.capability.clone(),
        reason: reason.to_string(),
    }
}

fn required_data_capability_string<'a>(
    call: &CapabilityCall,
    input: &'a serde_json::Map<String, Value>,
    field: &str,
) -> SandboxResult<&'a str> {
    input
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| data_capability_constraint(call, &format!("data {field} must be a string")))
}

fn reject_data_capability_fields(
    call: &CapabilityCall,
    input: &serde_json::Map<String, Value>,
    allowed: &[&str],
) -> SandboxResult<()> {
    if input.keys().any(|field| !allowed.contains(&field.as_str())) {
        return Err(data_capability_constraint(
            call,
            "data input contains an unsupported field",
        ));
    }
    Ok(())
}

fn escape_like_prefix(prefix: &str) -> String {
    prefix
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn data_capability_error(
    capability: &rustok_sandbox::CapabilityName,
    error: ArtifactDataError,
) -> SandboxError {
    match error {
        ArtifactDataError::InvalidScope
        | ArtifactDataError::InvalidKey
        | ArtifactDataError::InvalidObject
        | ArtifactDataError::InvalidPage
        | ArtifactDataError::InvalidBatch
        | ArtifactDataError::RevisionConflict
        | ArtifactDataError::NamespacePurged
        | ArtifactDataError::PurgePrecondition
        | ArtifactDataError::InvalidIdempotencyKey
        | ArtifactDataError::IdempotencyConflict
        | ArtifactDataError::ValueTooLarge { .. }
        | ArtifactDataError::DataContractSchemaViolation
        | ArtifactDataError::PolicyDenied => SandboxError::CapabilityDenied(capability.clone()),
        ArtifactDataError::InvalidUpgrade
        | ArtifactDataError::UpgradeHook(_)
        | ArtifactDataError::StaleUpgradePlan
        | ArtifactDataError::MigrationCheckpoint(_)
        | ArtifactDataError::DataContractUnavailable
        | ArtifactDataError::DataContractSchemaInvalid
        | ArtifactDataError::ObjectIntegrity
        | ArtifactDataError::Storage(_) => SandboxError::HostCapability {
            capability: capability.clone(),
            message: "artifact data capability is unavailable".to_string(),
        },
    }
}

/// Owner service for irreversible namespace deletion. Its authorization port
/// keeps retention and installation lifecycle policy outside guest-controlled
/// capability calls while the data owner keeps mutation, audit and outbox facts
/// in one transaction.
#[derive(Clone)]
pub struct SeaOrmArtifactDataPurgeService<A> {
    db: DatabaseConnection,
    authorizer: A,
}

impl<A> SeaOrmArtifactDataPurgeService<A>
where
    A: ArtifactDataPurgeAuthorizer,
{
    pub fn new(db: DatabaseConnection, authorizer: A) -> Self {
        Self { db, authorizer }
    }

    pub async fn purge(
        &self,
        request: ArtifactDataPurgeRequest,
    ) -> Result<ArtifactDataPurgeResult, ArtifactDataError> {
        validate_purge_request(&request)?;
        self.authorizer.authorize_purge(&request).await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        if let Some(row) = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT expected_namespace_revision, actor_id, reason, namespace_revision, purged_records
                     FROM module_artifact_data_purge_operations
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND idempotency_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    uuid_value(request.scope.tenant_id, backend),
                    request.scope.module_slug.clone().into(),
                    revision_value(request.scope.data_contract_revision)?,
                    uuid_value(request.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(storage_error)?
        {
            let expected_revision: i64 = row
                .try_get("", "expected_namespace_revision")
                .map_err(storage_error)?;
            let actor_id = uuid_from_row(&row, "actor_id", backend)?;
            let reason: String = row.try_get("", "reason").map_err(storage_error)?;
            if u64::try_from(expected_revision).ok() != Some(request.expected_namespace_revision)
                || actor_id != request.actor_id
                || reason != request.reason
            {
                return Err(ArtifactDataError::IdempotencyConflict);
            }
            let namespace_revision: i64 = row
                .try_get("", "namespace_revision")
                .map_err(storage_error)?;
            let purged_records: i64 = row
                .try_get("", "purged_records")
                .map_err(storage_error)?;
            transaction.commit().await.map_err(storage_error)?;
            return Ok(ArtifactDataPurgeResult {
                namespace_revision: u64::try_from(namespace_revision)
                    .map_err(|_| ArtifactDataError::PurgePrecondition)?,
                purged_records: u64::try_from(purged_records)
                    .map_err(|_| ArtifactDataError::PurgePrecondition)?,
            });
        }

        let namespace = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT namespace_revision, CASE WHEN purged_at IS NULL THEN 0 ELSE 1 END AS is_purged
                     FROM module_artifact_data_namespaces
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}{}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    namespace_lock_clause(backend),
                ),
                namespace_values(&request.scope, backend)?,
            ))
            .await
            .map_err(storage_error)?
            .ok_or(ArtifactDataError::PurgePrecondition)?;
        let current_revision: i64 = namespace
            .try_get("", "namespace_revision")
            .map_err(storage_error)?;
        let already_purged = namespace
            .try_get::<i64>("", "is_purged")
            .map_err(storage_error)?
            != 0;
        if already_purged
            || u64::try_from(current_revision).ok() != Some(request.expected_namespace_revision)
        {
            return Err(ArtifactDataError::PurgePrecondition);
        }
        let structured_records = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(&request.scope, backend)?,
            ))
            .await
            .map_err(storage_error)?
            .rows_affected();
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_operations
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(&request.scope, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        let object_records = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_objects
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(&request.scope, backend)?,
            ))
            .await
            .map_err(storage_error)?
            .rows_affected();
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_object_operations
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(&request.scope, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        let next_revision = u64::try_from(current_revision)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(ArtifactDataError::PurgePrecondition)?;
        let updated = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_namespaces
                     SET namespace_revision = {}, purged_at = {}, updated_at = {}
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                     AND namespace_revision = {} AND purged_at IS NULL",
                    placeholder(backend, 1),
                    now_expression(backend),
                    now_expression(backend),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                ),
                vec![
                    revision_value(next_revision)?,
                    uuid_value(request.scope.tenant_id, backend),
                    request.scope.module_slug.clone().into(),
                    revision_value(request.scope.data_contract_revision)?,
                    revision_value(request.expected_namespace_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(ArtifactDataError::PurgePrecondition);
        }
        let purged_records = structured_records
            .checked_add(object_records)
            .and_then(|count| i64::try_from(count).ok())
            .ok_or(ArtifactDataError::PurgePrecondition)?;
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_data_purge_operations
                     (tenant_id, module_slug, data_contract_revision, idempotency_key, expected_namespace_revision,
                      namespace_revision, actor_id, reason, purged_records, completed_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
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
                    uuid_value(request.scope.tenant_id, backend),
                    request.scope.module_slug.clone().into(),
                    revision_value(request.scope.data_contract_revision)?,
                    uuid_value(request.idempotency_key, backend),
                    revision_value(request.expected_namespace_revision)?,
                    revision_value(next_revision)?,
                    uuid_value(request.actor_id, backend),
                    request.reason.clone().into(),
                    purged_records.into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        OutboxTransport::new(self.db.clone())
            .write_to_outbox(
                &transaction,
                EventEnvelope::new(
                    Uuid::new_v4(),
                    Some(request.scope.tenant_id),
                    DomainEvent::ModuleArtifactDataPurged {
                        tenant_id: request.scope.tenant_id,
                        module_slug: request.scope.module_slug.clone(),
                        data_contract_revision: request.scope.data_contract_revision,
                        namespace_revision: next_revision,
                        purged_records: u64::try_from(purged_records)
                            .map_err(|_| ArtifactDataError::PurgePrecondition)?,
                    },
                ),
            )
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactDataPurgeResult {
            namespace_revision: next_revision,
            purged_records: u64::try_from(purged_records)
                .map_err(|_| ArtifactDataError::PurgePrecondition)?,
        })
    }
}

pub(crate) async fn configure_tenant_scope<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
) -> Result<(), ArtifactDataError> {
    if connection.get_database_backend() == DbBackend::Postgres {
        connection
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT set_config('rustok.tenant_id', $1, true)",
                vec![tenant_id.to_string().into()],
            ))
            .await
            .map_err(storage_error)?;
    }
    Ok(())
}

async fn ensure_active_namespace<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    backend: DbBackend,
) -> Result<(), ArtifactDataError> {
    connection
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_data_namespaces
                 (tenant_id, module_slug, data_contract_revision, namespace_revision, created_at, updated_at)
                 VALUES ({}, {}, {}, 1, {}, {}) ON CONFLICT DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                now_expression(backend),
                now_expression(backend),
            ),
            namespace_values(scope, backend)?,
        ))
        .await
        .map_err(storage_error)?;
    let active = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT namespace_revision FROM module_artifact_data_namespaces
                 WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                 AND purged_at IS NULL{}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                namespace_lock_clause(backend),
            ),
            namespace_values(scope, backend)?,
        ))
        .await
        .map_err(storage_error)?;
    if active.is_none() {
        return Err(ArtifactDataError::NamespacePurged);
    }
    Ok(())
}

fn validate_purge_request(request: &ArtifactDataPurgeRequest) -> Result<(), ArtifactDataError> {
    request.scope.validate()?;
    if request.expected_namespace_revision == 0
        || request.actor_id.is_nil()
        || request.idempotency_key.is_nil()
        || request.reason.trim().is_empty()
        || request.reason.len() > 2_000
    {
        return Err(ArtifactDataError::PurgePrecondition);
    }
    Ok(())
}

fn scope_values(
    scope: &ArtifactDataScope,
    backend: DbBackend,
    key: &str,
) -> Result<Vec<SqlValue>, ArtifactDataError> {
    Ok(vec![
        uuid_value(scope.tenant_id, backend),
        scope.module_slug.clone().into(),
        revision_value(scope.data_contract_revision)?,
        key.to_owned().into(),
    ])
}

fn namespace_values(
    scope: &ArtifactDataScope,
    backend: DbBackend,
) -> Result<Vec<SqlValue>, ArtifactDataError> {
    Ok(vec![
        uuid_value(scope.tenant_id, backend),
        scope.module_slug.clone().into(),
        revision_value(scope.data_contract_revision)?,
    ])
}

pub(crate) fn revision_value(value: u64) -> Result<SqlValue, ArtifactDataError> {
    i64::try_from(value)
        .map(|value| value.into())
        .map_err(|_| ArtifactDataError::RevisionConflict)
}

pub(crate) fn optional_revision_value(value: Option<u64>) -> Result<SqlValue, ArtifactDataError> {
    value
        .map(revision_value)
        .transpose()
        .map(|value| match value {
            Some(value) => value,
            None => SqlValue::BigInt(None),
        })
}

pub(crate) fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => SqlValue::Uuid(Some(Box::new(value))),
        _ => value.to_string().into(),
    }
}

pub(crate) fn uuid_from_row(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, ArtifactDataError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        _ => row
            .try_get::<String>("", column)
            .map_err(storage_error)?
            .parse()
            .map_err(storage_error),
    }
}

pub(crate) fn placeholder(backend: DbBackend, index: usize) -> String {
    match backend {
        DbBackend::Postgres => format!("${index}"),
        _ => format!("?{index}"),
    }
}

pub(crate) fn now_expression(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "NOW()",
        _ => "datetime('now')",
    }
}

pub(crate) fn namespace_lock_clause(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => " FOR UPDATE",
        _ => "",
    }
}

fn record_from_row(row: sea_orm::QueryResult) -> Result<ArtifactDataRecord, ArtifactDataError> {
    let revision: i64 = row.try_get("", "revision").map_err(storage_error)?;
    Ok(ArtifactDataRecord {
        key: row.try_get("", "data_key").map_err(storage_error)?,
        value: row.try_get("", "value").map_err(storage_error)?,
        revision: u64::try_from(revision).map_err(|_| ArtifactDataError::RevisionConflict)?,
    })
}

fn storage_error(error: impl std::fmt::Display) -> ArtifactDataError {
    ArtifactDataError::Storage(error.to_string())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactDataError {
    #[error("artifact data scope is invalid")]
    InvalidScope,
    #[error("artifact data key is invalid")]
    InvalidKey,
    #[error("artifact data object metadata is invalid")]
    InvalidObject,
    #[error("artifact data object failed its stored integrity check")]
    ObjectIntegrity,
    #[error("artifact data page is invalid")]
    InvalidPage,
    #[error("artifact data batch is invalid")]
    InvalidBatch,
    #[error("artifact data upgrade request is invalid")]
    InvalidUpgrade,
    #[error("artifact data upgrade hook failed: {0}")]
    UpgradeHook(String),
    #[error("artifact data upgrade plan is stale")]
    StaleUpgradePlan,
    #[error("artifact data migration checkpoint failed: {0}")]
    MigrationCheckpoint(String),
    #[error("artifact data contract is unavailable for the injected installation scope")]
    DataContractUnavailable,
    #[error("artifact data contract schema is invalid")]
    DataContractSchemaInvalid,
    #[error("artifact data value does not satisfy the admitted data contract schema")]
    DataContractSchemaViolation,
    #[error("artifact data revision conflict")]
    RevisionConflict,
    #[error("artifact data namespace was purged")]
    NamespacePurged,
    #[error("artifact data purge precondition failed")]
    PurgePrecondition,
    #[error("artifact data idempotency key is invalid")]
    InvalidIdempotencyKey,
    #[error("artifact data idempotency key was reused for a different key")]
    IdempotencyConflict,
    #[error("artifact data value exceeds {limit} bytes (received {actual})")]
    ValueTooLarge { limit: usize, actual: usize },
    #[error("artifact data policy denied the operation")]
    PolicyDenied,
    #[error("artifact data storage failed: {0}")]
    Storage(String),
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
    };

    use crate::ModuleBindingIdempotency;
    use async_trait::async_trait;
    use rustok_sandbox::{
        CapabilityCall, CapabilityCallContext, CapabilityName, ExecutionPhase, SandboxSubject,
    };
    use serde_json::json;

    use super::*;

    #[derive(Clone)]
    struct CompletedPageBroker {
        completed: Arc<AtomicBool>,
    }

    #[async_trait]
    impl ArtifactDataBroker for CompletedPageBroker {
        async fn get(
            &self,
            _: &ArtifactDataScope,
            _: &str,
        ) -> Result<Option<ArtifactDataRecord>, ArtifactDataError> {
            Err(ArtifactDataError::Storage(
                "not used by upgrade planning".to_string(),
            ))
        }

        async fn put(
            &self,
            _: &ArtifactDataScope,
            _: ArtifactDataWrite,
        ) -> Result<ArtifactDataRecord, ArtifactDataError> {
            Err(ArtifactDataError::Storage(
                "not used by upgrade planning".to_string(),
            ))
        }

        async fn put_batch(
            &self,
            _: &ArtifactDataScope,
            _: ArtifactDataBatchWrite,
        ) -> Result<Vec<ArtifactDataRecord>, ArtifactDataError> {
            Err(ArtifactDataError::Storage(
                "not used by upgrade planning".to_string(),
            ))
        }

        async fn list(
            &self,
            _: &ArtifactDataScope,
            _: ArtifactDataPageRequest,
        ) -> Result<ArtifactDataPage, ArtifactDataError> {
            self.completed.store(true, Ordering::SeqCst);
            Ok(ArtifactDataPage {
                records: vec![ArtifactDataRecord {
                    key: "state/current".to_string(),
                    value: json!({ "version": 1 }),
                    revision: 7,
                }],
                next_after_key: Some("state/current".to_string()),
            })
        }
    }

    #[derive(Clone)]
    struct UpgradeHook {
        read_completed: Arc<AtomicBool>,
    }

    #[async_trait]
    impl ArtifactDataUpgradeHook for UpgradeHook {
        async fn transform_data(
            &self,
            hook_binding_id: &str,
            input: ArtifactDataUpgradeInput,
        ) -> Result<Value, ArtifactDataError> {
            assert!(self.read_completed.load(Ordering::SeqCst));
            assert_eq!(hook_binding_id, "upgrade.v2");
            assert_eq!(input.record.key, "state/current");
            Ok(json!({ "version": 2 }))
        }
    }

    #[derive(Clone)]
    struct AcceptingSchemaValidator;

    #[async_trait]
    impl ArtifactDataSchemaValidator for AcceptingSchemaValidator {
        async fn validate_data_value(
            &self,
            scope: &ArtifactDataScope,
            value: &Value,
        ) -> Result<(), ArtifactDataError> {
            assert_eq!(scope.data_contract_revision, 2);
            assert_eq!(value, &json!({ "version": 2 }));
            Ok(())
        }
    }

    #[derive(Clone)]
    struct RecordingUpgradeBindingExecutor {
        calls: Arc<Mutex<Vec<(String, String, ExecutionPhase, Value)>>>,
    }

    #[async_trait]
    impl ArtifactBindingExecutor for RecordingUpgradeBindingExecutor {
        async fn dispatch_binding(
            &self,
            dispatch: ArtifactBindingDispatch<'_>,
        ) -> Result<Value, String> {
            self.calls.lock().expect("calls lock").push((
                dispatch.release.slug.clone(),
                dispatch.binding.id.clone(),
                dispatch.phase.clone(),
                dispatch.input,
            ));
            Ok(json!({ "version": 2 }))
        }
    }

    #[derive(Clone)]
    struct UpgradeApplyBroker {
        source: ArtifactDataRecord,
        target: Arc<Mutex<HashMap<String, (ArtifactDataRecord, Uuid)>>>,
    }

    #[async_trait]
    impl ArtifactDataBroker for UpgradeApplyBroker {
        async fn get(
            &self,
            scope: &ArtifactDataScope,
            key: &str,
        ) -> Result<Option<ArtifactDataRecord>, ArtifactDataError> {
            if scope.data_contract_revision == 1 {
                return Ok((key == self.source.key).then(|| self.source.clone()));
            }
            Ok(self
                .target
                .lock()
                .expect("target lock")
                .get(key)
                .map(|(record, _)| record.clone()))
        }

        async fn put(
            &self,
            scope: &ArtifactDataScope,
            write: ArtifactDataWrite,
        ) -> Result<ArtifactDataRecord, ArtifactDataError> {
            assert_eq!(scope.data_contract_revision, 2);
            assert!(write.create_only);
            let mut target = self.target.lock().expect("target lock");
            if let Some((record, idempotency_key)) = target.get(&write.key) {
                if *idempotency_key == write.idempotency_key
                    && record.value == write.value
                    && write.expected_revision.is_none()
                {
                    return Ok(record.clone());
                }
                return Err(ArtifactDataError::RevisionConflict);
            }
            let record = ArtifactDataRecord {
                key: write.key.clone(),
                value: write.value,
                revision: 1,
            };
            target.insert(write.key, (record.clone(), write.idempotency_key));
            Ok(record)
        }

        async fn put_batch(
            &self,
            _: &ArtifactDataScope,
            _: ArtifactDataBatchWrite,
        ) -> Result<Vec<ArtifactDataRecord>, ArtifactDataError> {
            Err(ArtifactDataError::Storage(
                "not used by upgrade application".to_string(),
            ))
        }

        async fn list(
            &self,
            _: &ArtifactDataScope,
            _: ArtifactDataPageRequest,
        ) -> Result<ArtifactDataPage, ArtifactDataError> {
            Err(ArtifactDataError::Storage(
                "not used by upgrade application".to_string(),
            ))
        }
    }

    #[derive(Clone)]
    struct RecordingCheckpointStore {
        requests: Arc<Mutex<Vec<ArtifactMigrationCheckpointRequest>>>,
        fail_first: Arc<AtomicBool>,
    }

    impl Default for RecordingCheckpointStore {
        fn default() -> Self {
            Self {
                requests: Arc::new(Mutex::new(Vec::new())),
                fail_first: Arc::new(AtomicBool::new(true)),
            }
        }
    }

    #[async_trait]
    impl ArtifactDataMigrationCheckpointStore for RecordingCheckpointStore {
        async fn record_data_upgrade_checkpoint(
            &self,
            request: ArtifactMigrationCheckpointRequest,
        ) -> Result<u64, ArtifactDataError> {
            if self.fail_first.swap(false, Ordering::SeqCst) {
                return Err(ArtifactDataError::MigrationCheckpoint(
                    "simulated retryable checkpoint failure".to_string(),
                ));
            }
            let revision = request.expected_revision + 1;
            self.requests.lock().expect("checkpoint lock").push(request);
            Ok(revision)
        }
    }

    #[test]
    fn scope_and_keys_reject_guest_controlled_namespace_escapes() {
        assert!(matches!(
            ArtifactDataScope {
                tenant_id: Uuid::nil(),
                module_slug: "module".into(),
                data_contract_revision: 1,
                policy_revision: 1,
            }
            .validate(),
            Err(ArtifactDataError::InvalidScope)
        ));
        for key in ["/host/path", "state/../escape", "state//key"] {
            assert!(matches!(
                validate_artifact_data_key(key),
                Err(ArtifactDataError::InvalidKey)
            ));
        }
    }

    #[test]
    fn sandbox_data_adapter_keeps_list_continuations_inside_the_prefix() {
        let mut call = CapabilityCall {
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::ModuleArtifact {
                installation_id: Uuid::new_v4(),
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:sample".to_string(),
            },
            context: CapabilityCallContext {
                phase: ExecutionPhase::Lifecycle,
                tenant_id: Some(Uuid::new_v4()),
                actor_id: None,
                trace_id: None,
            },
            capability: CapabilityName::new("platform.data").expect("capability name"),
            operation: "list".to_string(),
            input: json!({ "prefix": "state/", "after_key": "state/one", "limit": 10 }),
        };
        assert!(matches!(
            decode_data_capability_call(&call),
            Ok(DataCapabilityCall::List { .. })
        ));
        call.input = json!({ "prefix": "state/", "after_key": "other/one", "limit": 10 });
        assert!(decode_data_capability_call(&call).is_err());
    }

    #[test]
    fn sandbox_data_batch_requires_distinct_bounded_writes() {
        let idempotency_key = Uuid::new_v4();
        let call = CapabilityCall {
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::ModuleArtifact {
                installation_id: Uuid::new_v4(),
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:sample".to_string(),
            },
            context: CapabilityCallContext {
                phase: ExecutionPhase::Lifecycle,
                tenant_id: Some(Uuid::new_v4()),
                actor_id: None,
                trace_id: None,
            },
            capability: CapabilityName::new("platform.data").expect("capability name"),
            operation: "put_batch".to_string(),
            input: json!({
                "writes": [
                    { "key": "state/one", "value": 1, "idempotency_key": idempotency_key },
                    { "key": "state/two", "value": 2, "idempotency_key": Uuid::new_v4() }
                ]
            }),
        };
        assert!(matches!(
            decode_data_capability_call(&call),
            Ok(DataCapabilityCall::PutBatch { .. })
        ));

        let duplicate = CapabilityCall {
            input: json!({
                "writes": [
                    { "key": "state/one", "value": 1, "idempotency_key": idempotency_key },
                    { "key": "state/two", "value": 2, "idempotency_key": idempotency_key }
                ]
            }),
            ..call
        };
        assert!(decode_data_capability_call(&duplicate).is_err());
    }

    #[test]
    fn sandbox_object_data_adapter_accepts_only_bounded_base64_payloads() {
        let mut call = CapabilityCall {
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::ModuleArtifact {
                installation_id: Uuid::new_v4(),
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:sample".to_string(),
            },
            context: CapabilityCallContext {
                phase: ExecutionPhase::Manual,
                tenant_id: Some(Uuid::new_v4()),
                actor_id: None,
                trace_id: None,
            },
            capability: CapabilityName::new("platform.data.objects").expect("capability name"),
            operation: "put".to_string(),
            input: json!({
                "name": "exports/report.json",
                "content_type": "application/json",
                "data_base64": "e30=",
                "idempotency_key": Uuid::new_v4(),
            }),
        };
        assert!(matches!(
            decode_object_data_capability_call(&call),
            Ok(ObjectDataCapabilityCall::Put { .. })
        ));

        call.input = json!({
            "name": "exports/report.json",
            "content_type": "application/json",
            "data_base64": "not-base64",
            "idempotency_key": Uuid::new_v4(),
        });
        assert!(decode_object_data_capability_call(&call).is_err());
    }

    #[tokio::test]
    async fn upgrade_planning_reads_before_transforming_and_never_writes() {
        let completed = Arc::new(AtomicBool::new(false));
        let tenant_id = Uuid::new_v4();
        let source = ArtifactDataScope {
            tenant_id,
            module_slug: "sample_module".to_string(),
            data_contract_revision: 1,
            policy_revision: 1,
        };
        let planner = ArtifactDataUpgradePlanner::new(
            CompletedPageBroker {
                completed: Arc::clone(&completed),
            },
            UpgradeHook {
                read_completed: Arc::clone(&completed),
            },
            AcceptingSchemaValidator,
        );

        let plan = planner
            .plan(ArtifactDataUpgradeRequest {
                plan_id: Uuid::new_v4(),
                target_installation_id: Uuid::new_v4(),
                source,
                target: ArtifactDataScope {
                    tenant_id,
                    module_slug: "sample_module".to_string(),
                    data_contract_revision: 2,
                    policy_revision: 2,
                },
                hook_binding_id: "upgrade.v2".to_string(),
                page: ArtifactDataPageRequest {
                    prefix: "state/".to_string(),
                    after_key: None,
                    limit: 10,
                },
            })
            .await
            .expect("upgrade plan");

        assert_eq!(plan.records.len(), 1);
        assert_eq!(plan.records[0].source_revision, 7);
        assert_eq!(plan.records[0].value, json!({ "version": 2 }));
        assert_eq!(plan.next_after_key.as_deref(), Some("state/current"));
    }

    #[tokio::test]
    async fn upgrade_hook_requires_a_dedicated_admitted_binding() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let executor = RecordingUpgradeBindingExecutor {
            calls: Arc::clone(&calls),
        };
        let release = ArtifactReleaseRef {
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            digest: "sha256:artifact".to_string(),
        };
        let mut binding = ModuleRuntimeBinding {
            id: "upgrade.v2".to_string(),
            kind: ModuleRuntimeBindingKind::Command,
            entrypoint: "upgrade.v2".to_string(),
            input_schema_digest: "sha256:input".to_string(),
            output_schema_digest: "sha256:output".to_string(),
            permission: "sample_module.data.upgrade".to_string(),
            idempotency: ModuleBindingIdempotency::Required,
            limit_profile: "data_upgrade".to_string(),
            capabilities: Vec::new(),
            event_topics: Vec::new(),
            schedule: None,
            http: None,
        };
        assert!(matches!(
            ArtifactBindingDataUpgradeHook::new(executor.clone(), release.clone(), binding.clone()),
            Err(ArtifactDataError::InvalidUpgrade)
        ));

        binding.kind = ModuleRuntimeBindingKind::DataUpgrade;
        let hook = ArtifactBindingDataUpgradeHook::new(executor, release, binding)
            .expect("dedicated upgrade hook");
        let tenant_id = Uuid::new_v4();
        let source = ArtifactDataScope {
            tenant_id,
            module_slug: "sample_module".to_string(),
            data_contract_revision: 1,
            policy_revision: 1,
        };
        let transformed = hook
            .transform_data(
                "upgrade.v2",
                ArtifactDataUpgradeInput {
                    source: source.clone(),
                    target: ArtifactDataScope {
                        tenant_id,
                        module_slug: "sample_module".to_string(),
                        data_contract_revision: 2,
                        policy_revision: 2,
                    },
                    record: ArtifactDataRecord {
                        key: "state/current".to_string(),
                        value: json!({ "version": 1 }),
                        revision: 7,
                    },
                },
            )
            .await
            .expect("transformed value");

        assert_eq!(transformed, json!({ "version": 2 }));
        let calls = calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "sample_module");
        assert_eq!(calls[0].1, "upgrade.v2");
        assert_eq!(calls[0].2, ExecutionPhase::Manual);
        assert_eq!(calls[0].3["source"], serde_json::to_value(source).unwrap());
        assert_eq!(calls[0].3["record"]["revision"], 7);
    }

    #[tokio::test]
    async fn upgrade_application_retries_by_plan_id_before_checkpointing() {
        let tenant_id = Uuid::new_v4();
        let source = ArtifactDataScope {
            tenant_id,
            module_slug: "sample_module".to_string(),
            data_contract_revision: 1,
            policy_revision: 1,
        };
        let plan = ArtifactDataUpgradePlan {
            plan_id: Uuid::new_v4(),
            target_installation_id: Uuid::new_v4(),
            source,
            target: ArtifactDataScope {
                tenant_id,
                module_slug: "sample_module".to_string(),
                data_contract_revision: 2,
                policy_revision: 2,
            },
            hook_binding_id: "upgrade.v2".to_string(),
            records: vec![ArtifactDataUpgradeRecord {
                key: "state/current".to_string(),
                value: json!({ "version": 2 }),
                source_revision: 7,
            }],
            next_after_key: Some("state/current".to_string()),
        };
        let data = UpgradeApplyBroker {
            source: ArtifactDataRecord {
                key: "state/current".to_string(),
                value: json!({ "version": 1 }),
                revision: 7,
            },
            target: Arc::new(Mutex::new(HashMap::new())),
        };
        let checkpoints = RecordingCheckpointStore::default();
        let applier = ArtifactDataUpgradeApplier::new(data.clone(), checkpoints.clone());
        let request = ArtifactDataUpgradeApplyRequest {
            plan,
            installation_scope: ModuleInstallationScope::Tenant { tenant_id },
            expected_installation_revision: 4,
            has_irreversible_migration: true,
        };

        assert!(matches!(
            applier.apply(request.clone()).await,
            Err(ArtifactDataError::MigrationCheckpoint(_))
        ));
        assert_eq!(
            checkpoints.requests.lock().expect("checkpoint lock").len(),
            0
        );

        let retry = applier.apply(request).await.expect("idempotent retry");
        assert_eq!(retry.records[0].value, json!({ "version": 2 }));
        assert_eq!(retry.installation_revision, 5);
        assert_eq!(
            checkpoints.requests.lock().expect("checkpoint lock").len(),
            1
        );
    }

    #[test]
    fn object_metadata_never_accepts_a_physical_or_unbounded_identity() {
        let object = ArtifactDataObject {
            name: "exports/report.json".to_string(),
            content_type: "application/json".to_string(),
            size_bytes: 1024,
            digest_sha256: format!("sha256:{}", "a".repeat(64)),
            revision: 1,
        };
        assert!(object.validate().is_ok());

        let mut invalid = object;
        invalid.name = "../storage-key".to_string();
        assert_eq!(invalid.validate(), Err(ArtifactDataError::InvalidObject));
        invalid.name = "exports/report.json".to_string();
        invalid.digest_sha256 = "sha256:not-a-digest".to_string();
        assert_eq!(invalid.validate(), Err(ArtifactDataError::InvalidObject));
    }

    #[test]
    fn object_upload_derives_owner_verified_metadata() {
        let upload = ArtifactDataObjectUpload {
            name: "exports/report.json".to_string(),
            content_type: "application/json".to_string(),
            data: Bytes::from_static(b"{}"),
            expected_revision: None,
            idempotency_key: Uuid::new_v4(),
        };
        let object = object_for_upload(&upload).expect("bounded object upload");
        assert_eq!(object.size_bytes, 2);
        assert_eq!(
            object.digest_sha256,
            "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21e4d8f3f2a2fdb7fbe7a1b4d"
        );
        assert!(object.name.contains("report"));

        let mut invalid = upload;
        invalid.idempotency_key = Uuid::nil();
        assert_eq!(
            object_for_upload(&invalid),
            Err(ArtifactDataError::InvalidIdempotencyKey)
        );
    }
}
