use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;

use crate::core::{CONNECTOR_MODES, is_known_mode, parse_addresses};
use crate::model::IggyConnectorForm;
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

/// The connector fields the operator has changed. `None` means "unchanged", so the value the
/// backend resolved is what the form shows.
#[derive(Clone, Debug, Default)]
struct ConnectorEdits {
    mode: Option<String>,
    addresses: Option<String>,
    username: Option<String>,
    password_resolver: Option<String>,
    password_key: Option<String>,
    tls_enabled: Option<bool>,
    tls_domain: Option<String>,
}

fn backend_addresses(current: &crate::model::IggyConnectorConfiguration) -> String {
    current.external_addresses.join("\n")
}

fn backend_mode(current: &crate::model::IggyConnectorConfiguration) -> String {
    if is_known_mode(&current.desired_mode) {
        current.desired_mode.clone()
    } else {
        "bundled".to_string()
    }
}

/// A `deployment` resolver is owned by the deployment, not the operator: the form has always shown
/// the resolver menu's `env` value and an empty secret key for it, and still saves that back.
fn backend_password_resolver(current: &crate::model::IggyConnectorConfiguration) -> String {
    if current.password_resolver == "deployment" {
        "env".to_string()
    } else {
        current.password_resolver.clone()
    }
}

fn backend_password_key(current: &crate::model::IggyConnectorConfiguration) -> String {
    if current.password_resolver == "deployment" {
        String::new()
    } else {
        current.password_key.clone()
    }
}

#[component]
pub fn IggyConnectorAdmin() -> impl IntoView {
    let locale = use_context::<UiRouteContext>()
        .and_then(|context| context.locale)
        .unwrap_or_else(|| "en".to_string());
    let is_ru = locale.starts_with("ru");
    let token = use_token();
    let tenant = use_tenant();
    let configuration = loaded_resource(
        move || (token.get(), tenant.get()),
        move |(token, tenant)| async move { transport::fetch_configuration(token, tenant).await },
    );

    // The form is the resolved configuration with the operator's edits laid over it, not a set of
    // cells an `Effect` fills after the first render. An `Effect` is not driven before the server
    // produces the response, so a form built that way arrives empty.
    let (edits, set_edits) = signal(ConnectorEdits::default());
    let (saving, set_saving) = signal(false);
    let (save_result, set_save_result) = signal(Option::<Result<String, String>>::None);

    let resolved = move || configuration.get().and_then(Result::ok);
    // `Copy`, not `move`: every input captures it, and a moved closure would be `FnOnce`.
    let field = |read: fn(&ConnectorEdits) -> Option<String>,
                 backend: fn(&crate::model::IggyConnectorConfiguration) -> String| {
        let edited = edits.get();
        read(&edited).or_else(|| resolved().map(|current| backend(&current)))
    };

    let mode = Signal::derive(move || {
        let edited = edits.get();
        edited
            .mode
            .clone()
            .or_else(|| resolved().map(|current| backend_mode(&current)))
            .unwrap_or_else(|| "bundled".to_string())
    });
    let addresses = Signal::derive(move || field(|edits| edits.addresses.clone(), backend_addresses));
    let username = Signal::derive(move || {
        field(|edits| edits.username.clone(), |current| {
            current.external_username.clone()
        })
    });
    let password_resolver = Signal::derive(move || {
        field(
            |edits| edits.password_resolver.clone(),
            backend_password_resolver,
        )
        .unwrap_or_else(|| "env".to_string())
    });
    let password_key = Signal::derive(move || {
        field(|edits| edits.password_key.clone(), backend_password_key)
    });
    let tls_enabled = Signal::derive(move || {
        let edited = edits.get();
        edited
            .tls_enabled
            .or_else(|| resolved().map(|current| current.tls_enabled))
            .unwrap_or(false)
    });
    let tls_domain = Signal::derive(move || {
        field(|edits| edits.tls_domain.clone(), |current| {
            current.tls_domain.clone().unwrap_or_default()
        })
    });

    let save = move |_| {
        let token = token.get();
        let tenant = tenant.get();
        let input = IggyConnectorForm {
            mode: mode.get(),
            external_addresses: parse_addresses(&addresses.get()),
            external_username: username.get(),
            password_resolver: password_resolver.get(),
            password_key: password_key.get(),
            tls_enabled: tls_enabled.get(),
            tls_domain: {
                let value = tls_domain.get();
                (!value.trim().is_empty()).then_some(value)
            },
        };
        set_saving.set(true);
        set_save_result.set(None);
        spawn_local(async move {
            let result = transport::update_configuration(token, tenant, input)
                .await
                .map(|outcome| {
                    if outcome.restart_required {
                        "Saved. Restart the server to activate this connector mode.".to_string()
                    } else {
                        "Saved. This connector mode is already active.".to_string()
                    }
                })
                .map_err(|error| error.to_string());
            set_save_result.set(Some(result));
            set_saving.set(false);
        });
    };

    let title = if is_ru {
        "Коннектор Iggy"
    } else {
        "Iggy Connector"
    };
    let subtitle = if is_ru {
        "Выберите встроенный Iggy или подключение к внешнему кластеру."
    } else {
        "Choose bundled Iggy or connect to an external deployment."
    };

    view! {
        <section class="flex flex-1 flex-col gap-6 p-4 md:px-6">
            <header class="rounded-xl border border-border bg-card p-6 shadow-sm">
                <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                    "Capability module"
                </p>
                <h1 class="mt-2 text-2xl font-semibold text-card-foreground">{title}</h1>
                <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{subtitle}</p>
            </header>

            <Suspense fallback=move || view! { <div class="h-48 animate-pulse rounded-xl bg-muted"></div> }>
                {move || configuration.get().map(|result| match result {
                    Ok(current) => view! {
                        <div class="space-y-5 rounded-xl border border-border bg-card p-6 shadow-sm">
                            <div class="grid gap-3 md:grid-cols-2">
                                {CONNECTOR_MODES.into_iter().map(|(value, label, description)| {
                                    let unavailable = value == "bundled" && !current.bundled_available;
                                    view! {
                                        <button
                                            type="button"
                                            disabled=unavailable
                                            class=move || if mode.get() == value {
                                                "rounded-xl border border-primary bg-primary/5 p-4 text-left disabled:cursor-not-allowed disabled:opacity-50"
                                            } else {
                                                "rounded-xl border border-border bg-background p-4 text-left hover:border-primary/50 disabled:cursor-not-allowed disabled:opacity-50"
                                            }
                                            on:click=move |_| {
                                                set_edits.update(|edits| {
                                                    edits.mode = Some(value.to_string());
                                                });
                                                set_save_result.set(None);
                                            }
                                        >
                                            <span class="block font-semibold text-card-foreground">{label}</span>
                                            <span class="mt-1 block text-sm text-muted-foreground">{description}</span>
                                        </button>
                                    }
                                }).collect_view()}
                            </div>

                            <dl class="grid gap-2 text-sm sm:grid-cols-2 lg:max-w-2xl">
                                <dt class="text-muted-foreground">"Active mode"</dt>
                                <dd class="font-mono">{current.active_mode}</dd>
                                <dt class="text-muted-foreground">"Desired mode"</dt>
                                <dd class="font-mono">{current.desired_mode}</dd>
                                <dt class="text-muted-foreground">"Readiness"</dt>
                                <dd>{if current.configured { "Ready" } else { "Configuration required" }}</dd>
                            </dl>

                            {current.configuration_error.map(|error| view! {
                                <div class="rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-sm text-amber-800">
                                    {error}
                                </div>
                            })}
                        </div>
                    }.into_any(),
                    Err(error) => view! {
                        <div class="rounded-xl border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
                            {error.to_string()}
                        </div>
                    }.into_any(),
                })}
            </Suspense>

            <Show when=move || mode.get() == "external">
                <div class="grid gap-5 rounded-xl border border-border bg-card p-6 shadow-sm md:grid-cols-2">
                    <label class="space-y-2 md:col-span-2">
                        <span class="text-sm font-medium">"Iggy addresses"</span>
                        <textarea
                            class="min-h-24 w-full rounded-lg border border-input bg-background px-3 py-2 font-mono text-sm"
                            placeholder="iggy.example.com:8090"
                            prop:value=move || addresses.get()
                            on:input=move |event| set_edits.update(|edits| edits.addresses = Some(event_target_value(&event)))
                        />
                        <span class="block text-xs text-muted-foreground">"One host:port per line."</span>
                    </label>
                    <label class="space-y-2">
                        <span class="text-sm font-medium">"Username"</span>
                        <input
                            class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                            prop:value=move || username.get()
                            on:input=move |event| set_edits.update(|edits| edits.username = Some(event_target_value(&event)))
                        />
                    </label>
                    <label class="space-y-2">
                        <span class="text-sm font-medium">"Secret resolver"</span>
                        <select
                            class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                            prop:value=move || password_resolver.get()
                            on:change=move |event| set_edits.update(|edits| edits.password_resolver = Some(event_target_value(&event)))
                        >
                            <option value="env">"Environment variable"</option>
                            <option value="mounted_file">"Mounted file"</option>
                        </select>
                    </label>
                    <label class="space-y-2 md:col-span-2">
                        <span class="text-sm font-medium">"Password secret key"</span>
                        <input
                            class="w-full rounded-lg border border-input bg-background px-3 py-2 font-mono text-sm"
                            placeholder="RUSTOK_IGGY_PASSWORD"
                            prop:value=move || password_key.get()
                            on:input=move |event| set_edits.update(|edits| edits.password_key = Some(event_target_value(&event)))
                        />
                        <span class="block text-xs text-muted-foreground">
                            "Only the reference is stored. The password value never enters the database or UI."
                        </span>
                    </label>
                    <label class="flex items-center gap-3">
                        <input
                            type="checkbox"
                            prop:checked=move || tls_enabled.get()
                            on:change=move |event| set_edits.update(|edits| edits.tls_enabled = Some(event_target_checked(&event)))
                        />
                        <span class="text-sm font-medium">"Enable TLS"</span>
                    </label>
                    <Show when=move || tls_enabled.get()>
                        <label class="space-y-2">
                            <span class="text-sm font-medium">"TLS server name (optional)"</span>
                            <input
                                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                prop:value=move || tls_domain.get()
                                on:input=move |event| set_edits.update(|edits| edits.tls_domain = Some(event_target_value(&event)))
                            />
                        </label>
                    </Show>
                </div>
            </Show>

            <div class="flex flex-wrap items-center gap-4">
                <button
                    type="button"
                    class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-50"
                    disabled=move || saving.get()
                    on:click=save
                >
                    {move || if saving.get() { "Saving..." } else { "Save connector settings" }}
                </button>
                {move || save_result.get().map(|result| match result {
                    Ok(message) => view! { <span class="text-sm text-emerald-600">{message}</span> }.into_any(),
                    Err(error) => view! { <span class="text-sm text-destructive">{error}</span> }.into_any(),
                })}
            </div>
        </section>
    }
}
