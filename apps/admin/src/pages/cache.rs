use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use serde::{Deserialize, Serialize};

use crate::shared::api::queries::CACHE_HEALTH_QUERY;
use crate::shared::api::{request, ApiError};
use crate::shared::ui::{Alert, AlertVariant, PageHeader};
use crate::{t_string, use_i18n};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct GraphqlCacheHealthResponse {
    #[serde(rename = "cacheHealth")]
    cache_health: CacheHealthPayload,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CacheHealthPayload {
    #[serde(rename = "redisConfigured")]
    redis_configured: bool,
    #[serde(rename = "redisHealthy")]
    redis_healthy: bool,
    #[serde(rename = "redisError")]
    redis_error: Option<String>,
    backend: String,
}

#[derive(Clone, Debug, Serialize)]
struct EmptyVariables {}

async fn fetch_cache_health_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<GraphqlCacheHealthResponse, ApiError> {
    request::<EmptyVariables, GraphqlCacheHealthResponse>(
        CACHE_HEALTH_QUERY,
        EmptyVariables {},
        token,
        tenant_slug,
    )
    .await
}

async fn fetch_cache_health_server() -> Result<GraphqlCacheHealthResponse, ServerFnError> {
    cache_health_native().await
}

async fn fetch_cache_health(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<GraphqlCacheHealthResponse, String> {
    match fetch_cache_health_server().await {
        Ok(response) => Ok(response),
        Err(server_err) => fetch_cache_health_graphql(token, tenant_slug)
            .await
            .map_err(|graphql_err| {
                format!(
                    "native path failed: {}; graphql path failed: {}",
                    server_err, graphql_err
                )
            }),
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/cache-health")]
async fn cache_health_native() -> Result<GraphqlCacheHealthResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_cache::CacheService;

        let app_ctx = expect_context::<AppContext>();
        let payload = if let Some(cache) = app_ctx.shared_store.get::<CacheService>() {
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

        Ok(GraphqlCacheHealthResponse {
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

#[component]
pub fn CachePage() -> impl IntoView {
    let i18n = use_i18n();
    let token = use_token();
    let tenant = use_tenant();

    let health_resource = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            fetch_cache_health(token_value, tenant_value).await
        },
    );

    view! {
        <section class="px-10 py-8">
            <PageHeader
                title=t_string!(i18n, cache.title)
                subtitle=t_string!(i18n, cache.subtitle).to_string()
                eyebrow=t_string!(i18n, cache.eyebrow).to_string()
                actions=view! { <div /> }.into_any()
            />

            <div class="rounded-2xl bg-card p-6 shadow border border-border max-w-lg">
                <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                    {move || t_string!(i18n, cache.health.title)}
                </h4>
                <Suspense fallback=move || view! {
                    <div class="space-y-3">
                        {(0..3).map(|_| view! {
                            <div class="h-8 animate-pulse rounded-lg bg-muted" />
                        }).collect_view()}
                    </div>
                }>
                    {move || match health_resource.get() {
                        None => view! {
                            <div class="space-y-3">
                                {(0..3).map(|_| view! {
                                    <div class="h-8 animate-pulse rounded-lg bg-muted" />
                                }).collect_view()}
                            </div>
                        }.into_any(),
                        Some(Ok(response)) => {
                            let h = response.cache_health;
                            let backend = h.backend.clone();
                            let redis_error = h.redis_error.clone();
                            view! {
                                <dl class="grid grid-cols-2 gap-x-4 gap-y-3 text-sm">
                                    <dt class="text-muted-foreground">
                                        {t_string!(i18n, cache.health.backend)}
                                    </dt>
                                    <dd class="font-medium text-foreground font-mono">{backend}</dd>

                                    <dt class="text-muted-foreground">
                                        {t_string!(i18n, cache.health.configured)}
                                    </dt>
                                    <dd>
                                        {if h.redis_configured {
                                            view! {
                                                <span class="text-green-600 font-medium">
                                                    {t_string!(i18n, cache.yes)}
                                                </span>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <span class="text-muted-foreground">
                                                    {t_string!(i18n, cache.no)}
                                                </span>
                                            }.into_any()
                                        }}
                                    </dd>

                                    <dt class="text-muted-foreground">
                                        {t_string!(i18n, cache.health.healthy)}
                                    </dt>
                                    <dd>
                                        {if h.redis_healthy {
                                            view! {
                                                <span class="text-green-600 font-medium">
                                                    {t_string!(i18n, cache.yes)}
                                                </span>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <span class="text-red-600 font-medium">
                                                    {t_string!(i18n, cache.no)}
                                                </span>
                                            }.into_any()
                                        }}
                                    </dd>

                                    {redis_error.map(|err| view! {
                                        <dt class="text-muted-foreground">
                                            {t_string!(i18n, cache.health.error)}
                                        </dt>
                                        <dd class="text-destructive text-xs break-all">{err}</dd>
                                    })}
                                </dl>
                            }.into_any()
                        }
                        Some(Err(err)) => view! {
                            <Alert variant=AlertVariant::Destructive>
                                {err.to_string()}
                            </Alert>
                        }.into_any(),
                    }}
                </Suspense>
            </div>
        </section>
    }
}
