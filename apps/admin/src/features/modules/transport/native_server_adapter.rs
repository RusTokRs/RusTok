use leptos::prelude::*;
#[cfg(feature = "ssr")]
use serde::Serialize;
#[cfg(feature = "ssr")]
use serde::de::DeserializeOwned;

use super::types::*;
use crate::entities::module::{BuildJob, MarketplaceModule, ModuleInfo};
#[cfg(feature = "ssr")]
use crate::shared::api::api_base_url;

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
async fn effective_module_policy_view_native(
    tenant_id: uuid::Uuid,
) -> Result<rustok_api::ModuleEffectivePolicyView, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::HostRuntimeContext;

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let reader = runtime_ctx
        .shared_get::<rustok_modules::SharedModuleEffectivePolicyReader>()
        .ok_or_else(|| server_error("effective module policy reader is not configured"))?;

    reader
        .0
        .resolve(tenant_id)
        .await
        .map_err(|_| server_error("effective module policy is unavailable"))
}

#[cfg(feature = "ssr")]
async fn static_installed_modules_native()
-> Result<Vec<rustok_api::StaticInstalledModuleView>, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::HostRuntimeContext;

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let reader = runtime_ctx
        .shared_get::<rustok_modules::SharedStaticInstalledModuleReader>()
        .ok_or_else(|| server_error("static installed module reader is not configured"))?;

    reader
        .0
        .list()
        .await
        .map_err(|_| server_error("static installed module projection is unavailable"))
}

#[cfg(feature = "ssr")]
async fn static_module_registry_views_native(
    query: rustok_modules::StaticModuleRegistryQuery,
) -> Result<Vec<rustok_api::StaticModuleRegistryView>, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::HostRuntimeContext;

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let reader = runtime_ctx
        .shared_get::<rustok_modules::SharedStaticModuleRegistryReader>()
        .ok_or_else(|| server_error("static module registry reader is not configured"))?;

    reader
        .0
        .list(query)
        .await
        .map_err(|_| server_error("static module registry projection is unavailable"))
}

#[cfg(feature = "ssr")]
async fn static_tenant_module_views_native(
    tenant_id: uuid::Uuid,
    limit: u32,
) -> Result<Vec<rustok_api::StaticTenantModuleView>, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::HostRuntimeContext;

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let reader = runtime_ctx
        .shared_get::<rustok_modules::SharedStaticModuleLifecycleReader>()
        .ok_or_else(|| server_error("static module lifecycle reader is not configured"))?;

    reader
        .0
        .static_tenant_module_views(tenant_id, limit)
        .await
        .map_err(|_| server_error("static tenant lifecycle projection is unavailable"))
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

#[cfg(feature = "ssr")]
fn map_transition_service_error_native(
    error: rustok_modules::ModuleTransitionServiceError,
) -> ServerFnError {
    use rustok_modules::ModuleTransitionServiceError;

    match error {
        ModuleTransitionServiceError::NotFound(_) => {
            ServerFnError::new("CHECKPOINT_NOT_FOUND: Transition checkpoint not found")
        }
        ModuleTransitionServiceError::AuthorizationDenied => ServerFnError::new(
            "PERMISSION_DENIED: Permission denied for the module transition scope",
        ),
        ModuleTransitionServiceError::InvalidCommand(message) => {
            ServerFnError::new(format!("BAD_USER_INPUT: {message}"))
        }
        ModuleTransitionServiceError::RevisionConflict { expected, current } => {
            ServerFnError::new(format!(
                "REVISION_CONFLICT: Transition revision conflict: expected {expected}, current {current}"
            ))
        }
        ModuleTransitionServiceError::IdempotencyConflict => ServerFnError::new(
            "IDEMPOTENCY_CONFLICT: Idempotency key was used for a different transition command",
        ),
        ModuleTransitionServiceError::OperationInProgress => {
            ServerFnError::new("OPERATION_IN_PROGRESS: Transition command is already in progress")
        }
        ModuleTransitionServiceError::Coordinator(error) => {
            ServerFnError::new(format!("TRANSITION_COORDINATOR_ERROR: {error}"))
        }
        ModuleTransitionServiceError::Store(_) | ModuleTransitionServiceError::Outbox(_) => {
            ServerFnError::new("TRANSITION_UNAVAILABLE: Transition owner service is unavailable")
        }
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/module-transition-checkpoint")]
pub async fn module_transition_checkpoint_native(
    operation_id: String,
) -> Result<Option<rustok_api::ModuleTransitionCheckpointView>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let operation_id = uuid::Uuid::parse_str(&operation_id)
            .map_err(|_| server_error("BAD_USER_INPUT: operation_id must be a UUID"))?;
        let (app_ctx, _auth, tenant) = modules_server_context().await?;
        rustok_modules::ModuleControlPlane::new(app_ctx.db)
            .transitions()
            .checkpoint(operation_id, Some(tenant.id))
            .await
            .map(|checkpoint| checkpoint.map(rustok_api::ModuleTransitionCheckpointView::from))
            .map_err(map_transition_service_error_native)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = operation_id;
        Err(ServerFnError::new(
            "admin/module-transition-checkpoint requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/active-module-transitions")]
pub async fn active_module_transitions_native()
-> Result<Vec<rustok_api::ModuleTransitionCheckpointView>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (app_ctx, _auth, tenant) = modules_server_context().await?;
        rustok_modules::ModuleControlPlane::new(app_ctx.db)
            .transitions()
            .active_checkpoints(Some(tenant.id))
            .await
            .map(|checkpoints| {
                checkpoints
                    .into_iter()
                    .map(rustok_api::ModuleTransitionCheckpointView::from)
                    .collect()
            })
            .map_err(map_transition_service_error_native)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/active-module-transitions requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/module-retention-holds")]
pub async fn module_retention_holds_native()
-> Result<Vec<rustok_api::ModuleRetentionHoldView>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (app_ctx, _auth, tenant) = modules_server_context().await?;
        rustok_modules::ModuleControlPlane::new(app_ctx.db)
            .transitions()
            .retention_holds(Some(tenant.id))
            .await
            .map(|holds| {
                holds
                    .into_iter()
                    .map(rustok_api::ModuleRetentionHoldView::from)
                    .collect()
            })
            .map_err(map_transition_service_error_native)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/module-retention-holds requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/finalize-module-transition")]
pub async fn finalize_module_transition_native(
    operation_id: String,
    expected_revision: i64,
    idempotency_key: String,
) -> Result<rustok_api::ModuleTransitionCheckpointView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{Permission, has_effective_permission};

        let operation_id = uuid::Uuid::parse_str(&operation_id)
            .map_err(|_| server_error("BAD_USER_INPUT: operation_id must be a UUID"))?;
        let idempotency_key = uuid::Uuid::parse_str(&idempotency_key)
            .map_err(|_| server_error("BAD_USER_INPUT: idempotency_key must be a UUID"))?;
        if operation_id.is_nil() || idempotency_key.is_nil() || expected_revision <= 0 {
            return Err(server_error(
                "BAD_USER_INPUT: Transition finalization requires non-nil operation and idempotency identities plus a positive expected revision",
            ));
        }

        let (app_ctx, auth, tenant) = modules_server_context().await?;
        if !has_effective_permission(&auth.permissions, &Permission::MODULES_MANAGE) {
            return Err(server_error(
                "PERMISSION_DENIED: modules:manage required for transition finalization",
            ));
        }
        let receipt = rustok_modules::ModuleControlPlane::new(app_ctx.db)
            .transitions()
            .finalize(rustok_modules::ModuleTransitionFinalizeCommand {
                operation_id,
                expected_revision: u64::try_from(expected_revision).map_err(|_| {
                    server_error(
                        "BAD_USER_INPUT: Transition revision is outside the supported range",
                    )
                })?,
                context: rustok_modules::ModuleCommandContext {
                    actor_id: auth.user_id,
                    tenant_id: Some(tenant.id),
                    trace_id: format!("leptos:{idempotency_key}"),
                    correlation_id: idempotency_key,
                    idempotency_key,
                },
                actor_can_manage_modules: true,
            })
            .await
            .map_err(map_transition_service_error_native)?;
        Ok(receipt.checkpoint.into())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (operation_id, expected_revision, idempotency_key);
        Err(ServerFnError::new(
            "admin/finalize-module-transition requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/module-effective-policy")]
pub async fn module_effective_policy_native()
-> Result<rustok_api::ModuleEffectivePolicyView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (_app_ctx, _auth, tenant) = modules_server_context().await?;
        effective_module_policy_view_native(tenant.id).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/module-effective-policy requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/module-registry")]
pub async fn list_module_registry_native() -> Result<Vec<ModuleInfo>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos_axum::extract;

        let (_app_ctx, _auth, tenant) = modules_server_context().await?;
        let request_context = extract::<rustok_api::RequestContext>()
            .await
            .map_err(|error| server_error(error.to_string()))?;
        static_module_registry_views_native(rustok_modules::StaticModuleRegistryQuery {
            tenant_id: tenant.id,
            preferred_locale: request_context.locale,
            fallback_locale: tenant.default_locale,
            limit: rustok_modules::STATIC_MODULE_REGISTRY_MAX_LIMIT,
        })
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/module-registry requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/installed-modules")]
pub async fn list_installed_modules_native()
-> Result<Vec<rustok_api::StaticInstalledModuleView>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (_app_ctx, _auth, _tenant) = modules_server_context().await?;
        static_installed_modules_native().await
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/installed-modules requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/list-tenant-modules")]
pub async fn list_tenant_modules_native()
-> Result<Vec<rustok_api::StaticTenantModuleView>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (_app_ctx, _auth, tenant) = modules_server_context().await?;
        static_tenant_module_views_native(tenant.id, 100).await
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
            .map_err(|err| server_error(err.to_string()))
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
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = slug;
        Err(ServerFnError::new(
            "admin/marketplace-module requires the ssr feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/marketplace-registry-freshness")]
pub async fn marketplace_registry_freshness_native()
-> Result<Vec<rustok_api::MarketplaceRegistryFreshness>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{Permission, has_effective_permission};

        let (_app_ctx, auth, _tenant) = modules_server_context().await?;
        if !has_effective_permission(&auth.permissions, &Permission::MODULES_MANAGE) {
            return Err(server_error("modules:manage required"));
        }
        Ok(marketplace_catalog_handle()?.0.registry_freshness())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/marketplace-registry-freshness requires the ssr feature",
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

#[server(
    prefix = "/api/fn",
    endpoint = "admin/registry-fetch-publish-request-status"
)]
pub async fn fetch_registry_publish_request_status_native(
    token: String,
    tenant: String,
    request_id: String,
) -> Result<RegistryPublishStatus, ServerFnError> {
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
