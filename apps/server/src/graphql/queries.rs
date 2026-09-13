use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result};
use axum::http::StatusCode;
use chrono::{Duration, Utc};
use rustok_api::Permission;
use rustok_core::ModuleRegistry;
use rustok_telemetry::metrics;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QuerySelect,
};
use std::time::Instant;
use uuid::Uuid;

use crate::common::RequestContext;
use crate::context::{AuthContext, TenantContext};
use crate::error::Error as ServerError;
use crate::graphql::artifact_lifecycle::map_artifact_tenant_lifecycle_error;
use crate::graphql::types::{
    ActivityItem, ActivityUser, ArtifactDataPurgePreview, ArtifactSettingsPurgePreview,
    ArtifactTenantLifecycle, ArtifactUiActionAudit, ArtifactUiContribution, BuildJob,
    DashboardStats, InstalledModule, MarketplaceModule, MarketplaceRegistryFreshness,
    ModuleCompositionSnapshot, ModuleEffectivePolicyGql, ModuleOperationRecoveryPlan,
    ModuleRegistryItem, Tenant, TenantModule, User, UserConnection, UserEdge, UsersFilter,
};
use crate::models::_entities::users::Column as UsersColumn;
use crate::models::users;
use crate::services::artifact_ui::{
    list_authorized_artifact_ui_action_audit, list_authorized_artifact_ui_contributions,
};
use crate::services::dashboard_user_activity;
use crate::services::effective_module_policy::EffectiveModulePolicyService;
use crate::services::module_lifecycle::{ModuleLifecycleService, ModuleOperationRecoveryError};
use crate::services::platform_composition::PlatformCompositionService;
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_api::graphql::GraphQLError;
use rustok_api::graphql::{PageInfo, PaginationInput, encode_cursor};
use rustok_build::SharedBuildControl;
use rustok_modules::{
    ModuleControlPlane, SharedStaticModuleRegistryReader, StaticModuleRegistryQuery,
};

fn build_control_from_context(ctx: &Context<'_>) -> Result<SharedBuildControl> {
    ctx.data::<ServerRuntimeContext>()?
        .shared_get::<SharedBuildControl>()
        .ok_or_else(|| {
            <FieldError as GraphQLError>::internal_error("build control is not configured")
        })
}

fn calculate_percent_change(current: i64, previous: i64) -> f64 {
    if previous == 0 {
        if current == 0 { 0.0 } else { 100.0 }
    } else {
        ((current - previous) as f64 / previous as f64) * 100.0
    }
}

fn clamp_collection_limit(limit: Option<i32>) -> usize {
    limit.unwrap_or(100).clamp(1, 100) as usize
}

fn requested_collection_limit(limit: Option<i32>) -> Option<u64> {
    limit.map(|value| value.max(0) as u64)
}

fn marketplace_module_from_view(entry: rustok_api::MarketplaceModule) -> MarketplaceModule {
    entry.into()
}

fn map_module_operation_recovery_error(error: ModuleOperationRecoveryError) -> FieldError {
    match error {
        ModuleOperationRecoveryError::OperationNotFound => {
            <FieldError as GraphQLError>::bad_user_input("Module operation not found")
        }
        ModuleOperationRecoveryError::InvalidCommandIdentity => {
            <FieldError as GraphQLError>::bad_user_input(
                "Module recovery command identity is invalid",
            )
        }
        ModuleOperationRecoveryError::InvalidIdempotencyKey => {
            <FieldError as GraphQLError>::bad_user_input(
                "Module operation idempotency key is invalid",
            )
        }
        ModuleOperationRecoveryError::NotRetryable(reason) => {
            FieldError::new(format!("Module operation is not retryable: {reason}"))
                .extend_with(|_, ext| {
                    ext.set("code", "MODULE_OPERATION_NOT_RETRYABLE");
                    ext.set("retryable_issue", false);
                })
        }
        ModuleOperationRecoveryError::StateMismatch {
            requested_enabled,
            current_enabled,
        } => FieldError::new(format!(
            "Module operation state mismatch: requested enabled={requested_enabled}, current enabled={current_enabled}"
        ))
        .extend_with(|_, ext| {
            ext.set("code", "MODULE_OPERATION_STATE_MISMATCH");
            ext.set("retryable_issue", false);
        }),
        ModuleOperationRecoveryError::PostHookFailed(err) => {
            FieldError::new(format!("Module hook failed: {err}"))
                .extend_with(|_, ext| {
                    ext.set("code", "MODULE_HOOK_FAILED");
                    ext.set("retryable_issue", true);
                    ext.set("operation_issue", "post_hook_failed");
                })
        }
        ModuleOperationRecoveryError::IdempotencyConflict => FieldError::new(
            "Module operation idempotency key was reused for a different command",
        )
        .extend_with(|_, ext| {
            ext.set("code", "IDEMPOTENCY_CONFLICT");
            ext.set("retryable_issue", false);
        }),
        ModuleOperationRecoveryError::RevisionConflict { expected, current } => FieldError::new(
            format!(
                "Static module lifecycle changed since revision {expected}; current revision is {current}",
            ),
        )
        .extend_with(|_, ext| {
            ext.set("code", "REVISION_CONFLICT");
            ext.set("retryable_issue", false);
            ext.set("expected_revision", expected);
            ext.set("current_revision", current);
        }),
        ModuleOperationRecoveryError::OperationInProgress => {
            FieldError::new("A static module lifecycle operation is already active")
                .extend_with(|_, ext| {
                    ext.set("code", "MODULE_LIFECYCLE_OPERATION_IN_PROGRESS");
                    ext.set("retryable_issue", false);
                })
        }
        ModuleOperationRecoveryError::Database(err) => {
            <FieldError as GraphQLError>::internal_error(&err.to_string())
        }
        ModuleOperationRecoveryError::Policy(err) => {
            <FieldError as GraphQLError>::internal_error(&err)
        }
        ModuleOperationRecoveryError::Toggle(err) => {
            <FieldError as GraphQLError>::internal_error(&err.to_string())
        }
    }
}

pub(crate) async fn ensure_modules_read_permission(ctx: &Context<'_>) -> Result<()> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let db = ctx.data::<DatabaseConnection>()?;
    let tenant = ctx.data::<TenantContext>()?;

    let can_read_modules = RbacService::has_any_permission(
        db,
        &tenant.id,
        &auth.user_id,
        &[
            Permission::MODULES_READ,
            Permission::MODULES_LIST,
            Permission::MODULES_MANAGE,
        ],
    )
    .await
    .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

    if !can_read_modules {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: modules:read, modules:list, or modules:manage required",
        ));
    }

    Ok(())
}

async fn effective_module_policy_view(
    ctx: &Context<'_>,
) -> Result<rustok_api::ModuleEffectivePolicyView> {
    let db = ctx.data::<DatabaseConnection>()?;
    let tenant_id = ctx.data::<TenantContext>()?.id;
    let registry = ctx.data::<ModuleRegistry>()?;

    EffectiveModulePolicyService::resolve_view(db, registry, tenant_id)
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                error = %error,
                "effective module policy GraphQL read failed"
            );
            <FieldError as GraphQLError>::internal_error("effective module policy is unavailable")
        })
}

fn map_artifact_ui_contribution_error(error: ServerError) -> FieldError {
    match error {
        ServerError::NotFound => {
            <FieldError as GraphQLError>::not_found("Artifact installation is unavailable")
        }
        error => {
            tracing::error!(%error, "artifact UI contribution GraphQL read failed");
            <FieldError as GraphQLError>::internal_error(
                "Artifact UI contributions are unavailable",
            )
        }
    }
}

fn map_artifact_ui_action_audit_error(error: ServerError) -> FieldError {
    match error {
        ServerError::NotFound => {
            <FieldError as GraphQLError>::not_found("Artifact UI action is unavailable")
        }
        ServerError::Http(error) if error.status == StatusCode::FORBIDDEN => {
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied for artifact UI action",
            )
        }
        error => {
            tracing::error!(%error, "artifact UI action audit GraphQL read failed");
            <FieldError as GraphQLError>::internal_error("Artifact UI action audit is unavailable")
        }
    }
}

async fn ensure_modules_manage_permission(ctx: &Context<'_>) -> Result<()> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let db = ctx.data::<DatabaseConnection>()?;
    let tenant = ctx.data::<TenantContext>()?;
    let can_manage_modules =
        RbacService::has_permission(db, &tenant.id, &auth.user_id, &Permission::MODULES_MANAGE)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

    if !can_manage_modules {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: modules:manage required",
        ));
    }

    Ok(())
}

#[derive(Default)]
pub struct RootQuery;

#[Object]
impl RootQuery {
    async fn health(&self) -> &str {
        "GraphQL is working!"
    }

    async fn api_version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    async fn current_tenant(&self, ctx: &Context<'_>) -> Result<Tenant> {
        let tenant = ctx.data::<TenantContext>()?;
        Ok(Tenant {
            id: tenant.id,
            name: tenant.name.clone(),
            slug: tenant.slug.clone(),
        })
    }

    async fn enabled_modules(&self, ctx: &Context<'_>, limit: Option<i32>) -> Result<Vec<String>> {
        ensure_modules_read_permission(ctx).await?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);
        let modules = effective_module_policy_view(ctx)
            .await?
            .enabled_module_slugs()
            .into_iter()
            .take(limit)
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.enabled_modules",
            requested_limit,
            limit as u64,
            modules.len(),
        );

        Ok(modules)
    }

    async fn module_effective_policy(&self, ctx: &Context<'_>) -> Result<ModuleEffectivePolicyGql> {
        ensure_modules_read_permission(ctx).await?;
        effective_module_policy_view(ctx).await.map(Into::into)
    }

    async fn module_registry(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<ModuleRegistryItem>> {
        ensure_modules_read_permission(ctx).await?;

        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let reader = ctx.data::<SharedStaticModuleRegistryReader>()?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);
        let modules = reader
            .0
            .list(StaticModuleRegistryQuery {
                tenant_id: tenant.id,
                preferred_locale: request_context.locale.clone(),
                fallback_locale: tenant.default_locale.clone(),
                limit: limit as u32,
            })
            .await
            .map_err(|_| {
                <FieldError as GraphQLError>::internal_error(
                    "static module registry projection is unavailable",
                )
            })?
            .into_iter()
            .map(ModuleRegistryItem::from)
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.module_registry",
            requested_limit,
            limit as u64,
            modules.len(),
        );

        Ok(modules)
    }

    async fn tenant_modules(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<TenantModule>> {
        ensure_modules_read_permission(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);
        let owner_limit = u32::try_from(limit)
            .map_err(|_| <FieldError as GraphQLError>::internal_error("invalid module limit"))?;
        let modules = EffectiveModulePolicyService::static_tenant_module_views(
            db,
            registry,
            tenant.id,
            owner_limit,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        let modules = modules
            .into_iter()
            .map(TenantModule::from)
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.tenant_modules",
            requested_limit,
            limit as u64,
            modules.len(),
        );

        Ok(modules)
    }

    /// Returns only the owner-issued tenant intent facts required to submit a
    /// revision-CAS artifact enablement command. The artifact descriptor,
    /// admission evidence, and owner tables remain private.
    async fn artifact_tenant_lifecycle(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
    ) -> Result<ArtifactTenantLifecycle> {
        ensure_modules_read_permission(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let lifecycle = ModuleControlPlane::new(db.clone())
            .installation()
            .artifact_tenant_lifecycle_snapshot(installation_id, tenant.id)
            .await
            .map_err(map_artifact_tenant_lifecycle_error)?;

        let revision = i64::try_from(lifecycle.revision).map_err(|_| {
            <FieldError as GraphQLError>::internal_error(
                "Artifact tenant lifecycle revision is outside the GraphQL range",
            )
        })?;
        let expected_revision = i64::try_from(lifecycle.expected_revision).map_err(|_| {
            <FieldError as GraphQLError>::internal_error(
                "Artifact tenant lifecycle revision is outside the GraphQL range",
            )
        })?;
        Ok(ArtifactTenantLifecycle {
            installation_id: lifecycle.installation_id,
            enabled: lifecycle.enabled,
            revision,
            expected_revision,
        })
    }

    /// Returns only host-safe, exact-locale contributions that the current
    /// principal may render for one active artifact installation. The request
    /// context owns locale resolution; callers cannot provide a fallback.
    async fn artifact_ui_contributions(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
    ) -> Result<Vec<ArtifactUiContribution>> {
        ensure_modules_read_permission(ctx).await?;
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let request = ctx.data::<RequestContext>()?;

        list_authorized_artifact_ui_contributions(
            runtime_ctx,
            tenant.id,
            auth.user_id,
            installation_id,
            &request.locale,
        )
        .await
        .map_err(map_artifact_ui_contribution_error)
        .map(|views| {
            views
                .into_iter()
                .map(ArtifactUiContribution::from)
                .collect()
        })
    }

    /// Returns redacted execution evidence for the admitted Action or Form
    /// contribution. The caller cannot select a raw binding ID, and the same
    /// dynamic RBAC permission that authorizes execution guards this read.
    async fn artifact_ui_action_audit(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
        contribution_id: String,
    ) -> Result<Vec<ArtifactUiActionAudit>> {
        ensure_modules_read_permission(ctx).await?;
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        list_authorized_artifact_ui_action_audit(
            runtime_ctx,
            tenant.id,
            auth.user_id,
            installation_id,
            &contribution_id,
        )
        .await
        .map_err(map_artifact_ui_action_audit_error)
        .map(|entries| {
            entries
                .into_iter()
                .map(ArtifactUiActionAudit::from)
                .collect()
        })
    }

    /// Previews a dynamic artifact settings purge. Purge is permitted only if
    /// the installation is uninstalled/retired and a protected recovery point exists.
    async fn preview_tenant_artifact_settings_purge(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
    ) -> Result<ArtifactSettingsPurgePreview> {
        ensure_modules_read_permission(ctx).await?;
        let tenant = ctx.data::<TenantContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;
        let preview = ModuleControlPlane::new(db.clone())
            .artifact_settings_purge_preview()
            .preview(tenant.id, installation_id)
            .await
            .map_err(crate::graphql::mutations::map_artifact_settings_recovery_error)?;
        let recovery_ready = ctx
            .data::<crate::services::artifact_purge_recovery_host::ArtifactSettingsRecoveryRuntime>(
            )
            .is_ok();
        Ok(ArtifactSettingsPurgePreview {
            installation_id: preview.installation_id,
            data_owner_id: preview.data_owner_id,
            settings_instance_id: preview.settings_instance_id,
            settings_revision: i64::try_from(preview.settings_revision).map_err(|_| {
                <FieldError as GraphQLError>::internal_error(
                    "Artifact settings revision exceeds the GraphQL integer range",
                )
            })?,
            has_recovery_point: preview.has_recovery_point,
            recovery_point_id: preview.recovery_point_id,
            can_purge: preview.can_purge && recovery_ready,
            reason: if preview.can_purge && !recovery_ready {
                "Protected settings recovery is unavailable".to_string()
            } else {
                preview.reason
            },
        })
    }

    /// Previews lifecycle eligibility for an exact retired installation.
    /// Active tenant-visible installations with the same slug block this preview.
    /// Traffic, job, and write-drain fences are not established by this read.
    async fn preview_tenant_artifact_data_purge(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
    ) -> Result<ArtifactDataPurgePreview> {
        ensure_modules_read_permission(ctx).await?;
        let tenant = ctx.data::<TenantContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;
        let preview = ModuleControlPlane::new(db.clone())
            .artifact_data_purge_preview()
            .preview(tenant.id, installation_id)
            .await
            .map_err(crate::graphql::mutations::map_artifact_data_purge_error)?;
        Ok(ArtifactDataPurgePreview {
            installation_id: preview.installation_id,
            namespace_revision: i64::try_from(preview.namespace_revision).map_err(|_| {
                <FieldError as GraphQLError>::internal_error(
                    "Artifact data namespace revision exceeds the GraphQL integer range",
                )
            })?,
            records_to_purge: i64::try_from(preview.records_to_purge).map_err(|_| {
                <FieldError as GraphQLError>::internal_error(
                    "Artifact data record count exceeds the GraphQL integer range",
                )
            })?,
            can_purge: preview.can_purge,
            reason: preview.reason,
        })
    }

    async fn installed_modules(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<InstalledModule>> {
        ensure_modules_read_permission(ctx).await?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);

        let db = ctx.data::<DatabaseConnection>()?;
        let modules = PlatformCompositionService::installed_modules(db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            .into_iter()
            .take(limit)
            .map(InstalledModule::from)
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.installed_modules",
            requested_limit,
            limit as u64,
            modules.len(),
        );

        Ok(modules)
    }

    /// Returns the immutable composition revision needed by every static
    /// module-set mutation. The manifest itself stays behind the owner facade.
    async fn module_composition_snapshot(
        &self,
        ctx: &Context<'_>,
    ) -> Result<ModuleCompositionSnapshot> {
        ensure_modules_read_permission(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let snapshot = PlatformCompositionService::active_snapshot_view(db)
            .await
            .map_err(|error| <FieldError as GraphQLError>::internal_error(&error.to_string()))?;
        Ok(ModuleCompositionSnapshot::from(snapshot))
    }

    async fn marketplace(
        &self,
        ctx: &Context<'_>,
        search: Option<String>,
        category: Option<String>,
        tag: Option<String>,
        source: Option<String>,
        trust_level: Option<String>,
        only_compatible: Option<bool>,
        installed_only: Option<bool>,
        limit: Option<i32>,
    ) -> Result<Vec<MarketplaceModule>> {
        ensure_modules_read_permission(ctx).await?;

        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);
        let modules = ctx
            .data::<rustok_modules::SharedModuleMarketplaceCatalog>()?
            .0
            .list(rustok_modules::ModuleMarketplaceQuery {
                search,
                category,
                tag,
                source,
                trust_level,
                only_compatible: only_compatible.unwrap_or(true),
                installed_only: installed_only.unwrap_or(false),
                preferred_locale: Some(request_context.locale.clone()),
                fallback_locale: Some(tenant.default_locale.clone()),
                limit: limit as u32,
            })
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            .into_iter()
            .map(marketplace_module_from_view)
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.marketplace",
            requested_limit,
            limit as u64,
            modules.len(),
        );

        Ok(modules)
    }

    async fn marketplace_module(
        &self,
        ctx: &Context<'_>,
        slug: String,
    ) -> Result<Option<MarketplaceModule>> {
        ensure_modules_read_permission(ctx).await?;

        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        ctx.data::<rustok_modules::SharedModuleMarketplaceCatalog>()?
            .0
            .get(
                &slug,
                Some(request_context.locale.clone()),
                Some(tenant.default_locale.clone()),
            )
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))
            .map(|entry| entry.map(marketplace_module_from_view))
    }

    async fn marketplace_registry_freshness(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<MarketplaceRegistryFreshness>> {
        ensure_modules_manage_permission(ctx).await?;
        Ok(ctx
            .data::<rustok_modules::SharedModuleMarketplaceCatalog>()?
            .0
            .registry_freshness()
            .into_iter()
            .map(MarketplaceRegistryFreshness::from)
            .collect())
    }

    async fn module_operation_recovery_plan(
        &self,
        ctx: &Context<'_>,
        operation_id: Uuid,
    ) -> Result<Option<ModuleOperationRecoveryPlan>> {
        ensure_modules_read_permission(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;
        let plan = match ModuleLifecycleService::module_operation_recovery_plan(
            db,
            registry,
            tenant.id,
            operation_id,
        )
        .await
        {
            Ok(plan) => plan,
            Err(ModuleOperationRecoveryError::OperationNotFound) => return Ok(None),
            Err(err) => return Err(map_module_operation_recovery_error(err)),
        };

        Ok(Some(ModuleOperationRecoveryPlan::from(
            rustok_api::ModuleOperationRecoveryPlanView::from(plan),
        )))
    }

    async fn failed_module_operation_recovery_plans(
        &self,
        ctx: &Context<'_>,
        module_slug: Option<String>,
        limit: Option<i32>,
    ) -> Result<Vec<ModuleOperationRecoveryPlan>> {
        ensure_modules_read_permission(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);
        let plans = ModuleLifecycleService::failed_module_operation_recovery_plans(
            db,
            registry,
            tenant.id,
            module_slug.as_deref(),
        )
        .await
        .map_err(map_module_operation_recovery_error)?
        .into_iter()
        .take(limit)
        .map(|plan| {
            ModuleOperationRecoveryPlan::from(rustok_api::ModuleOperationRecoveryPlanView::from(
                plan,
            ))
        })
        .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.failed_module_operation_recovery_plans",
            requested_limit,
            limit as u64,
            plans.len(),
        );

        Ok(plans)
    }

    async fn active_build(&self, ctx: &Context<'_>) -> Result<Option<BuildJob>> {
        ensure_modules_read_permission(ctx).await?;

        let build = build_control_from_context(ctx)?
            .0
            .active_build()
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(build.as_ref().map(BuildJob::from_snapshot))
    }

    async fn build_history(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 20)] limit: i32,
        #[graphql(default = 0)] offset: i32,
    ) -> Result<Vec<BuildJob>> {
        ensure_modules_read_permission(ctx).await?;

        let requested_limit = limit.max(0) as u64;
        let limit = limit.clamp(1, 100) as u64;
        let offset = offset.max(0) as u64;

        let builds = build_control_from_context(ctx)?
            .0
            .list_builds_page(limit, offset)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let builds = builds
            .iter()
            .map(BuildJob::from_snapshot)
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.build_history",
            Some(requested_limit),
            limit,
            builds.len(),
        );

        Ok(builds)
    }

    async fn me(&self, ctx: &Context<'_>) -> Result<Option<User>> {
        let auth = match ctx.data_opt::<AuthContext>() {
            Some(auth) => auth,
            None => return Ok(None),
        };
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let user = users::Entity::find()
            .filter(UsersColumn::Id.eq(auth.user_id))
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .one(db)
            .await
            .map_err(|err| err.to_string())?;

        Ok(user.as_ref().map(User::from))
    }

    async fn user(&self, ctx: &Context<'_>, id: uuid::Uuid) -> Result<Option<User>> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;

        let can_read_users = RbacService::has_permission(
            db,
            &tenant.id,
            &auth.user_id,
            &rustok_api::Permission::USERS_READ,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if !can_read_users {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: users:read required",
            ));
        }

        let user = users::Entity::find_by_id(id)
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .one(db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(user.as_ref().map(User::from))
    }

    async fn users(
        &self,
        ctx: &Context<'_>,
        #[graphql(default)] pagination: PaginationInput,
        filter: Option<UsersFilter>,
        search: Option<String>,
    ) -> Result<UserConnection> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;

        let can_list_users = RbacService::has_permission(
            db,
            &tenant.id,
            &auth.user_id,
            &rustok_api::Permission::USERS_LIST,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if !can_list_users {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: users:list required",
            ));
        }

        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let mut query = users::Entity::find().filter(UsersColumn::TenantId.eq(tenant.id));

        if let Some(filter) = filter {
            if let Some(role) = filter.role {
                let role: rustok_core::UserRole = role.into();
                let user_ids = RbacService::get_user_ids_for_role(db, &tenant.id, role)
                    .await
                    .map_err(|err| {
                        <FieldError as GraphQLError>::internal_error(&err.to_string())
                    })?;
                query = query.filter(UsersColumn::Id.is_in(user_ids));
            }

            if let Some(status) = filter.status {
                let status: rustok_core::UserStatus = status.into();
                query = query.filter(UsersColumn::Status.eq(status.to_string()));
            }
        }

        if let Some(search) = search {
            let search = search.trim();
            if !search.is_empty() {
                let condition = Condition::any()
                    .add(UsersColumn::Email.contains(search))
                    .add(UsersColumn::Name.contains(search));
                query = query.filter(condition);
            }
        }
        let count_started_at = Instant::now();
        let total = query
            .clone()
            .count(db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;
        metrics::record_read_path_query(
            "graphql",
            "root.users",
            "count",
            count_started_at.elapsed().as_secs_f64(),
            total.max(0) as u64,
        );

        let page_started_at = Instant::now();
        let users = query
            .offset(offset as u64)
            .limit(limit as u64)
            .all(db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        metrics::record_read_path_query(
            "graphql",
            "root.users",
            "users_page",
            page_started_at.elapsed().as_secs_f64(),
            users.len() as u64,
        );

        let edges = users
            .iter()
            .enumerate()
            .map(|(index, user)| UserEdge {
                node: User::from(user),
                cursor: encode_cursor(offset + index as i64),
            })
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.users",
            Some(requested_limit),
            limit as u64,
            edges.len(),
        );

        Ok(UserConnection {
            edges,
            page_info: PageInfo::new(total, offset, limit),
        })
    }

    async fn dashboard_stats(&self, ctx: &Context<'_>) -> Result<DashboardStats> {
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let now = Utc::now();
        let current_period_start = now - Duration::days(30);
        let previous_period_start = current_period_start - Duration::days(30);

        let user_stats_started_at = Instant::now();
        let user_stats = dashboard_user_activity::load_user_stats_snapshot(
            db,
            tenant.id,
            current_period_start,
            previous_period_start,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        metrics::record_read_path_query(
            "graphql",
            "root.dashboard_stats",
            "users_snapshot",
            user_stats_started_at.elapsed().as_secs_f64(),
            user_stats.total_count.max(0) as u64,
        );

        #[cfg(feature = "mod-content")]
        let (total_posts, current_posts, previous_posts) = {
            let post_stats_started_at = Instant::now();
            let post_stats = rustok_content::load_post_stats_snapshot(
                db,
                tenant.id,
                current_period_start,
                previous_period_start,
            )
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
            metrics::record_read_path_query(
                "graphql",
                "root.dashboard_stats",
                "posts_snapshot",
                post_stats_started_at.elapsed().as_secs_f64(),
                post_stats.total_count.max(0) as u64,
            );
            (
                post_stats.total_count,
                post_stats.current_count,
                post_stats.previous_count,
            )
        };
        #[cfg(not(feature = "mod-content"))]
        let (total_posts, current_posts, previous_posts) = (0, 0, 0);

        #[cfg(feature = "mod-order")]
        let (
            total_orders,
            total_revenue,
            current_orders,
            previous_orders,
            current_revenue,
            previous_revenue,
        ) = {
            let order_stats_started_at = Instant::now();
            let order_stats = rustok_order::load_order_stats_snapshot(
                db,
                tenant.id,
                current_period_start,
                previous_period_start,
            )
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
            metrics::record_read_path_query(
                "graphql",
                "root.dashboard_stats",
                "orders_snapshot",
                order_stats_started_at.elapsed().as_secs_f64(),
                order_stats.total_orders.max(0) as u64,
            );
            (
                order_stats.total_orders,
                order_stats.total_revenue,
                order_stats.current_orders,
                order_stats.previous_orders,
                order_stats.current_revenue,
                order_stats.previous_revenue,
            )
        };
        #[cfg(not(feature = "mod-order"))]
        let (
            total_orders,
            total_revenue,
            current_orders,
            previous_orders,
            current_revenue,
            previous_revenue,
        ) = (0, 0, 0, 0, 0, 0);

        Ok(DashboardStats {
            total_users: user_stats.total_count,
            total_posts,
            total_orders,
            total_revenue,
            users_change: calculate_percent_change(
                user_stats.current_count,
                user_stats.previous_count,
            ),
            posts_change: calculate_percent_change(current_posts, previous_posts),
            orders_change: calculate_percent_change(current_orders, previous_orders),
            revenue_change: calculate_percent_change(current_revenue, previous_revenue),
        })
    }

    async fn recent_activity(
        &self,
        ctx: &Context<'_>,
        #[graphql(default)] limit: i64,
    ) -> Result<Vec<ActivityItem>> {
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let requested_limit = limit.max(0) as u64;
        let limit = limit.clamp(1, 50);

        let recent_users_started_at = Instant::now();
        let recent_users =
            dashboard_user_activity::load_recent_user_activity(db, tenant.id, limit as u64)
                .await
                .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        metrics::record_read_path_query(
            "graphql",
            "root.recent_activity",
            "recent_users",
            recent_users_started_at.elapsed().as_secs_f64(),
            recent_users.len() as u64,
        );

        let activities = recent_users
            .into_iter()
            .map(|user| ActivityItem {
                id: user.id.to_string(),
                r#type: "user.created".to_string(),
                description: format!("New user {} joined", user.email),
                timestamp: user.created_at.to_rfc3339(),
                user: Some(ActivityUser {
                    id: user.id.to_string(),
                    name: user.name,
                }),
            })
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.recent_activity",
            Some(requested_limit),
            limit as u64,
            activities.len(),
        );

        Ok(activities)
    }

    /// Query the current status of a governed module release transition.
    async fn module_transition_checkpoint(
        &self,
        ctx: &Context<'_>,
        operation_id: Uuid,
    ) -> Result<Option<crate::graphql::transition_lifecycle::ModuleTransitionCheckpointGql>> {
        ensure_modules_read_permission(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let checkpoint = ModuleControlPlane::new(db.clone())
            .transitions()
            .checkpoint(operation_id, Some(tenant.id))
            .await
            .map_err(crate::graphql::transition_lifecycle::map_transition_service_error)?;
        Ok(checkpoint
            .map(rustok_api::ModuleTransitionCheckpointView::from)
            .map(Into::into))
    }

    /// Query all active (non-terminal) governed module release transitions.
    async fn active_module_transitions(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<crate::graphql::transition_lifecycle::ModuleTransitionCheckpointGql>> {
        ensure_modules_read_permission(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let checkpoints = ModuleControlPlane::new(db.clone())
            .transitions()
            .active_checkpoints(Some(tenant.id))
            .await
            .map_err(crate::graphql::transition_lifecycle::map_transition_service_error)?;
        Ok(checkpoints
            .into_iter()
            .map(rustok_api::ModuleTransitionCheckpointView::from)
            .map(Into::into)
            .collect())
    }

    /// Query all active artifact retention holds protecting CAS blobs, slots, and recovery points.
    async fn module_retention_holds(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<crate::graphql::transition_lifecycle::RetentionHoldGql>> {
        ensure_modules_read_permission(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let holds = ModuleControlPlane::new(db.clone())
            .transitions()
            .retention_holds(Some(tenant.id))
            .await
            .map_err(crate::graphql::transition_lifecycle::map_transition_service_error)?;
        Ok(holds
            .into_iter()
            .map(rustok_api::ModuleRetentionHoldView::from)
            .map(Into::into)
            .collect())
    }
}
