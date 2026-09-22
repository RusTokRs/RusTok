use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;

use crate::core::{CONNECTOR_MODES, is_known_mode, parse_addresses};
use crate::model::IggyConnectorForm;
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
pub fn IggyConnectorAdmin() -> impl IntoView {
    let locale = use_context::<UiRouteContext>()
        .and_then(|context| context.locale)
        .unwrap_or_else(|| "en".to_string());
    let is_ru = locale.starts_with("ru");
    let token = use_token();
    let tenant = use_tenant();
    let configuration = local_resource(
        move || (token.get(), tenant.get()),
        move |(token, tenant)| async move { transport::fetch_configuration(token, tenant).await },
    );

    let (mode, set_mode) = signal("bundled".to_string());
    let (addresses, set_addresses) = signal(String::new());
    let (username, set_username) = signal(String::new());
    let (password_resolver, set_password_resolver) = signal("env".to_string());
    let (password_key, set_password_key) = signal(String::new());
    let (tls_enabled, set_tls_enabled) = signal(false);
    let (tls_domain, set_tls_domain) = signal(String::new());
    let (loaded, set_loaded) = signal(false);
    let (saving, set_saving) = signal(false);
    let (save_result, set_save_result) = signal(Option::<Result<String, String>>::None);

    Effect::new(move |_| {
        if loaded.get_untracked() {
            return;
        }
        if let Some(Ok(current)) = configuration.get() {
            if is_known_mode(&current.desired_mode) {
                set_mode.set(current.desired_mode);
            }
            set_addresses.set(current.external_addresses.join("\n"));
            set_username.set(current.external_username);
            if current.password_resolver != "deployment" {
                set_password_resolver.set(current.password_resolver);
                set_password_key.set(current.password_key);
            }
            set_tls_enabled.set(current.tls_enabled);
            set_tls_domain.set(current.tls_domain.unwrap_or_default());
            set_loaded.set(true);
        }
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
                                                set_mode.set(value.to_string());
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
                            on:input=move |event| set_addresses.set(event_target_value(&event))
                        />
                        <span class="block text-xs text-muted-foreground">"One host:port per line."</span>
                    </label>
                    <label class="space-y-2">
                        <span class="text-sm font-medium">"Username"</span>
                        <input
                            class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                            prop:value=move || username.get()
                            on:input=move |event| set_username.set(event_target_value(&event))
                        />
                    </label>
                    <label class="space-y-2">
                        <span class="text-sm font-medium">"Secret resolver"</span>
                        <select
                            class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                            prop:value=move || password_resolver.get()
                            on:change=move |event| set_password_resolver.set(event_target_value(&event))
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
                            on:input=move |event| set_password_key.set(event_target_value(&event))
                        />
                        <span class="block text-xs text-muted-foreground">
                            "Only the reference is stored. The password value never enters the database or UI."
                        </span>
                    </label>
                    <label class="flex items-center gap-3">
                        <input
                            type="checkbox"
                            prop:checked=move || tls_enabled.get()
                            on:change=move |event| set_tls_enabled.set(event_target_checked(&event))
                        />
                        <span class="text-sm font-medium">"Enable TLS"</span>
                    </label>
                    <Show when=move || tls_enabled.get()>
                        <label class="space-y-2">
                            <span class="text-sm font-medium">"TLS server name (optional)"</span>
                            <input
                                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                prop:value=move || tls_domain.get()
                                on:input=move |event| set_tls_domain.set(event_target_value(&event))
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
