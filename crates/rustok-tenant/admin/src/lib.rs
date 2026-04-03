mod api;
mod model;

use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};

#[component]
pub fn TenantAdmin() -> impl IntoView {
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
                        "tenant"
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">"Tenant Runtime"</h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        "Module-owned overview for active tenant state and effective module enablement."
                    </p>
                </div>
            </header>

            <Suspense fallback=move || view! { <div class="h-32 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || {
                    bootstrap.get().map(|result| match result {
                        Ok(bootstrap) => view! {
                            <section class="grid gap-4 lg:grid-cols-4">
                                <InfoCard label="Tenant" value=bootstrap.tenant.slug.clone() />
                                <InfoCard label="Name" value=bootstrap.tenant.name.clone() />
                                <InfoCard
                                    label="Domain"
                                    value=bootstrap.tenant.domain.clone().unwrap_or_else(|| "n/a".to_string())
                                />
                                <InfoCard
                                    label="Status"
                                    value=if bootstrap.tenant.is_active {
                                        "active".to_string()
                                    } else {
                                        "inactive".to_string()
                                    }
                                />
                            </section>

                            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                                <div class="flex items-center justify-between gap-4">
                                    <div>
                                        <h2 class="text-lg font-semibold text-card-foreground">"Registered Modules"</h2>
                                        <p class="text-sm text-muted-foreground">
                                            "Core modules stay enabled by contract; optional modules reflect tenant-side state."
                                        </p>
                                    </div>
                                    <div class="text-sm text-muted-foreground">
                                        {format!("Updated {}", bootstrap.tenant.updated_at)}
                                    </div>
                                </div>
                                <div class="mt-4 grid gap-3">
                                    {bootstrap
                                        .modules
                                        .into_iter()
                                        .map(|module| {
                                            view! {
                                                <div class="rounded-xl border border-border bg-background px-4 py-3">
                                                    <div class="flex flex-col gap-2 lg:flex-row lg:items-center lg:justify-between">
                                                        <div>
                                                            <div class="font-medium text-card-foreground">{module.name}</div>
                                                            <div class="text-xs text-muted-foreground">{module.slug}</div>
                                                            <div class="mt-1 text-sm text-muted-foreground">{module.description}</div>
                                                        </div>
                                                        <div class="flex flex-wrap gap-2 text-xs">
                                                            <span class="rounded-full border border-border px-3 py-1">{module.kind}</span>
                                                            <span class="rounded-full border border-border px-3 py-1">{module.source}</span>
                                                            <span class="rounded-full border border-border px-3 py-1">
                                                                {if module.enabled { "enabled" } else { "disabled" }}
                                                            </span>
                                                        </div>
                                                    </div>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </section>
                        }
                        .into_any(),
                        Err(err) => view! {
                            <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-5 py-4 text-sm text-destructive">
                                {format!("Failed to load tenant bootstrap: {err}")}
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
fn InfoCard(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="rounded-2xl border border-border bg-card p-6 shadow-sm">
            <div class="text-sm text-muted-foreground">{label}</div>
            <div class="mt-2 text-lg font-semibold text-card-foreground">{value}</div>
        </div>
    }
}
