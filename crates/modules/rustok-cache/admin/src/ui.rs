use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;
use serde_json::Value;

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
pub fn CacheAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let is_ru = route_context
        .locale
        .as_deref()
        .map(|l| l.starts_with("ru"))
        .unwrap_or(false);

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

    let title_text = if is_ru { "Конфигурация кэша" } else { "Cache Configuration" };
    let desc_text = if is_ru {
        "Выберите режим кэширования и параметры подключения к Redis"
    } else {
        "Select caching layer and Redis connection settings"
    };
    let mode_label = if is_ru { "Режим кэша" } else { "Cache Mode" };
    let mode_inmemory = if is_ru { "Встроенный кэш (In-Memory)" } else { "In-Memory (Local RAM)" };
    let mode_redis = if is_ru { "Redis (Распределённый)" } else { "Redis (Distributed)" };
    let mode_hybrid = if is_ru { "Гибридный (L1 ОЗУ + L2 Redis)" } else { "Hybrid (L1 RAM + L2 Redis)" };

    let redis_title = if is_ru { "Настройки Redis" } else { "Redis Settings" };
    let host_label = if is_ru { "Хост" } else { "Host" };
    let port_label = if is_ru { "Порт" } else { "Port" };
    let pass_label = if is_ru { "Пароль" } else { "Password" };
    let pass_placeholder = if is_ru { "Оставьте пустым, чтобы не менять" } else { "Leave blank to keep current" };
    let db_label = if is_ru { "База данных (DB)" } else { "Database (DB)" };
    let url_label = if is_ru { "URL подключения (опционально)" } else { "Connection URL (optional)" };

    let save_text = if is_ru { "Сохранить настройки" } else { "Save Settings" };
    let saving_text = if is_ru { "Сохранение..." } else { "Saving..." };
    let saved_success_text = if is_ru { "Настройки кэша успешно сохранены" } else { "Cache settings updated successfully" };

    let diag_title = if is_ru { "Диагностика" } else { "Diagnostics" };
    let backend_label = if is_ru { "Бэкенд" } else { "Backend" };
    let conf_label = if is_ru { "Redis настроен" } else { "Redis Configured" };
    let avail_label = if is_ru { "Redis доступен" } else { "Redis Available" };
    let err_label = if is_ru { "Ошибка" } else { "Error" };
    let yes_text = if is_ru { "Да" } else { "Yes" };
    let no_text = if is_ru { "Нет" } else { "No" };

    view! {
        <div class="grid grid-cols-1 gap-6 max-w-2xl">
            {/* Diagnostics Card */}
            <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                    {diag_title}
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
                                    <dt class="text-muted-foreground">{backend_label}</dt>
                                    <dd class="font-medium text-foreground font-mono">{backend}</dd>

                                    <dt class="text-muted-foreground">{conf_label}</dt>
                                    <dd>
                                        {if h.redis_configured {
                                            view! { <span class="text-green-600 font-medium">{yes_text}</span> }.into_any()
                                        } else {
                                            view! { <span class="text-muted-foreground">{no_text}</span> }.into_any()
                                        }}
                                    </dd>

                                    <dt class="text-muted-foreground">{avail_label}</dt>
                                    <dd>
                                        {if h.redis_healthy {
                                            view! { <span class="text-green-600 font-medium">{yes_text}</span> }.into_any()
                                        } else {
                                            view! { <span class="text-red-600 font-medium">{no_text}</span> }.into_any()
                                        }}
                                    </dd>

                                    {redis_error.map(|err| view! {
                                        <dt class="text-muted-foreground">{err_label}</dt>
                                        <dd class="text-destructive text-xs break-all">{err}</dd>
                                    })}
                                </dl>
                            }.into_any()
                        }
                        Some(Err(err)) => view! {
                            <div class="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                                {err.to_string()}
                            </div>
                        }.into_any(),
                    }}
                </Suspense>
            </div>

            {/* Configuration Card */}
            <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                <h4 class="mb-1 text-lg font-semibold text-card-foreground">
                    {title_text}
                </h4>
                <p class="mb-6 text-sm text-muted-foreground">
                    {desc_text}
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
                                        {mode_label}
                                    </label>
                                    <select
                                        prop:value=move || mode.get()
                                        on:change=move |ev| {
                                            set_mode.set(event_target_value(&ev));
                                        }
                                        class="h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm font-medium text-foreground shadow-xs outline-none transition-[color,box-shadow] focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                                    >
                                        <option value="in-memory">{mode_inmemory}</option>
                                        <option value="redis">{mode_redis}</option>
                                        <option value="hybrid">{mode_hybrid}</option>
                                    </select>
                                </div>

                                {/* Conditionally rendered Redis parameters */}
                                <Show when=move || mode.get() != "in-memory">
                                    <div class="space-y-4 rounded-lg border border-border bg-muted/30 p-4 transition-all">
                                        <h5 class="text-sm font-semibold text-foreground">
                                            {redis_title}
                                        </h5>
                                        <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
                                            <div class="flex flex-col gap-1.5">
                                                <label class="text-sm font-medium text-foreground">{host_label}</label>
                                                <input
                                                    type="text"
                                                    prop:value=move || redis_host.get()
                                                    on:input=move |ev| set_redis_host.set(event_target_value(&ev))
                                                    placeholder="127.0.0.1"
                                                    class="h-9 rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-1 focus-visible:ring-ring"
                                                />
                                            </div>

                                            <div class="flex flex-col gap-1.5">
                                                <label class="text-sm font-medium text-foreground">{port_label}</label>
                                                <input
                                                    type="number"
                                                    min="1"
                                                    max="65535"
                                                    prop:value=move || redis_port.get()
                                                    on:input=move |ev| set_redis_port.set(event_target_value(&ev))
                                                    placeholder="6379"
                                                    class="h-9 rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-1 focus-visible:ring-ring"
                                                />
                                            </div>

                                            <div class="flex flex-col gap-1.5">
                                                <label class="text-sm font-medium text-foreground">{pass_label}</label>
                                                <input
                                                    type="password"
                                                    prop:value=move || redis_password.get()
                                                    on:input=move |ev| set_redis_password.set(event_target_value(&ev))
                                                    placeholder=pass_placeholder
                                                    class="h-9 rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-1 focus-visible:ring-ring"
                                                />
                                            </div>

                                            <div class="flex flex-col gap-1.5">
                                                <label class="text-sm font-medium text-foreground">{db_label}</label>
                                                <input
                                                    type="number"
                                                    min="0"
                                                    max="255"
                                                    prop:value=move || redis_db.get()
                                                    on:input=move |ev| set_redis_db.set(event_target_value(&ev))
                                                    placeholder="0"
                                                    class="h-9 rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-1 focus-visible:ring-ring"
                                                />
                                            </div>
                                        </div>

                                        <div class="flex flex-col gap-1.5 pt-2 border-t">
                                            <label class="text-sm font-medium text-foreground">{url_label}</label>
                                            <input
                                                type="text"
                                                prop:value=move || redis_url.get()
                                                on:input=move |ev| set_redis_url.set(event_target_value(&ev))
                                                placeholder="redis://:password@host:port/db"
                                                class="h-9 rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-1 focus-visible:ring-ring"
                                            />
                                        </div>
                                    </div>
                                </Show>

                                <Show when=move || save_result.get().is_some()>
                                    {move || match save_result.get() {
                                        Some(Ok(true)) => view! {
                                            <div class="rounded-lg border border-green-500/50 bg-green-500/10 p-3 text-sm text-green-700 dark:text-green-400">
                                                {saved_success_text}
                                            </div>
                                        }.into_any(),
                                        Some(Err(e)) => view! {
                                            <div class="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                                                {e}
                                            </div>
                                        }.into_any(),
                                        _ => view! { <div /> }.into_any(),
                                    }}
                                </Show>

                                <div class="flex justify-end pt-2">
                                    <button
                                        type="button"
                                        on:click=move |_| save()
                                        disabled=move || saving.get()
                                        class="inline-flex items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-xs hover:bg-primary/90 disabled:opacity-50"
                                    >
                                        {move || if saving.get() { saving_text } else { save_text }}
                                    </button>
                                </div>
                            </div>
                        }.into_any()
                    }}
                </Suspense>
            </div>
        </div>
    }
}
