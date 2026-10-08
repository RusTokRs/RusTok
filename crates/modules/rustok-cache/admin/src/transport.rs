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
        UiTransportPath::Graphql => {
            rustok_graphql::execute(
                &rustok_graphql::graphql_url(),
                rustok_graphql::GraphqlRequest::new(CACHE_HEALTH_QUERY, Some(EmptyVariables {})),
                token,
                tenant_slug,
                None,
            )
            .await
            .map_err(|e| e.to_string())
        }
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
        UiTransportPath::Graphql => {
            rustok_graphql::execute(
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
            .map_err(|e| e.to_string())
        }
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

    #[server(CacheHealthNative, "/api")]
    pub async fn cache_health_native() -> Result<GraphqlCacheHealthResponse, ServerFnError> {
        #[cfg(feature = "ssr")]
        {
            let redis_healthy = false;
            let redis_configured = false;
            let backend = "in-memory".to_string();

            Ok(GraphqlCacheHealthResponse {
                cache_health: CacheHealthPayload {
                    redis_configured,
                    redis_healthy,
                    redis_error: None,
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
            Ok(GraphqlPlatformSettingsResponse {
                platform_settings: PlatformSettingsData {
                    category: "cache".to_string(),
                    settings: serde_json::json!({
                        "mode": "in-memory",
                        "redis_host": "127.0.0.1",
                        "redis_port": 6379,
                        "redis_password": "",
                        "redis_db": 0,
                        "redis_url": ""
                    })
                    .to_string(),
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
            Ok(GraphqlUpdatePlatformSettingsResponse {
                update_platform_settings: UpdatePlatformSettingsPayload {
                    success: true,
                    category: "cache".to_string(),
                    settings,
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
