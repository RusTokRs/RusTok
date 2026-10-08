use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use serde_json::Value;

use crate::features::cache::transport;
use crate::shared::ui::{Alert, AlertVariant, Button, Input, PageHeader};
use crate::use_admin_locale;

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
    let i18n = use_admin_locale();
    let token = use_token();
    let tenant = use_tenant();

    let (mode, set_mode) = signal("in-memory".to_string());
    let (redis_host, set_redis_host) = signal("127.0.0.1".to_string());
    let (redis_port, set_redis_port) = signal("6379".to_string());
    let (redis_password, set_redis_password) = signal(String::new());
    let (redis_db, set_redis_db) = signal("0".to_string());
    let (redis_url, set_redis_url) = signal(String::new());
    let (saving, set_saving) = signal(false);
    let (save_result, set_save_result) = signal(Option::<Result<bool, String>>::None);
    let (loaded, set_loaded) = signal(false);

    let health_resource = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            transport::fetch_cache_health(token_value, tenant_value).await
        },
    );

    let settings_resource = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            transport::fetch_cache_settings(token_value, tenant_value).await
        },
    );

    Effect::new(move |_| {
        if let Some(Ok(response)) = settings_resource.get()
            && !loaded.get_untracked()
        {
            if let Ok(val) = serde_json::from_str::<Value>(&response.platform_settings.settings) {
                if let Some(m) = val.get("mode").and_then(|v| v.as_str()) {
                    set_mode.set(m.to_string());
                }
                if let Some(h) = val.get("redis_host").and_then(|v| v.as_str()) {
                    set_redis_host.set(h.to_string());
                }
                if let Some(p) = val.get("redis_port").and_then(|v| v.as_u64()) {
                    set_redis_port.set(p.to_string());
                }
                if let Some(pw) = val.get("redis_password").and_then(|v| v.as_str()) {
                    set_redis_password.set(pw.to_string());
                }
                if let Some(db) = val.get("redis_db").and_then(|v| v.as_u64()) {
                    set_redis_db.set(db.to_string());
                }
                if let Some(u) = val.get("redis_url").and_then(|v| v.as_str()) {
                    set_redis_url.set(u.to_string());
                }
            }
            set_loaded.set(true);
        }
    });

    let save = move || {
        let token_val = token.get();
        let tenant_val = tenant.get();
        let m = mode.get();
        let host = redis_host.get();
        let port: u16 = redis_port.get().parse().unwrap_or(6379);
        let password = redis_password.get();
        let db: u32 = redis_db.get().parse().unwrap_or(0);
        let url = redis_url.get();

        let mut map = serde_json::Map::new();
        map.insert("mode".to_string(), serde_json::Value::String(m));
        map.insert("redis_host".to_string(), serde_json::Value::String(host));
        map.insert("redis_port".to_string(), serde_json::json!(port));
        map.insert("redis_db".to_string(), serde_json::json!(db));
        map.insert("redis_url".to_string(), serde_json::Value::String(url));
        if !password.is_empty() {
            map.insert("redis_password".to_string(), serde_json::Value::String(password));
        }

        let settings = serde_json::Value::Object(map);

        set_saving.set(true);
        set_save_result.set(None);

        spawn_local(async move {
            let result =
                transport::update_cache_settings(token_val, tenant_val, settings.to_string()).await;

            match result {
                Ok(success) => set_save_result.set(Some(Ok(success))),
                Err(error) => set_save_result.set(Some(Err(error))),
            }
            set_saving.set(false);
        });
    };

    view! {
        <section class="flex flex-1 flex-col p-4 md:px-6">
            <PageHeader
                title=i18n.translate("cache.title")
                subtitle=i18n.translate("cache.subtitle").to_string()
                eyebrow=i18n.translate("cache.eyebrow").to_string()
            />

            <div class="grid grid-cols-1 gap-6 max-w-2xl">
                {/* Diagnostics Card */}
                <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                    <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                        {move || i18n.translate("cache.health.title")}
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
                                            {i18n.translate("cache.health.backend")}
                                        </dt>
                                        <dd class="font-medium text-foreground font-mono">{backend}</dd>

                                        <dt class="text-muted-foreground">
                                            {i18n.translate("cache.health.configured")}
                                        </dt>
                                        <dd>
                                            {if h.redis_configured {
                                                view! {
                                                    <span class="text-green-600 font-medium">
                                                        {i18n.translate("cache.yes")}
                                                    </span>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <span class="text-muted-foreground">
                                                        {i18n.translate("cache.no")}
                                                    </span>
                                                }.into_any()
                                            }}
                                        </dd>

                                        <dt class="text-muted-foreground">
                                            {i18n.translate("cache.health.healthy")}
                                        </dt>
                                        <dd>
                                            {if h.redis_healthy {
                                                view! {
                                                    <span class="text-green-600 font-medium">
                                                        {i18n.translate("cache.yes")}
                                                    </span>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <span class="text-red-600 font-medium">
                                                        {i18n.translate("cache.no")}
                                                    </span>
                                                }.into_any()
                                            }}
                                        </dd>

                                        {redis_error.map(|err| view! {
                                            <dt class="text-muted-foreground">
                                                {i18n.translate("cache.health.error")}
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

                {/* Configuration Card */}
                <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                    <h4 class="mb-1 text-lg font-semibold text-card-foreground">
                        {move || i18n.translate("cache.settings.title")}
                    </h4>
                    <p class="mb-6 text-sm text-muted-foreground">
                        {move || i18n.translate("cache.settings.description")}
                    </p>

                    <Suspense fallback=move || view! {
                        <div class="space-y-4">
                            {(0..3).map(|_| view! {
                                <div class="h-10 animate-pulse rounded-lg bg-muted" />
                            }).collect_view()}
                        </div>
                    }>
                        {move || {
                            let _ = settings_resource.get();
                            view! {
                                <div class="space-y-6">
                                    <div class="flex flex-col gap-2">
                                        <label class="text-sm font-medium leading-none text-foreground">
                                            {i18n.translate("cache.settings.mode")}
                                        </label>
                                        <select
                                            prop:value=move || mode.get()
                                            on:change=move |ev| {
                                                set_mode.set(event_target_value(&ev));
                                            }
                                            class="h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm font-medium text-foreground shadow-xs outline-none transition-[color,box-shadow] focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                                        >
                                            <option value="in-memory">{move || i18n.translate("cache.settings.mode.inmemory")}</option>
                                            <option value="redis">{move || i18n.translate("cache.settings.mode.redis")}</option>
                                            <option value="hybrid">{move || i18n.translate("cache.settings.mode.hybrid")}</option>
                                        </select>
                                    </div>

                                    {/* Conditionally rendered Redis parameters */}
                                    <Show when=move || mode.get() != "in-memory">
                                        <div class="space-y-4 rounded-lg border border-border bg-muted/30 p-4 transition-all">
                                            <h5 class="text-sm font-semibold text-foreground">
                                                {move || i18n.translate("cache.settings.redis.title")}
                                            </h5>
                                            <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
                                                <Input
                                                    value=redis_host
                                                    set_value=set_redis_host
                                                    placeholder="127.0.0.1"
                                                    label=move || i18n.translate("cache.settings.redis.host")
                                                />
                                                <Input
                                                    value=redis_port
                                                    set_value=set_redis_port
                                                    placeholder="6379"
                                                    label=move || i18n.translate("cache.settings.redis.port")
                                                />
                                                <Input
                                                    value=redis_password
                                                    set_value=set_redis_password
                                                    placeholder=move || i18n.translate("cache.settings.redis.password.placeholder")
                                                    type_="password"
                                                    label=move || i18n.translate("cache.settings.redis.password")
                                                />
                                                <Input
                                                    value=redis_db
                                                    set_value=set_redis_db
                                                    placeholder="0"
                                                    label=move || i18n.translate("cache.settings.redis.db")
                                                />
                                            </div>
                                            <Input
                                                value=redis_url
                                                set_value=set_redis_url
                                                placeholder=move || i18n.translate("cache.settings.redis.url.placeholder")
                                                label=move || i18n.translate("cache.settings.redis.url")
                                            />
                                        </div>
                                    </Show>

                                    <Show when=move || save_result.get().is_some()>
                                        {move || match save_result.get() {
                                            Some(Ok(true)) => view! {
                                                <Alert variant=AlertVariant::Success>
                                                    {i18n.translate("cache.settings.saved")}
                                                </Alert>
                                            }.into_any(),
                                            Some(Err(e)) => view! {
                                                <Alert variant=AlertVariant::Destructive>
                                                    {e}
                                                </Alert>
                                            }.into_any(),
                                            _ => view! { <div /> }.into_any(),
                                        }}
                                    </Show>

                                    <div class="flex justify-end pt-2">
                                        <Button
                                            on_click=Callback::new(move |_| save())
                                            disabled=Signal::derive(move || saving.get())
                                        >
                                            {move || if saving.get() {
                                                i18n.translate("cache.settings.saving").to_string()
                                            } else {
                                                i18n.translate("cache.settings.save").to_string()
                                            }}
                                        </Button>
                                    </div>
                                </div>
                            }.into_any()
                        }}
                    </Suspense>
                </div>
            </div>
        </section>
    }
}
