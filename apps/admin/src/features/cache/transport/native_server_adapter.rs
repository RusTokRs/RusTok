use leptos::prelude::*;

#[cfg(feature = "ssr")]
use crate::features::cache::model::CacheHealthPayload;
use crate::features::cache::model::CacheHealthResponse;

#[server(prefix = "/api/fn", endpoint = "admin/cache-health")]
pub(super) async fn cache_health_native() -> Result<CacheHealthResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_cache::CacheService;

        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let payload = if let Some(cache) = runtime.shared_get::<CacheService>() {
            let report = cache.health().await;
            CacheHealthPayload {
                redis_configured: report.redis_configured,
                redis_healthy: report.redis_healthy,
                redis_error: report.redis_error,
                backend: if report.redis_configured {
                    "redis".to_string()
                } else {
                    "in-memory".to_string()
                },
            }
        } else {
            CacheHealthPayload {
                redis_configured: false,
                redis_healthy: false,
                redis_error: None,
                backend: "none".to_string(),
            }
        };

        Ok(CacheHealthResponse {
            cache_health: payload,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/cache-health requires the `ssr` feature",
        ))
    }
}
