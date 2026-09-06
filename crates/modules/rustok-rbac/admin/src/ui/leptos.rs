use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;

use crate::core::{build_rbac_admin_overview_view_model, format_rbac_admin_bootstrap_error};
use crate::i18n::t;
use crate::transport;

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
pub fn RbacAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;

    let bootstrap = local_resource(
        move || (token.get(), tenant.get()),
        move |_| async move { transport::fetch_bootstrap().await },
    );

    view! {
        <div class="space-y-6">
            <header class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-2">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        {t(locale.as_deref(), "rbac.badge", "rbac")}
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">
                        {t(locale.as_deref(), "rbac.title", "RBAC Runtime")}
                    </h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        {t(locale.as_deref(), "rbac.subtitle", "Module-owned overview for the live permission snapshot and module-declared access vocabulary.")}
                    </p>
                </div>
            </header>

            <Suspense fallback=move || view! { <div class="h-32 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || {
                    bootstrap.get().map(|result| match result {
                        Ok(bootstrap) => {
                            let view_model = build_rbac_admin_overview_view_model(
                                locale.as_deref(),
                                bootstrap,
                            );
                            let granted_permissions = view_model.granted_permissions;
                            view! {
                                <section class="grid gap-4 lg:grid-cols-3">
                                    {view_model
                                        .info_cards
                                        .into_iter()
                                        .map(|card| view! { <InfoCard label=card.label value=card.value /> })
                                        .collect_view()}
                                </section>

                                <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                                    <div class="flex items-center justify-between gap-4">
                                        <div>
                                            <h2 class="text-lg font-semibold text-card-foreground">
                                                {granted_permissions.title}
                                            </h2>
                                            <p class="text-sm text-muted-foreground">
                                                {granted_permissions.subtitle}
                                            </p>
                                        </div>
                                        <div class="text-sm text-muted-foreground">
                                            {granted_permissions.count_label}
                                        </div>
                                    </div>
                                    <div class="mt-4 flex flex-wrap gap-2">
                                        {granted_permissions
                                            .permissions
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
                                    <h2 class="text-lg font-semibold text-card-foreground">
                                        {t(locale.as_deref(), "rbac.roles.title", "Built-in Roles")}
                                    </h2>
                                    <div class="mt-4 grid gap-3">
                                        {view_model.roles.into_iter().map(|role| view! {
                                            <div class="rounded-xl border border-border bg-background px-4 py-3">
                                                <div class="flex items-center justify-between gap-3">
                                                    <span class="font-medium text-card-foreground">{role.display_name}</span>
                                                    <span class="font-mono text-xs text-muted-foreground">{role.slug}</span>
                                                </div>
                                                <div class="mt-2 flex flex-wrap gap-2">
                                                    {role.permissions.into_iter().map(|permission| view! {
                                                        <span class="rounded-full border border-border px-3 py-1 text-xs text-muted-foreground">{permission}</span>
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        }).collect_view()}
                                    </div>
                                </section>

                                <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                                    <h2 class="text-lg font-semibold text-card-foreground">
                                        {t(locale.as_deref(), "rbac.catalog.title", "Module Permission Catalog")}
                                    </h2>
                                    <div class="mt-4 grid gap-3">
                                        {view_model
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
                            .into_any()
                        }
                        Err(err) => view! {
                            <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-5 py-4 text-sm text-destructive">
                                {format_rbac_admin_bootstrap_error(locale.as_deref(), err)}
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
            <div class="mt-2 text-lg font-semibold text-card-foreground break-all">{value}</div>
        </div>
    }
}
