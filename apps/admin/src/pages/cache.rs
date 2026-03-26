use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use serde::{Deserialize, Serialize};

use crate::shared::api::queries::CACHE_HEALTH_QUERY;
use crate::shared::api::{request, ApiError};
use crate::shared::ui::PageHeader;

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

#[component]
pub fn CachePage() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();

    let health_resource = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            request::<EmptyVariables, GraphqlCacheHealthResponse>(
                CACHE_HEALTH_QUERY,
                EmptyVariables {},
                token_value,
                tenant_value,
            )
            .await
        },
    );

    view! {
        <section class="px-10 py-8">
            <PageHeader
                title="Cache"
                subtitle="Cache backend health and configuration".to_string()
                eyebrow="Infrastructure".to_string()
                actions=view! { <div /> }.into_any()
            />

            <div class="rounded-2xl bg-card p-6 shadow border border-border max-w-lg">
                <h4 class="mb-4 text-lg font-semibold text-card-foreground">"Cache Health"</h4>
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
                                    <dt class="text-muted-foreground">"Backend"</dt>
                                    <dd class="font-medium text-foreground font-mono">{backend}</dd>

                                    <dt class="text-muted-foreground">"Redis configured"</dt>
                                    <dd>
                                        {if h.redis_configured {
                                            view! { <span class="text-green-600 font-medium">"Yes"</span> }.into_any()
                                        } else {
                                            view! { <span class="text-muted-foreground">"No"</span> }.into_any()
                                        }}
                                    </dd>

                                    <dt class="text-muted-foreground">"Redis healthy"</dt>
                                    <dd>
                                        {if h.redis_healthy {
                                            view! { <span class="text-green-600 font-medium">"Yes"</span> }.into_any()
                                        } else {
                                            view! { <span class="text-red-600 font-medium">"No"</span> }.into_any()
                                        }}
                                    </dd>

                                    {redis_error.map(|err| view! {
                                        <dt class="text-muted-foreground">"Error"</dt>
                                        <dd class="text-destructive text-xs break-all">{err}</dd>
                                    })}
                                </dl>
                            }.into_any()
                        }
                        Some(Err(err)) => view! {
                            <div class="rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {match err {
                                    ApiError::Unauthorized => "Unauthorized".to_string(),
                                    ApiError::Http(code) => format!("HTTP error {}", code),
                                    ApiError::Network => "Network error".to_string(),
                                    ApiError::Graphql(msg) => format!("GraphQL error: {}", msg),
                                }}
                            </div>
                        }.into_any(),
                    }}
                </Suspense>
            </div>
        </section>
    }
}
