use std::collections::{HashMap, HashSet};

use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result};
use axum::http::StatusCode;
use chrono::{Duration, Utc};
use rustok_api::Permission;
use rustok_core::ModuleRegistry;
use rustok_telemetry::metrics;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QuerySelect,
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
    DashboardStats, InstalledModule, MarketplaceModule, MarketplaceModuleVersion,
    MarketplaceRegistryFreshness, ModuleCompositionSnapshot, ModuleOperationRecoveryPlan,
    ModuleRegistryItem, ModuleSettingField, Tenant, TenantModule, User, UserConnection, UserEdge,
    UsersFilter,
};
use crate::models::_entities::users::Column as UsersColumn;
use crate::models::users;
use crate::services::artifact_ui::{
    list_authorized_artifact_ui_action_audit, list_authorized_artifact_ui_contributions,
};
use crate::services::dashboard_user_activity;
use crate::services::effective_module_policy::EffectiveModulePolicyService;
use crate::services::marketplace_catalog::MarketplaceCatalogQuery;
use crate::services::marketplace_catalog_adapter::project_marketplace_catalog_entries;
use crate::services::module_lifecycle::{ModuleLifecycleService, ModuleOperationRecoveryError};
use crate::services::platform_composition::PlatformCompositionService;
use crate::services::rbac_service::RbacService;
use crate::services::registry_principal::RegistryPrincipalRef;
use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_api::graphql::GraphQLError;
use rustok_api::graphql::{PageInfo, PaginationInput, encode_cursor};
use rustok_build::SharedBuildControl;
use rustok_modules::ModuleControlPlane;

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

fn marketplace_module_from_owner_entry(
    entry: rustok_modules::ModuleMarketplaceEntry,
) -> MarketplaceModule {
    MarketplaceModule {
        slug: entry.slug,
        name: entry.name,
        latest_version: entry.latest_version,
        description: entry.description,
        source: entry.source,
        kind: entry.kind,
        category: entry.category,
        tags: entry.tags,
        icon_url: entry.icon_url,
        banner_url: entry.banner_url,
        screenshots: entry.screenshots,
        crate_name: entry.crate_name,
        dependencies: entry.dependencies,
        ownership: entry.ownership,
        trust_level: entry.trust_level,
        rustok_min_version: entry.rustok_min_version,
        rustok_max_version: entry.rustok_max_version,
        publisher: entry.publisher,
        checksum_sha256: entry.checksum_sha256,
        signature_present: entry.signature_present,
        versions: entry
            .versions
            .into_iter()
            .map(|version| MarketplaceModuleVersion {
                version: version.version,
                changelog: version.changelog,
                yanked: version.yanked,
                published_at: version.published_at,
                checksum_sha256: version.checksum_sha256,
                signature_present: version.signature_present,
            })
            .collect(),
        has_admin_ui: entry.has_admin_ui,
        has_storefront_ui: entry.has_storefront_ui,
        ui_classification: entry.ui_classification,
        registry_lifecycle: entry
            .registry_lifecycle
            .map(registry_module_lifecycle_from_snapshot),
        compatible: entry.compatible,
        recommended_admin_surfaces: entry.recommended_admin_surfaces,
        showcase_admin_surfaces: entry.showcase_admin_surfaces,
        settings_schema: owner_settings_schema_fields(entry.settings_schema),
        installed: entry.installed,
        installed_version: entry.installed_version,
        update_available: entry.update_available,
    }
}

fn owner_settings_schema_fields(
    schema: std::collections::BTreeMap<String, rustok_modules::ModuleSettingSpec>,
) -> Vec<ModuleSettingField> {
    schema
        .into_iter()
        .map(|(key, spec)| {
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
                .or(spec.item_type.clone());
            let mut shape = serde_json::Map::new();
            if !spec.properties.is_empty() {
                shape.insert(
                    "properties".to_string(),
                    serde_json::to_value(&spec.properties)
                        .expect("owner settings schema must serialize"),
                );
            }
            if let Some(items) = spec.items.as_deref() {
                shape.insert(
                    "items".to_string(),
                    serde_json::to_value(items).expect("owner settings schema must serialize"),
                );
            }
            ModuleSettingField {
                key,
                value_type: spec.value_type,
                required: spec.required,
                default_value: spec.default,
                description: spec.description,
                min: spec.min,
                max: spec.max,
                options: spec.options,
                object_keys,
                item_type,
                shape: (!shape.is_empty()).then_some(serde_json::Value::Object(shape)),
            }
        })
        .collect()
}

fn registry_module_lifecycle_from_snapshot(
    snapshot: rustok_modules::ModuleGovernanceLifecycleSnapshot,
) -> crate::graphql::types::RegistryModuleLifecycle {
    crate::graphql::types::RegistryModuleLifecycle {
        moderation_policy: crate::graphql::types::RegistryModerationPolicyLifecycle {
            mode: snapshot.moderation_policy.mode,
            live_publish_supported: snapshot.moderation_policy.live_publish_supported,
            live_governance_supported: snapshot.moderation_policy.live_governance_supported,
            manual_review_required: snapshot.moderation_policy.manual_review_required,
            restriction_reason_code: snapshot.moderation_policy.restriction_reason_code,
            restriction_reason: snapshot.moderation_policy.restriction_reason,
        },
        owner_binding: snapshot.owner_binding.map(|owner| {
            crate::graphql::types::RegistryOwnerLifecycle {
                owner: RegistryPrincipalRef::from_json_value(&owner.owner_principal).into(),
                bound_by: RegistryPrincipalRef::from_json_value(&owner.bound_by_principal).into(),
                bound_at: owner.bound_at,
                updated_at: owner.updated_at,
            }
        }),
        latest_request: snapshot.latest_request.map(|request| {
            crate::graphql::types::RegistryPublishRequestLifecycle {
                id: request.id,
                revision: request.revision,
                status: request.status,
                requested_by: RegistryPrincipalRef::from_json_value(
                    &request.requested_by_principal,
                )
                .into(),
                publisher: request
                    .publisher_principal
                    .as_ref()
                    .map(RegistryPrincipalRef::from_json_value)
                    .map(Into::into),
                approved_by: request
                    .approved_by_principal
                    .as_ref()
                    .map(RegistryPrincipalRef::from_json_value)
                    .map(Into::into),
                rejected_by: request
                    .rejected_by_principal
                    .as_ref()
                    .map(RegistryPrincipalRef::from_json_value)
                    .map(Into::into),
                rejection_reason: request.rejection_reason,
                changes_requested_by: request
                    .changes_requested_by_principal
                    .as_ref()
                    .map(RegistryPrincipalRef::from_json_value)
                    .map(Into::into),
                changes_requested_reason: request.changes_requested_reason,
                changes_requested_reason_code: request.changes_requested_reason_code,
                changes_requested_at: request.changes_requested_at,
                held_by: request
                    .held_by_principal
                    .as_ref()
                    .map(RegistryPrincipalRef::from_json_value)
                    .map(Into::into),
                held_reason: request.held_reason,
                held_reason_code: request.held_reason_code,
                held_at: request.held_at,
                held_from_status: request.held_from_status,
                warnings: request.warnings,
                errors: request.errors,
                created_at: request.created_at,
                updated_at: request.updated_at,
                published_at: request.published_at,
            }
        }),
        latest_release: snapshot.latest_release.map(|release| {
            crate::graphql::types::RegistryReleaseLifecycle {
                version: release.version,
                status: release.status,
                publisher: RegistryPrincipalRef::from_json_value(&release.publisher_principal)
                    .into(),
                checksum_sha256: release.checksum_sha256,
                published_at: release.published_at,
                yanked_reason: release.yanked_reason,
                yanked_by: release
                    .yanked_by_principal
                    .as_ref()
                    .map(RegistryPrincipalRef::from_json_value)
                    .map(Into::into),
                yanked_at: release.yanked_at,
            }
        }),
        recent_events: snapshot
            .recent_events
            .into_iter()
            .map(
                |event| crate::graphql::types::RegistryGovernanceEventLifecycle {
                    id: event.id,
                    event_type: event.event_type,
                    actor: RegistryPrincipalRef::from_json_value(&event.actor_principal).into(),
                    publisher: event
                        .publisher_principal
                        .as_ref()
                        .map(RegistryPrincipalRef::from_json_value)
                        .map(Into::into),
                    payload: crate::graphql::types::RegistryGovernanceEventPayloadLifecycle {
                        reason: event.payload.reason,
                        reason_code: event.payload.reason_code,
                        detail: event.payload.detail,
                        version: event.payload.version,
                        stage_key: event.payload.stage_key,
                        attempt_number: event.payload.attempt_number,
                        owner_transition: event.payload.owner_transition.map(|transition| {
                            crate::graphql::types::RegistryOwnerTransitionLifecycle {
                                previous_owner: transition
                                    .previous_owner_principal
                                    .as_ref()
                                    .map(RegistryPrincipalRef::from_json_value)
                                    .map(Into::into),
                                new_owner: transition
                                    .new_owner_principal
                                    .as_ref()
                                    .map(RegistryPrincipalRef::from_json_value)
                                    .map(Into::into),
                                bound_by: transition
                                    .bound_by_principal
                                    .as_ref()
                                    .map(RegistryPrincipalRef::from_json_value)
                                    .map(Into::into),
                            }
                        }),
                        warnings: event.payload.warnings,
                        errors: event.payload.errors,
                        mode: event.payload.mode,
                    },
                    created_at: event.created_at,
                },
            )
            .collect(),
        follow_up_gates: snapshot
            .follow_up_gates
            .into_iter()
            .map(
                |gate| crate::graphql::types::RegistryFollowUpGateLifecycle {
                    key: gate.key,
                    status: gate.status,
                    detail: gate.detail,
                    updated_at: gate.updated_at,
                },
            )
            .collect(),
        validation_stages: snapshot
            .validation_stages
            .into_iter()
            .map(
                |stage| crate::graphql::types::RegistryValidationStageLifecycle {
                    key: stage.key,
                    status: stage.status,
                    detail: stage.detail,
                    attempt_number: stage.attempt_number,
                    updated_at: stage.updated_at,
                    started_at: stage.started_at,
                    finished_at: stage.finished_at,
                    execution_mode: stage.execution_mode,
                    runnable: stage.runnable,
                    requires_manual_confirmation: stage.requires_manual_confirmation,
                    allowed_terminal_reason_codes: stage.allowed_terminal_reason_codes,
                    suggested_pass_reason_code: stage.suggested_pass_reason_code,
                    suggested_failure_reason_code: stage.suggested_failure_reason_code,
                    suggested_blocked_reason_code: stage.suggested_blocked_reason_code,
                },
            )
            .collect(),
        governance_actions: snapshot
            .governance_actions
            .into_iter()
            .map(
                |action| crate::graphql::types::RegistryGovernanceActionLifecycle {
                    key: action.key,
                    reason_required: action.reason_required,
                    reason_code_required: action.reason_code_required,
                    reason_codes: action.reason_codes,
                    destructive: action.destructive,
                },
            )
            .collect(),
    }
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

fn sql_uuid(val: Uuid, backend: sea_orm::DbBackend) -> sea_orm::Value {
    match backend {
        sea_orm::DbBackend::Postgres => sea_orm::Value::Uuid(Some(val)),
        _ => val.to_string().into(),
    }
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

#[derive(Clone, Copy)]
struct MarketplaceProjectionLocales<'a> {
    preferred: Option<&'a str>,
    fallback: Option<&'a str>,
}

async fn load_marketplace_catalog(
    runtime_ctx: &ServerRuntimeContext,
    manifest: &crate::modules::ModulesManifest,
    registry: &ModuleRegistry,
    query: &MarketplaceCatalogQuery,
    locales: MarketplaceProjectionLocales<'_>,
) -> Result<Vec<rustok_modules::ModuleMarketplaceEntry>> {
    project_marketplace_catalog_entries(
        runtime_ctx,
        manifest,
        registry,
        query,
        locales.preferred,
        locales.fallback,
    )
    .await
    .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))
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
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);
        let modules = EffectiveModulePolicyService::list_enabled(db, registry, tenant.id)
            .await
            .map_err(|err| err.to_string())?
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

    async fn module_registry(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<ModuleRegistryItem>> {
        ensure_modules_read_permission(ctx).await?;

        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let db = runtime_ctx.db();
        let tenant = ctx.data::<TenantContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);
        let manifest = PlatformCompositionService::active_manifest(db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        let query = MarketplaceCatalogQuery::default();
        let catalog_by_slug: HashMap<String, rustok_modules::ModuleMarketplaceEntry> =
            load_marketplace_catalog(
                runtime_ctx,
                &manifest,
                registry,
                &query,
                MarketplaceProjectionLocales {
                    preferred: Some(request_context.locale.as_str()),
                    fallback: Some(tenant.default_locale.as_str()),
                },
            )
            .await?
            .into_iter()
            .map(|module| (module.slug.clone(), module))
            .collect();
        let enabled_modules = EffectiveModulePolicyService::list_enabled(db, registry, tenant.id)
            .await
            .map_err(|err| err.to_string())?;
        let enabled_set: HashSet<String> = enabled_modules.into_iter().collect();
        let registry_modules = registry.list().into_iter().take(limit).collect::<Vec<_>>();
        let lifecycle_revisions = EffectiveModulePolicyService::static_lifecycle_snapshots(
            db,
            registry,
            tenant.id,
            registry_modules
                .iter()
                .map(|module| module.slug().to_string()),
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let modules = registry_modules
            .into_iter()
            .map(|module| {
                let catalog_entry = catalog_by_slug.get(module.slug());
                let lifecycle_revision = lifecycle_revisions
                    .get(module.slug())
                    .map(|snapshot| snapshot.revision)
                    .ok_or_else(|| {
                        <FieldError as GraphQLError>::internal_error(
                            "missing static module lifecycle revision",
                        )
                    })?;
                let lifecycle_revision = i64::try_from(lifecycle_revision).map_err(|_| {
                    <FieldError as GraphQLError>::internal_error(
                        "static module lifecycle revision is outside the GraphQL range",
                    )
                })?;

                Ok(ModuleRegistryItem {
                    module_slug: module.slug().to_string(),
                    name: module.name().to_string(),
                    description: module.description().to_string(),
                    version: module.version().to_string(),
                    kind: if registry.is_core(module.slug()) {
                        "core".to_string()
                    } else {
                        "optional".to_string()
                    },
                    enabled: registry.is_core(module.slug()) || enabled_set.contains(module.slug()),
                    lifecycle_revision,
                    dependencies: module
                        .dependencies()
                        .iter()
                        .map(|dependency| dependency.to_string())
                        .collect(),
                    ownership: catalog_entry
                        .map(|entry| entry.ownership.clone())
                        .unwrap_or_else(|| "third_party".to_string()),
                    trust_level: catalog_entry
                        .map(|entry| entry.trust_level.clone())
                        .unwrap_or_else(|| "unverified".to_string()),
                    has_admin_ui: catalog_entry.is_some_and(|entry| entry.has_admin_ui),
                    has_storefront_ui: catalog_entry.is_some_and(|entry| entry.has_storefront_ui),
                    ui_classification: catalog_entry
                        .map(|entry| entry.ui_classification.clone())
                        .unwrap_or_else(|| "no_ui".to_string()),
                    recommended_admin_surfaces: catalog_entry
                        .map(|entry| entry.recommended_admin_surfaces.clone())
                        .unwrap_or_default(),
                    showcase_admin_surfaces: catalog_entry
                        .map(|entry| entry.showcase_admin_surfaces.clone())
                        .unwrap_or_default(),
                    settings_schema: catalog_entry
                        .map(|entry| owner_settings_schema_fields(entry.settings_schema.clone()))
                        .unwrap_or_default(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

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
        let modules = EffectiveModulePolicyService::tenant_override_snapshots(
            db,
            registry,
            tenant.id,
            owner_limit,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        let lifecycle_revisions = EffectiveModulePolicyService::static_lifecycle_snapshots(
            db,
            registry,
            tenant.id,
            modules.iter().map(|module| module.module_slug.clone()),
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let modules = modules
            .into_iter()
            .map(|module| {
                let revision = lifecycle_revisions
                    .get(&module.module_slug)
                    .map(|snapshot| snapshot.revision)
                    .ok_or_else(|| {
                        <FieldError as GraphQLError>::internal_error(
                            "missing static module lifecycle revision",
                        )
                    })?;
                Ok(TenantModule {
                    module_slug: module.module_slug,
                    enabled: module.enabled,
                    settings: module.settings.to_string(),
                    revision: i64::try_from(revision).map_err(|_| {
                        <FieldError as GraphQLError>::internal_error(
                            "static module lifecycle revision is outside the GraphQL range",
                        )
                    })?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

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
        let backend = db.get_database_backend();

        let placeholder = if backend == sea_orm::DbBackend::Postgres {
            "$1"
        } else {
            "?1"
        };
        let tenant_placeholder = if backend == sea_orm::DbBackend::Postgres {
            "$2"
        } else {
            "?2"
        };
        let query = format!(
            "SELECT i.data_owner_id, i.settings_instance_id, a.status, \
                    COALESCE(s.revision, 0) AS settings_revision \
             FROM module_artifact_installations i \
             LEFT JOIN module_artifact_admissions a ON a.installation_id = i.installation_id \
             LEFT JOIN module_artifact_settings_instances s ON s.settings_instance_id = i.settings_instance_id \
             WHERE i.installation_id = {placeholder} AND (i.tenant_id = {tenant_placeholder} OR i.scope_kind = 'platform')"
        );
        let row = db
            .query_one_raw(sea_orm::Statement::from_sql_and_values(
                backend,
                query,
                vec![
                    sql_uuid(installation_id, backend),
                    sql_uuid(tenant.id, backend),
                ],
            ))
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?
            .ok_or_else(|| {
                <FieldError as GraphQLError>::not_found(
                    "Artifact installation not found in tenant scope",
                )
            })?;

        let data_owner_id: Uuid = match backend {
            sea_orm::DbBackend::Postgres => row
                .try_get("", "data_owner_id")
                .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?,
            _ => row
                .try_get::<String>("", "data_owner_id")
                .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?
                .parse()
                .map_err(|e: uuid::Error| {
                    <FieldError as GraphQLError>::internal_error(&e.to_string())
                })?,
        };
        let settings_instance_id: Uuid = match backend {
            sea_orm::DbBackend::Postgres => row
                .try_get("", "settings_instance_id")
                .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?,
            _ => row
                .try_get::<String>("", "settings_instance_id")
                .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?
                .parse()
                .map_err(|e: uuid::Error| {
                    <FieldError as GraphQLError>::internal_error(&e.to_string())
                })?,
        };
        let status: Option<String> = row.try_get("", "status").ok();
        let settings_revision: i64 = row.try_get("", "settings_revision").unwrap_or(0);

        let rp_query = format!(
            "SELECT recovery_point_id FROM module_artifact_settings_recovery_points \
             WHERE tenant_id = {placeholder} AND installation_id = {tenant_placeholder} \
             ORDER BY retention_revision DESC LIMIT 1"
        );
        let rp_row = db
            .query_one_raw(sea_orm::Statement::from_sql_and_values(
                backend,
                rp_query,
                vec![
                    sql_uuid(tenant.id, backend),
                    sql_uuid(installation_id, backend),
                ],
            ))
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        let recovery_point_id: Option<Uuid> = if let Some(row) = rp_row {
            match backend {
                sea_orm::DbBackend::Postgres => row.try_get("", "recovery_point_id").ok(),
                _ => row
                    .try_get::<String>("", "recovery_point_id")
                    .ok()
                    .and_then(|s| s.parse().ok()),
            }
        } else {
            None
        };

        let has_recovery_point = recovery_point_id.is_some();
        let is_retired = matches!(
            status.as_deref(),
            Some("inactive") | Some("rolled_back") | None
        );

        let (can_purge, reason) = if !is_retired {
            (false, "Artifact is active or installed. Purge is denied until the artifact is uninstalled/retired.".to_string())
        } else if !has_recovery_point {
            (false, "No protected recovery point exists. An encrypted recovery point must be created before purge.".to_string())
        } else {
            (true, "Installation is retired and recovery point verified. Settings purge is safe to apply.".to_string())
        };

        Ok(ArtifactSettingsPurgePreview {
            installation_id,
            data_owner_id,
            settings_instance_id,
            settings_revision,
            has_recovery_point,
            recovery_point_id,
            can_purge,
            reason,
        })
    }

    /// Previews a structured artifact data purge. Purge is permitted only if
    /// the namespace is uninstalled/retired and no active writes are in flight.
    async fn preview_tenant_artifact_data_purge(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
    ) -> Result<ArtifactDataPurgePreview> {
        ensure_modules_read_permission(ctx).await?;
        let tenant = ctx.data::<TenantContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;
        let backend = db.get_database_backend();

        let placeholder = if backend == sea_orm::DbBackend::Postgres {
            "$1"
        } else {
            "?1"
        };
        let tenant_placeholder = if backend == sea_orm::DbBackend::Postgres {
            "$2"
        } else {
            "?2"
        };

        let query = format!(
            "SELECT n.namespace_revision, a.status \
             FROM module_artifact_data_namespaces n \
             JOIN module_artifact_installations i ON i.slug = n.module_slug AND i.tenant_id = n.tenant_id \
             LEFT JOIN module_artifact_admissions a ON a.installation_id = i.installation_id \
             WHERE i.installation_id = {placeholder} AND n.tenant_id = {tenant_placeholder}"
        );
        let row = db
            .query_one_raw(sea_orm::Statement::from_sql_and_values(
                backend,
                query,
                vec![
                    sql_uuid(installation_id, backend),
                    sql_uuid(tenant.id, backend),
                ],
            ))
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if let Some(row) = row {
            let namespace_revision: i64 = row.try_get("", "namespace_revision").unwrap_or(0);
            let status: Option<String> = row.try_get("", "status").ok();
            let is_retired = matches!(
                status.as_deref(),
                Some("inactive") | Some("rolled_back") | None
            );

            let count_query = format!(
                "SELECT COUNT(*) AS count FROM module_artifact_data_records \
                 WHERE tenant_id = {tenant_placeholder}"
            );
            let count_row = db
                .query_one_raw(sea_orm::Statement::from_sql_and_values(
                    backend,
                    count_query,
                    vec![sql_uuid(tenant.id, backend)],
                ))
                .await
                .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

            let records_to_purge: i64 = count_row
                .and_then(|r| r.try_get("", "count").ok())
                .unwrap_or(0);

            let (can_purge, reason) = if !is_retired {
                (
                    false,
                    "Artifact data purge denied: artifact is active or installed.".to_string(),
                )
            } else {
                (
                    true,
                    format!("Namespace is retired. {records_to_purge} records will be purged."),
                )
            };

            Ok(ArtifactDataPurgePreview {
                installation_id,
                namespace_revision,
                records_to_purge,
                can_purge,
                reason,
            })
        } else {
            Ok(ArtifactDataPurgePreview {
                installation_id,
                namespace_revision: 0,
                records_to_purge: 0,
                can_purge: false,
                reason: "No data namespace found for artifact installation in tenant scope"
                    .to_string(),
            })
        }
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
            .iter()
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
        let snapshot = PlatformCompositionService::active_snapshot(db)
            .await
            .map_err(|error| <FieldError as GraphQLError>::internal_error(&error.to_string()))?;
        Ok(ModuleCompositionSnapshot {
            revision: snapshot.revision,
        })
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
            .map(marketplace_module_from_owner_entry)
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
            .map(|entry| entry.map(marketplace_module_from_owner_entry))
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

        Ok(Some(ModuleOperationRecoveryPlan::from(&plan)))
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
        .map(|plan| ModuleOperationRecoveryPlan::from(&plan))
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
        let db = ctx.data::<DatabaseConnection>()?;
        let checkpoint =
            rustok_modules::TransitionCheckpointStore::load_checkpoint(db, operation_id)
                .await
                .map_err(crate::graphql::transition_lifecycle::map_transition_store_error)?;
        Ok(checkpoint.map(Into::into))
    }

    /// Query all active (non-terminal) governed module release transitions.
    async fn active_module_transitions(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<crate::graphql::transition_lifecycle::ModuleTransitionCheckpointGql>> {
        let db = ctx.data::<DatabaseConnection>()?;
        let checkpoints = rustok_modules::TransitionCheckpointStore::list_active_checkpoints(db)
            .await
            .map_err(crate::graphql::transition_lifecycle::map_transition_store_error)?;
        Ok(checkpoints.into_iter().map(Into::into).collect())
    }

    /// Query all active artifact retention holds protecting CAS blobs, slots, and recovery points.
    async fn module_retention_holds(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<crate::graphql::transition_lifecycle::RetentionHoldGql>> {
        let db = ctx.data::<DatabaseConnection>()?;
        let holds = rustok_modules::RetentionHoldStore::list_active_holds(db)
            .await
            .map_err(crate::graphql::transition_lifecycle::map_transition_store_error)?;
        Ok(holds.into_iter().map(Into::into).collect())
    }
}
