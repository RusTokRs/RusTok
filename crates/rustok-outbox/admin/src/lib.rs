mod api;
mod model;

use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};

#[component]
pub fn OutboxAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();

    let bootstrap = Resource::new(
        move || (token.get(), tenant.get()),
        move |_| async move { api::fetch_bootstrap().await },
    );

    view! {
        <div class="space-y-6">
            <header class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-2">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        "outbox"
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">"Outbox Relay"</h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        "Module-owned overview for transactional event persistence, retry pressure, and relay health."
                    </p>
                </div>
            </header>

            <Suspense fallback=move || view! { <div class="h-32 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || {
                    bootstrap.get().map(|result| match result {
                        Ok(bootstrap) => view! {
                            <section class="grid gap-4 lg:grid-cols-3 xl:grid-cols-4">
                                <InfoCard label="Health".to_string() value=bootstrap.health.clone() />
                                <InfoCard
                                    label="Tenant context".to_string()
                                    value=bootstrap.tenant_slug.clone().unwrap_or_else(|| "global".to_string())
                                />
                                {bootstrap
                                    .counters
                                    .into_iter()
                                    .map(|counter| view! {
                                        <InfoCard label=counter.label value=counter.value.to_string() />
                                    })
                                    .collect_view()}
                            </section>

                            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                                <h2 class="text-lg font-semibold text-card-foreground">"Relay Notes"</h2>
                                <ul class="mt-3 space-y-2 text-sm text-muted-foreground">
                                    {bootstrap
                                        .relay_notes
                                        .into_iter()
                                        .map(|note| view! { <li>{note}</li> })
                                        .collect_view()}
                                </ul>
                            </section>
                        }
                        .into_any(),
                        Err(err) => view! {
                            <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-5 py-4 text-sm text-destructive">
                                {format!("Failed to load outbox bootstrap: {err}")}
                            </div>
                        }
                        .into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn InfoCard(label: String, value: String) -> impl IntoView {
    view! {
        <div class="rounded-2xl border border-border bg-card p-6 shadow-sm">
            <div class="text-sm text-muted-foreground">{label}</div>
            <div class="mt-2 text-lg font-semibold text-card-foreground">{value}</div>
        </div>
    }
}
