use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui::{
    Alert, AlertVariant, Badge, BadgeVariant, Button, ButtonVariant, Card, CardContent,
    CardDescription, CardHeader, CardTitle, Input, Label,
};
use rustok_ui_core::UiRouteContext;
use std::sync::Arc;

use crate::core::{
    CacheAdminFormDraft, CacheAdminLabels, CacheDiagnosticsViewModel, CacheModeOption,
    build_cache_settings_payload, parse_cache_settings_json,
};
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
    let labels = Arc::new(CacheAdminLabels::resolve(route_context.locale.as_deref()));

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
            if let Some(draft) = parse_cache_settings_json(&response.platform_settings.settings) {
                set_mode.set(draft.mode);
                set_redis_host.set(draft.redis_host);
                set_redis_port.set(draft.redis_port);
                set_redis_password.set(draft.redis_password);
                set_redis_db.set(draft.redis_db);
                set_redis_url.set(draft.redis_url);
            }
            set_loaded.set(true);
        }
    });

    let save = move || {
        let token_val = token.get();
        let tenant_val = tenant.get();
        let draft = CacheAdminFormDraft {
            mode: mode.get(),
            redis_host: redis_host.get(),
            redis_port: redis_port.get(),
            redis_password: redis_password.get(),
            redis_db: redis_db.get(),
            redis_url: redis_url.get(),
        };

        let payload = build_cache_settings_payload(&draft);
        let settings_str = payload.to_string();

        set_saving.set(true);
        set_save_result.set(None);

        spawn_local(async move {
            let res = transport::update_cache_settings(token_val, tenant_val, settings_str).await;
            set_saving.set(false);
            match res {
                Ok(_) => {
                    set_save_result.set(Some(Ok(true)));
                    set_redis_password.set(String::new());
                }
                Err(e) => {
                    set_save_result.set(Some(Err(e)));
                }
            }
        });
    };

    let selected_mode = move || CacheModeOption::from_str(&mode.get());
    let show_redis = move || selected_mode().requires_redis_fields();

    let title_str = labels.title.clone();
    let subtitle_str = labels.subtitle.clone();
    let eyebrow_str = labels.eyebrow.clone();
    let health_title_str = labels.health_title.clone();
    let settings_title_str = labels.settings_title.clone();
    let settings_desc_str = labels.settings_description.clone();
    let mode_label_str = labels.mode_label.clone();
    let mode_inmem_str = labels.mode_inmemory.clone();
    let mode_redis_str = labels.mode_redis.clone();
    let mode_hybrid_str = labels.mode_hybrid.clone();
    let save_btn_str = labels.save.clone();

    let l_health = Arc::clone(&labels);
    let l_redis = Arc::clone(&labels);
    let l_feedback = Arc::clone(&labels);

    view! {
        <div class="space-y-6">
            <header>
                <span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    {eyebrow_str}
                </span>
                <h1 class="text-2xl font-bold tracking-tight text-foreground">
                    {title_str}
                </h1>
                <p class="text-sm text-muted-foreground mt-1">
                    {subtitle_str}
                </p>
            </header>

            // Diagnostics / Health Card using leptos-ui Card and Badge
            <Card>
                <CardHeader>
                    <CardTitle>{health_title_str}</CardTitle>
                </CardHeader>
                <CardContent>
                    {
                        let lh = Arc::clone(&l_health);
                        move || match health_resource.get() {
                            None => view! {
                                <div class="text-sm text-muted-foreground animate-pulse">
                                    "Loading..."
                                </div>
                            }.into_any(),
                            Some(Err(err)) => view! {
                                <Alert variant=AlertVariant::Destructive>
                                    {err}
                                </Alert>
                            }.into_any(),
                            Some(Ok(res)) => {
                                let vm = CacheDiagnosticsViewModel::from_payload(&res.cache_health);
                                let yes_text1 = lh.yes.clone();
                                let yes_text2 = lh.yes.clone();
                                let no_text1 = lh.no.clone();
                                let no_text2 = lh.no.clone();
                                let configured_badge = if vm.is_redis_configured {
                                    view! { <Badge variant=BadgeVariant::Success>{yes_text1}</Badge> }.into_any()
                                } else {
                                    view! { <Badge variant=BadgeVariant::Secondary>{no_text1}</Badge> }.into_any()
                                };
                                let health_badge = if vm.is_redis_healthy {
                                    view! { <Badge variant=BadgeVariant::Success>{yes_text2}</Badge> }.into_any()
                                } else if vm.is_redis_configured {
                                    let err_text = vm.error_message.clone().unwrap_or(no_text2);
                                    let display_text = format!("{} ({})", lh.no, err_text);
                                    view! { <Badge variant=BadgeVariant::Destructive>{display_text}</Badge> }.into_any()
                                } else {
                                    view! { <span class="text-muted-foreground">"—"</span> }.into_any()
                                };
                                view! {
                                    <div class="grid grid-cols-1 md:grid-cols-3 gap-4 text-sm">
                                        <div class="p-3 bg-muted/40 rounded border border-border/50">
                                            <div class="text-xs text-muted-foreground">{lh.health_backend.clone()}</div>
                                            <div class="font-medium text-foreground mt-1 font-mono">{vm.backend_name}</div>
                                        </div>
                                        <div class="p-3 bg-muted/40 rounded border border-border/50">
                                            <div class="text-xs text-muted-foreground mb-1">{lh.health_configured.clone()}</div>
                                            <div>{configured_badge}</div>
                                        </div>
                                        <div class="p-3 bg-muted/40 rounded border border-border/50">
                                            <div class="text-xs text-muted-foreground mb-1">{lh.health_healthy.clone()}</div>
                                            <div>{health_badge}</div>
                                        </div>
                                    </div>
                                }.into_any()
                            }
                        }
                    }
                </CardContent>
            </Card>

            // Configuration Form Card using leptos-ui Card, Input, Label, Button, Alert
            <Card>
                <CardHeader>
                    <CardTitle>{settings_title_str}</CardTitle>
                    <CardDescription>{settings_desc_str}</CardDescription>
                </CardHeader>
                <CardContent>
                    <div class="space-y-6 max-w-xl">
                        // Cache Mode Selector
                        <div>
                            <Label class="mb-2 block">
                                {mode_label_str}
                            </Label>
                            <div class="space-y-2">
                                <label class="flex items-center gap-3 p-3 rounded border border-border hover:bg-muted/30 cursor-pointer transition-colors">
                                    <input
                                        type="radio"
                                        name="cache_mode"
                                        value="in-memory"
                                        checked=move || mode.get() == "in-memory"
                                        on:change=move |_| set_mode.set("in-memory".to_string())
                                        class="accent-primary"
                                    />
                                    <div>
                                        <div class="font-medium text-sm text-foreground">
                                            {mode_inmem_str}
                                        </div>
                                        <div class="text-xs text-muted-foreground">
                                            "Moka cache in local process RAM (default)"
                                        </div>
                                    </div>
                                </label>
                                <label class="flex items-center gap-3 p-3 rounded border border-border hover:bg-muted/30 cursor-pointer transition-colors">
                                    <input
                                        type="radio"
                                        name="cache_mode"
                                        value="redis"
                                        checked=move || mode.get() == "redis"
                                        on:change=move |_| set_mode.set("redis".to_string())
                                        class="accent-primary"
                                    />
                                    <div>
                                        <div class="font-medium text-sm text-foreground">
                                            {mode_redis_str}
                                        </div>
                                        <div class="text-xs text-muted-foreground">
                                            "Centralized Redis instance or cluster"
                                        </div>
                                    </div>
                                </label>
                                <label class="flex items-center gap-3 p-3 rounded border border-border hover:bg-muted/30 cursor-pointer transition-colors">
                                    <input
                                        type="radio"
                                        name="cache_mode"
                                        value="hybrid"
                                        checked=move || mode.get() == "hybrid"
                                        on:change=move |_| set_mode.set("hybrid".to_string())
                                        class="accent-primary"
                                    />
                                    <div>
                                        <div class="font-medium text-sm text-foreground">
                                            {mode_hybrid_str}
                                        </div>
                                        <div class="text-xs text-muted-foreground">
                                            "L1 in-memory + L2 Redis with pub/sub sync"
                                        </div>
                                    </div>
                                </label>
                            </div>
                        </div>

                        // Redis Settings Sub-Section using leptos-ui Input and Label
                        {
                            let lr = Arc::clone(&l_redis);
                            move || {
                                if show_redis() {
                                    let l_curr = Arc::clone(&lr);
                                    let redis_title = l_curr.redis_title.clone();
                                    let host_lbl = l_curr.redis_host.clone();
                                    let port_lbl = l_curr.redis_port.clone();
                                    let pwd_lbl = l_curr.redis_password.clone();
                                    let pwd_ph = l_curr.redis_password_placeholder.clone();
                                    let db_lbl = l_curr.redis_db.clone();
                                    let url_lbl = l_curr.redis_url.clone();
                                    let url_ph = l_curr.redis_url_placeholder.clone();

                                    view! {
                                        <div class="pt-4 border-t border-border space-y-4">
                                            <h3 class="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
                                                {redis_title}
                                            </h3>
                                            <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
                                                <div>
                                                    <Label class="mb-1 block">
                                                        {host_lbl}
                                                    </Label>
                                                    <Input
                                                        value=redis_host
                                                        set_value=set_redis_host
                                                    />
                                                </div>
                                                <div>
                                                    <Label class="mb-1 block">
                                                        {port_lbl}
                                                    </Label>
                                                    <Input
                                                        r#type="number"
                                                        value=redis_port
                                                        set_value=set_redis_port
                                                    />
                                                </div>
                                            </div>

                                            <div>
                                                <Label class="mb-1 block">
                                                    {pwd_lbl}
                                                </Label>
                                                <Input
                                                    r#type="password"
                                                    placeholder=pwd_ph
                                                    value=redis_password
                                                    set_value=set_redis_password
                                                />
                                            </div>

                                            <div>
                                                <Label class="mb-1 block">
                                                    {db_lbl}
                                                </Label>
                                                <Input
                                                    r#type="number"
                                                    value=redis_db
                                                    set_value=set_redis_db
                                                />
                                            </div>

                                            <div>
                                                <Label class="mb-1 block">
                                                    {url_lbl}
                                                </Label>
                                                <Input
                                                    placeholder=url_ph
                                                    value=redis_url
                                                    set_value=set_redis_url
                                                />
                                            </div>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }
                            }
                        }

                        // Save Result Alerts using leptos-ui Alert
                        {
                            let l_fb = Arc::clone(&l_feedback);
                            move || {
                                let saved_text = l_fb.saved.clone();
                                match save_result.get() {
                                    Some(Ok(_)) => view! {
                                        <Alert variant=AlertVariant::Success>
                                            {saved_text}
                                        </Alert>
                                    }.into_any(),
                                    Some(Err(err)) => view! {
                                        <Alert variant=AlertVariant::Destructive>
                                            {err}
                                        </Alert>
                                    }.into_any(),
                                    None => view! { <div></div> }.into_any(),
                                }
                            }
                        }

                        // Save Button using leptos-ui Button with loading state
                        <div>
                            <Button
                                variant=ButtonVariant::Default
                                disabled=saving.get()
                                loading=saving.get()
                                on_click=Box::new(save)
                            >
                                {save_btn_str}
                            </Button>
                        </div>
                    </div>
                </CardContent>
            </Card>
        </div>
    }
}
