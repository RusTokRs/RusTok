use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;

use crate::core::{OutboxAdminShellText, outbox_info_cards};
use crate::i18n::t;
use crate::transport;

/// Builds a resource the response resolves.
///
/// The module admin mounts inside the layout route that declares `SsrMode::Async`, so a
/// `LocalResource` here is `pending()` on the server and the reader is served the `Suspense`
/// fallback. See `DECISIONS/2026-11-13-standalone-admin-editor-delivery.md`.
fn loaded_resource<S, Fut, T>(
    source: impl Fn() -> S + Send + Sync + 'static,
    fetcher: impl Fn(S) -> Fut + Send + Sync + 'static,
) -> Resource<T>
where
    S: PartialEq + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = T> + Send + 'static,
    T: Send + Sync + 'static,
{
    Resource::new(source, fetcher)
}

#[component]
pub fn OutboxAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let shell = shell_text(locale.as_deref());

    let bootstrap = loaded_resource(
        move || (token.get(), tenant.get()),
        move |_| async move { transport::fetch_bootstrap().await },
    );

    let outbox_bootstrap = bootstrap;
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
                    outbox_bootstrap.get().map(|result| match result {
                        Ok(bootstrap) => {
                            let text = shell.clone();
                            let cards = outbox_info_cards(&bootstrap, &text);
                            view! {
                                <section class="grid gap-4 lg:grid-cols-3 xl:grid-cols-4">
                                    {cards
                                        .into_iter()
                                        .map(|card| view! { <InfoCard label=card.label value=card.value /> })
                                        .collect_view()}
                                </section>

                                <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                                    <h2 class="text-lg font-semibold text-card-foreground">
                                        {text.relay_notes_title}
                                    </h2>
                                    <ul class="mt-3 space-y-2 text-sm text-muted-foreground">
                                        {bootstrap
                                            .relay_notes
                                            .into_iter()
                                            .map(|note| view! { <li>{note}</li> })
                                            .collect_view()}
                                    </ul>
                                </section>
                            }
                            .into_any()
                        }
                        Err(err) => view! {
                            <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-5 py-4 text-sm text-destructive">
                                {format!("{}: {err}", shell.load_error_prefix.clone())}
                            </div>
                        }
                        .into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

fn shell_text(locale: Option<&str>) -> OutboxAdminShellText {
    OutboxAdminShellText {
        badge: t(locale, "outbox.badge", "outbox"),
        title: t(locale, "outbox.title", "Outbox Relay"),
        subtitle: t(
            locale,
            "outbox.subtitle",
            "Module-owned overview for transactional event persistence, retry pressure, and relay health.",
        ),
        health_label: t(locale, "outbox.health", "Health"),
        tenant_context_label: t(locale, "outbox.tenantContext", "Tenant context"),
        global_tenant_label: t(locale, "outbox.global", "global"),
        relay_notes_title: t(locale, "outbox.notes.title", "Relay Notes"),
        load_error_prefix: t(
            locale,
            "outbox.error.loadBootstrap",
            "Failed to load outbox bootstrap",
        ),
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
