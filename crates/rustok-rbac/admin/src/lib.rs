mod api;
mod model;

use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};

#[component]
pub fn RbacAdmin() -> impl IntoView {
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
                        "rbac"
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">"RBAC Runtime"</h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        "Module-owned overview for the live permission snapshot and module-declared access vocabulary."
                    </p>
                </div>
            </header>

            <Suspense fallback=move || view! { <div class="h-32 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || {
                    bootstrap.get().map(|result| match result {
                        Ok(bootstrap) => view! {
                            <section class="grid gap-4 lg:grid-cols-3">
                                <InfoCard label="Tenant" value=bootstrap.tenant_slug.clone() />
                                <InfoCard label="Role" value=bootstrap.inferred_role.clone() />
                                <InfoCard label="User ID" value=bootstrap.current_user_id.clone() />
                            </section>

                            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                                <div class="flex items-center justify-between gap-4">
                                    <div>
                                        <h2 class="text-lg font-semibold text-card-foreground">"Granted Permissions"</h2>
                                        <p class="text-sm text-muted-foreground">
                                            "Live snapshot derived from the current security context."
                                        </p>
                                    </div>
                                    <div class="text-sm text-muted-foreground">
                                        {format!("{} permissions", bootstrap.granted_permissions.len())}
                                    </div>
                                </div>
                                <div class="mt-4 flex flex-wrap gap-2">
                                    {bootstrap
                                        .granted_permissions
                                        .into_iter()
                                        .map(|permission| view! {
                                            <span class="rounded-full border border-border bg-background px-3 py-1 text-xs text-muted-foreground">
                                                {permission}
                                            </span>
                                        })
                                        .collect_view()}
                                </div>
                            </section>

                            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                                <h2 class="text-lg font-semibold text-card-foreground">"Host Surfaces"</h2>
                                <div class="mt-4 flex flex-wrap gap-3">
                                    {bootstrap
                                        .host_surfaces
                                        .into_iter()
                                        .map(|surface| view! {
                                            <a
                                                href=surface.href
                                                class="inline-flex items-center rounded-lg border border-border bg-background px-4 py-2 text-sm text-card-foreground transition hover:bg-muted"
                                            >
                                                {surface.label}
                                            </a>
                                        })
                                        .collect_view()}
                                </div>
                            </section>

                            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                                <h2 class="text-lg font-semibold text-card-foreground">"Module Permission Catalog"</h2>
                                <div class="mt-4 grid gap-3">
                                    {bootstrap
                                        .module_permissions
                                        .into_iter()
                                        .map(|group| view! {
                                            <div class="rounded-xl border border-border bg-background px-4 py-3">
                                                <div class="font-medium text-card-foreground">{group.module_slug}</div>
                                                <div class="mt-2 flex flex-wrap gap-2">
                                                    {group
                                                        .permissions
                                                        .into_iter()
                                                        .map(|permission| view! {
                                                            <span class="rounded-full border border-border px-3 py-1 text-xs text-muted-foreground">
                                                                {permission}
                                                            </span>
                                                        })
                                                        .collect_view()}
                                                </div>
                                            </div>
                                        })
                                        .collect_view()}
                                </div>
                            </section>
                        }
                        .into_any(),
                        Err(err) => view! {
                            <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-5 py-4 text-sm text-destructive">
                                {format!("Failed to load RBAC bootstrap: {err}")}
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
            <div class="mt-2 text-lg font-semibold text-card-foreground break-all">{value}</div>
        </div>
    }
}
