//! Artifact data upgrade hooks, planners, and appliers.

use async_trait::async_trait;
use rustok_sandbox::ExecutionPhase;
use sea_orm::DatabaseConnection;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::*;
use super::error::*;
use super::traits::*;
use super::types::*;
use super::validation::*;

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

pub(crate) fn validate_upgrade_request(request: &ArtifactDataUpgradeRequest) -> Result<(), ArtifactDataError> {
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
                context: request.context.clone(),
                reason: request.reason,
            })
            .await?;
        Ok(ArtifactDataUpgradeApplyResult {
            records,
            installation_revision,
        })
    }
}

pub(crate) fn validate_upgrade_apply_request(
    request: &ArtifactDataUpgradeApplyRequest,
) -> Result<(), ArtifactDataError> {
    validate_upgrade_plan(&request.plan)?;
    let scope_tenant_id = match request.installation_scope {
        ModuleInstallationScope::Platform => None,
        ModuleInstallationScope::Tenant { tenant_id } if !tenant_id.is_nil() => Some(tenant_id),
        ModuleInstallationScope::Tenant { .. } => return Err(ArtifactDataError::InvalidUpgrade),
    };
    if request.expected_installation_revision == 0
        || request.expected_installation_revision > i64::MAX as u64
        || request.reason.trim().is_empty()
        || request.context.tenant_id != scope_tenant_id
        || request.context.validate().is_err()
    {
        return Err(ArtifactDataError::InvalidUpgrade);
    }
    Ok(())
}

pub(crate) fn validate_upgrade_plan(plan: &ArtifactDataUpgradePlan) -> Result<(), ArtifactDataError> {
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

pub(crate) fn upgrade_record_idempotency_key(
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
