use async_graphql::{
    ComplexObject, Context, Enum, InputObject, Json, Result, SimpleObject, dataloader::DataLoader,
};
use rustok_api::{
    ArtifactBindingExecutionAuditEntry, ArtifactUiContributionView,
    ArtifactUiContributionViewContent, ArtifactUiSurface as ArtifactUiSurfaceContract,
    ModuleCompositionSnapshotView, ModuleEffectivePolicyDecisionView,
    ModuleEffectivePolicyDenialReasonView, ModuleEffectivePolicyView, Permission,
    PlatformBuildSnapshot, PlatformBuildStage, PlatformBuildStatus, PlatformDeploymentProfile,
    StaticInstalledModuleView, StaticModuleRegistryView, StaticTenantModuleView,
};
use rustok_core::{UserRole, UserStatus};
use sea_orm::DatabaseConnection;
use std::str::FromStr;
use uuid::Uuid;

use crate::common::RequestContext;
use crate::graphql::loaders::TenantNameLoader;
use crate::models::users;
use crate::services::flex_attached_values::FlexAttachedValuesService;
use crate::services::module_lifecycle::ModuleLifecycleStateSnapshot;
use crate::services::rbac_service::RbacService;
use rustok_api::graphql::PageInfo;
use rustok_build::BuildEvent;
use rustok_build::build::{BuildStage, BuildStatus};

#[derive(SimpleObject, Clone)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub status: String,
    pub created_at: String,
    #[graphql(skip)]
    pub tenant_id: Uuid,
    #[graphql(skip)]
    pub metadata: serde_json::Value,
}

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlUserRole {
    SuperAdmin,
    Admin,
    Manager,
    Customer,
}

impl From<GqlUserRole> for UserRole {
    fn from(role: GqlUserRole) -> Self {
        match role {
            GqlUserRole::SuperAdmin => UserRole::SuperAdmin,
            GqlUserRole::Admin => UserRole::Admin,
            GqlUserRole::Manager => UserRole::Manager,
            GqlUserRole::Customer => UserRole::Customer,
        }
    }
}

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlUserStatus {
    Active,
    Inactive,
    Banned,
}

impl From<GqlUserStatus> for UserStatus {
    fn from(status: GqlUserStatus) -> Self {
        match status {
            GqlUserStatus::Active => UserStatus::Active,
            GqlUserStatus::Inactive => UserStatus::Inactive,
            GqlUserStatus::Banned => UserStatus::Banned,
        }
    }
}

#[derive(InputObject, Debug, Clone)]
pub struct UsersFilter {
    pub role: Option<GqlUserRole>,
    pub status: Option<GqlUserStatus>,
}

#[derive(InputObject, Debug, Clone)]
pub struct CreateUserInput {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
    pub role: Option<GqlUserRole>,
    pub status: Option<GqlUserStatus>,
    /// Optional custom fields validated against the tenant's active schema.
    pub custom_fields: Option<serde_json::Value>,
}

#[derive(InputObject, Debug, Clone)]
pub struct UpdateUserInput {
    pub email: Option<String>,
    pub password: Option<String>,
    pub name: Option<String>,
    pub role: Option<GqlUserRole>,
    pub status: Option<GqlUserStatus>,
    /// Optional custom fields patch — merged into existing metadata.
    pub custom_fields: Option<serde_json::Value>,
}

#[ComplexObject]
impl User {
    async fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.email.clone())
    }

    async fn role(&self, ctx: &Context<'_>) -> Result<String> {
        let db = ctx.data::<DatabaseConnection>()?;
        let role = RbacService::get_user_role(db, &self.tenant_id, &self.id)
            .await
            .map_err(|err| err.to_string())?;
        Ok(role.to_string())
    }

    async fn can(&self, ctx: &Context<'_>, action: String) -> Result<bool> {
        let db = ctx.data::<DatabaseConnection>()?;
        let permission = Permission::from_str(&action).map_err(|err| err.to_string())?;

        RbacService::has_permission(db, &self.tenant_id, &self.id, &permission)
            .await
            .map_err(|err| err.to_string().into())
    }

    async fn tenant_name(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        let loader = ctx.data::<DataLoader<TenantNameLoader>>()?;
        loader.load_one(self.tenant_id).await
    }

    async fn custom_fields(&self, ctx: &Context<'_>) -> Result<Option<serde_json::Value>> {
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<crate::context::TenantContext>()?;
        let preferred_locale = ctx
            .data_opt::<RequestContext>()
            .map(|request| request.locale.as_str())
            .unwrap_or(tenant.default_locale.as_str());

        FlexAttachedValuesService::resolve_merged_payload(
            db,
            self.tenant_id,
            "user",
            self.id,
            &self.metadata,
            preferred_locale,
            tenant.default_locale.as_str(),
        )
        .await
        .map_err(|err| err.to_string().into())
    }
}

impl From<&users::Model> for User {
    fn from(model: &users::Model) -> Self {
        Self {
            id: model.id,
            email: model.email.clone(),
            name: model.name.clone(),
            status: model.status.to_string(),
            created_at: model.created_at.to_rfc3339(),
            tenant_id: model.tenant_id,
            metadata: model.metadata.clone(),
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct TenantModule {
    pub module_slug: String,
    pub enabled: bool,
    pub settings: String,
    pub revision: i64,
}

impl From<StaticTenantModuleView> for TenantModule {
    fn from(module: StaticTenantModuleView) -> Self {
        Self {
            module_slug: module.module_slug,
            enabled: module.enabled,
            settings: module.settings,
            revision: module.revision,
        }
    }
}

impl TryFrom<ModuleLifecycleStateSnapshot> for TenantModule {
    type Error = &'static str;

    fn try_from(module: ModuleLifecycleStateSnapshot) -> Result<Self, Self::Error> {
        StaticTenantModuleView::try_from(module).map(Self::from)
    }
}

/// Tenant-specific availability intent for one admitted artifact installation.
/// `expected_revision` is the only revision value accepted by the next
/// enablement command; it is one when no explicit intent has been persisted.
#[derive(SimpleObject, Clone)]
pub struct ArtifactTenantLifecycle {
    pub installation_id: Uuid,
    pub enabled: bool,
    pub revision: i64,
    pub expected_revision: i64,
}

/// Owner-issued activation receipt for an installation in the authenticated
/// tenant scope. The predecessor is the exact direct serving predecessor, not
/// an arbitrary historical installation.
#[derive(SimpleObject, Clone)]
pub struct ArtifactActivation {
    pub installation_id: Uuid,
    pub operation_id: Uuid,
    pub predecessor_installation_id: Option<Uuid>,
    pub installation_revision: i64,
    pub predecessor_revision: Option<i64>,
}

/// Owner-issued deactivation receipt for an installation in the authenticated
/// tenant scope. It removes runtime bindings without deleting evidence or data.
#[derive(SimpleObject, Clone)]
pub struct ArtifactDeactivation {
    pub installation_id: Uuid,
    pub operation_id: Uuid,
    pub revision: i64,
}

/// Owner-issued uninstall receipt for an inactive installation in the
/// authenticated tenant scope. Physical retention and collection are separate.
#[derive(SimpleObject, Clone)]
pub struct ArtifactUninstall {
    pub installation_id: Uuid,
    pub operation_id: Uuid,
    pub revision: i64,
}

/// Owner-issued direct-predecessor rollback receipt in the authenticated tenant
/// scope. The returned target is the newly selected serving installation.
#[derive(SimpleObject, Clone)]
pub struct ArtifactRollback {
    pub operation_id: Uuid,
    pub source_installation_id: Uuid,
    pub target_installation_id: Uuid,
    pub source_revision: i64,
    pub target_revision: i64,
}

/// Preview evidence for an artifact settings purge.
#[derive(SimpleObject, Clone)]
pub struct ArtifactSettingsPurgePreview {
    pub installation_id: Uuid,
    pub data_owner_id: Uuid,
    pub settings_instance_id: Uuid,
    pub settings_revision: i64,
    pub has_recovery_point: bool,
    pub recovery_point_id: Option<Uuid>,
    pub can_purge: bool,
    pub reason: String,
}

/// Owner-issued receipt for a completed settings purge.
#[derive(SimpleObject, Clone)]
pub struct ArtifactSettingsPurgeReceipt {
    pub purge_operation_id: Uuid,
    pub recovery_point_id: Uuid,
    pub tombstone_revision: i64,
}

/// Preview evidence for an artifact structured data purge.
#[derive(SimpleObject, Clone)]
pub struct ArtifactDataPurgePreview {
    pub installation_id: Uuid,
    pub namespace_revision: i64,
    pub records_to_purge: i64,
    pub can_purge: bool,
    pub reason: String,
}

/// Owner-issued receipt for a completed structured data purge.
#[derive(SimpleObject, Clone)]
pub struct ArtifactDataPurgeReceipt {
    pub namespace_revision: i64,
    pub purged_records: i64,
}

/// Owner-issued receipt for a protected artifact settings recovery point.
#[derive(SimpleObject, Clone)]
pub struct ArtifactSettingsRecoveryPointReceipt {
    pub recovery_point_id: Uuid,
    pub settings_instance_id: Uuid,
    pub settings_revision: i64,
    pub retain_until: String,
}

/// Owner-issued receipt for restoring settings from a recovery point.
#[derive(SimpleObject, Clone)]
pub struct ArtifactSettingsRestoreReceipt {
    pub restore_operation_id: Uuid,
    pub recovery_point_id: Uuid,
    pub new_settings_instance_id: Uuid,
    pub target_installation_id: Option<Uuid>,
}

/// GraphQL adapter over the canonical host-safe artifact UI projection. The
/// content remains its exact tagged JSON contract because its shape is chosen
/// by the admitted contribution surface, not by a guest-provided GraphQL type.
#[derive(SimpleObject, Clone)]
pub struct ArtifactUiContribution {
    pub id: String,
    pub surface: ArtifactUiSurface,
    pub content: Json<ArtifactUiContributionViewContent>,
}

/// Typed GraphQL representation of the canonical host presentation surface.
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum ArtifactUiSurface {
    AdminSettings,
    AdminActions,
    AdminStatus,
    AdminHelp,
    AdminNavigation,
    AdminTable,
    AdminForm,
    StorefrontSlot,
}

impl From<ArtifactUiSurfaceContract> for ArtifactUiSurface {
    fn from(surface: ArtifactUiSurfaceContract) -> Self {
        match surface {
            ArtifactUiSurfaceContract::AdminSettings => Self::AdminSettings,
            ArtifactUiSurfaceContract::AdminActions => Self::AdminActions,
            ArtifactUiSurfaceContract::AdminStatus => Self::AdminStatus,
            ArtifactUiSurfaceContract::AdminHelp => Self::AdminHelp,
            ArtifactUiSurfaceContract::AdminNavigation => Self::AdminNavigation,
            ArtifactUiSurfaceContract::AdminTable => Self::AdminTable,
            ArtifactUiSurfaceContract::AdminForm => Self::AdminForm,
            ArtifactUiSurfaceContract::StorefrontSlot => Self::StorefrontSlot,
        }
    }
}

impl From<ArtifactUiContributionView> for ArtifactUiContribution {
    fn from(view: ArtifactUiContributionView) -> Self {
        Self {
            id: view.id,
            surface: view.surface.into(),
            content: Json(view.content),
        }
    }
}

/// GraphQL adapter over one canonical redacted artifact-binding audit entry.
/// The owner has already selected and authorized the binding through its
/// declared UI contribution before this value is constructed.
#[derive(SimpleObject, Clone)]
pub struct ArtifactUiActionAudit {
    pub execution_id: Uuid,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub error_code: Option<String>,
}

impl From<ArtifactBindingExecutionAuditEntry> for ArtifactUiActionAudit {
    fn from(entry: ArtifactBindingExecutionAuditEntry) -> Self {
        Self {
            execution_id: entry.execution_id,
            status: entry.status,
            started_at: entry.started_at,
            finished_at: entry.finished_at,
            duration_ms: entry.duration_ms,
            error_code: entry.error_code,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct ModuleOperationRecoveryPlan {
    pub operation_id: Uuid,
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub requested_enabled: bool,
    pub previous_effective_enabled: bool,
    pub status: String,
    pub issue: String,
    pub retryable: bool,
    pub recommended_action: String,
    pub correlation_id: Option<String>,
    pub requested_by: Option<String>,
    pub error_message: Option<String>,
}

impl From<rustok_api::ModuleOperationRecoveryPlanView> for ModuleOperationRecoveryPlan {
    fn from(plan: rustok_api::ModuleOperationRecoveryPlanView) -> Self {
        Self {
            operation_id: plan
                .operation_id
                .parse()
                .expect("owner recovery view contains a UUID operation identity"),
            tenant_id: plan
                .tenant_id
                .parse()
                .expect("owner recovery view contains a UUID tenant identity"),
            module_slug: plan.module_slug,
            requested_enabled: plan.requested_enabled,
            previous_effective_enabled: plan.previous_effective_enabled,
            status: plan.status,
            issue: plan.issue,
            retryable: plan.retryable,
            recommended_action: plan.recommended_action,
            correlation_id: plan.correlation_id,
            requested_by: plan.requested_by,
            error_message: plan.error_message,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct InstalledModule {
    pub slug: String,
    pub source: String,
    pub crate_name: String,
    pub version: Option<String>,
    pub required: bool,
    pub dependencies: Vec<String>,
}

/// Minimal immutable composition version exposed to control-plane clients for
/// mandatory optimistic-concurrency inputs. The manifest remains owner-owned
/// and is deliberately not duplicated on this transport object.
#[derive(SimpleObject, Clone)]
pub struct ModuleCompositionSnapshot {
    pub revision: i64,
}

impl From<ModuleCompositionSnapshotView> for ModuleCompositionSnapshot {
    fn from(snapshot: ModuleCompositionSnapshotView) -> Self {
        Self {
            revision: snapshot.revision,
        }
    }
}

/// GraphQL transport projection of the owner-issued module availability
/// decision. The full policy evidence remains inside the modules owner.
#[derive(SimpleObject, Clone)]
#[graphql(name = "ModuleEffectivePolicy")]
pub struct ModuleEffectivePolicyGql {
    pub policy_revision: String,
    pub decisions: Vec<ModuleEffectivePolicyDecisionGql>,
}

impl From<ModuleEffectivePolicyView> for ModuleEffectivePolicyGql {
    fn from(policy: ModuleEffectivePolicyView) -> Self {
        Self {
            policy_revision: policy.policy_revision,
            decisions: policy.decisions.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(name = "ModuleEffectivePolicyDecision")]
pub struct ModuleEffectivePolicyDecisionGql {
    pub module_slug: String,
    pub enabled: bool,
    pub policy_revision: String,
    pub denial_reasons: Vec<ModuleEffectivePolicyDenialReasonGql>,
}

impl From<ModuleEffectivePolicyDecisionView> for ModuleEffectivePolicyDecisionGql {
    fn from(decision: ModuleEffectivePolicyDecisionView) -> Self {
        Self {
            module_slug: decision.module_slug,
            enabled: decision.enabled,
            policy_revision: decision.policy_revision,
            denial_reasons: decision
                .denial_reasons
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(name = "ModuleEffectivePolicyDenialReason")]
pub struct ModuleEffectivePolicyDenialReasonGql {
    pub kind: String,
    pub module_slug: Option<String>,
}

impl From<ModuleEffectivePolicyDenialReasonView> for ModuleEffectivePolicyDenialReasonGql {
    fn from(reason: ModuleEffectivePolicyDenialReasonView) -> Self {
        let kind = reason.code().to_string();
        let module_slug = reason.related_module_slug().map(str::to_owned);

        Self { kind, module_slug }
    }
}

impl From<StaticInstalledModuleView> for InstalledModule {
    fn from(module: StaticInstalledModuleView) -> Self {
        Self {
            slug: module.slug,
            source: module.source,
            crate_name: module.crate_name,
            version: module.version,
            required: module.required,
            dependencies: module.dependencies,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct MarketplaceModuleVersion {
    pub version: String,
    pub changelog: Option<String>,
    pub yanked: bool,
    pub published_at: Option<String>,
    pub checksum_sha256: Option<String>,
    pub signature_present: bool,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryPublishRequestLifecycle {
    pub id: String,
    pub revision: i64,
    pub status: String,
    pub requested_by: String,
    pub publisher: Option<String>,
    pub approved_by: Option<String>,
    pub rejected_by: Option<String>,
    pub rejection_reason: Option<String>,
    pub changes_requested_by: Option<String>,
    pub changes_requested_reason: Option<String>,
    pub changes_requested_reason_code: Option<String>,
    pub changes_requested_at: Option<String>,
    pub held_by: Option<String>,
    pub held_reason: Option<String>,
    pub held_reason_code: Option<String>,
    pub held_at: Option<String>,
    pub held_from_status: Option<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryReleaseLifecycle {
    pub version: String,
    pub status: String,
    pub publisher: String,
    pub checksum_sha256: Option<String>,
    pub published_at: String,
    pub yanked_reason: Option<String>,
    pub yanked_by: Option<String>,
    pub yanked_at: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryOwnerLifecycle {
    pub owner: String,
    pub bound_by: String,
    pub bound_at: String,
    pub updated_at: String,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryOwnerTransitionLifecycle {
    pub previous_owner: Option<String>,
    pub new_owner: Option<String>,
    pub bound_by: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryAutomatedCheckLifecycle {
    pub key: String,
    pub status: String,
    pub detail: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryGovernanceEventPayloadLifecycle {
    pub reason: Option<String>,
    pub reason_code: Option<String>,
    pub detail: Option<String>,
    pub version: Option<String>,
    pub stage_key: Option<String>,
    pub attempt_number: Option<i32>,
    pub owner_transition: Option<RegistryOwnerTransitionLifecycle>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub mode: Option<String>,
    pub automated_checks: Vec<RegistryAutomatedCheckLifecycle>,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryGovernanceEventLifecycle {
    pub id: String,
    pub event_type: String,
    pub actor: String,
    pub publisher: Option<String>,
    pub payload: RegistryGovernanceEventPayloadLifecycle,
    pub created_at: String,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryFollowUpGateLifecycle {
    pub key: String,
    pub status: String,
    pub detail: String,
    pub updated_at: String,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryValidationStageLifecycle {
    pub key: String,
    pub status: String,
    pub detail: String,
    pub attempt_number: i32,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub execution_mode: String,
    pub runnable: bool,
    pub requires_manual_confirmation: bool,
    pub allowed_terminal_reason_codes: Vec<String>,
    pub suggested_pass_reason_code: Option<String>,
    pub suggested_failure_reason_code: Option<String>,
    pub suggested_blocked_reason_code: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryModerationPolicyLifecycle {
    pub mode: String,
    pub live_publish_supported: bool,
    pub live_governance_supported: bool,
    pub manual_review_required: bool,
    pub restriction_reason_code: Option<String>,
    pub restriction_reason: String,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryGovernanceActionLifecycle {
    pub key: String,
    pub reason_required: bool,
    pub reason_code_required: bool,
    pub reason_codes: Vec<String>,
    pub destructive: bool,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryModuleLifecycle {
    pub moderation_policy: RegistryModerationPolicyLifecycle,
    pub owner_binding: Option<RegistryOwnerLifecycle>,
    pub latest_request: Option<RegistryPublishRequestLifecycle>,
    pub latest_release: Option<RegistryReleaseLifecycle>,
    pub recent_events: Vec<RegistryGovernanceEventLifecycle>,
    pub follow_up_gates: Vec<RegistryFollowUpGateLifecycle>,
    pub validation_stages: Vec<RegistryValidationStageLifecycle>,
    pub governance_actions: Vec<RegistryGovernanceActionLifecycle>,
}

#[derive(SimpleObject, Clone)]
pub struct ModuleSettingField {
    pub key: String,
    #[graphql(name = "type")]
    pub value_type: String,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
    pub description: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub options: Vec<serde_json::Value>,
    pub object_keys: Vec<String>,
    pub item_type: Option<String>,
    pub shape: Option<serde_json::Value>,
}

#[derive(SimpleObject, Clone)]
pub struct MarketplaceModule {
    pub slug: String,
    pub name: String,
    pub latest_version: String,
    pub description: String,
    pub source: String,
    pub kind: String,
    pub category: String,
    pub tags: Vec<String>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub screenshots: Vec<String>,
    pub crate_name: String,
    pub dependencies: Vec<String>,
    pub ownership: String,
    pub trust_level: String,
    pub rustok_min_version: Option<String>,
    pub rustok_max_version: Option<String>,
    pub publisher: Option<String>,
    pub checksum_sha256: Option<String>,
    pub signature_present: bool,
    pub versions: Vec<MarketplaceModuleVersion>,
    pub has_admin_ui: bool,
    pub has_storefront_ui: bool,
    pub ui_classification: String,
    pub registry_lifecycle: Option<RegistryModuleLifecycle>,
    pub compatible: bool,
    pub recommended_admin_surfaces: Vec<String>,
    pub showcase_admin_surfaces: Vec<String>,
    pub settings_schema: Vec<ModuleSettingField>,
    pub installed: bool,
    pub installed_version: Option<String>,
    pub update_available: bool,
}

impl From<rustok_api::MarketplaceModuleVersion> for MarketplaceModuleVersion {
    fn from(value: rustok_api::MarketplaceModuleVersion) -> Self {
        Self {
            version: value.version,
            changelog: value.changelog,
            yanked: value.yanked,
            published_at: value.published_at,
            checksum_sha256: value.checksum_sha256,
            signature_present: value.signature_present,
        }
    }
}

impl From<rustok_api::RegistryOwnerTransitionLifecycle> for RegistryOwnerTransitionLifecycle {
    fn from(value: rustok_api::RegistryOwnerTransitionLifecycle) -> Self {
        Self {
            previous_owner: value.previous_owner,
            new_owner: value.new_owner,
            bound_by: value.bound_by,
        }
    }
}

impl From<rustok_api::RegistryAutomatedCheckLifecycle> for RegistryAutomatedCheckLifecycle {
    fn from(value: rustok_api::RegistryAutomatedCheckLifecycle) -> Self {
        Self {
            key: value.key,
            status: value.status,
            detail: value.detail,
        }
    }
}

impl From<rustok_api::RegistryGovernanceEventPayloadLifecycle>
    for RegistryGovernanceEventPayloadLifecycle
{
    fn from(value: rustok_api::RegistryGovernanceEventPayloadLifecycle) -> Self {
        Self {
            reason: value.reason,
            reason_code: value.reason_code,
            detail: value.detail,
            version: value.version,
            stage_key: value.stage_key,
            attempt_number: value.attempt_number,
            owner_transition: value.owner_transition.map(Into::into),
            warnings: value.warnings,
            errors: value.errors,
            mode: value.mode,
            automated_checks: value.automated_checks.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<rustok_api::RegistryGovernanceEventLifecycle> for RegistryGovernanceEventLifecycle {
    fn from(value: rustok_api::RegistryGovernanceEventLifecycle) -> Self {
        Self {
            id: value.id,
            event_type: value.event_type,
            actor: value.actor,
            publisher: value.publisher,
            payload: value.payload.into(),
            created_at: value.created_at,
        }
    }
}

impl From<rustok_api::RegistryFollowUpGateLifecycle> for RegistryFollowUpGateLifecycle {
    fn from(value: rustok_api::RegistryFollowUpGateLifecycle) -> Self {
        Self {
            key: value.key,
            status: value.status,
            detail: value.detail,
            updated_at: value.updated_at,
        }
    }
}

impl From<rustok_api::RegistryValidationStageLifecycle> for RegistryValidationStageLifecycle {
    fn from(value: rustok_api::RegistryValidationStageLifecycle) -> Self {
        Self {
            key: value.key,
            status: value.status,
            detail: value.detail,
            attempt_number: value.attempt_number,
            updated_at: value.updated_at,
            started_at: value.started_at,
            finished_at: value.finished_at,
            execution_mode: value.execution_mode,
            runnable: value.runnable,
            requires_manual_confirmation: value.requires_manual_confirmation,
            allowed_terminal_reason_codes: value.allowed_terminal_reason_codes,
            suggested_pass_reason_code: value.suggested_pass_reason_code,
            suggested_failure_reason_code: value.suggested_failure_reason_code,
            suggested_blocked_reason_code: value.suggested_blocked_reason_code,
        }
    }
}

impl From<rustok_api::RegistryGovernanceActionLifecycle> for RegistryGovernanceActionLifecycle {
    fn from(value: rustok_api::RegistryGovernanceActionLifecycle) -> Self {
        Self {
            key: value.key,
            reason_required: value.reason_required,
            reason_code_required: value.reason_code_required,
            reason_codes: value.reason_codes,
            destructive: value.destructive,
        }
    }
}

impl From<rustok_api::RegistryModerationPolicyLifecycle> for RegistryModerationPolicyLifecycle {
    fn from(value: rustok_api::RegistryModerationPolicyLifecycle) -> Self {
        Self {
            mode: value.mode,
            live_publish_supported: value.live_publish_supported,
            live_governance_supported: value.live_governance_supported,
            manual_review_required: value.manual_review_required,
            restriction_reason_code: value.restriction_reason_code,
            restriction_reason: value.restriction_reason,
        }
    }
}

impl From<rustok_api::RegistryOwnerLifecycle> for RegistryOwnerLifecycle {
    fn from(value: rustok_api::RegistryOwnerLifecycle) -> Self {
        Self {
            owner: value.owner,
            bound_by: value.bound_by,
            bound_at: value.bound_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<rustok_api::RegistryPublishRequestLifecycle> for RegistryPublishRequestLifecycle {
    fn from(value: rustok_api::RegistryPublishRequestLifecycle) -> Self {
        Self {
            id: value.id,
            revision: value.revision,
            status: value.status,
            requested_by: value.requested_by,
            publisher: value.publisher,
            approved_by: value.approved_by,
            rejected_by: value.rejected_by,
            rejection_reason: value.rejection_reason,
            changes_requested_by: value.changes_requested_by,
            changes_requested_reason: value.changes_requested_reason,
            changes_requested_reason_code: value.changes_requested_reason_code,
            changes_requested_at: value.changes_requested_at,
            held_by: value.held_by,
            held_reason: value.held_reason,
            held_reason_code: value.held_reason_code,
            held_at: value.held_at,
            held_from_status: value.held_from_status,
            warnings: value.warnings,
            errors: value.errors,
            created_at: value.created_at,
            updated_at: value.updated_at,
            published_at: value.published_at,
        }
    }
}

impl From<rustok_api::RegistryReleaseLifecycle> for RegistryReleaseLifecycle {
    fn from(value: rustok_api::RegistryReleaseLifecycle) -> Self {
        Self {
            version: value.version,
            status: value.status,
            publisher: value.publisher,
            checksum_sha256: value.checksum_sha256,
            published_at: value.published_at,
            yanked_reason: value.yanked_reason,
            yanked_by: value.yanked_by,
            yanked_at: value.yanked_at,
        }
    }
}

impl From<rustok_api::RegistryModuleLifecycle> for RegistryModuleLifecycle {
    fn from(value: rustok_api::RegistryModuleLifecycle) -> Self {
        Self {
            moderation_policy: value.moderation_policy.into(),
            owner_binding: value.owner_binding.map(Into::into),
            latest_request: value.latest_request.map(Into::into),
            latest_release: value.latest_release.map(Into::into),
            recent_events: value.recent_events.into_iter().map(Into::into).collect(),
            follow_up_gates: value.follow_up_gates.into_iter().map(Into::into).collect(),
            validation_stages: value
                .validation_stages
                .into_iter()
                .map(Into::into)
                .collect(),
            governance_actions: value
                .governance_actions
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<rustok_api::ModuleSettingField> for ModuleSettingField {
    fn from(value: rustok_api::ModuleSettingField) -> Self {
        Self {
            key: value.key,
            value_type: value.value_type,
            required: value.required,
            default_value: value.default_value,
            description: value.description,
            min: value.min,
            max: value.max,
            options: value.options,
            object_keys: value.object_keys,
            item_type: value.item_type,
            shape: value.shape,
        }
    }
}

impl From<rustok_api::MarketplaceModule> for MarketplaceModule {
    fn from(value: rustok_api::MarketplaceModule) -> Self {
        Self {
            slug: value.slug,
            name: value.name,
            latest_version: value.latest_version,
            description: value.description,
            source: value.source,
            kind: value.kind,
            category: value.category,
            tags: value.tags,
            icon_url: value.icon_url,
            banner_url: value.banner_url,
            screenshots: value.screenshots,
            crate_name: value.crate_name,
            dependencies: value.dependencies,
            ownership: value.ownership,
            trust_level: value.trust_level,
            rustok_min_version: value.rustok_min_version,
            rustok_max_version: value.rustok_max_version,
            publisher: value.publisher,
            checksum_sha256: value.checksum_sha256,
            signature_present: value.signature_present,
            versions: value.versions.into_iter().map(Into::into).collect(),
            has_admin_ui: value.has_admin_ui,
            has_storefront_ui: value.has_storefront_ui,
            ui_classification: value.ui_classification,
            registry_lifecycle: value.registry_lifecycle.map(Into::into),
            compatible: value.compatible,
            recommended_admin_surfaces: value.recommended_admin_surfaces,
            showcase_admin_surfaces: value.showcase_admin_surfaces,
            settings_schema: value.settings_schema.into_iter().map(Into::into).collect(),
            installed: value.installed,
            installed_version: value.installed_version,
            update_available: value.update_available,
        }
    }
}

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum MarketplaceRegistryStatus {
    Unknown,
    Ready,
    Degraded,
}

#[derive(SimpleObject, Clone, Debug, Eq, PartialEq)]
pub struct MarketplaceRegistryFreshness {
    pub registry_id: String,
    pub status: MarketplaceRegistryStatus,
    pub last_success_unix_ms: Option<u64>,
    pub consecutive_failures: u64,
}

impl From<rustok_api::MarketplaceRegistryFreshness> for MarketplaceRegistryFreshness {
    fn from(value: rustok_api::MarketplaceRegistryFreshness) -> Self {
        Self {
            registry_id: value.registry_id,
            status: match value.status {
                rustok_api::MarketplaceRegistryStatus::Unknown => {
                    MarketplaceRegistryStatus::Unknown
                }
                rustok_api::MarketplaceRegistryStatus::Ready => MarketplaceRegistryStatus::Ready,
                rustok_api::MarketplaceRegistryStatus::Degraded => {
                    MarketplaceRegistryStatus::Degraded
                }
            },
            last_success_unix_ms: value.last_success_unix_ms,
            consecutive_failures: value.consecutive_failures,
        }
    }
}

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlBuildStatus {
    Queued,
    Running,
    Success,
    Failed,
    Cancelled,
}

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlBuildEventKind {
    Requested,
    Started,
    Progress,
    Completed,
    Cancelled,
    Failed,
}

impl From<BuildStatus> for GqlBuildStatus {
    fn from(status: BuildStatus) -> Self {
        match status {
            BuildStatus::Queued => Self::Queued,
            BuildStatus::Running => Self::Running,
            BuildStatus::Success => Self::Success,
            BuildStatus::Failed => Self::Failed,
            BuildStatus::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlBuildStage {
    Pending,
    Checkout,
    Build,
    Test,
    Deploy,
    Complete,
}

impl From<BuildStage> for GqlBuildStage {
    fn from(stage: BuildStage) -> Self {
        match stage {
            BuildStage::Pending => Self::Pending,
            BuildStage::Checkout => Self::Checkout,
            BuildStage::Build => Self::Build,
            BuildStage::Test => Self::Test,
            BuildStage::Deploy => Self::Deploy,
            BuildStage::Complete => Self::Complete,
        }
    }
}

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlDeploymentProfile {
    Monolith,
    ServerWithAdmin,
    ServerWithStorefront,
    HeadlessApi,
    Worker,
    Registry,
}

impl From<PlatformDeploymentProfile> for GqlDeploymentProfile {
    fn from(profile: PlatformDeploymentProfile) -> Self {
        match profile {
            PlatformDeploymentProfile::Monolith => Self::Monolith,
            PlatformDeploymentProfile::ServerWithAdmin => Self::ServerWithAdmin,
            PlatformDeploymentProfile::ServerWithStorefront => Self::ServerWithStorefront,
            PlatformDeploymentProfile::HeadlessApi => Self::HeadlessApi,
            PlatformDeploymentProfile::Worker => Self::Worker,
            PlatformDeploymentProfile::Registry => Self::Registry,
        }
    }
}

impl From<PlatformBuildStatus> for GqlBuildStatus {
    fn from(status: PlatformBuildStatus) -> Self {
        match status {
            PlatformBuildStatus::Queued => Self::Queued,
            PlatformBuildStatus::Running => Self::Running,
            PlatformBuildStatus::Success => Self::Success,
            PlatformBuildStatus::Failed => Self::Failed,
            PlatformBuildStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<PlatformBuildStage> for GqlBuildStage {
    fn from(stage: PlatformBuildStage) -> Self {
        match stage {
            PlatformBuildStage::Pending => Self::Pending,
            PlatformBuildStage::Checkout => Self::Checkout,
            PlatformBuildStage::Build => Self::Build,
            PlatformBuildStage::Test => Self::Test,
            PlatformBuildStage::Deploy => Self::Deploy,
            PlatformBuildStage::Complete => Self::Complete,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct BuildJob {
    pub id: String,
    pub status: GqlBuildStatus,
    pub stage: GqlBuildStage,
    pub progress: i32,
    pub profile: GqlDeploymentProfile,
    pub manifest_ref: String,
    pub manifest_hash: String,
    pub manifest_revision: i64,
    pub modules_delta: String,
    pub build_command: Option<String>,
    pub build_features: Vec<String>,
    pub build_target: Option<String>,
    pub build_profile: Option<String>,
    pub requested_by: String,
    pub reason: Option<String>,
    pub logs_url: Option<String>,
    pub error_message: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl BuildJob {
    pub fn from_snapshot(snapshot: &PlatformBuildSnapshot) -> Self {
        Self {
            id: snapshot.id.clone(),
            status: snapshot.status.into(),
            stage: snapshot.stage.into(),
            progress: snapshot.progress,
            profile: snapshot.profile.into(),
            manifest_ref: snapshot.manifest_ref.clone(),
            manifest_hash: snapshot.manifest_hash.clone(),
            manifest_revision: snapshot.manifest_revision,
            modules_delta: snapshot.modules_delta.clone(),
            build_command: snapshot.build_command.clone(),
            build_features: snapshot.build_features.clone(),
            build_target: snapshot.build_target.clone(),
            build_profile: snapshot.build_profile.clone(),
            requested_by: snapshot.requested_by.clone(),
            reason: snapshot.reason.clone(),
            logs_url: snapshot.logs_url.clone(),
            error_message: snapshot.error_message.clone(),
            started_at: snapshot.started_at.clone(),
            finished_at: snapshot.finished_at.clone(),
            created_at: snapshot.created_at.clone(),
            updated_at: snapshot.updated_at.clone(),
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct BuildProgressEvent {
    pub kind: GqlBuildEventKind,
    pub build_id: String,
    pub status: GqlBuildStatus,
    pub stage: GqlBuildStage,
    pub progress: i32,
    pub error_message: Option<String>,
}

impl BuildProgressEvent {
    pub fn from_event(event: BuildEvent) -> Self {
        match event {
            BuildEvent::BuildRequested { build_id, .. } => Self {
                kind: GqlBuildEventKind::Requested,
                build_id: build_id.to_string(),
                status: GqlBuildStatus::Queued,
                stage: GqlBuildStage::Pending,
                progress: 0,
                error_message: None,
            },
            BuildEvent::BuildStarted {
                build_id,
                stage,
                progress,
            } => Self {
                kind: GqlBuildEventKind::Started,
                build_id: build_id.to_string(),
                status: GqlBuildStatus::Running,
                stage: stage.into(),
                progress,
                error_message: None,
            },
            BuildEvent::BuildProgress {
                build_id,
                stage,
                progress,
            } => Self {
                kind: GqlBuildEventKind::Progress,
                build_id: build_id.to_string(),
                status: GqlBuildStatus::Running,
                stage: stage.into(),
                progress,
                error_message: None,
            },
            BuildEvent::BuildCompleted { build_id } => Self {
                kind: GqlBuildEventKind::Completed,
                build_id: build_id.to_string(),
                status: GqlBuildStatus::Success,
                stage: GqlBuildStage::Complete,
                progress: 100,
                error_message: None,
            },
            BuildEvent::BuildCancelled {
                build_id,
                stage,
                progress,
            } => Self {
                kind: GqlBuildEventKind::Cancelled,
                build_id: build_id.to_string(),
                status: GqlBuildStatus::Cancelled,
                stage: stage.into(),
                progress,
                error_message: None,
            },
            BuildEvent::BuildFailed {
                build_id,
                stage,
                progress,
                error,
            } => Self {
                kind: GqlBuildEventKind::Failed,
                build_id: build_id.to_string(),
                status: GqlBuildStatus::Failed,
                stage: stage.into(),
                progress,
                error_message: Some(error),
            },
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct DeleteUserPayload {
    pub success: bool,
}

#[derive(SimpleObject, Clone)]
pub struct ModuleRegistryItem {
    pub module_slug: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub kind: String,
    pub enabled: bool,
    pub lifecycle_revision: i64,
    pub dependencies: Vec<String>,
    pub ownership: String,
    pub trust_level: String,
    pub has_admin_ui: bool,
    pub has_storefront_ui: bool,
    pub ui_classification: String,
    pub recommended_admin_surfaces: Vec<String>,
    pub showcase_admin_surfaces: Vec<String>,
}

impl From<StaticModuleRegistryView> for ModuleRegistryItem {
    fn from(module: StaticModuleRegistryView) -> Self {
        Self {
            module_slug: module.module_slug,
            name: module.name,
            description: module.description,
            version: module.version,
            kind: module.kind,
            enabled: module.enabled,
            lifecycle_revision: module.lifecycle_revision,
            dependencies: module.dependencies,
            ownership: module.ownership,
            trust_level: module.trust_level,
            has_admin_ui: module.has_admin_ui,
            has_storefront_ui: module.has_storefront_ui,
            ui_classification: module.ui_classification,
            recommended_admin_surfaces: module.recommended_admin_surfaces,
            showcase_admin_surfaces: module.showcase_admin_surfaces,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct UserEdge {
    pub node: User,
    pub cursor: String,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct UserConnection {
    pub edges: Vec<UserEdge>,
    pub page_info: PageInfo,
}

#[derive(SimpleObject, Clone)]
pub struct DashboardStats {
    pub total_users: i64,
    pub total_posts: i64,
    pub total_orders: i64,
    pub total_revenue: i64,
    pub users_change: f64,
    pub posts_change: f64,
    pub orders_change: f64,
    pub revenue_change: f64,
}

#[derive(SimpleObject, Clone)]
pub struct ActivityItem {
    pub id: String,
    pub r#type: String,
    pub description: String,
    pub timestamp: String,
    pub user: Option<ActivityUser>,
}

#[derive(SimpleObject, Clone)]
pub struct ActivityUser {
    pub id: String,
    pub name: Option<String>,
}

#[cfg(test)]
mod tests {
    use rustok_api::{
        ArtifactUiActionConfirmation, ArtifactUiContributionView,
        ArtifactUiContributionViewContent, ArtifactUiSurface as ArtifactUiSurfaceContract,
    };

    use super::{ArtifactUiContribution, ArtifactUiSurface};

    #[test]
    fn artifact_ui_adapter_preserves_the_canonical_projection() {
        let view = ArtifactUiContributionView {
            id: "profile_form".to_string(),
            surface: ArtifactUiSurfaceContract::AdminForm,
            content: ArtifactUiContributionViewContent::Form {
                title: "Profile".to_string(),
                schema: serde_json::json!({"type": "object"}),
                confirmation: ArtifactUiActionConfirmation::Acknowledge,
                destructive: false,
            },
        };

        let contribution = ArtifactUiContribution::from(view.clone());
        assert_eq!(contribution.id, view.id);
        assert_eq!(contribution.surface, ArtifactUiSurface::AdminForm);
        assert_eq!(contribution.content.0, view.content);
    }
}
