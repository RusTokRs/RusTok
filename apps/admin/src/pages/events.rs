use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_transport::UiTransportPath;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::app::providers::enabled_modules::use_enabled_modules;

use crate::shared::api::queries::{
    EVENTS_STATUS_QUERY, PLATFORM_SETTINGS_QUERY, UPDATE_PLATFORM_SETTINGS_MUTATION,
};
use crate::shared::api::request;
use crate::shared::ui::{Alert, AlertVariant, Button, Input, PageHeader};
use crate::{t_string, use_i18n};

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct EventsStatusResponse {
    #[serde(rename = "eventsStatus")]
    pub(super) events_status: EventsStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct EventsStatus {
    #[serde(rename = "configuredTransport")]
    pub(super) configured_transport: String,
    #[serde(rename = "iggyMode")]
    pub(super) iggy_mode: String,
    #[serde(rename = "relayIntervalMs")]
    pub(super) relay_interval_ms: u64,
    #[serde(rename = "dlqEnabled")]
    pub(super) dlq_enabled: bool,
    #[serde(rename = "maxAttempts")]
    pub(super) max_attempts: i32,
    #[serde(rename = "pendingEvents")]
    pub(super) pending_events: i64,
    #[serde(rename = "dlqEvents")]
    pub(super) dlq_events: i64,
    #[serde(rename = "availableTransports")]
    pub(super) available_transports: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct PlatformSettingsResponse {
    #[serde(rename = "platformSettings")]
    pub(super) platform_settings: PlatformSettingsPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct PlatformSettingsPayload {
    pub(super) settings: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct UpdateSettingsResponse {
    #[serde(rename = "updatePlatformSettings")]
    update_platform_settings: UpdateSettingsPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct UpdateSettingsPayload {
    success: bool,
}

#[derive(Clone, Debug, Serialize)]
struct PlatformSettingsVariables {
    category: String,
}

#[derive(Clone, Debug, Serialize)]
struct UpdateSettingsVariables {
    input: UpdateSettingsInput,
}

#[derive(Clone, Debug, Serialize)]
struct UpdateSettingsInput {
    category: String,
    settings: String,
}

#[derive(Clone, Debug, Serialize)]
struct EmptyVariables {}

async fn fetch_events_status_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<EventsStatusResponse, crate::shared::api::ApiError> {
    request::<EmptyVariables, EventsStatusResponse>(
        EVENTS_STATUS_QUERY,
        EmptyVariables {},
        token,
        tenant_slug,
    )
    .await
}

async fn fetch_events_status_server() -> Result<EventsStatusResponse, ServerFnError> {
    super::native_server_adapter::events_status_native().await
}

async fn fetch_events_status(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<EventsStatusResponse, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => fetch_events_status_server()
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => fetch_events_status_graphql(token, tenant_slug)
            .await
            .map_err(|error| error.to_string()),
    }
}

async fn fetch_platform_settings_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PlatformSettingsResponse, crate::shared::api::ApiError> {
    request::<PlatformSettingsVariables, PlatformSettingsResponse>(
        PLATFORM_SETTINGS_QUERY,
        PlatformSettingsVariables {
            category: "events".to_string(),
        },
        token,
        tenant_slug,
    )
    .await
}

async fn fetch_platform_settings_server() -> Result<PlatformSettingsResponse, ServerFnError> {
    super::native_server_adapter::event_settings_native().await
}

async fn fetch_platform_settings(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PlatformSettingsResponse, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => fetch_platform_settings_server()
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => fetch_platform_settings_graphql(token, tenant_slug)
            .await
            .map_err(|error| error.to_string()),
    }
}

#[component]
pub fn EventsPage() -> impl IntoView {
    let i18n = use_i18n();
    let token = use_token();
    let tenant = use_tenant();
    let enabled_modules = use_enabled_modules();

    // Runtime status from server
    let status_resource = local_resource(
        move || (token.get(), tenant.get()),
        move |(t, tn)| async move { fetch_events_status(t, tn).await },
    );

    // Desired settings from DB
    let settings_resource = local_resource(
        move || (token.get(), tenant.get()),
        move |(t, tn)| async move { fetch_platform_settings(t, tn).await },
    );

    // Form state for desired transport and external Iggy settings.
    let (selected_transport, set_selected_transport) = signal(String::new());
    let (relay_interval_ms, set_relay_interval_ms) = signal(String::from("1000"));
    let (max_attempts, set_max_attempts) = signal(String::from("5"));
    let (dlq_enabled, set_dlq_enabled) = signal(true);
    let (iggy_addresses, set_iggy_addresses) = signal(String::from("127.0.0.1:8090"));
    let (iggy_protocol, set_iggy_protocol) = signal(String::from("tcp"));
    let (iggy_username, set_iggy_username) = signal(String::from("iggy"));
    let (iggy_password, set_iggy_password) = signal(String::new());
    let (iggy_tls, set_iggy_tls) = signal(false);
    let (iggy_stream, set_iggy_stream) = signal(String::from("rustok"));
    let (iggy_partitions, set_iggy_partitions) = signal(String::from("8"));
    let (iggy_replication, set_iggy_replication) = signal(String::from("1"));
    let (loaded, set_loaded) = signal(false);
    let (saving, set_saving) = signal(false);
    let (save_result, set_save_result) = signal(Option::<Result<bool, String>>::None);

    // Populate form from DB settings
    Effect::new(move |_| {
        if let Some(Ok(resp)) = settings_resource.get() {
            if !loaded.get_untracked() {
                if let Ok(val) = serde_json::from_str::<Value>(&resp.platform_settings.settings) {
                    if let Some(t) = val.get("transport").and_then(|v| v.as_str()) {
                        set_selected_transport.set(t.to_string());
                    }
                    if let Some(v) = val.get("relay_interval_ms").and_then(|v| v.as_u64()) {
                        set_relay_interval_ms.set(v.to_string());
                    }
                    if let Some(v) = val.get("max_attempts").and_then(|v| v.as_i64()) {
                        set_max_attempts.set(v.to_string());
                    }
                    if let Some(v) = val.get("dlq_enabled").and_then(|v| v.as_bool()) {
                        set_dlq_enabled.set(v);
                    }
                    if let Some(v) = val.get("iggy_addresses").and_then(|v| v.as_str()) {
                        set_iggy_addresses.set(v.to_string());
                    }
                    if let Some(v) = val.get("iggy_protocol").and_then(|v| v.as_str()) {
                        set_iggy_protocol.set(v.to_string());
                    }
                    if let Some(v) = val.get("iggy_username").and_then(|v| v.as_str()) {
                        set_iggy_username.set(v.to_string());
                    }
                    if let Some(v) = val.get("iggy_tls").and_then(|v| v.as_bool()) {
                        set_iggy_tls.set(v);
                    }
                    if let Some(v) = val.get("iggy_stream").and_then(|v| v.as_str()) {
                        set_iggy_stream.set(v.to_string());
                    }
                    if let Some(v) = val.get("iggy_partitions").and_then(|v| v.as_u64()) {
                        set_iggy_partitions.set(v.to_string());
                    }
                    if let Some(v) = val.get("iggy_replication").and_then(|v| v.as_u64()) {
                        set_iggy_replication.set(v.to_string());
                    }
                }
                // Fall back to runtime transport if nothing in DB
                if selected_transport.get_untracked().is_empty() {
                    if let Some(Ok(status)) = status_resource.get() {
                        set_selected_transport
                            .set(status.events_status.configured_transport.clone());
                    }
                }
                set_loaded.set(true);
            }
        }
    });

    // All 4 transports always shown; iggy_enabled drives the warning
    let iggy_enabled = Signal::derive(move || {
        let modules = enabled_modules.get();
        modules.iter().any(|s| s.to_lowercase().contains("iggy"))
    });

    let available = Signal::derive(move || {
        vec![
            (
                "memory".to_string(),
                t_string!(i18n, events.transport.memory).to_string(),
            ),
            (
                "outbox".to_string(),
                t_string!(i18n, events.transport.outbox).to_string(),
            ),
            (
                "iggy_embedded".to_string(),
                t_string!(i18n, events.transport.iggyEmbedded).to_string(),
            ),
            (
                "iggy_external".to_string(),
                t_string!(i18n, events.transport.iggyExternal).to_string(),
            ),
        ]
    });

    let show_iggy_warning =
        Signal::derive(move || selected_transport.get().starts_with("iggy") && !iggy_enabled.get());

    let show_outbox_settings = Signal::derive(move || {
        let t = selected_transport.get();
        t == "outbox" || t == "iggy_embedded" || t == "iggy_external"
    });

    let show_iggy_external = Signal::derive(move || selected_transport.get() == "iggy_external");

    let save = move |_| {
        let token_val = token.get();
        let tenant_val = tenant.get();
        let settings = serde_json::json!({
            "transport": selected_transport.get(),
            "relay_interval_ms": relay_interval_ms.get().parse::<u64>().unwrap_or(1000),
            "max_attempts": max_attempts.get().parse::<i32>().unwrap_or(5),
            "dlq_enabled": dlq_enabled.get(),
            "iggy_addresses": iggy_addresses.get(),
            "iggy_protocol": iggy_protocol.get(),
            "iggy_username": iggy_username.get(),
            "iggy_password": iggy_password.get(),
            "iggy_tls": iggy_tls.get(),
            "iggy_stream": iggy_stream.get(),
            "iggy_partitions": iggy_partitions.get().parse::<u32>().unwrap_or(8),
            "iggy_replication": iggy_replication.get().parse::<u8>().unwrap_or(1),
        });

        set_saving.set(true);
        set_save_result.set(None);
        spawn_local(async move {
            let result = request::<UpdateSettingsVariables, UpdateSettingsResponse>(
                UPDATE_PLATFORM_SETTINGS_MUTATION,
                UpdateSettingsVariables {
                    input: UpdateSettingsInput {
                        category: "events".to_string(),
                        settings: settings.to_string(),
                    },
                },
                token_val,
                tenant_val,
            )
            .await;
            match result {
                Ok(r) => set_save_result.set(Some(Ok(r.update_platform_settings.success))),
                Err(e) => set_save_result.set(Some(Err(format!("{:?}", e)))),
            }
            set_saving.set(false);
        });
    };

    view! {
        <section class="flex flex-1 flex-col gap-6 p-4 md:px-6">
            <PageHeader
                title=t_string!(i18n, events.title)
                subtitle=t_string!(i18n, events.subtitle).to_string()
                eyebrow=t_string!(i18n, events.eyebrow).to_string()
            />

            // ── Transport selector ────────────────────────────────────────────
            <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                    {move || t_string!(i18n, events.transport.label)}
                </h4>
                <Suspense fallback=move || view! {
                    <div class="h-10 animate-pulse rounded-lg bg-muted" />
                }>
                    {move || {
                        let opts = available.get();
                        let current = selected_transport.get();
                        view! {
                            <select
                                class="w-full max-w-sm rounded-lg border border-input bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-ring"
                                on:change=move |e| set_selected_transport.set(event_target_value(&e))
                            >
                                {opts.into_iter().map(|(value, label)| {
                                    let selected = value == current;
                                    view! {
                                        <option value=value selected=selected>{label}</option>
                                    }
                                }).collect_view()}
                            </select>
                        }.into_any()
                    }}
                </Suspense>
                <p class="mt-2 text-xs text-amber-600">
                    {move || t_string!(i18n, events.transport.restartRequired)}
                </p>
                <Show when=move || show_iggy_warning.get()>
                    <Alert variant=AlertVariant::Warning class="mt-3">
                        {move || t_string!(i18n, events.transport.moduleDisabledWarning)}
                    </Alert>
                </Show>
            </div>

            // ── Outbox settings ───────────────────────────────────────────────
            <Show when=move || show_outbox_settings.get()>
                <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                    <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                        {move || t_string!(i18n, events.outbox.title)}
                    </h4>
                    <div class="grid gap-4 sm:grid-cols-2 max-w-xl">
                        <Input
                            value=relay_interval_ms
                            set_value=set_relay_interval_ms
                            placeholder="1000"
                            label=move || t_string!(i18n, events.outbox.relayIntervalMs)
                        />
                        <Input
                            value=max_attempts
                            set_value=set_max_attempts
                            placeholder="5"
                            label=move || t_string!(i18n, events.outbox.maxAttempts)
                        />
                    </div>
                    <div class="mt-4 flex items-center gap-3">
                        <input
                            type="checkbox"
                            id="dlq-enabled"
                            class="h-4 w-4 rounded border-input"
                            prop:checked=dlq_enabled
                            on:change=move |e| set_dlq_enabled.set(event_target_checked(&e))
                        />
                        <label for="dlq-enabled" class="text-sm text-foreground">
                            {move || t_string!(i18n, events.outbox.dlqEnabled)}
                        </label>
                    </div>
                </div>
            </Show>

            // ── External Iggy form ────────────────────────────────────────────
            <Show when=move || show_iggy_external.get()>
                <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                    <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                        {move || t_string!(i18n, events.iggy.title)}
                    </h4>
                    <div class="grid gap-4 sm:grid-cols-2 max-w-xl">
                        <div class="sm:col-span-2">
                            <Input
                                value=iggy_addresses
                                set_value=set_iggy_addresses
                                placeholder="127.0.0.1:8090"
                                label=move || t_string!(i18n, events.iggy.addresses)
                            />
                            <p class="mt-1 text-xs text-muted-foreground">"Comma-separated list of addresses"</p>
                        </div>
                        <div>
                            <label class="mb-1 block text-xs font-medium text-muted-foreground">
                                {move || t_string!(i18n, events.iggy.protocol)}
                            </label>
                            <select
                                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-ring"
                                on:change=move |e| set_iggy_protocol.set(event_target_value(&e))
                            >
                                <option value="tcp" selected=move || iggy_protocol.get() == "tcp">"TCP"</option>
                                <option value="http" selected=move || iggy_protocol.get() == "http">"HTTP"</option>
                            </select>
                        </div>
                        <Input
                            value=iggy_username
                            set_value=set_iggy_username
                            placeholder="iggy"
                            label=move || t_string!(i18n, events.iggy.username)
                        />
                        <Input
                            value=iggy_password
                            set_value=set_iggy_password
                            placeholder="••••••••"
                            type_="password"
                            label=move || t_string!(i18n, events.iggy.password)
                        />
                        <div class="flex items-center gap-3 pt-4">
                            <input
                                type="checkbox"
                                id="iggy-tls"
                                class="h-4 w-4 rounded border-input"
                                prop:checked=iggy_tls
                                on:change=move |e| set_iggy_tls.set(event_target_checked(&e))
                            />
                            <label for="iggy-tls" class="text-sm text-foreground">
                                {move || t_string!(i18n, events.iggy.tlsEnabled)}
                            </label>
                        </div>
                        <Input
                            value=iggy_stream
                            set_value=set_iggy_stream
                            placeholder="rustok"
                            label=move || t_string!(i18n, events.iggy.stream)
                        />
                        <Input
                            value=iggy_partitions
                            set_value=set_iggy_partitions
                            placeholder="8"
                            label=move || t_string!(i18n, events.iggy.partitions)
                        />
                        <Input
                            value=iggy_replication
                            set_value=set_iggy_replication
                            placeholder="1"
                            label=move || t_string!(i18n, events.iggy.replication)
                        />
                    </div>
                </div>
            </Show>

            // ── Save button ───────────────────────────────────────────────────
            <div class="flex items-center gap-4">
                <Button on_click=save disabled=saving.into()>
                    {move || if saving.get() {
                        t_string!(i18n, events.saving).to_string()
                    } else {
                        t_string!(i18n, events.save).to_string()
                    }}
                </Button>
                <Show when=move || save_result.get().is_some()>
                    {move || match save_result.get() {
                        Some(Ok(true)) => view! {
                            <span class="text-sm text-green-600">
                                {t_string!(i18n, events.saved)}
                            </span>
                        }.into_any(),
                        Some(Err(e)) => view! {
                            <span class="text-sm text-destructive">{e}</span>
                        }.into_any(),
                        _ => view! { <span /> }.into_any(),
                    }}
                </Show>
            </div>

            // ── Runtime status ────────────────────────────────────────────────
            <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                    {move || t_string!(i18n, events.status.title)}
                </h4>
                <Suspense fallback=move || view! {
                    <div class="space-y-2">
                        {(0..4).map(|_| view! { <div class="h-6 animate-pulse rounded bg-muted" /> }).collect_view()}
                    </div>
                }>
                    {move || match status_resource.get() {
                        None => view! { <div /> }.into_any(),
                        Some(Ok(resp)) => {
                            let s = resp.events_status;
                            view! {
                                <dl class="grid grid-cols-2 gap-x-4 gap-y-3 text-sm max-w-sm">
                                    <dt class="text-muted-foreground">
                                        {t_string!(i18n, events.status.transport)}
                                    </dt>
                                    <dd class="font-mono font-medium text-foreground">
                                        {s.configured_transport}
                                    </dd>
                                    <dt class="text-muted-foreground">
                                        {t_string!(i18n, events.status.pendingEvents)}
                                    </dt>
                                    <dd class="font-medium text-foreground">{s.pending_events}</dd>
                                    <dt class="text-muted-foreground">
                                        {t_string!(i18n, events.status.dlqEvents)}
                                    </dt>
                                    <dd class="font-medium text-foreground">{s.dlq_events}</dd>
                                    <dt class="text-muted-foreground">
                                        {t_string!(i18n, events.status.relayInterval)}
                                    </dt>
                                    <dd class="font-medium text-foreground">
                                        {s.relay_interval_ms} " ms"
                                    </dd>
                                </dl>
                            }.into_any()
                        }
                        Some(Err(err)) => view! {
                            <Alert variant=AlertVariant::Destructive>
                                {err}
                            </Alert>
                        }.into_any(),
                    }}
                </Suspense>
            </div>
        </section>
    }
}
