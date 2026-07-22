use std::collections::{HashMap, HashSet};

use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result};
use chrono::{Duration, Utc};
use rustok_api::Permission;
use rustok_core::ModuleRegistry;
use rustok_telemetry::metrics;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QuerySelect,
};
use semver::{Version, VersionReq};
use std::time::Instant;
use uuid::Uuid;

use crate::common::RequestContext;
use crate::context::{AuthContext, TenantContext};
use crate::graphql::types::{
    ActivityItem, ActivityUser, BuildJob, DashboardStats, InstalledModule, MarketplaceModule,
    MarketplaceModuleVersion, ModuleOperationRecoveryPlan, ModuleRegistryItem, ModuleSettingField,
    ReleaseInfo, Tenant, TenantModule, User, UserConnection, UserEdge, UsersFilter,
};
use crate::models::_entities::users::Column as UsersColumn;
use crate::models::users;
use crate::modules::ManifestManager;
use crate::services::dashboard_user_activity;
use crate::services::effective_module_policy::EffectiveModulePolicyService;
use crate::services::marketplace_catalog::marketplace_catalog_from_context;
use crate::services::marketplace_catalog::MarketplaceCatalogQuery;
use crate::services::module_lifecycle::{ModuleLifecycleService, ModuleOperationRecoveryError};
use crate::services::platform_composition::PlatformCompositionService;
use crate::services::rbac_service::RbacService;
use crate::services::registry_governance::RegistryGovernanceService;
use crate::services::registry_principal::RegistryPrincipalRef;
use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_api::graphql::GraphQLError;
use rustok_api::graphql::{encode_cursor, PageInfo, PaginationInput};
use rustok_build::SharedBuildControl;

fn build_control_from_context(ctx: &Context<'_>) -> Result<SharedBuildControl> {
    ctx.data::<ServerRuntimeContext>()?
        .shared_get::<SharedBuildControl>()
        .ok_or_else(|| {
            <FieldError as GraphQLError>::internal_error("build control is not configured")
        })
}

fn calculate_percent_change(current: i64, previous: i64) -> f64 {
    if previous == 0 {
        if current == 0 {
            0.0
        } else {
            100.0
        }
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

fn humanize_slug(slug: &str) -> String {
    slug.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn fallback_module_category(slug: &str) -> &'static str {
    match slug {
        "content" | "blog" | "forum" | "pages" => "content",
        "commerce" => "commerce",
        "tenant" | "rbac" | "index" | "outbox" => "platform",
        _ => "extensions",
    }
}

fn normalize_version_req(raw: &str, is_max: bool) -> String {
    let trimmed = raw.trim();
    let wildcard = trimmed.replace(".x", ".*").replace(".X", ".*");
    let has_operator = wildcard.contains('<')
        || wildcard.contains('>')
        || wildcard.contains('=')
        || wildcard.contains('~')
        || wildcard.contains('^')
        || wildcard.contains('*')
        || wildcard.contains(',');

    if has_operator {
        return wildcard;
    }

    if is_max {
        format!("<= {wildcard}")
    } else {
        format!(">= {wildcard}")
    }
}

fn current_platform_version() -> Option<Version> {
    Version::parse(env!("CARGO_PKG_VERSION")).ok()
}

fn is_catalog_module_compatible(entry: &crate::modules::CatalogManifestModule) -> bool {
    let Some(platform_version) = current_platform_version() else {
        return true;
    };

    let min_ok = entry
        .rustok_min_version
        .as_deref()
        .and_then(|raw| VersionReq::parse(&normalize_version_req(raw, false)).ok())
        .is_none_or(|req| req.matches(&platform_version));
    let max_ok = entry
        .rustok_max_version
        .as_deref()
        .and_then(|raw| VersionReq::parse(&normalize_version_req(raw, true)).ok())
        .is_none_or(|req| req.matches(&platform_version));

    min_ok && max_ok
}

fn marketplace_module_from_catalog_entry(
    entry: crate::modules::CatalogManifestModule,
    registry: &ModuleRegistry,
    installed_modules: &[crate::modules::InstalledManifestModule],
) -> MarketplaceModule {
    let catalog_version_fallback = entry
        .versions
        .first()
        .map(|version| version.version.clone());
    let compatible = is_catalog_module_compatible(&entry);
    let signature_present = entry.signature.is_some();
    let runtime_module = registry.get(&entry.slug);
    let installed_module = installed_modules
        .iter()
        .find(|module| module.slug == entry.slug);
    let latest_version = runtime_module
        .map(|module| module.version().to_string())
        .or_else(|| entry.version.clone())
        .or(catalog_version_fallback)
        .unwrap_or_else(|| "workspace".to_string());
    let installed_version = installed_module.and_then(|module| module.version.clone());
    let dependencies = runtime_module
        .map(|module| {
            module
                .dependencies()
                .iter()
                .map(|dependency| dependency.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| entry.depends_on.clone());
    let versions = if entry.versions.is_empty() {
        vec![MarketplaceModuleVersion {
            version: latest_version.clone(),
            changelog: None,
            yanked: false,
            published_at: None,
            checksum_sha256: entry.checksum_sha256.clone(),
            signature_present,
        }]
    } else {
        entry
            .versions
            .iter()
            .map(|version| MarketplaceModuleVersion {
                version: version.version.clone(),
                changelog: version.changelog.clone(),
                yanked: version.yanked,
                published_at: version.published_at.clone(),
                checksum_sha256: version.checksum_sha256.clone(),
                signature_present: version.signature.is_some(),
            })
            .collect()
    };

    MarketplaceModule {
        slug: entry.slug.clone(),
        name: entry
            .name
            .clone()
            .or_else(|| runtime_module.map(|module| module.name().to_string()))
            .unwrap_or_else(|| humanize_slug(&entry.slug)),
        latest_version: latest_version.clone(),
        description: entry
            .description
            .clone()
            .or_else(|| runtime_module.map(|module| module.description().to_string()))
            .unwrap_or_else(|| {
                format!(
                    "{} module from {} source",
                    humanize_slug(&entry.slug),
                    entry.source
                )
            }),
        source: entry.source.clone(),
        kind: if entry.required || registry.is_core(&entry.slug) {
            "core".to_string()
        } else {
            "optional".to_string()
        },
        category: entry
            .category
            .clone()
            .unwrap_or_else(|| fallback_module_category(&entry.slug).to_string()),
        tags: entry.tags.clone(),
        icon_url: entry.icon_url.clone(),
        banner_url: entry.banner_url.clone(),
        screenshots: entry.screenshots.clone(),
        crate_name: entry.crate_name,
        dependencies,
        ownership: entry.ownership,
        trust_level: entry.trust_level,
        rustok_min_version: entry.rustok_min_version,
        rustok_max_version: entry.rustok_max_version,
        publisher: entry.publisher,
        checksum_sha256: entry.checksum_sha256,
        signature_present,
        versions,
        has_admin_ui: entry.has_admin_ui,
        has_storefront_ui: entry.has_storefront_ui,
        ui_classification: entry.ui_classification,
        registry_lifecycle: None,
        compatible,
        recommended_admin_surfaces: entry.recommended_admin_surfaces,
        showcase_admin_surfaces: entry.showcase_admin_surfaces,
        settings_schema: settings_schema_fields(&entry.settings_schema),
        installed: installed_module.is_some(),
        installed_version: installed_version.clone(),
        update_available: installed_version
            .as_ref()
            .is_some_and(|version| version != &latest_version),
    }
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

fn settings_schema_fields(
    schema: &HashMap<String, crate::modules::ModuleSettingSpec>,
) -> Vec<ModuleSettingField> {
    let mut keys = schema.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    keys.into_iter()
        .filter_map(|key| {
            schema
                .get(&key)
                .map(|spec| ModuleSettingField::from_spec(key, spec))
        })
        .collect()
}

fn marketplace_modules_from_catalog(
    entries: Vec<crate::modules::CatalogManifestModule>,
    registry: &ModuleRegistry,
    installed_modules: &[crate::modules::InstalledManifestModule],
) -> Vec<MarketplaceModule> {
    entries
        .into_iter()
        .map(|entry| marketplace_module_from_catalog_entry(entry, registry, installed_modules))
        .collect()
}

fn trust_level_matches(module: &MarketplaceModule, trust_level: Option<&str>) -> bool {
    trust_level.is_none_or(|trust_level| module.trust_level.eq_ignore_ascii_case(trust_level))
}

fn source_matches(module: &MarketplaceModule, source: Option<&str>) -> bool {
    source.is_none_or(|source| module.source.eq_ignore_ascii_case(source))
}

fn map_module_operation_recovery_error(error: ModuleOperationRecoveryError) -> FieldError {
    match error {
        ModuleOperationRecoveryError::OperationNotFound => {
            <FieldError as GraphQLError>::bad_user_input("Module operation not found")
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

async fn ensure_modules_read_permission(ctx: &Context<'_>) -> Result<()> {
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

#[derive(Clone, Copy)]
struct MarketplaceProjectionLocales<'a> {
    preferred: Option<&'a str>,
    fallback: Option<&'a str>,
}

async fn load_marketplace_catalog(
    db: &DatabaseConnection,
    runtime_ctx: &ServerRuntimeContext,
    manifest: &crate::modules::ModulesManifest,
    registry: &ModuleRegistry,
    query: &MarketplaceCatalogQuery,
    locales: MarketplaceProjectionLocales<'_>,
) -> Result<Vec<crate::modules::CatalogManifestModule>> {
    let modules = marketplace_catalog_from_context(runtime_ctx)
        .list_modules(manifest, registry, query)
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

    RegistryGovernanceService::new(db.clone())
        .apply_catalog_projection(modules, locales.preferred, locales.fallback)
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))
}

async fn load_marketplace_module(
    db: &DatabaseConnection,
    runtime_ctx: &ServerRuntimeContext,
    manifest: &crate::modules::ModulesManifest,
    registry: &ModuleRegistry,
    query: &MarketplaceCatalogQuery,
    slug: &str,
    locales: MarketplaceProjectionLocales<'_>,
) -> Result<Option<crate::modules::CatalogManifestModule>> {
    let module = marketplace_catalog_from_context(runtime_ctx)
        .get_module(manifest, registry, query, slug)
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
    let Some(module) = module else {
        return Ok(None);
    };

    let mut projected = RegistryGovernanceService::new(db.clone())
        .apply_catalog_projection(vec![module], locales.preferred, locales.fallback)
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
    Ok(projected.pop())
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
        let catalog_by_slug: HashMap<String, crate::modules::CatalogManifestModule> =
            load_marketplace_catalog(
                db,
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

        let modules = registry
            .list()
            .into_iter()
            .take(limit)
            .map(|module| {
                let catalog_entry = catalog_by_slug.get(module.slug());

                ModuleRegistryItem {
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
                        .map(|entry| settings_schema_fields(&entry.settings_schema))
                        .unwrap_or_default(),
                }
            })
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
        let modules = EffectiveModulePolicyService::tenant_override_snapshots(
            db,
            registry,
            tenant.id,
            owner_limit,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let modules = modules
            .into_iter()
            .map(|module| TenantModule {
                module_slug: module.module_slug,
                enabled: module.enabled,
                settings: module.settings.to_string(),
            })
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

    async fn installed_modules(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<InstalledModule>> {
        ensure_modules_read_permission(ctx).await?;
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);

        let db = ctx.data::<DatabaseConnection>()?;
        let manifest = PlatformCompositionService::active_manifest(db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let modules = ManifestManager::installed_modules(&manifest)
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

    #[allow(clippy::too_many_arguments)]
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

    async fn module_operation_recovery_plan(
        &self,
        ctx: &Context<'_>,
        operation_id: Uuid,
    ) -> Result<Option<ModuleOperationRecoveryPlan>> {
        ensure_modules_read_permission(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let plan =
            match ModuleLifecycleService::module_operation_recovery_plan(db, operation_id).await {
                Ok(plan) => plan,
                Err(ModuleOperationRecoveryError::OperationNotFound) => return Ok(None),
                Err(err) => return Err(map_module_operation_recovery_error(err)),
            };

        if plan.tenant_id != tenant.id {
            return Ok(None);
        }

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
        let requested_limit = requested_collection_limit(limit);
        let limit = clamp_collection_limit(limit);
        let plans = ModuleLifecycleService::failed_module_operation_recovery_plans(
            db,
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

        Ok(build.as_ref().map(BuildJob::from_model))
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

        let builds = builds.iter().map(BuildJob::from_model).collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.build_history",
            Some(requested_limit),
            limit,
            builds.len(),
        );

        Ok(builds)
    }

    async fn active_release(&self, ctx: &Context<'_>) -> Result<Option<ReleaseInfo>> {
        ensure_modules_read_permission(ctx).await?;

        let release = build_control_from_context(ctx)?
            .0
            .active_release()
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(release.as_ref().map(ReleaseInfo::from_model))
    }

    async fn release_history(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 20)] limit: i32,
        #[graphql(default = 0)] offset: i32,
    ) -> Result<Vec<ReleaseInfo>> {
        ensure_modules_read_permission(ctx).await?;

        let requested_limit = limit.max(0) as u64;
        let limit = limit.clamp(1, 100) as u64;
        let offset = offset.max(0) as u64;

        let releases = build_control_from_context(ctx)?
            .0
            .list_releases_page(limit, offset)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let releases = releases
            .iter()
            .map(ReleaseInfo::from_model)
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "root.release_history",
            Some(requested_limit),
            limit,
            releases.len(),
        );

        Ok(releases)
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
}

#[cfg(test)]
mod tests {
    use super::{
        is_catalog_module_compatible, marketplace_module_from_catalog_entry, normalize_version_req,
        source_matches, trust_level_matches,
    };
    use crate::graphql::types::MarketplaceModule;
    use crate::modules::{CatalogManifestModule, InstalledManifestModule};
    use rustok_core::ModuleRegistry;
    use std::collections::HashMap;

    fn catalog_module(min: Option<&str>, max: Option<&str>) -> CatalogManifestModule {
        CatalogManifestModule {
            slug: "seo".to_string(),
            source: "registry".to_string(),
            crate_name: "rustok-seo".to_string(),
            name: None,
            category: None,
            tags: Vec::new(),
            icon_url: None,
            banner_url: None,
            screenshots: Vec::new(),
            version: Some("1.2.0".to_string()),
            description: None,
            git: None,
            rev: None,
            path: None,
            required: false,
            depends_on: Vec::new(),
            ownership: "third_party".to_string(),
            trust_level: "unverified".to_string(),
            rustok_min_version: min.map(str::to_string),
            rustok_max_version: max.map(str::to_string),
            publisher: None,
            checksum_sha256: None,
            signature: None,
            versions: Vec::new(),
            has_admin_ui: false,
            has_storefront_ui: false,
            ui_classification: "no_ui".to_string(),
            recommended_admin_surfaces: Vec::new(),
            showcase_admin_surfaces: Vec::new(),
            settings_schema: HashMap::new(),
        }
    }

    #[test]
    fn normalize_version_req_adds_bounds_for_plain_versions() {
        assert_eq!(normalize_version_req("0.5.0", false), ">= 0.5.0");
        assert_eq!(normalize_version_req("1.0.0", true), "<= 1.0.0");
        assert_eq!(normalize_version_req("1.x", true), "1.*");
    }

    #[test]
    fn compatibility_accepts_unbounded_catalog_entry() {
        assert!(is_catalog_module_compatible(&catalog_module(None, None)));
    }

    #[test]
    fn compatibility_rejects_entry_above_current_platform_max() {
        assert!(!is_catalog_module_compatible(&catalog_module(
            None,
            Some("0.0.1")
        )));
    }

    #[test]
    fn trust_level_filter_matches_case_insensitively() {
        let module = MarketplaceModule {
            slug: "seo".to_string(),
            name: "SEO".to_string(),
            latest_version: "1.2.0".to_string(),
            description: "SEO tools".to_string(),
            source: "registry".to_string(),
            kind: "optional".to_string(),
            category: "extensions".to_string(),
            tags: Vec::new(),
            icon_url: None,
            banner_url: None,
            screenshots: Vec::new(),
            crate_name: "rustok-seo".to_string(),
            dependencies: Vec::new(),
            ownership: "third_party".to_string(),
            trust_level: "verified".to_string(),
            rustok_min_version: None,
            rustok_max_version: None,
            publisher: None,
            checksum_sha256: None,
            signature_present: false,
            versions: Vec::new(),
            has_admin_ui: false,
            has_storefront_ui: false,
            ui_classification: "no_ui".to_string(),
            registry_lifecycle: None,
            compatible: true,
            recommended_admin_surfaces: Vec::new(),
            showcase_admin_surfaces: Vec::new(),
            settings_schema: Vec::new(),
            installed: false,
            installed_version: None,
            update_available: false,
        };

        assert!(trust_level_matches(&module, Some("VERIFIED")));
        assert!(!trust_level_matches(&module, Some("community")));
    }

    #[test]
    fn source_filter_matches_case_insensitively() {
        let module = MarketplaceModule {
            slug: "seo".to_string(),
            name: "SEO".to_string(),
            latest_version: "1.2.0".to_string(),
            description: "SEO tools".to_string(),
            source: "registry".to_string(),
            kind: "optional".to_string(),
            category: "extensions".to_string(),
            tags: Vec::new(),
            icon_url: None,
            banner_url: None,
            screenshots: Vec::new(),
            crate_name: "rustok-seo".to_string(),
            dependencies: Vec::new(),
            ownership: "third_party".to_string(),
            trust_level: "verified".to_string(),
            rustok_min_version: None,
            rustok_max_version: None,
            publisher: None,
            checksum_sha256: None,
            signature_present: false,
            versions: Vec::new(),
            has_admin_ui: false,
            has_storefront_ui: false,
            ui_classification: "no_ui".to_string(),
            registry_lifecycle: None,
            compatible: true,
            recommended_admin_surfaces: Vec::new(),
            showcase_admin_surfaces: Vec::new(),
            settings_schema: Vec::new(),
            installed: false,
            installed_version: None,
            update_available: false,
        };

        assert!(source_matches(&module, Some("REGISTRY")));
        assert!(!source_matches(&module, Some("path")));
    }

    #[test]
    fn marketplace_mapping_uses_catalog_name_and_description_without_runtime_module() {
        let mut entry = catalog_module(None, None);
        entry.name = Some("SEO Toolkit".to_string());
        entry.description = Some("Search optimization bundle".to_string());

        let module = marketplace_module_from_catalog_entry(
            entry,
            &ModuleRegistry::new(),
            &Vec::<InstalledManifestModule>::new(),
        );

        assert_eq!(module.name, "SEO Toolkit");
        assert_eq!(module.description, "Search optimization bundle");
    }

    #[test]
    fn marketplace_mapping_prefers_manifest_category_when_present() {
        let mut entry = catalog_module(None, None);
        entry.slug = "search".to_string();
        entry.category = Some("search".to_string());

        let module = marketplace_module_from_catalog_entry(
            entry,
            &ModuleRegistry::new(),
            &Vec::<InstalledManifestModule>::new(),
        );

        assert_eq!(module.category, "search");
    }

    #[test]
    fn marketplace_mapping_preserves_manifest_tags() {
        let mut entry = catalog_module(None, None);
        entry.tags = vec!["discovery".to_string(), "search".to_string()];

        let module = marketplace_module_from_catalog_entry(
            entry,
            &ModuleRegistry::new(),
            &Vec::<InstalledManifestModule>::new(),
        );

        assert_eq!(module.tags, vec!["discovery", "search"]);
    }

    #[test]
    fn marketplace_mapping_derives_latest_version_from_version_trail() {
        let mut entry = catalog_module(None, None);
        entry.version = None;
        entry.versions = vec![
            crate::modules::CatalogModuleVersion {
                version: "2.1.0".to_string(),
                changelog: None,
                yanked: false,
                published_at: Some("2026-03-10T00:00:00Z".to_string()),
                checksum_sha256: None,
                signature: None,
            },
            crate::modules::CatalogModuleVersion {
                version: "1.9.0".to_string(),
                changelog: None,
                yanked: false,
                published_at: Some("2026-03-01T00:00:00Z".to_string()),
                checksum_sha256: None,
                signature: None,
            },
        ];

        let module = marketplace_module_from_catalog_entry(
            entry,
            &ModuleRegistry::new(),
            &Vec::<InstalledManifestModule>::new(),
        );

        assert_eq!(module.latest_version, "2.1.0");
        assert_eq!(module.versions.len(), 2);
        assert_eq!(module.versions[0].version, "2.1.0");
    }

    #[test]
    fn marketplace_mapping_preserves_visual_metadata() {
        let mut entry = catalog_module(None, None);
        entry.icon_url = Some("https://cdn.example.test/modules/seo/icon.svg".to_string());
        entry.banner_url = Some("https://cdn.example.test/modules/seo/banner.png".to_string());
        entry.screenshots = vec![
            "https://cdn.example.test/modules/seo/screenshot-1.png".to_string(),
            "https://cdn.example.test/modules/seo/screenshot-2.png".to_string(),
        ];

        let module = marketplace_module_from_catalog_entry(
            entry,
            &ModuleRegistry::new(),
            &Vec::<InstalledManifestModule>::new(),
        );

        assert_eq!(
            module.icon_url.as_deref(),
            Some("https://cdn.example.test/modules/seo/icon.svg")
        );
        assert_eq!(
            module.banner_url.as_deref(),
            Some("https://cdn.example.test/modules/seo/banner.png")
        );
        assert_eq!(
            module.screenshots,
            vec![
                "https://cdn.example.test/modules/seo/screenshot-1.png",
                "https://cdn.example.test/modules/seo/screenshot-2.png",
            ]
        );
    }

    #[test]
    fn marketplace_mapping_falls_back_to_legacy_slug_category_when_manifest_category_missing() {
        let mut entry = catalog_module(None, None);
        entry.slug = "search".to_string();

        let module = marketplace_module_from_catalog_entry(
            entry,
            &ModuleRegistry::new(),
            &Vec::<InstalledManifestModule>::new(),
        );

        assert_eq!(module.category, "extensions");
    }
}
