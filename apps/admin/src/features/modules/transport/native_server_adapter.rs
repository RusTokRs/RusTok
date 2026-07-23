use leptos::prelude::*;
#[cfg(feature = "ssr")]
use serde::Serialize;
#[cfg(feature = "ssr")]
use serde::de::DeserializeOwned;

use super::types::*;
#[allow(unused_imports)]
use crate::entities::module::model::{
    MarketplaceModuleVersion, RegistryFollowUpGateLifecycle, RegistryGovernanceActionLifecycle,
    RegistryGovernanceEventLifecycle, RegistryGovernanceEventPayloadLifecycle,
    RegistryModuleLifecycle, RegistryOwnerLifecycle, RegistryPublishRequestLifecycle,
    RegistryReleaseLifecycle, RegistryValidationStageLifecycle,
    registry_principal_label_from_value,
};
use crate::entities::module::{
    BuildJob, InstalledModule, MarketplaceModule, ModuleInfo, ReleaseInfo, TenantModule,
};
#[cfg(feature = "ssr")]
use crate::shared::api::api_base_url;

#[cfg(feature = "ssr")]
use super::manifest::*;

#[cfg(feature = "ssr")]
pub async fn registry_governance_get_native<T>(
    path: String,
    token: String,
    tenant: String,
) -> Result<T, ServerFnError>
where
    T: DeserializeOwned,
{
    registry_governance_http_request_native::<(), T>(
        reqwest::Method::GET,
        path,
        token,
        tenant,
        None,
    )
    .await
}

#[cfg(feature = "ssr")]
pub async fn registry_governance_request_native<B, T>(
    method: reqwest::Method,
    path: String,
    token: String,
    tenant: String,
    body: &B,
) -> Result<T, ServerFnError>
where
    B: Serialize + ?Sized,
    T: DeserializeOwned,
{
    registry_governance_http_request_native(method, path, token, tenant, Some(body)).await
}

#[cfg(feature = "ssr")]
pub async fn registry_governance_http_request_native<B, T>(
    method: reqwest::Method,
    path: String,
    token: String,
    tenant: String,
    body: Option<&B>,
) -> Result<T, ServerFnError>
where
    B: Serialize + ?Sized,
    T: DeserializeOwned,
{
    let url = format!(
        "{}{}",
        api_base_url(),
        if path.starts_with('/') {
            path
        } else {
            format!("/{path}")
        }
    );
    let client = reqwest::Client::new();
    let mut request = client
        .request(method, url)
        .bearer_auth(token)
        .header("X-Tenant-ID", tenant);

    if let Some(body) = body {
        request = request.json(body);
    }

    let response = request
        .send()
        .await
        .map_err(|err| ServerFnError::new(err.to_string()))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| ServerFnError::new(err.to_string()))?;

    if !status.is_success() {
        return Err(ServerFnError::new(format!(
            "registry governance request failed with status {status}: {text}"
        )));
    }

    serde_json::from_str(&text).map_err(|err| ServerFnError::new(err.to_string()))
}

#[cfg(feature = "ssr")]
#[derive(Clone, Debug, Serialize)]
pub struct RegistryValidationRequestPayload {
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    #[serde(rename = "dry_run")]
    pub dry_run: bool,
}

#[cfg(feature = "ssr")]
#[derive(Clone, Debug, Serialize)]
pub struct RegistryDecisionRequestPayload {
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    #[serde(rename = "dry_run")]
    pub dry_run: bool,
    pub reason: Option<String>,
    pub reason_code: Option<String>,
}

#[cfg(feature = "ssr")]
#[derive(Clone, Debug, Serialize)]
pub struct RegistryOwnerTransferPayload {
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    #[serde(rename = "dry_run")]
    pub dry_run: bool,
    pub slug: String,
    #[serde(rename = "new_owner_user_id")]
    pub new_owner_user_id: String,
    pub reason: Option<String>,
    pub reason_code: Option<String>,
}

#[cfg(feature = "ssr")]
#[derive(Clone, Debug, Serialize)]
pub struct RegistryYankPayload {
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    #[serde(rename = "dry_run")]
    pub dry_run: bool,
    pub slug: String,
    pub version: String,
    pub reason: Option<String>,
    pub reason_code: Option<String>,
}

#[cfg(feature = "ssr")]
pub fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

#[cfg(feature = "ssr")]
pub fn default_module_ownership() -> String {
    "third_party".to_string()
}

#[cfg(feature = "ssr")]
pub fn default_module_trust_level() -> String {
    "unverified".to_string()
}

#[cfg(feature = "ssr")]
#[derive(Clone)]
struct ModulesServerRuntime {
    db: sea_orm::DatabaseConnection,
    build_control: Option<rustok_build::SharedBuildControl>,
}

#[cfg(feature = "ssr")]
async fn modules_server_context() -> Result<
    (
        ModulesServerRuntime,
        rustok_api::AuthContext,
        rustok_api::TenantContext,
    ),
    ServerFnError,
> {
    use leptos::prelude::expect_context;
    use leptos_axum::extract;
    use rustok_api::Permission;
    use rustok_api::{
        AuthContext, HostRuntimeContext, TenantContext, has_any_effective_permission,
    };

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let auth = extract::<AuthContext>()
        .await
        .map_err(|err| server_error(err.to_string()))?;
    let tenant = extract::<TenantContext>()
        .await
        .map_err(|err| server_error(err.to_string()))?;

    if !has_any_effective_permission(
        &auth.permissions,
        &[
            Permission::MODULES_READ,
            Permission::MODULES_LIST,
            Permission::MODULES_MANAGE,
        ],
    ) {
        return Err(ServerFnError::new(
            "modules:read, modules:list, or modules:manage required",
        ));
    }

    Ok((
        ModulesServerRuntime {
            db: runtime_ctx.db_clone(),
            build_control: runtime_ctx.shared_get(),
        },
        auth,
        tenant,
    ))
}

#[cfg(feature = "ssr")]
pub fn upper_snake(value: &str) -> String {
    value
        .replace('-', "_")
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_uppercase())
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(feature = "ssr")]
pub async fn active_runtime_platform_snapshot(
    db: &sea_orm::DatabaseConnection,
) -> Result<RuntimePlatformSnapshot, ServerFnError> {
    let snapshot = rustok_modules::ModuleControlPlane::new(db.clone())
        .composition()
        .active_snapshot()
        .await
        .map_err(|err| server_error(err.to_string()))?;
    Ok(RuntimePlatformSnapshot {
        revision: snapshot.revision,
        manifest: serde_json::from_value(snapshot.manifest)
            .map_err(|err| server_error(format!("failed to decode platform manifest: {err}")))?,
    })
}

#[cfg(feature = "ssr")]
pub async fn effective_enabled_modules_native(
    db: &sea_orm::DatabaseConnection,
    registry: &rustok_core::ModuleRegistry,
    tenant_id: uuid::Uuid,
) -> Result<std::collections::HashSet<String>, ServerFnError> {
    let manifest = active_runtime_platform_snapshot(db).await?.manifest;
    rustok_modules::ModuleControlPlane::new(db.clone())
        .effective_policy(registry, manifest.settings.default_enabled)
        .resolve_enabled(tenant_id)
        .await
        .map_err(|err| server_error(err.to_string()))
}

#[cfg(feature = "ssr")]
fn required_principal_label(
    value: &serde_json::Value,
    field: &str,
) -> Result<String, ServerFnError> {
    registry_principal_label_from_value(value).ok_or_else(|| {
        server_error(format!(
            "Registry principal field '{field}' is missing a displayable principal label"
        ))
    })
}

#[cfg(feature = "ssr")]
fn optional_principal_label(value: Option<&serde_json::Value>) -> Option<String> {
    value.and_then(registry_principal_label_from_value)
}

#[cfg(feature = "ssr")]
fn map_governance_lifecycle_snapshot(
    snapshot: rustok_modules::ModuleGovernanceLifecycleSnapshot,
) -> Result<RegistryModuleLifecycle, ServerFnError> {
    Ok(RegistryModuleLifecycle {
        moderation_policy: crate::entities::module::model::RegistryModerationPolicyLifecycle {
            mode: snapshot.moderation_policy.mode,
            live_publish_supported: snapshot.moderation_policy.live_publish_supported,
            live_governance_supported: snapshot.moderation_policy.live_governance_supported,
            manual_review_required: snapshot.moderation_policy.manual_review_required,
            restriction_reason_code: snapshot.moderation_policy.restriction_reason_code,
            restriction_reason: snapshot.moderation_policy.restriction_reason,
        },
        owner_binding: snapshot
            .owner_binding
            .map(|owner| -> Result<RegistryOwnerLifecycle, ServerFnError> {
                Ok(RegistryOwnerLifecycle {
                    owner: required_principal_label(&owner.owner_principal, "owner")?,
                    bound_by: required_principal_label(&owner.bound_by_principal, "bound_by")?,
                    bound_at: owner.bound_at,
                    updated_at: owner.updated_at,
                })
            })
            .transpose()?,
        latest_request: snapshot
            .latest_request
            .map(
                |request| -> Result<RegistryPublishRequestLifecycle, ServerFnError> {
                    Ok(RegistryPublishRequestLifecycle {
                        id: request.id,
                        status: request.status,
                        requested_by: required_principal_label(
                            &request.requested_by_principal,
                            "requested_by",
                        )?,
                        publisher: optional_principal_label(request.publisher_principal.as_ref()),
                        approved_by: optional_principal_label(
                            request.approved_by_principal.as_ref(),
                        ),
                        rejected_by: optional_principal_label(
                            request.rejected_by_principal.as_ref(),
                        ),
                        rejection_reason: request.rejection_reason,
                        changes_requested_by: optional_principal_label(
                            request.changes_requested_by_principal.as_ref(),
                        ),
                        changes_requested_reason: request.changes_requested_reason,
                        changes_requested_reason_code: request.changes_requested_reason_code,
                        changes_requested_at: request.changes_requested_at,
                        held_by: optional_principal_label(request.held_by_principal.as_ref()),
                        held_reason: request.held_reason,
                        held_reason_code: request.held_reason_code,
                        held_at: request.held_at,
                        held_from_status: request.held_from_status,
                        warnings: request.warnings,
                        errors: request.errors,
                        created_at: request.created_at,
                        updated_at: request.updated_at,
                        published_at: request.published_at,
                    })
                },
            )
            .transpose()?,
        latest_release: snapshot
            .latest_release
            .map(
                |release| -> Result<RegistryReleaseLifecycle, ServerFnError> {
                    Ok(RegistryReleaseLifecycle {
                        version: release.version,
                        status: release.status,
                        publisher: required_principal_label(
                            &release.publisher_principal,
                            "publisher",
                        )?,
                        checksum_sha256: release.checksum_sha256,
                        published_at: release.published_at,
                        yanked_reason: release.yanked_reason,
                        yanked_by: optional_principal_label(release.yanked_by_principal.as_ref()),
                        yanked_at: release.yanked_at,
                    })
                },
            )
            .transpose()?,
        recent_events: snapshot
            .recent_events
            .into_iter()
            .map(|event| {
                let owner_transition = event.payload.owner_transition.map(|transition| {
                    crate::entities::module::model::RegistryOwnerTransitionLifecycle {
                        previous_owner: optional_principal_label(
                            transition.previous_owner_principal.as_ref(),
                        ),
                        new_owner: optional_principal_label(
                            transition.new_owner_principal.as_ref(),
                        ),
                        bound_by: optional_principal_label(transition.bound_by_principal.as_ref()),
                    }
                });
                Ok(RegistryGovernanceEventLifecycle {
                    id: event.id,
                    event_type: event.event_type,
                    actor: required_principal_label(&event.actor_principal, "actor")?,
                    publisher: optional_principal_label(event.publisher_principal.as_ref()),
                    payload: RegistryGovernanceEventPayloadLifecycle {
                        reason: event.payload.reason,
                        reason_code: event.payload.reason_code,
                        detail: event.payload.detail,
                        version: event.payload.version,
                        stage_key: event.payload.stage_key,
                        attempt_number: event.payload.attempt_number,
                        owner_transition,
                        warnings: event.payload.warnings,
                        errors: event.payload.errors,
                        mode: event.payload.mode,
                    },
                    created_at: event.created_at,
                })
            })
            .collect::<Result<Vec<_>, ServerFnError>>()?,
        follow_up_gates: snapshot
            .follow_up_gates
            .into_iter()
            .map(|gate| RegistryFollowUpGateLifecycle {
                key: gate.key,
                status: gate.status,
                detail: gate.detail,
                updated_at: gate.updated_at,
            })
            .collect(),
        validation_stages: snapshot
            .validation_stages
            .into_iter()
            .map(|stage| RegistryValidationStageLifecycle {
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
            })
            .collect(),
        governance_actions: snapshot
            .governance_actions
            .into_iter()
            .map(|action| RegistryGovernanceActionLifecycle {
                key: action.key,
                reason_required: action.reason_required,
                reason_code_required: action.reason_code_required,
                reason_codes: action.reason_codes,
                destructive: action.destructive,
            })
            .collect(),
    })
}

#[cfg(feature = "ssr")]
fn owner_setting_fields(
    schema: std::collections::BTreeMap<String, rustok_modules::ModuleSettingSpec>,
) -> Result<Vec<crate::entities::module::ModuleSettingField>, ServerFnError> {
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
                        .map_err(|err| server_error(err.to_string()))?,
                );
            }
            if let Some(items) = spec.items.as_deref() {
                shape.insert(
                    "items".to_string(),
                    serde_json::to_value(items).map_err(|err| server_error(err.to_string()))?,
                );
            }
            Ok(crate::entities::module::ModuleSettingField {
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
            })
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn map_marketplace_entry(
    entry: rustok_modules::ModuleMarketplaceEntry,
) -> Result<MarketplaceModule, ServerFnError> {
    Ok(MarketplaceModule {
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
            .map(map_governance_lifecycle_snapshot)
            .transpose()?,
        compatible: entry.compatible,
        recommended_admin_surfaces: entry.recommended_admin_surfaces,
        showcase_admin_surfaces: entry.showcase_admin_surfaces,
        settings_schema: owner_setting_fields(entry.settings_schema)?,
        installed: entry.installed,
        installed_version: entry.installed_version,
        update_available: entry.update_available,
    })
}

#[cfg(feature = "ssr")]
fn marketplace_catalog_handle()
-> Result<rustok_modules::SharedModuleMarketplaceCatalog, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::HostRuntimeContext;

    expect_context::<HostRuntimeContext>()
        .shared_get::<rustok_modules::SharedModuleMarketplaceCatalog>()
        .ok_or_else(|| server_error("module marketplace catalog handle is not configured"))
}

#[server(prefix = "/api/fn", endpoint = "admin/list-enabled-modules")]
pub async fn list_enabled_modules_native() -> Result<Vec<String>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use leptos_axum::extract;
        use rustok_api::Permission;
        use rustok_api::{
            AuthContext, HostRuntimeContext, TenantContext, has_any_effective_permission,
        };
        use rustok_core::ModuleRegistry;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let app_ctx = ModulesServerRuntime {
            db: runtime_ctx.db_clone(),
            build_control: runtime_ctx.shared_get(),
        };
        let auth = extract::<AuthContext>().await.map_err(ServerFnError::new)?;
        let tenant = extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        if !has_any_effective_permission(
            &auth.permissions,
            &[
                Permission::MODULES_READ,
                Permission::MODULES_LIST,
                Permission::MODULES_MANAGE,
            ],
        ) {
            return Err(ServerFnError::new(
                "modules:read, modules:list, or modules:manage required",
            ));
        }

        let registry = expect_context::<ModuleRegistry>();
        let mut modules = effective_enabled_modules_native(&app_ctx.db, &registry, tenant.id)
            .await?
            .into_iter()
            .collect::<Vec<_>>();

        modules.sort();
        Ok(modules)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/list-enabled-modules requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/module-registry")]
pub async fn list_module_registry_native() -> Result<Vec<ModuleInfo>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::app::modules::module_runtime_metadata;
        use leptos::prelude::expect_context;
        use rustok_core::ModuleRegistry;

        let (app_ctx, _auth, tenant) = modules_server_context().await?;
        let registry = expect_context::<ModuleRegistry>();
        let enabled_modules =
            effective_enabled_modules_native(&app_ctx.db, &registry, tenant.id).await?;

        Ok(registry
            .list()
            .into_iter()
            .map(|module| {
                let metadata = module_runtime_metadata(module.slug());
                ModuleInfo {
                    module_slug: module.slug().to_string(),
                    name: module.name().to_string(),
                    description: module.description().to_string(),
                    version: module.version().to_string(),
                    kind: if registry.is_core(module.slug()) {
                        "core".to_string()
                    } else {
                        "optional".to_string()
                    },
                    dependencies: module
                        .dependencies()
                        .iter()
                        .map(|dependency| dependency.to_string())
                        .collect(),
                    enabled: registry.is_core(module.slug())
                        || enabled_modules.contains(module.slug()),
                    ownership: metadata
                        .map(|metadata| metadata.ownership.to_string())
                        .unwrap_or_else(|| "third_party".to_string()),
                    trust_level: metadata
                        .map(|metadata| metadata.trust_level.to_string())
                        .unwrap_or_else(|| "unverified".to_string()),
                    has_admin_ui: false,
                    has_storefront_ui: false,
                    ui_classification: "no-ui".to_string(),
                    recommended_admin_surfaces: metadata
                        .map(|metadata| {
                            metadata
                                .recommended_admin_surfaces
                                .iter()
                                .map(|surface| surface.to_string())
                                .collect()
                        })
                        .unwrap_or_default(),
                    showcase_admin_surfaces: metadata
                        .map(|metadata| {
                            metadata
                                .showcase_admin_surfaces
                                .iter()
                                .map(|surface| surface.to_string())
                                .collect()
                        })
                        .unwrap_or_default(),
                }
            })
            .collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/module-registry requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/installed-modules")]
pub async fn list_installed_modules_native() -> Result<Vec<InstalledModule>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let manifest = active_runtime_platform_snapshot(&app_ctx.db)
            .await?
            .manifest;

        let mut modules = manifest
            .modules
            .into_iter()
            .map(|(slug, spec)| InstalledModule {
                slug,
                source: spec.source,
                crate_name: spec.crate_name,
                version: spec.version,
                required: spec.required,
                dependencies: spec.depends_on,
            })
            .collect::<Vec<_>>();
        modules.sort_by(|left, right| left.slug.cmp(&right.slug));
        Ok(modules)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/installed-modules requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/list-tenant-modules")]
pub async fn list_tenant_modules_native() -> Result<Vec<TenantModule>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_core::ModuleRegistry;

        let (app_ctx, _auth, tenant) = modules_server_context().await?;
        let registry = expect_context::<ModuleRegistry>();
        let manifest = active_runtime_platform_snapshot(&app_ctx.db)
            .await?
            .manifest;

        rustok_modules::ModuleControlPlane::new(app_ctx.db)
            .effective_policy(&registry, manifest.settings.default_enabled)
            .tenant_override_snapshots(tenant.id, 100)
            .await
            .map(|modules| {
                let mut modules = modules
                    .into_iter()
                    .map(|module| TenantModule {
                        module_slug: module.module_slug,
                        enabled: module.enabled,
                        settings: module.settings.to_string(),
                    })
                    .collect::<Vec<_>>();
                modules.sort_by(|left, right| left.module_slug.cmp(&right.module_slug));
                modules
            })
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/list-tenant-modules requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/marketplace")]
pub async fn list_marketplace_modules_native(
    search: Option<String>,
    category: Option<String>,
    tag: Option<String>,
    source: Option<String>,
    trust_level: Option<String>,
    only_compatible: Option<bool>,
    installed_only: Option<bool>,
) -> Result<Vec<MarketplaceModule>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (_app_ctx, _auth, tenant) = modules_server_context().await?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        marketplace_catalog_handle()?
            .0
            .list(rustok_modules::ModuleMarketplaceQuery {
                search,
                category,
                tag,
                source,
                trust_level,
                only_compatible: only_compatible.unwrap_or(true),
                installed_only: installed_only.unwrap_or(false),
                preferred_locale: Some(request_context.locale),
                fallback_locale: Some(tenant.default_locale),
                limit: 100,
            })
            .await
            .map_err(|err| server_error(err.to_string()))?
            .into_iter()
            .map(map_marketplace_entry)
            .collect()
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            search,
            category,
            tag,
            source,
            trust_level,
            only_compatible,
            installed_only,
        );
        Err(ServerFnError::new(
            "admin/marketplace requires the ssr feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/marketplace-module")]
pub async fn marketplace_module_native(
    slug: String,
) -> Result<Option<MarketplaceModule>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (_app_ctx, _auth, tenant) = modules_server_context().await?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        marketplace_catalog_handle()?
            .0
            .get(
                &slug,
                Some(request_context.locale),
                Some(tenant.default_locale),
            )
            .await
            .map_err(|err| server_error(err.to_string()))?
            .map(map_marketplace_entry)
            .transpose()
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = slug;
        Err(ServerFnError::new(
            "admin/marketplace-module requires the ssr feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/active-build")]
pub async fn active_build_native() -> Result<Option<BuildJob>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let build_control = app_ctx
            .build_control
            .ok_or_else(|| server_error("build control is not configured"))?;
        let build = build_control
            .0
            .active_build()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        Ok(build)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/active-build requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/active-release")]
pub async fn active_release_native() -> Result<Option<ReleaseInfo>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let build_control = app_ctx
            .build_control
            .ok_or_else(|| server_error("build control is not configured"))?;
        let release = build_control
            .0
            .active_release()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        Ok(release)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/active-release requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/build-history")]
pub async fn build_history_native(limit: i32, offset: i32) -> Result<Vec<BuildJob>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let build_control = app_ctx
            .build_control
            .ok_or_else(|| server_error("build control is not configured"))?;
        let limit = u64::try_from(limit.clamp(1, 100))
            .map_err(|_| server_error("invalid build history limit"))?;
        let offset = u64::try_from(offset.max(0))
            .map_err(|_| server_error("invalid build history offset"))?;
        build_control
            .0
            .list_builds_page(limit, offset)
            .await
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (limit, offset);
        Err(ServerFnError::new(
            "admin/build-history requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/update-module-settings")]
pub async fn update_module_settings_native(
    module_slug: String,
    settings: String,
) -> Result<TenantModule, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::app::modules::module_runtime_metadata;
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::has_any_effective_permission;
        use rustok_core::ModuleRegistry;

        let (app_ctx, auth, tenant) = modules_server_context().await?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::MODULES_MANAGE]) {
            return Err(ServerFnError::new("modules:manage required"));
        }

        let registry = expect_context::<ModuleRegistry>();
        if registry.get(&module_slug).is_none() {
            return Err(server_error("Unknown module"));
        }

        let raw_settings: serde_json::Value = serde_json::from_str(&settings)
            .map_err(|err| server_error(format!("invalid module settings JSON: {err}")))?;
        let metadata = module_runtime_metadata(&module_slug)
            .ok_or_else(|| server_error("Unknown module settings schema"))?;
        let schema: std::collections::HashMap<String, rustok_modules::ModuleSettingSpec> =
            serde_json::from_str(metadata.settings_schema_json)
                .map_err(|err| server_error(format!("invalid compiled settings schema: {err}")))?;
        let normalized_settings =
            rustok_modules::normalize_module_settings(&module_slug, &schema, raw_settings)
                .map_err(|err| server_error(err.to_string()))?;
        let snapshot = active_runtime_platform_snapshot(&app_ctx.db).await?;
        let record = rustok_modules::ModuleControlPlane::new(app_ctx.db)
            .lifecycle(&registry, snapshot.manifest.settings.default_enabled)
            .persist_static_normalized_settings(
                tenant.id,
                &module_slug,
                normalized_settings.clone(),
            )
            .await
            .map_err(|err| server_error(err.to_string()))?;

        Ok(TenantModule {
            module_slug,
            enabled: record.enabled,
            settings: normalized_settings.to_string(),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (module_slug, settings);
        Err(ServerFnError::new(
            "admin/update-module-settings requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/rollback-build")]
pub async fn rollback_build_native(build_id: String) -> Result<BuildJob, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::Permission;
        use rustok_api::has_any_effective_permission;

        let (app_ctx, auth, tenant) = modules_server_context().await?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::MODULES_MANAGE]) {
            return Err(ServerFnError::new("modules:manage required"));
        }

        let build_id = uuid::Uuid::parse_str(build_id.trim())
            .map_err(|err| server_error(format!("invalid build id: {err}")))?;
        let build_control = app_ctx
            .build_control
            .ok_or_else(|| server_error("build control is not configured"))?;
        let restored_build = build_control
            .0
            .rollback_build(rustok_build::BuildRollbackCommand {
                build_id,
                tenant_id: tenant.id,
                actor_id: auth.user_id,
            })
            .await
            .map_err(|err| server_error(err.to_string()))?;

        Ok(restored_build)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = build_id;
        Err(ServerFnError::new(
            "admin/rollback-build requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "admin/registry-fetch-publish-request-status"
)]
pub async fn fetch_registry_publish_request_status_native(
    token: String,
    tenant: String,
    request_id: String,
) -> Result<RegistryPublishStatusContract, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_get_native(format!("/v2/catalog/publish/{request_id}"), token, tenant)
            .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id);
        Err(ServerFnError::new(
            "admin/registry-fetch-publish-request-status requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "admin/registry-validate-publish-request"
)]
pub async fn validate_registry_publish_request_native(
    token: String,
    tenant: String,
    request_id: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            format!("/v2/catalog/publish/{request_id}/validate"),
            token,
            tenant,
            &RegistryValidationRequestPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id, dry_run);
        Err(ServerFnError::new(
            "admin/registry-validate-publish-request requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "admin/registry-approve-publish-request"
)]
pub async fn approve_registry_publish_request_native(
    token: String,
    tenant: String,
    request_id: String,
    reason: Option<String>,
    reason_code: Option<String>,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            format!("/v2/catalog/publish/{request_id}/approve"),
            token,
            tenant,
            &RegistryDecisionRequestPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                reason,
                reason_code,
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id, reason, reason_code, dry_run);
        Err(ServerFnError::new(
            "admin/registry-approve-publish-request requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/registry-reject-publish-request")]
pub async fn reject_registry_publish_request_native(
    token: String,
    tenant: String,
    request_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            format!("/v2/catalog/publish/{request_id}/reject"),
            token,
            tenant,
            &RegistryDecisionRequestPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                reason: Some(reason),
                reason_code: Some(reason_code),
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id, reason, reason_code, dry_run);
        Err(ServerFnError::new(
            "admin/registry-reject-publish-request requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "admin/registry-request-changes-publish-request"
)]
pub async fn request_changes_registry_publish_request_native(
    token: String,
    tenant: String,
    request_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            format!("/v2/catalog/publish/{request_id}/request-changes"),
            token,
            tenant,
            &RegistryDecisionRequestPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                reason: Some(reason),
                reason_code: Some(reason_code),
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id, reason, reason_code, dry_run);
        Err(ServerFnError::new(
            "admin/registry-request-changes-publish-request requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/registry-hold-publish-request")]
pub async fn hold_registry_publish_request_native(
    token: String,
    tenant: String,
    request_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            format!("/v2/catalog/publish/{request_id}/hold"),
            token,
            tenant,
            &RegistryDecisionRequestPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                reason: Some(reason),
                reason_code: Some(reason_code),
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id, reason, reason_code, dry_run);
        Err(ServerFnError::new(
            "admin/registry-hold-publish-request requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/registry-resume-publish-request")]
pub async fn resume_registry_publish_request_native(
    token: String,
    tenant: String,
    request_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            format!("/v2/catalog/publish/{request_id}/resume"),
            token,
            tenant,
            &RegistryDecisionRequestPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                reason: Some(reason),
                reason_code: Some(reason_code),
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id, reason, reason_code, dry_run);
        Err(ServerFnError::new(
            "admin/registry-resume-publish-request requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/registry-transfer-owner")]
pub async fn transfer_registry_owner_native(
    token: String,
    tenant: String,
    slug: String,
    new_owner_user_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            "/v2/catalog/owner-transfer".to_string(),
            token,
            tenant,
            &RegistryOwnerTransferPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                slug,
                new_owner_user_id,
                reason: Some(reason),
                reason_code: Some(reason_code),
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            token,
            tenant,
            slug,
            new_owner_user_id,
            reason,
            reason_code,
            dry_run,
        );
        Err(ServerFnError::new(
            "admin/registry-transfer-owner requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/registry-yank-release")]
pub async fn yank_registry_release_native(
    token: String,
    tenant: String,
    slug: String,
    version: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            "/v2/catalog/yank".to_string(),
            token,
            tenant,
            &RegistryYankPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                slug,
                version,
                reason: Some(reason),
                reason_code: Some(reason_code),
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, slug, version, reason, reason_code, dry_run);
        Err(ServerFnError::new(
            "admin/registry-yank-release requires the `ssr` feature",
        ))
    }
}
