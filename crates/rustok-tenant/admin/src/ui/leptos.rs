use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;

use crate::{core, i18n::t, transport};

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
pub fn TenantAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;

    let bootstrap = local_resource(
        move || (token.get(), tenant.get()),
        move |_| async move { transport::fetch_bootstrap().await },
    );

    let shell = core::shell_copy(
        t(locale.as_deref(), "tenant.badge", "tenant"),
        t(locale.as_deref(), "tenant.title", "Tenant Runtime"),
        t(
            locale.as_deref(),
            "tenant.subtitle",
            "Module-owned overview for active tenant state and effective module enablement.",
        ),
    );
    let modules_copy = core::modules_copy(
        t(
            locale.as_deref(),
            "tenant.modules.title",
            "Registered Modules",
        ),
        t(
            locale.as_deref(),
            "tenant.modules.subtitle",
            "Core modules stay enabled by contract; optional modules reflect tenant-side state.",
        ),
        t(locale.as_deref(), "tenant.modules.updated", "Updated"),
        t(locale.as_deref(), "tenant.modules.enabled", "enabled"),
        t(locale.as_deref(), "tenant.modules.disabled", "disabled"),
    );
    let error_copy = core::error_copy(t(
        locale.as_deref(),
        "tenant.error.loadBootstrap",
        "Failed to load tenant bootstrap",
    ));

    view! {
        <div class="space-y-6">
            <header class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-2">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        {shell.badge.clone()}
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">
                        {shell.title.clone()}
                    </h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        {shell.subtitle.clone()}
                    </p>
                </div>
            </header>

            <Suspense fallback=move || view! { <div class="h-32 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || {
                    bootstrap.get().map(|result| match result {
                        Ok(bootstrap) => {
                            let info = core::info_cards(
                                &bootstrap,
                                t(locale.as_deref(), "tenant.info.tenant", "Tenant"),
                                t(locale.as_deref(), "tenant.info.name", "Name"),
                                t(locale.as_deref(), "tenant.info.domain", "Domain"),
                                t(locale.as_deref(), "tenant.info.status", "Status"),
                                t(locale.as_deref(), "tenant.value.notAvailable", "n/a"),
                                t(locale.as_deref(), "tenant.value.active", "active"),
                                t(locale.as_deref(), "tenant.value.inactive", "inactive"),
                            );
                            view! {
                            <section class="grid gap-4 lg:grid-cols-4">
                                <InfoCard label=info.tenant_label value=bootstrap.tenant.slug.clone() />
                                <InfoCard label=info.name_label value=bootstrap.tenant.name.clone() />
                                <InfoCard label=info.domain_label value=info.domain_value />
                                <InfoCard label=info.status_label value=info.status_value />
                            </section>

                            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                                <div class="flex items-center justify-between gap-4">
                                    <div>
                                        <h2 class="text-lg font-semibold text-card-foreground">
                                            {modules_copy.title.clone()}
                                        </h2>
                                        <p class="text-sm text-muted-foreground">
                                            {modules_copy.subtitle.clone()}
                                        </p>
                                    </div>
                                    <div class="text-sm text-muted-foreground">
                                        {format!("{} {}", modules_copy.updated_prefix.clone(), bootstrap.tenant.updated_at)}
                                    </div>
                                </div>
                                <div class="mt-4 grid gap-3">
                                    {bootstrap
                                        .modules
                                        .into_iter()
                                        .map(|module| core::module_view_model(module, &modules_copy))
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
                                                            <span class="rounded-full border border-border px-3 py-1">{module.enabled_label}</span>
                                                        </div>
                                                    </div>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </section>
                        }
                        .into_any()
                        },
                        Err(err) => view! {
                            <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-5 py-4 text-sm text-destructive">
                                {core::load_bootstrap_error_message(&error_copy, err)}
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
