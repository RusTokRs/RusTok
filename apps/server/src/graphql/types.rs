use async_graphql::{
    ComplexObject, Context, Enum, InputObject, Json, Result, SimpleObject, dataloader::DataLoader,
};
use rustok_api::{
    ArtifactBindingExecutionAuditEntry, ArtifactUiContributionView,
    ArtifactUiContributionViewContent, ArtifactUiSurface as ArtifactUiSurfaceContract, Permission,
    PlatformBuildSnapshot, PlatformBuildStage, PlatformBuildStatus, PlatformDeploymentProfile,
};
use rustok_core::{UserRole, UserStatus};
use sea_orm::DatabaseConnection;
use std::str::FromStr;
use uuid::Uuid;

use crate::common::RequestContext;
use crate::graphql::loaders::TenantNameLoader;
use crate::models::users;
use crate::modules::{InstalledManifestModule, ModuleSettingSpec, module_setting_shape_value};
use crate::services::flex_attached_values::FlexAttachedValuesService;
use crate::services::rbac_service::RbacService;
use crate::services::registry_principal::RegistryPrincipalRef;
use rustok_api::graphql::PageInfo;
use rustok_build::BuildEvent;
use rustok_build::build::{BuildStage, BuildStatus};
use rustok_modules::ModuleOperationRecoveryPlan as ServiceModuleOperationRecoveryPlan;

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

impl From<&ServiceModuleOperationRecoveryPlan> for ModuleOperationRecoveryPlan {
    fn from(plan: &ServiceModuleOperationRecoveryPlan) -> Self {
        Self {
            operation_id: plan.operation_id,
            tenant_id: plan.tenant_id,
            module_slug: plan.module_slug.clone(),
            requested_enabled: plan.requested_enabled,
            previous_effective_enabled: plan.previous_effective_enabled,
            status: plan.status.as_str().to_string(),
            issue: plan.issue.as_str().to_string(),
            retryable: plan.retryable,
            recommended_action: plan.recommended_action.as_str().to_string(),
            correlation_id: plan.correlation_id.clone(),
            requested_by: plan.requested_by.clone(),
            error_message: plan.error_message.clone(),
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct InstalledModule {
    pub slug: String,
    pub source: String,
    pub crate_name: String,
    pub version: Option<String>,
    pub git: Option<String>,
    pub rev: Option<String>,
    pub path: Option<String>,
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

impl From<&InstalledManifestModule> for InstalledModule {
    fn from(module: &InstalledManifestModule) -> Self {
        Self {
            slug: module.slug.clone(),
            source: module.source.clone(),
            crate_name: module.crate_name.clone(),
            version: module.version.clone(),
            git: module.git.clone(),
            rev: module.rev.clone(),
            path: module.path.clone(),
            required: module.required,
            dependencies: module.depends_on.clone(),
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
pub struct RegistryPrincipal {
    pub kind: String,
    pub user_id: Option<String>,
    pub subject: String,
    pub display_label: String,
    pub legacy_label: Option<String>,
}

impl From<RegistryPrincipalRef> for RegistryPrincipal {
    fn from(value: RegistryPrincipalRef) -> Self {
        Self {
            kind: match value.kind {
                crate::services::registry_principal::RegistryPrincipalKind::User => "user",
                crate::services::registry_principal::RegistryPrincipalKind::Runner => "runner",
                crate::services::registry_principal::RegistryPrincipalKind::Legacy => "legacy",
            }
            .to_string(),
            user_id: value.user_id.map(|value| value.to_string()),
            subject: value.subject,
            display_label: value.display_label,
            legacy_label: value.legacy_label,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct RegistryPublishRequestLifecycle {
    pub id: String,
    pub revision: i64,
    pub status: String,
    pub requested_by: RegistryPrincipal,
    pub publisher: Option<RegistryPrincipal>,
    pub approved_by: Option<RegistryPrincipal>,
    pub rejected_by: Option<RegistryPrincipal>,
    pub rejection_reason: Option<String>,
    pub changes_requested_by: Option<RegistryPrincipal>,
    pub changes_requested_reason: Option<String>,
    pub changes_requested_reason_code: Option<String>,
    pub changes_requested_at: Option<String>,
    pub held_by: Option<RegistryPrincipal>,
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
    pub publisher: RegistryPrincipal,
    pub checksum_sha256: Option<String>,
    pub published_at: String,
    pub yanked_reason: Option<String>,
    pub yanked_by: Option<RegistryPrincipal>,
    pub yanked_at: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryOwnerLifecycle {
    pub owner: RegistryPrincipal,
    pub bound_by: RegistryPrincipal,
    pub bound_at: String,
    pub updated_at: String,
}

#[derive(SimpleObject, Clone)]
pub struct RegistryOwnerTransitionLifecycle {
    pub previous_owner: Option<RegistryPrincipal>,
    pub new_owner: Option<RegistryPrincipal>,
    pub bound_by: Option<RegistryPrincipal>,
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
}

#[derive(SimpleObject, Clone)]
pub struct RegistryGovernanceEventLifecycle {
    pub id: String,
    pub event_type: String,
    pub actor: RegistryPrincipal,
    pub publisher: Option<RegistryPrincipal>,
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

impl ModuleSettingField {
    pub fn from_spec(key: String, spec: &ModuleSettingSpec) -> Self {
        let object_keys = if spec.properties.is_empty() {
            spec.object_keys.clone()
        } else {
            let mut keys = spec.properties.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            keys
        };
        let item_type = spec
            .items
            .as_deref()
            .map(|item| item.value_type.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| spec.item_type.clone());

        Self {
            key,
            value_type: spec.value_type.clone(),
            required: spec.required,
            default_value: spec.default.clone(),
            description: spec.description.clone(),
            min: spec.min,
            max: spec.max,
            options: spec.options.clone(),
            object_keys,
            item_type,
            shape: module_setting_shape_value(spec),
        }
    }
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
    pub settings_schema: Vec<ModuleSettingField>,
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
