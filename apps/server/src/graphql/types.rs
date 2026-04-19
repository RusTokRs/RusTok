use async_graphql::{
    dataloader::DataLoader, ComplexObject, Context, Enum, InputObject, Result, SimpleObject,
};
use rustok_core::{Permission, UserRole, UserStatus};
use std::str::FromStr;
use uuid::Uuid;

use crate::common::RequestContext;
use crate::graphql::common::PageInfo;
use crate::graphql::loaders::TenantNameLoader;
use crate::models::build::{BuildStage, BuildStatus, DeploymentProfile, Model as BuildModel};
use crate::models::release::{Model as ReleaseModel, ReleaseStatus};
use crate::models::users;
use crate::modules::{
    module_setting_shape_value, BuildExecutionPlan, InstalledManifestModule, ModuleSettingSpec,
};
use crate::services::build_service::BuildEvent;
use crate::services::flex_attached_values::FlexAttachedValuesService;
use crate::services::rbac_service::RbacService;
use crate::services::registry_principal::RegistryPrincipalRef;

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
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let role = RbacService::get_user_role(&app_ctx.db, &self.tenant_id, &self.id)
            .await
            .map_err(|err| err.to_string())?;
        Ok(role.to_string())
    }

    async fn can(&self, ctx: &Context<'_>, action: String) -> Result<bool> {
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let permission = Permission::from_str(&action).map_err(|err| err.to_string())?;

        RbacService::has_permission(&app_ctx.db, &self.tenant_id, &self.id, &permission)
            .await
            .map_err(|err| err.to_string().into())
    }

    async fn tenant_name(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        let loader = ctx.data::<DataLoader<TenantNameLoader>>()?;
        loader.load_one(self.tenant_id).await
    }

    async fn custom_fields(&self, ctx: &Context<'_>) -> Result<Option<serde_json::Value>> {
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let tenant = ctx.data::<crate::context::TenantContext>()?;
        let preferred_locale = ctx
            .data_opt::<RequestContext>()
            .map(|request| request.locale.as_str())
            .unwrap_or(tenant.default_locale.as_str());

        FlexAttachedValuesService::resolve_merged_payload(
            &app_ctx.db,
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
pub enum GqlBuildStatus {
    Queued,
    Running,
    Success,
    Failed,
    Cancelled,
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
}

impl From<DeploymentProfile> for GqlDeploymentProfile {
    fn from(profile: DeploymentProfile) -> Self {
        match profile {
            DeploymentProfile::Monolith => Self::Monolith,
            DeploymentProfile::ServerWithAdmin => Self::ServerWithAdmin,
            DeploymentProfile::ServerWithStorefront => Self::ServerWithStorefront,
            DeploymentProfile::HeadlessApi => Self::HeadlessApi,
        }
    }
}

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlReleaseStatus {
    Pending,
    Deploying,
    Active,
    RolledBack,
    Failed,
}

impl From<ReleaseStatus> for GqlReleaseStatus {
    fn from(status: ReleaseStatus) -> Self {
        match status {
            ReleaseStatus::Pending => Self::Pending,
            ReleaseStatus::Deploying => Self::Deploying,
            ReleaseStatus::Active => Self::Active,
            ReleaseStatus::RolledBack => Self::RolledBack,
            ReleaseStatus::Failed => Self::Failed,
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
    pub modules_delta: String,
    pub build_command: Option<String>,
    pub build_features: Vec<String>,
    pub build_target: Option<String>,
    pub build_profile: Option<String>,
    pub requested_by: String,
    pub reason: Option<String>,
    pub release_id: Option<String>,
    pub logs_url: Option<String>,
    pub error_message: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn build_execution_plan(value: Option<&serde_json::Value>) -> Option<BuildExecutionPlan> {
    value
        .and_then(|value| value.get("execution_plan"))
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

fn build_modules_delta_summary(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };

    if let Some(summary) = value.as_str() {
        return summary.to_string();
    }

    if let Some(summary) = value.get("summary").and_then(serde_json::Value::as_str) {
        return summary.to_string();
    }

    if let Some(object) = value.as_object() {
        let mut slugs = object.keys().cloned().collect::<Vec<_>>();
        slugs.sort();
        return slugs.join(",");
    }

    value.to_string()
}

impl BuildJob {
    pub fn from_model(model: &BuildModel) -> Self {
        let execution_plan = build_execution_plan(model.modules_delta.as_ref());

        Self {
            id: model.id.to_string(),
            status: model.status.clone().into(),
            stage: model.stage.clone().into(),
            progress: model.progress,
            profile: model.profile.clone().into(),
            manifest_ref: model.manifest_ref.clone(),
            manifest_hash: model.manifest_hash.clone(),
            modules_delta: build_modules_delta_summary(model.modules_delta.as_ref()),
            build_command: execution_plan
                .as_ref()
                .map(|plan| plan.cargo_command.clone()),
            build_features: execution_plan
                .as_ref()
                .map(|plan| plan.cargo_features.clone())
                .unwrap_or_default(),
            build_target: execution_plan
                .as_ref()
                .and_then(|plan| plan.cargo_target.clone()),
            build_profile: execution_plan
                .as_ref()
                .map(|plan| plan.cargo_profile.clone()),
            requested_by: model.requested_by.clone(),
            reason: model.reason.clone(),
            release_id: model.release_id.clone(),
            logs_url: model.logs_url.clone(),
            error_message: model.error_message.clone(),
            started_at: model.started_at.map(|value| value.to_rfc3339()),
            finished_at: model.finished_at.map(|value| value.to_rfc3339()),
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct BuildProgressEvent {
    pub build_id: String,
    pub status: GqlBuildStatus,
    pub stage: GqlBuildStage,
    pub progress: i32,
    pub release_id: Option<String>,
    pub error_message: Option<String>,
}

impl BuildProgressEvent {
    pub fn from_event(event: BuildEvent) -> Self {
        match event {
            BuildEvent::BuildRequested { build_id, .. } => Self {
                build_id: build_id.to_string(),
                status: GqlBuildStatus::Queued,
                stage: GqlBuildStage::Pending,
                progress: 0,
                release_id: None,
                error_message: None,
            },
            BuildEvent::BuildStarted {
                build_id,
                stage,
                progress,
            } => Self {
                build_id: build_id.to_string(),
                status: GqlBuildStatus::Running,
                stage: stage.into(),
                progress,
                release_id: None,
                error_message: None,
            },
            BuildEvent::BuildProgress {
                build_id,
                stage,
                progress,
            } => Self {
                build_id: build_id.to_string(),
                status: GqlBuildStatus::Running,
                stage: stage.into(),
                progress,
                release_id: None,
                error_message: None,
            },
            BuildEvent::BuildCompleted {
                build_id,
                release_id,
            } => Self {
                build_id: build_id.to_string(),
                status: GqlBuildStatus::Success,
                stage: GqlBuildStage::Complete,
                progress: 100,
                release_id,
                error_message: None,
            },
            BuildEvent::BuildCancelled {
                build_id,
                stage,
                progress,
            } => Self {
                build_id: build_id.to_string(),
                status: GqlBuildStatus::Cancelled,
                stage: stage.into(),
                progress,
                release_id: None,
                error_message: None,
            },
            BuildEvent::BuildFailed {
                build_id,
                stage,
                progress,
                error,
            } => Self {
                build_id: build_id.to_string(),
                status: GqlBuildStatus::Failed,
                stage: stage.into(),
                progress,
                release_id: None,
                error_message: Some(error),
            },
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct ReleaseInfo {
    pub id: String,
    pub build_id: String,
    pub status: GqlReleaseStatus,
    pub environment: String,
    pub container_image: Option<String>,
    pub server_artifact_url: Option<String>,
    pub admin_artifact_url: Option<String>,
    pub storefront_artifact_url: Option<String>,
    pub manifest_hash: String,
    pub modules: Vec<String>,
    pub previous_release_id: Option<String>,
    pub deployed_at: Option<String>,
    pub rolled_back_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ReleaseInfo {
    pub fn from_model(model: &ReleaseModel) -> Self {
        Self {
            id: model.id.clone(),
            build_id: model.build_id.to_string(),
            status: model.status.clone().into(),
            environment: model.environment.clone(),
            container_image: model.container_image.clone(),
            server_artifact_url: model.server_artifact_url.clone(),
            admin_artifact_url: model.admin_artifact_url.clone(),
            storefront_artifact_url: model.storefront_artifact_url.clone(),
            manifest_hash: model.manifest_hash.clone(),
            modules: serde_json::from_value(model.modules.clone()).unwrap_or_default(),
            previous_release_id: model.previous_release_id.clone(),
            deployed_at: model.deployed_at.map(|value| value.to_rfc3339()),
            rolled_back_at: model.rolled_back_at.map(|value| value.to_rfc3339()),
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
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

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
#[derive(InputObject, Debug, Clone)]
pub struct PromoteTopicToPostInput {
    pub topic_id: Uuid,
    pub locale: String,
    pub blog_category_id: Option<Uuid>,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
#[derive(InputObject, Debug, Clone)]
pub struct DemotePostToTopicInput {
    pub post_id: Uuid,
    pub locale: String,
    pub forum_category_id: Uuid,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[cfg(feature = "mod-content")]
#[derive(InputObject, Debug, Clone)]
pub struct SplitTopicInput {
    pub topic_id: Uuid,
    pub locale: String,
    pub reply_ids: Vec<Uuid>,
    pub new_title: String,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[cfg(feature = "mod-content")]
#[derive(InputObject, Debug, Clone)]
pub struct MergeTopicsInput {
    pub target_topic_id: Uuid,
    pub source_topic_ids: Vec<Uuid>,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[cfg(feature = "mod-content")]
#[derive(SimpleObject, Debug, Clone)]
pub struct ContentOrchestrationPayload {
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub moved_comments: u64,
}

#[cfg(feature = "mod-content")]
#[derive(SimpleObject, Debug, Clone)]
pub struct ResolvedCanonicalRoute {
    pub target_kind: String,
    pub target_id: Uuid,
    pub locale: String,
    pub matched_url: String,
    pub canonical_url: String,
    pub redirect_required: bool,
}

#[cfg(feature = "mod-content")]
impl From<rustok_content::ResolvedContentRoute> for ResolvedCanonicalRoute {
    fn from(value: rustok_content::ResolvedContentRoute) -> Self {
        Self {
            target_kind: value.target_kind,
            target_id: value.target_id,
            locale: value.locale,
            matched_url: value.matched_url,
            canonical_url: value.canonical_url,
            redirect_required: value.redirect_required,
        }
    }
}
