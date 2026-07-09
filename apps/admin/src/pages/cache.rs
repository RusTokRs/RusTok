use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};

use crate::features::cache::transport;
use crate::shared::ui::{Alert, AlertVariant, PageHeader};
use crate::{t_string, use_i18n};

fn local_resource<S, Fut, T>(
    source: impl Fn() -> S + 'static,
    fetcher: impl Fn(S) -> Fut + 'static,
) -> LocalResource<T>
where
    S: 'static,
    Fut: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    LocalResource::new(move || fetcher(source()))
}

#[component]
pub fn CachePage() -> impl IntoView {
    let i18n = use_i18n();
    let token = use_token();
    let tenant = use_tenant();

    let health_resource = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            transport::fetch_cache_health(token_value, tenant_value).await
        },
    );

    view! {
        <section class="flex flex-1 flex-col p-4 md:px-6">
            <PageHeader
                title=t_string!(i18n, cache.title)
                subtitle=t_string!(i18n, cache.subtitle).to_string()
                eyebrow=t_string!(i18n, cache.eyebrow).to_string()
            />

            <div class="rounded-xl border border-border bg-card p-6 shadow-sm max-w-lg">
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
