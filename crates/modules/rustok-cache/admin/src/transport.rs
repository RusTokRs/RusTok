use leptos::server_fn::ServerFnError;
use rustok_ui_transport::UiTransportPath;
use serde::Serialize;

use crate::model::*;

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

const CACHE_HEALTH_QUERY: &str = r#"
query CacheHealth {
  cacheHealth {
    redisConfigured
    redisHealthy
    redisError
    backend
  }
}
"#;

const PLATFORM_SETTINGS_QUERY: &str = r#"
query PlatformSettings($category: String!) {
  platformSettings(category: $category) {
    category
    settings
  }
}
"#;

const UPDATE_PLATFORM_SETTINGS_MUTATION: &str = r#"
mutation UpdatePlatformSettings($input: UpdatePlatformSettingsInput!) {
  updatePlatformSettings(input: $input) {
    success
    category
    settings
  }
}
"#;

#[derive(Clone, Debug, Serialize)]
struct EmptyVariables {}

#[derive(Clone, Debug, Serialize)]
struct CategoryVariables {
    category: String,
}

#[derive(Clone, Debug, Serialize)]
struct UpdateSettingsInput {
    category: String,
    settings: String,
}

#[derive(Clone, Debug, Serialize)]
struct UpdateSettingsVariables {
    input: UpdateSettingsInput,
}

pub async fn fetch_cache_health(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<GraphqlCacheHealthResponse, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::cache_health_native()
            .await
            .map_err(|e| e.to_string()),
        UiTransportPath::Graphql => rustok_graphql::execute(
            &rustok_graphql::graphql_url(),
            rustok_graphql::GraphqlRequest::new(CACHE_HEALTH_QUERY, Some(EmptyVariables {})),
            token,
            tenant_slug,
            None,
        )
        .await
        .map_err(|e| e.to_string()),
    }
}

pub async fn fetch_cache_settings(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<GraphqlPlatformSettingsResponse, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::cache_settings_native()
            .await
            .map_err(|e| e.to_string()),
        UiTransportPath::Graphql => rustok_graphql::execute(
            &rustok_graphql::graphql_url(),
            rustok_graphql::GraphqlRequest::new(
                PLATFORM_SETTINGS_QUERY,
                Some(CategoryVariables {
                    category: "cache".to_string(),
                }),
            ),
            token,
            tenant_slug,
            None,
        )
        .await
        .map_err(|e| e.to_string()),
    }
}

pub async fn update_cache_settings(
    token: Option<String>,
    tenant_slug: Option<String>,
    settings_json: String,
) -> Result<bool, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::update_cache_settings_native(settings_json)
                .await
                .map(|resp| resp.update_platform_settings.success)
                .map_err(|e| e.to_string())
        }
        UiTransportPath::Graphql => {
            let res: GraphqlUpdatePlatformSettingsResponse = rustok_graphql::execute(
                &rustok_graphql::graphql_url(),
                rustok_graphql::GraphqlRequest::new(
                    UPDATE_PLATFORM_SETTINGS_MUTATION,
                    Some(UpdateSettingsVariables {
                        input: UpdateSettingsInput {
                            category: "cache".to_string(),
                            settings: settings_json,
                        },
                    }),
                ),
                token,
                tenant_slug,
                None,
            )
            .await
            .map_err(|e| e.to_string())?;

            Ok(res.update_platform_settings.success)
        }
    }
}

mod native_server_adapter {
    use super::*;
    use leptos::server;

    /// Permission the host requires for cache diagnostics and the persisted `cache` category.
    ///
    /// The GraphQL path uses host-global authority (`settings:manage` included); the native path
    /// checks the same capability from the authenticated snapshot so both transports agree on who
    /// may read or change cache configuration.
    #[cfg(feature = "ssr")]
    fn ensure_cache_admin_permission(
        auth: &rustok_api::AuthContext,
    ) -> Result<(), ServerFnError> {
        if rustok_api::has_effective_permission(
            &auth.permissions,
            &rustok_api::Permission::SETTINGS_MANAGE,
        ) {
            return Ok(());
        }
        Err(ServerFnError::new("Permission denied: settings:manage required"))
    }

    #[cfg(feature = "ssr")]
    async fn native_scope() -> Result<
        (
            rustok_api::HostRuntimeContext,
            rustok_api::AuthContext,
            rustok_api::TenantContext,
        ),
        ServerFnError,
    > {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(|error| ServerFnError::new(format!("cache admin authentication: {error}")))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|error| ServerFnError::new(format!("cache admin tenant scope: {error}")))?;
        ensure_cache_admin_permission(&auth)?;
        // The cache settings category is per-tenant; the GraphQL transport enforces the same
        // boundary (`require_tenant_settings_scope`), so both paths must agree.
        if auth.tenant_id != tenant.id {
            tracing::warn!(
                auth_tenant_id = %auth.tenant_id,
                resolved_tenant_id = %tenant.id,
                code = "cache.tenant_scope_mismatch",
                "cache admin authority cannot cross the resolved tenant boundary"
            );
            return Err(ServerFnError::new("Cache admin access is denied"));
        }
        let host = leptos::prelude::use_context::<rustok_api::HostRuntimeContext>().ok_or_else(|| {
            ServerFnError::new("cache admin runtime context is not available")
        })?;
        Ok((host, auth, tenant))
    }

    #[server(CacheHealthNative, "/api")]
    pub async fn cache_health_native() -> Result<GraphqlCacheHealthResponse, ServerFnError> {
        #[cfg(feature = "ssr")]
        {
            let (host, _auth, _tenant) = native_scope().await?;
            let cache = host.shared_get::<rustok_cache::CacheService>().ok_or_else(|| {
                ServerFnError::new("cache runtime is not published in the host context")
            })?;

            let report = cache.health().await;
            let backend = if report.redis_configured {
                "redis"
            } else {
                "in-memory"
            }
            .to_string();

            Ok(GraphqlCacheHealthResponse {
                cache_health: CacheHealthPayload {
                    redis_configured: report.redis_configured,
                    redis_healthy: report.redis_healthy,
                    redis_error: report.redis_error,
                    backend,
                },
            })
        }
        #[cfg(not(feature = "ssr"))]
        {
            Err(ServerFnError::ServerError("SSR feature not enabled".into()))
        }
    }

    #[server(CacheSettingsNative, "/api")]
    pub async fn cache_settings_native() -> Result<GraphqlPlatformSettingsResponse, ServerFnError> {
        #[cfg(feature = "ssr")]
        {
            let (host, _auth, tenant) = native_scope().await?;
            let port = host
                .shared_get::<rustok_cache::SharedCacheSettingsAdmin>()
                .ok_or_else(|| {
                    ServerFnError::new("cache settings admin port is not published in the host")
                })?;

            let settings = port
                .port()
                .read(tenant.id)
                .await
                .map_err(ServerFnError::new)?;
            let settings = serde_json::to_string(&settings)
                .map_err(|error| ServerFnError::new(format!("cache settings encoding: {error}")))?;

            Ok(GraphqlPlatformSettingsResponse {
                platform_settings: PlatformSettingsData {
                    category: "cache".to_string(),
                    settings,
                },
            })
        }
        #[cfg(not(feature = "ssr"))]
        {
            Err(ServerFnError::ServerError("SSR feature not enabled".into()))
        }
    }

    #[server(UpdateCacheSettingsNative, "/api")]
    pub async fn update_cache_settings_native(
        settings: String,
    ) -> Result<GraphqlUpdatePlatformSettingsResponse, ServerFnError> {
        #[cfg(feature = "ssr")]
        {
            let (host, auth, tenant) = native_scope().await?;
            let port = host
                .shared_get::<rustok_cache::SharedCacheSettingsAdmin>()
                .ok_or_else(|| {
                    ServerFnError::new("cache settings admin port is not published in the host")
                })?;

            let draft: serde_json::Value = serde_json::from_str(&settings).map_err(|error| {
                ServerFnError::new(format!("Invalid JSON in cache settings: {error}"))
            })?;

            // Persistence, validation and secret preservation belong to the host settings service;
            // the adapter only reports what was actually stored.
            let stored = port
                .port()
                .update(tenant.id, Some(auth.user_id), draft)
                .await
                .map_err(ServerFnError::new)?;
            let stored = serde_json::to_string(&stored).map_err(|error| {
                ServerFnError::new(format!("cache settings encoding: {error}"))
            })?;

            Ok(GraphqlUpdatePlatformSettingsResponse {
                update_platform_settings: UpdatePlatformSettingsPayload {
                    success: true,
                    category: "cache".to_string(),
                    settings: stored,
                },
            })
        }
        #[cfg(not(feature = "ssr"))]
        {
            let _ = settings;
            Err(ServerFnError::ServerError("SSR feature not enabled".into()))
        }
    }
}
