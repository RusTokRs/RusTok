use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_leptos::{
    Alert, AlertVariant, Badge, BadgeVariant, Button, ButtonVariant, Card, CardContent,
    CardDescription, CardHeader, CardTitle, Input, Label,
};
use rustok_auth_ui::ui::leptos::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;
use std::sync::Arc;

use crate::core::{
    CacheAdminFormDraft, CacheAdminLabels, CacheDiagnosticsViewModel, CacheModeOption,
    build_cache_settings_payload, parse_cache_settings_json,
};
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

/// The cache settings fields the operator has changed. `None` means "unchanged", so the value the
/// backend resolved is what the field shows.
#[derive(Clone, Debug, Default)]
struct CacheSettingsEdits {
    mode: Option<String>,
    redis_host: Option<String>,
    redis_port: Option<String>,
    redis_password: Option<String>,
    redis_db: Option<String>,
    redis_url: Option<String>,
}

#[component]
pub fn CacheAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let labels = Arc::new(CacheAdminLabels::resolve(route_context.locale.as_deref()));

    // The form is the resolved settings with the operator's edits laid over them, not a set of cells
    // an `Effect` fills after the first render: an `Effect` is not driven before the server produces
    // the response, so a form built that way arrives empty and a field nobody touched carries a
    // value nobody set.
    let (edits, set_edits) = signal(CacheSettingsEdits::default());
    let (saving, set_saving) = signal(false);
    let (save_result, set_save_result) = signal(Option::<Result<bool, String>>::None);
    let (schema_version_override, set_schema_version_override) = signal(Option::<i32>::None);

    // Resolved by the request: this page mounts inside the `Async` admin layout, so a
    // `LocalResource` here leaves the reader with the `Suspense` fallback.
    let health_resource = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            transport::fetch_cache_health(token_value, tenant_value).await
        },
    );

    let settings_resource = loaded_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            transport::fetch_cache_settings(token_value, tenant_value).await
        },
    );

    let resolved = move || {
        settings_resource
            .get()
            .and_then(Result::ok)
            .map(|response| {
                (
                    parse_cache_settings_json(&response.platform_settings.settings)
                        .unwrap_or_default(),
                    response.platform_settings.schema_version,
                )
            })
    };
    let backend = move || resolved().map(|(draft, _)| draft);
    // `Copy`, not `move`: every field captures it, and a moved closure would be `FnOnce`.
    let field = |read: fn(&CacheSettingsEdits) -> Option<String>,
                 backend: fn(&CacheAdminFormDraft) -> String| {
        let edited = edits.get();
        read(&edited).or_else(|| backend().map(|draft| backend(&draft)))
    };

    let mode = Signal::derive(move || {
        let edited = edits.get();
        edited
            .mode
            .clone()
            .or_else(|| backend().map(|draft| draft.mode))
            .unwrap_or_else(|| "in-memory".to_string())
    });
    let redis_host = Signal::derive(move || {
        field(
            |edits| edits.redis_host.clone(),
            |draft| draft.redis_host.clone(),
        )
    });
    let redis_port = Signal::derive(move || {
        field(
            |edits| edits.redis_port.clone(),
            |draft| draft.redis_port.clone(),
        )
    });
    let redis_password = Signal::derive(move || {
        let edited = edits.get();
        edited
            .redis_password
            .clone()
            .or_else(|| backend().map(|draft| draft.redis_password))
            .unwrap_or_default()
    });
    let redis_db = Signal::derive(move || {
        field(
            |edits| edits.redis_db.clone(),
            |draft| draft.redis_db.clone(),
        )
    });
    let redis_url = Signal::derive(move || {
        field(
            |edits| edits.redis_url.clone(),
            |draft| draft.redis_url.clone(),
        )
    });
    let schema_version = Signal::derive(move || {
        if let Some(version) = schema_version_override.get() {
            return version;
        }
        resolved().map(|(_, version)| version).unwrap_or(0)
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

        let payload = match build_cache_settings_payload(&draft) {
            Ok(payload) => payload,
            Err(error) => {
                // Surface the validation error instead of sending a silently corrected payload.
                set_saving.set(false);
                set_save_result.set(Some(Err(error)));
                return;
            }
        };
        let settings_str = payload.to_string();
        let expected_schema_version = schema_version.get();

        set_saving.set(true);
        set_save_result.set(None);

        spawn_local(async move {
            let res = transport::update_cache_settings(
                token_val,
                tenant_val,
                settings_str,
                expected_schema_version,
            )
            .await;
            set_saving.set(false);
            match res {
                Ok(response) => {
                    set_schema_version_override
                        .set(Some(response.update_platform_settings.schema_version));
                    set_save_result.set(Some(Ok(true)));
                    // The saved secret is never echoed back, so the field drops what was typed.
                    set_edits.update(|edits| edits.redis_password = Some(String::new()));
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
                        let health_view = health_resource;
                        move || match health_view.get() {
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
                                    view! { <span class="text-muted-foreground">"вЂ”"</span> }.into_any()
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
                                        on:change=move |_| set_edits.update(|edits| edits.mode = Some("in-memory".to_string()))
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
                                        on:change=move |_| set_edits.update(|edits| edits.mode = Some("redis".to_string()))
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
                                        on:change=move |_| set_edits.update(|edits| edits.mode = Some("hybrid".to_string()))
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
                                                        derived_value=redis_host
                                                        set_value=SignalSetter::map(move |value: String| {
                                                            set_edits.update(|edits| edits.redis_host = Some(value));
                                                        })
                                                    />
                                                </div>
                                                <div>
                                                    <Label class="mb-1 block">
                                                        {port_lbl}
                                                    </Label>
                                                    <Input
                                                        r#type="number"
                                                        derived_value=redis_port
                                                        set_value=SignalSetter::map(move |value: String| {
                                                            set_edits.update(|edits| edits.redis_port = Some(value));
                                                        })
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
                                                    derived_value=redis_password
                                                    set_value=SignalSetter::map(move |value: String| {
                                                        set_edits.update(|edits| edits.redis_password = Some(value));
                                                    })
                                                />
                                            </div>

                                            <div>
                                                <Label class="mb-1 block">
                                                    {db_lbl}
                                                </Label>
                                                <Input
                                                    r#type="number"
                                                    derived_value=redis_db
                                                    set_value=SignalSetter::map(move |value: String| {
                                                        set_edits.update(|edits| edits.redis_db = Some(value));
                                                    })
                                                />
                                            </div>

                                            <div>
                                                <Label class="mb-1 block">
                                                    {url_lbl}
                                                </Label>
                                                <Input
                                                    placeholder=url_ph
                                                    derived_value=redis_url
                                                    set_value=SignalSetter::map(move |value: String| {
                                                        set_edits.update(|edits| edits.redis_url = Some(value));
                                                    })
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
