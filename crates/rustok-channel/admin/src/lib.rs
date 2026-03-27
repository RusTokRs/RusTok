mod api;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_api::context::ChannelResolutionSource;
use rustok_api::UiRouteContext;

use crate::model::{
    BindChannelModulePayload, BindChannelOauthAppPayload, ChannelAdminBootstrap, ChannelDetail,
    CreateChannelPayload, CreateChannelTargetPayload,
};

#[component]
pub fn ChannelAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let token = leptos_auth::hooks::use_token();
    let tenant = leptos_auth::hooks::use_tenant();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (feedback, set_feedback) = signal(Option::<String>::None);
    let (error, set_error) = signal(Option::<String>::None);
    let create_slug = RwSignal::new(String::new());
    let create_name = RwSignal::new(String::new());
    let create_busy = RwSignal::new(false);

    let bootstrap = Resource::new(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            api::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let on_create = move |ev: SubmitEvent| {
        ev.prevent_default();
        create_busy.set(true);
        set_feedback.set(None);
        set_error.set(None);

        spawn_local({
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            let slug = create_slug.get_untracked();
            let name = create_name.get_untracked();
            async move {
                let result = api::create_channel(
                    token_value,
                    tenant_value,
                    &CreateChannelPayload {
                        tenant_id: None,
                        slug,
                        name,
                        settings: Some(serde_json::json!({})),
                    },
                )
                .await;

                match result {
                    Ok(channel) => {
                        set_feedback.set(Some(format!("Channel `{}` created.", channel.slug)));
                        create_slug.set(String::new());
                        create_name.set(String::new());
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_error.set(Some(err)),
                }

                create_busy.set(false);
            }
        });
    };

    let route_segment = route_context
        .route_segment
        .clone()
        .unwrap_or_else(|| "channels".to_string());

    view! {
        <div class="space-y-6">
            <header class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                    <div class="space-y-2">
                        <span class="inline-flex items-center rounded-full border border-amber-300 bg-amber-50 px-3 py-1 text-xs font-semibold uppercase tracking-wide text-amber-700">
                            "Experimental Core"
                        </span>
                        <h1 class="text-2xl font-semibold text-card-foreground">"Channel Management"</h1>
                        <p class="max-w-3xl text-sm text-muted-foreground">
                            "Channels define platform-level external delivery context, targets, enabled module surfaces, and bound OAuth apps."
                        </p>
                    </div>
                    <div class="rounded-xl border border-border bg-background px-4 py-3 text-sm text-muted-foreground">
                        {format!("Route: /modules/{route_segment}")}
                    </div>
                </div>
            </header>

            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h2 class="text-lg font-semibold text-card-foreground">"Create Channel"</h2>
                    <p class="text-sm text-muted-foreground">
                        "Start small: create the channel first, then attach targets and bindings below."
                    </p>
                </div>
                <form class="mt-5 grid gap-4 lg:grid-cols-[1fr_1fr_auto]" on:submit=on_create>
                    <input
                        type="text"
                        class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                        placeholder="slug"
                        prop:value=create_slug
                        on:input=move |ev| create_slug.set(event_target_value(&ev))
                    />
                    <input
                        type="text"
                        class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                        placeholder="name"
                        prop:value=create_name
                        on:input=move |ev| create_name.set(event_target_value(&ev))
                    />
                    <button
                        type="submit"
                        class="inline-flex h-10 items-center justify-center rounded-lg bg-primary px-4 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                        disabled=move || create_busy.get()
                    >
                        {move || if create_busy.get() { "Creating..." } else { "Create" }}
                    </button>
                </form>
                <Show when=move || feedback.get().is_some()>
                    <div class="mt-4 rounded-xl border border-emerald-300 bg-emerald-50 px-4 py-3 text-sm text-emerald-700">
                        {move || feedback.get().unwrap_or_default()}
                    </div>
                </Show>
                <Show when=move || error.get().is_some()>
                    <div class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                        {move || error.get().unwrap_or_default()}
                    </div>
                </Show>
            </section>

            <Suspense fallback=move || view! { <div class="h-48 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || {
                    bootstrap.get().map(|result| match result {
                        Ok(bootstrap) => view! {
                            <div class="space-y-6">
                                <RuntimeContext bootstrap=bootstrap.clone() />
                                {if bootstrap.channels.is_empty() {
                                    view! {
                                        <div class="rounded-2xl border border-dashed border-border bg-card p-8 text-center text-sm text-muted-foreground">
                                            "No channels configured yet."
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="space-y-4">
                                            {bootstrap.channels.into_iter().map(|channel| view! {
                                                <ChannelCard
                                                    channel=channel
                                                    available_modules=bootstrap.available_modules.clone()
                                                    oauth_apps=bootstrap.oauth_apps.clone()
                                                    token=token.get()
                                                    tenant=tenant.get()
                                                    set_feedback=set_feedback
                                                    set_error=set_error
                                                    set_refresh_nonce=set_refresh_nonce
                                                />
                                            }).collect_view()}
                                        </div>
                                    }.into_any()
                                }}
                            </div>
                        }.into_any(),
                        Err(err) => view! {
                            <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-5 py-4 text-sm text-destructive">
                                {format!("Failed to load channel bootstrap: {err}")}
                            </div>
                        }.into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn RuntimeContext(bootstrap: ChannelAdminBootstrap) -> impl IntoView {
    view! {
        <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
            <div class="space-y-1">
                <h2 class="text-lg font-semibold text-card-foreground">"Runtime Context"</h2>
                <p class="text-sm text-muted-foreground">"Channel resolved by middleware for the current request."</p>
            </div>
            {match bootstrap.current_channel {
                Some(current) => view! {
                    <div class="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-5">
                        <InfoPill label="Slug" value=current.slug />
                        <InfoPill label="Name" value=current.name />
                        <InfoPill label="Source" value=resolution_source_label(&current.resolution_source) />
                        <InfoPill label="Target" value=current.target_value.unwrap_or_else(|| "n/a".to_string()) />
                        <InfoPill label="Type" value=current.target_type.unwrap_or_else(|| "n/a".to_string()) />
                    </div>
                    <div class="mt-4 rounded-xl border border-sky-200 bg-sky-50 px-4 py-3 text-sm text-sky-800">
                        {resolution_source_description(&current.resolution_source)}
                    </div>
                }.into_any(),
                None => view! {
                    <div class="mt-4 rounded-xl border border-dashed border-border px-4 py-3 text-sm text-muted-foreground">
                        "No channel was resolved for the current request yet."
                    </div>
                }.into_any(),
            }}
        </section>
    }
}

#[component]
fn ChannelCard(
    channel: ChannelDetail,
    available_modules: Vec<crate::model::AvailableModuleItem>,
    oauth_apps: Vec<crate::model::AvailableOauthAppItem>,
    token: Option<String>,
    tenant: Option<String>,
    set_feedback: WriteSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
    set_refresh_nonce: WriteSignal<u64>,
) -> impl IntoView {
    let has_available_modules = !available_modules.is_empty();
    let has_available_oauth_apps = !oauth_apps.is_empty();
    let is_default_channel = channel.channel.is_default;
    let editing_target_id = RwSignal::new(Option::<String>::None);
    let editing_module_slug = RwSignal::new(Option::<String>::None);
    let editing_oauth_app_id = RwSignal::new(Option::<String>::None);
    let initial_module_slug = RwSignal::new(
        available_modules
            .first()
            .map(|item| item.slug.clone())
            .unwrap_or_default(),
    );
    let initial_oauth_app_id = RwSignal::new(
        oauth_apps
            .first()
            .map(|item| item.id.clone())
            .unwrap_or_default(),
    );
    let target_type = RwSignal::new("web_domain".to_string());
    let target_value = RwSignal::new(String::new());
    let target_primary = RwSignal::new(true);
    let bind_module_slug = RwSignal::new(initial_module_slug.get_untracked());
    let bind_module_enabled = RwSignal::new(true);
    let bind_oauth_app_id = RwSignal::new(initial_oauth_app_id.get_untracked());
    let bind_oauth_role = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let channel_id = channel.channel.id.clone();
    let channel_slug = channel.channel.slug.clone();
    let token_for_target = token.clone();
    let tenant_for_target = tenant.clone();
    let channel_id_for_target = channel_id.clone();
    let channel_slug_for_target = channel_slug.clone();
    let token_for_default = token.clone();
    let tenant_for_default = tenant.clone();
    let channel_id_for_default = channel_id.clone();
    let token_for_target_delete = token.clone();
    let tenant_for_target_delete = tenant.clone();
    let channel_id_for_target_delete = channel_id.clone();
    let channel_slug_for_target_delete = channel_slug.clone();
    let token_for_module = token.clone();
    let tenant_for_module = tenant.clone();
    let channel_id_for_module = channel_id.clone();
    let channel_slug_for_module = channel_slug.clone();
    let token_for_module_delete = token.clone();
    let tenant_for_module_delete = tenant.clone();
    let channel_id_for_module_delete = channel_id.clone();
    let channel_slug_for_module_delete = channel_slug.clone();
    let token_for_app = token;
    let tenant_for_app = tenant;
    let channel_id_for_app = channel_id;
    let channel_slug_for_app = channel_slug;
    let token_for_app_delete = token_for_app.clone();
    let tenant_for_app_delete = tenant_for_app.clone();
    let channel_id_for_app_delete = channel_id_for_app.clone();
    let channel_slug_for_app_delete = channel_slug_for_app.clone();
    let cancel_target_edit = move |_| {
        editing_target_id.set(None);
        target_type.set("web_domain".to_string());
        target_value.set(String::new());
        target_primary.set(true);
    };
    let cancel_module_edit = move |_| {
        editing_module_slug.set(None);
        bind_module_slug.set(initial_module_slug.get_untracked());
        bind_module_enabled.set(true);
    };
    let cancel_oauth_edit = move |_| {
        editing_oauth_app_id.set(None);
        bind_oauth_app_id.set(initial_oauth_app_id.get_untracked());
        bind_oauth_role.set(String::new());
    };

    let make_default = move |_| {
        busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        spawn_local({
            let token = token_for_default.clone();
            let tenant = tenant_for_default.clone();
            let channel_id = channel_id_for_default.clone();
            async move {
                let result = api::make_default_channel(token, tenant, &channel_id).await;
                match result {
                    Ok(channel) => {
                        set_feedback.set(Some(format!(
                            "Channel `{}` is now the tenant default channel.",
                            channel.slug
                        )));
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_error.set(Some(err)),
                }
                busy.set(false);
            }
        });
    };

    let create_target = move |ev: SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        spawn_local({
            let token = token_for_target.clone();
            let tenant = tenant_for_target.clone();
            let channel_id = channel_id_for_target.clone();
            let channel_slug = channel_slug_for_target.clone();
            let editing_target_id_value = editing_target_id.get_untracked();
            async move {
                let payload = CreateChannelTargetPayload {
                    target_type: target_type.get_untracked(),
                    value: target_value.get_untracked(),
                    is_primary: target_primary.get_untracked(),
                    settings: Some(serde_json::json!({})),
                };
                let result = match editing_target_id_value.as_deref() {
                    Some(target_id) => {
                        api::update_target(token, tenant, &channel_id, target_id, &payload).await
                    }
                    None => api::create_target(token, tenant, &channel_id, &payload).await,
                };
                match result {
                    Ok(target) => {
                        let action = if editing_target_id_value.is_some() {
                            "updated for"
                        } else {
                            "added to"
                        };
                        set_feedback.set(Some(format!(
                            "Target `{}` {action} channel `{}`.",
                            target.value, channel_slug
                        )));
                        editing_target_id.set(None);
                        target_type.set("web_domain".to_string());
                        target_value.set(String::new());
                        target_primary.set(true);
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_error.set(Some(err)),
                }
                busy.set(false);
            }
        });
    };

    let bind_module_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        spawn_local({
            let token = token_for_module.clone();
            let tenant = tenant_for_module.clone();
            let channel_id = channel_id_for_module.clone();
            let channel_slug = channel_slug_for_module.clone();
            async move {
                let result = api::bind_module(
                    token,
                    tenant,
                    &channel_id,
                    &BindChannelModulePayload {
                        module_slug: bind_module_slug.get_untracked(),
                        is_enabled: bind_module_enabled.get_untracked(),
                        settings: Some(serde_json::json!({})),
                    },
                )
                .await;
                match result {
                    Ok(_) => {
                        set_feedback.set(Some(format!(
                            "Module binding {} channel `{}`.",
                            if editing_module_slug.get_untracked().is_some() {
                                "updated for"
                            } else {
                                "saved for"
                            },
                            channel_slug,
                        )));
                        editing_module_slug.set(None);
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_error.set(Some(err)),
                }
                busy.set(false);
            }
        });
    };

    let bind_oauth_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        spawn_local({
            let token = token_for_app.clone();
            let tenant = tenant_for_app.clone();
            let channel_id = channel_id_for_app.clone();
            let channel_slug = channel_slug_for_app.clone();
            async move {
                let result = api::bind_oauth_app(
                    token,
                    tenant,
                    &channel_id,
                    &BindChannelOauthAppPayload {
                        oauth_app_id: bind_oauth_app_id.get_untracked(),
                        role: optional_text(bind_oauth_role.get_untracked()),
                    },
                )
                .await;
                match result {
                    Ok(_) => {
                        set_feedback.set(Some(format!(
                            "OAuth app binding {} channel `{}`.",
                            if editing_oauth_app_id.get_untracked().is_some() {
                                "updated for"
                            } else {
                                "saved for"
                            },
                            channel_slug,
                        )));
                        editing_oauth_app_id.set(None);
                        bind_oauth_role.set(String::new());
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_error.set(Some(err)),
                }
                busy.set(false);
            }
        });
    };

    view! {
        <article class="rounded-2xl border border-border bg-card p-6 shadow-sm">
            <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                <div class="space-y-2">
                    <div class="flex flex-wrap gap-2">
                        <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                            {channel.channel.slug.clone()}
                        </span>
                        <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                            {channel.channel.status.clone()}
                        </span>
                        {if is_default_channel {
                            view! {
                                <span class="inline-flex items-center rounded-full border border-sky-300 bg-sky-50 px-3 py-1 text-xs font-medium text-sky-700">
                                    "Default"
                                </span>
                            }.into_any()
                        } else {
                            ().into_any()
                        }}
                    </div>
                    <h2 class="text-xl font-semibold text-card-foreground">{channel.channel.name.clone()}</h2>
                    <p class="text-sm text-muted-foreground">
                        {format!("{} target(s), {} module binding(s), {} app binding(s)", channel.targets.len(), channel.module_bindings.len(), channel.oauth_apps.len())}
                    </p>
                </div>
                <div class="space-y-3">
                    {if is_default_channel {
                        view! {
                            <div class="rounded-lg border border-sky-200 bg-sky-50 px-4 py-3 text-sm text-sky-800">
                                "Used as the tenant's explicit default channel when no header, query or host selector matches."
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <button
                                type="button"
                                class="inline-flex h-10 items-center justify-center rounded-lg border border-border bg-background px-4 text-sm font-medium text-card-foreground transition hover:bg-muted disabled:opacity-50"
                                disabled=move || busy.get()
                                on:click=make_default
                            >
                                "Make Default"
                            </button>
                        }.into_any()
                    }}
                    <div class="grid gap-2 md:grid-cols-2">
                    <InfoPill label="ID" value=short_id(&channel.channel.id) />
                    <InfoPill label="Updated" value=channel.channel.updated_at.clone() />
                    </div>
                </div>
            </div>

            <div class="mt-6 grid gap-6 xl:grid-cols-3">
                <section class="space-y-4 rounded-xl border border-border bg-background p-4">
                    <div class="flex items-center justify-between gap-3">
                        <h3 class="text-base font-semibold text-card-foreground">
                            {move || if editing_target_id.get().is_some() { "Edit Target" } else { "Targets" }}
                        </h3>
                        <Show when=move || editing_target_id.get().is_some()>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition hover:bg-muted"
                                on:click=cancel_target_edit
                            >
                                "Cancel"
                            </button>
                        </Show>
                    </div>
                    {if channel.targets.is_empty() {
                        view! {
                            <EmptyState
                                title="No targets yet."
                                body="Add the first target to make this channel discoverable through a concrete delivery surface."
                            />
                        }.into_any()
                    } else {
                        view! {
                            <div class="space-y-2">
                                {channel.targets.iter().map(|target| view! {
                                    <div class="rounded-lg border border-border px-3 py-2 text-sm">
                                        <div class="flex items-start justify-between gap-3">
                                            <div>
                                                <div class="font-medium text-card-foreground">{target.value.clone()}</div>
                                                <div class="mt-1 text-xs text-muted-foreground">
                                                    {format!("{}{}", target.target_type, if target.is_primary { " · primary" } else { "" })}
                                                </div>
                                            </div>
                                            <button
                                                type="button"
                                                class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition hover:bg-muted"
                                                disabled=move || busy.get()
                                                on:click={
                                                    let target = target.clone();
                                                    move |_| {
                                                        editing_target_id.set(Some(target.id.clone()));
                                                        target_type.set(target.target_type.clone());
                                                        target_value.set(target.value.clone());
                                                        target_primary.set(target.is_primary);
                                                    }
                                                }
                                            >
                                                "Edit"
                                            </button>
                                            <button
                                                type="button"
                                                class="rounded-lg border border-rose-200 px-3 py-1 text-xs font-medium text-rose-700 transition hover:bg-rose-50 disabled:opacity-50"
                                                disabled=move || busy.get()
                                                on:click={
                                                    let target = target.clone();
                                                    let token = token_for_target_delete.clone();
                                                    let tenant = tenant_for_target_delete.clone();
                                                    let channel_id = channel_id_for_target_delete.clone();
                                                    let channel_slug = channel_slug_for_target_delete.clone();
                                                    move |_| {
                                                        busy.set(true);
                                                        set_feedback.set(None);
                                                        set_error.set(None);
                                                        spawn_local({
                                                            let target = target.clone();
                                                            let token = token.clone();
                                                            let tenant = tenant.clone();
                                                            let channel_id = channel_id.clone();
                                                            let channel_slug = channel_slug.clone();
                                                            async move {
                                                                let result = api::delete_target(
                                                                    token,
                                                                    tenant,
                                                                    &channel_id,
                                                                    &target.id,
                                                                )
                                                                .await;
                                                                match result {
                                                                    Ok(deleted) => {
                                                                        if editing_target_id
                                                                            .get_untracked()
                                                                            .as_deref()
                                                                            == Some(target.id.as_str())
                                                                        {
                                                                            editing_target_id.set(None);
                                                                            target_type.set("web_domain".to_string());
                                                                            target_value.set(String::new());
                                                                            target_primary.set(true);
                                                                        }
                                                                        set_feedback.set(Some(format!(
                                                                            "Target `{}` removed from channel `{}`.",
                                                                            deleted.value, channel_slug
                                                                        )));
                                                                        set_refresh_nonce.update(|value| *value += 1);
                                                                    }
                                                                    Err(err) => set_error.set(Some(err)),
                                                                }
                                                                busy.set(false);
                                                            }
                                                        });
                                                    }
                                                }
                                            >
                                                "Delete"
                                            </button>
                                        </div>
                                    </div>
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                    <form class="space-y-3" on:submit=create_target>
                        <select class="w-full rounded-lg border border-input bg-card px-3 py-2 text-sm" on:change=move |ev| target_type.set(event_target_value(&ev))>
                            <option value="web_domain">"web_domain"</option>
                            <option value="mobile_app">"mobile_app"</option>
                            <option value="api_client">"api_client"</option>
                            <option value="embedded">"embedded"</option>
                            <option value="external">"external"</option>
                        </select>
                        <input type="text" class="w-full rounded-lg border border-input bg-card px-3 py-2 text-sm" placeholder="example.com or app id" prop:value=target_value on:input=move |ev| target_value.set(event_target_value(&ev)) />
                        <label class="flex items-center gap-2 text-sm text-muted-foreground">
                            <input type="checkbox" prop:checked=target_primary on:change=move |ev| target_primary.set(event_target_checked(&ev)) />
                            "Primary target"
                        </label>
                        <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>
                            {move || if editing_target_id.get().is_some() { "Save Target" } else { "Add Target" }}
                        </button>
                    </form>
                </section>

                <section class="space-y-4 rounded-xl border border-border bg-background p-4">
                    <div class="flex items-center justify-between gap-3">
                        <h3 class="text-base font-semibold text-card-foreground">
                            {move || if editing_module_slug.get().is_some() { "Edit Module Binding" } else { "Module Bindings" }}
                        </h3>
                        <Show when=move || editing_module_slug.get().is_some()>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition hover:bg-muted"
                                on:click=cancel_module_edit
                            >
                                "Cancel"
                            </button>
                        </Show>
                    </div>
                    {if channel.module_bindings.is_empty() {
                        view! {
                            <EmptyState
                                title="No module bindings yet."
                                body="Bindings are optional in v0. Add one when this channel should explicitly enable or disable a module surface."
                            />
                        }.into_any()
                    } else {
                        view! {
                            <div class="space-y-2">
                                {channel.module_bindings.iter().map(|binding| view! {
                                    <div class="rounded-lg border border-border px-3 py-2 text-sm">
                                        <div class="flex items-start justify-between gap-3">
                                            <div>
                                                <div class="font-medium text-card-foreground">{binding.module_slug.clone()}</div>
                                                <div class="mt-1 text-xs text-muted-foreground">{if binding.is_enabled { "enabled" } else { "disabled" }}</div>
                                            </div>
                                            <button
                                                type="button"
                                                class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition hover:bg-muted"
                                                disabled=move || busy.get()
                                                on:click={
                                                    let binding = binding.clone();
                                                    move |_| {
                                                        editing_module_slug.set(Some(binding.module_slug.clone()));
                                                        bind_module_slug.set(binding.module_slug.clone());
                                                        bind_module_enabled.set(binding.is_enabled);
                                                    }
                                                }
                                            >
                                                "Edit"
                                            </button>
                                            <button
                                                type="button"
                                                class="rounded-lg border border-rose-200 px-3 py-1 text-xs font-medium text-rose-700 transition hover:bg-rose-50 disabled:opacity-50"
                                                disabled=move || busy.get()
                                                on:click={
                                                    let binding = binding.clone();
                                                    let token = token_for_module_delete.clone();
                                                    let tenant = tenant_for_module_delete.clone();
                                                    let channel_id = channel_id_for_module_delete.clone();
                                                    let channel_slug = channel_slug_for_module_delete.clone();
                                                    move |_| {
                                                        busy.set(true);
                                                        set_feedback.set(None);
                                                        set_error.set(None);
                                                        spawn_local({
                                                            let binding = binding.clone();
                                                            let token = token.clone();
                                                            let tenant = tenant.clone();
                                                            let channel_id = channel_id.clone();
                                                            let channel_slug = channel_slug.clone();
                                                            async move {
                                                                let result = api::delete_module_binding(
                                                                    token,
                                                                    tenant,
                                                                    &channel_id,
                                                                    &binding.id,
                                                                )
                                                                .await;
                                                                match result {
                                                                    Ok(deleted) => {
                                                                        if editing_module_slug
                                                                            .get_untracked()
                                                                            .as_deref()
                                                                            == Some(binding.module_slug.as_str())
                                                                        {
                                                                            editing_module_slug.set(None);
                                                                            bind_module_slug.set(initial_module_slug.get_untracked());
                                                                            bind_module_enabled.set(true);
                                                                        }
                                                                        set_feedback.set(Some(format!(
                                                                            "Module binding `{}` removed from channel `{}`.",
                                                                            deleted.module_slug, channel_slug
                                                                        )));
                                                                        set_refresh_nonce.update(|value| *value += 1);
                                                                    }
                                                                    Err(err) => set_error.set(Some(err)),
                                                                }
                                                                busy.set(false);
                                                            }
                                                        });
                                                    }
                                                }
                                            >
                                                "Delete"
                                            </button>
                                        </div>
                                    </div>
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                    <form class="space-y-3" on:submit=bind_module_submit>
                        {if has_available_modules {
                            view! {
                                <select class="w-full rounded-lg border border-input bg-card px-3 py-2 text-sm" prop:value=bind_module_slug on:change=move |ev| bind_module_slug.set(event_target_value(&ev))>
                                    {available_modules.clone().into_iter().map(|item| {
                                        let label = format!("{} ({})", item.name, item.kind);
                                        let slug = item.slug;
                                        view! {
                                            <option value=slug.clone()>{label}</option>
                                        }
                                    }).collect_view()}
                                </select>
                            }.into_any()
                        } else {
                            view! {
                                <div class="rounded-lg border border-dashed border-border px-3 py-2 text-sm text-muted-foreground">
                                    "No module descriptors are currently available for binding."
                                </div>
                            }.into_any()
                        }}
                        <label class="flex items-center gap-2 text-sm text-muted-foreground">
                            <input type="checkbox" prop:checked=bind_module_enabled on:change=move |ev| bind_module_enabled.set(event_target_checked(&ev)) />
                            "Enabled for this channel"
                        </label>
                        <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get() || !has_available_modules>
                            {move || if editing_module_slug.get().is_some() { "Update Module Binding" } else { "Save Module Binding" }}
                        </button>
                    </form>
                </section>

                <section class="space-y-4 rounded-xl border border-border bg-background p-4">
                    <div class="flex items-center justify-between gap-3">
                        <h3 class="text-base font-semibold text-card-foreground">
                            {move || if editing_oauth_app_id.get().is_some() { "Edit OAuth App Binding" } else { "OAuth Apps" }}
                        </h3>
                        <Show when=move || editing_oauth_app_id.get().is_some()>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition hover:bg-muted"
                                on:click=cancel_oauth_edit
                            >
                                "Cancel"
                            </button>
                        </Show>
                    </div>
                    {if channel.oauth_apps.is_empty() {
                        view! {
                            <EmptyState
                                title="No OAuth app bindings yet."
                                body="Bind an existing OAuth app when this channel needs an integration-level relationship without introducing a second credential subsystem."
                            />
                        }.into_any()
                    } else {
                        view! {
                            <div class="space-y-2">
                                {channel.oauth_apps.iter().map(|binding| view! {
                                    <div class="rounded-lg border border-border px-3 py-2 text-sm">
                                        <div class="flex items-start justify-between gap-3">
                                            <div>
                                                <div class="font-medium text-card-foreground">{binding.oauth_app_id.clone()}</div>
                                                <div class="mt-1 text-xs text-muted-foreground">{binding.role.clone().unwrap_or_else(|| "no role".to_string())}</div>
                                            </div>
                                            <button
                                                type="button"
                                                class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition hover:bg-muted"
                                                disabled=move || busy.get()
                                                on:click={
                                                    let binding = binding.clone();
                                                    move |_| {
                                                        editing_oauth_app_id.set(Some(binding.oauth_app_id.clone()));
                                                        bind_oauth_app_id.set(binding.oauth_app_id.clone());
                                                        bind_oauth_role.set(binding.role.clone().unwrap_or_default());
                                                    }
                                                }
                                            >
                                                "Edit"
                                            </button>
                                            <button
                                                type="button"
                                                class="rounded-lg border border-rose-200 px-3 py-1 text-xs font-medium text-rose-700 transition hover:bg-rose-50 disabled:opacity-50"
                                                disabled=move || busy.get()
                                                on:click={
                                                    let binding = binding.clone();
                                                    let token = token_for_app_delete.clone();
                                                    let tenant = tenant_for_app_delete.clone();
                                                    let channel_id = channel_id_for_app_delete.clone();
                                                    let channel_slug = channel_slug_for_app_delete.clone();
                                                    move |_| {
                                                        busy.set(true);
                                                        set_feedback.set(None);
                                                        set_error.set(None);
                                                        spawn_local({
                                                            let binding = binding.clone();
                                                            let token = token.clone();
                                                            let tenant = tenant.clone();
                                                            let channel_id = channel_id.clone();
                                                            let channel_slug = channel_slug.clone();
                                                            async move {
                                                                let result = api::delete_oauth_app_binding(
                                                                    token,
                                                                    tenant,
                                                                    &channel_id,
                                                                    &binding.id,
                                                                )
                                                                .await;
                                                                match result {
                                                                    Ok(deleted) => {
                                                                        if editing_oauth_app_id
                                                                            .get_untracked()
                                                                            .as_deref()
                                                                            == Some(binding.oauth_app_id.as_str())
                                                                        {
                                                                            editing_oauth_app_id.set(None);
                                                                            bind_oauth_app_id.set(initial_oauth_app_id.get_untracked());
                                                                            bind_oauth_role.set(String::new());
                                                                        }
                                                                        set_feedback.set(Some(format!(
                                                                            "OAuth app binding `{}` revoked for channel `{}`.",
                                                                            deleted.oauth_app_id, channel_slug
                                                                        )));
                                                                        set_refresh_nonce.update(|value| *value += 1);
                                                                    }
                                                                    Err(err) => set_error.set(Some(err)),
                                                                }
                                                                busy.set(false);
                                                            }
                                                        });
                                                    }
                                                }
                                            >
                                                "Revoke"
                                            </button>
                                        </div>
                                    </div>
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                    <form class="space-y-3" on:submit=bind_oauth_submit>
                        {if has_available_oauth_apps {
                            view! {
                                <select class="w-full rounded-lg border border-input bg-card px-3 py-2 text-sm" prop:value=bind_oauth_app_id on:change=move |ev| bind_oauth_app_id.set(event_target_value(&ev))>
                                    {oauth_apps.clone().into_iter().map(|item| {
                                        let label = format!("{} ({})", item.name, item.app_type);
                                        let id = item.id;
                                        view! {
                                            <option value=id.clone()>{label}</option>
                                        }
                                    }).collect_view()}
                                </select>
                            }.into_any()
                        } else {
                            view! {
                                <div class="rounded-lg border border-dashed border-border px-3 py-2 text-sm text-muted-foreground">
                                    "No active OAuth apps are available for this tenant yet."
                                </div>
                            }.into_any()
                        }}
                        <input type="text" class="w-full rounded-lg border border-input bg-card px-3 py-2 text-sm" placeholder="role (optional)" prop:value=bind_oauth_role on:input=move |ev| bind_oauth_role.set(event_target_value(&ev)) />
                        <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get() || !has_available_oauth_apps>
                            {move || if editing_oauth_app_id.get().is_some() { "Update OAuth App Binding" } else { "Bind OAuth App" }}
                        </button>
                    </form>
                </section>
            </div>
        </article>
    }
}

#[component]
fn InfoPill(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="rounded-xl border border-border bg-background px-4 py-3">
            <div class="text-xs font-medium uppercase tracking-wide text-muted-foreground">{label}</div>
            <div class="mt-1 text-sm font-medium text-card-foreground">{value}</div>
        </div>
    }
}

#[component]
fn EmptyState(title: &'static str, body: &'static str) -> impl IntoView {
    view! {
        <div class="rounded-lg border border-dashed border-border px-3 py-4 text-sm">
            <div class="font-medium text-card-foreground">{title}</div>
            <div class="mt-1 text-muted-foreground">{body}</div>
        </div>
    }
}

fn short_id(value: &str) -> String {
    value.chars().take(8).collect()
}

fn optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn resolution_source_label(source: &ChannelResolutionSource) -> String {
    match source {
        ChannelResolutionSource::HeaderId => "Header ID".to_string(),
        ChannelResolutionSource::HeaderSlug => "Header Slug".to_string(),
        ChannelResolutionSource::Query => "Query".to_string(),
        ChannelResolutionSource::Host => "Host".to_string(),
        ChannelResolutionSource::Default => "Default".to_string(),
    }
}

fn resolution_source_description(source: &ChannelResolutionSource) -> &'static str {
    match source {
        ChannelResolutionSource::HeaderId => {
            "The current request explicitly selected this channel through the X-Channel-ID header."
        }
        ChannelResolutionSource::HeaderSlug => {
            "The current request explicitly selected this channel through the X-Channel-Slug header."
        }
        ChannelResolutionSource::Query => {
            "The current request selected this channel through the query parameter fallback."
        }
        ChannelResolutionSource::Host => {
            "The current request matched this channel through host-based target resolution."
        }
        ChannelResolutionSource::Default => {
            "No explicit channel selector matched, so the tenant's explicit default channel was used."
        }
    }
}
