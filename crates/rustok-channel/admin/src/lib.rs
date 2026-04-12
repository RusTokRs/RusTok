mod api;
mod i18n;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_api::context::ChannelResolutionSource;
use rustok_api::{AdminQueryKey, UiRouteContext};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};

use crate::i18n::t;
use crate::model::{
    BindChannelModulePayload, BindChannelOauthAppPayload, ChannelAdminBootstrap, ChannelDetail,
    CreateChannelPayload, CreateChannelTargetPayload,
};

#[component]
pub fn ChannelAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let selected_channel_query = use_route_query_value(AdminQueryKey::ChannelId.as_str());
    let query_writer = use_route_query_writer();
    let token = leptos_auth::hooks::use_token();
    let tenant = leptos_auth::hooks::use_tenant();
    let badge_label = t(ui_locale.as_deref(), "channel.badge", "Experimental Core");
    let title_label = t(ui_locale.as_deref(), "channel.title", "Channel Management");
    let subtitle_label = t(
        ui_locale.as_deref(),
        "channel.subtitle",
        "Channels define platform-level external delivery context, targets, enabled module surfaces, and bound OAuth apps.",
    );
    let route_label = t(ui_locale.as_deref(), "channel.route", "Route: {route}");
    let create_title = t(
        ui_locale.as_deref(),
        "channel.create.title",
        "Create Channel",
    );
    let create_subtitle = t(
        ui_locale.as_deref(),
        "channel.create.subtitle",
        "Start small: create the channel first, then attach targets and bindings below.",
    );
    let slug_placeholder = t(
        ui_locale.as_deref(),
        "channel.create.slugPlaceholder",
        "slug",
    );
    let name_placeholder = t(
        ui_locale.as_deref(),
        "channel.create.namePlaceholder",
        "name",
    );
    let creating_label = t(
        ui_locale.as_deref(),
        "channel.create.creating",
        "Creating...",
    );
    let create_label = t(ui_locale.as_deref(), "channel.create.submit", "Create");
    let empty_channels_label = t(
        ui_locale.as_deref(),
        "channel.empty.channels",
        "No channels configured yet.",
    );
    let load_bootstrap_error = t(
        ui_locale.as_deref(),
        "channel.error.loadBootstrap",
        "Failed to load channel bootstrap",
    );

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
    let create_channel_query_writer = query_writer.clone();

    Effect::new(move |_| {
        let selected_channel_id = selected_channel_query.get();
        match (selected_channel_id.as_deref(), bootstrap.get()) {
            (Some(channel_id), Some(Ok(bootstrap)))
                if !bootstrap.channels.iter().any(|channel| channel.channel.id == channel_id) =>
            {
                query_writer.clear_key(AdminQueryKey::ChannelId.as_str());
            }
            _ => {}
        }
    });

    let on_create = move |ev: SubmitEvent| {
        ev.prevent_default();
        create_busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        let ui_locale = ui_locale.clone();
        let create_channel_query_writer = create_channel_query_writer.clone();

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
                        set_feedback.set(Some(
                            t(
                                ui_locale.as_deref(),
                                "channel.feedback.created",
                                "Channel `{slug}` created.",
                            )
                            .replace("{slug}", channel.slug.as_str()),
                        ));
                        create_slug.set(String::new());
                        create_name.set(String::new());
                        create_channel_query_writer.replace_value(
                            AdminQueryKey::ChannelId.as_str(),
                            channel.id.clone(),
                        );
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_error.set(Some(err.to_string())),
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
                            {badge_label.clone()}
                        </span>
                        <h1 class="text-2xl font-semibold text-card-foreground">{title_label.clone()}</h1>
                        <p class="max-w-3xl text-sm text-muted-foreground">
                            {subtitle_label.clone()}
                        </p>
                    </div>
                    <div class="rounded-xl border border-border bg-background px-4 py-3 text-sm text-muted-foreground">
                        {route_label.replace("{route}", format!("/modules/{route_segment}").as_str())}
                    </div>
                </div>
            </header>

            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h2 class="text-lg font-semibold text-card-foreground">{create_title.clone()}</h2>
                    <p class="text-sm text-muted-foreground">
                        {create_subtitle.clone()}
                    </p>
                </div>
                <form class="mt-5 grid gap-4 lg:grid-cols-[1fr_1fr_auto]" on:submit=on_create>
                    <input
                        type="text"
                        class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                        placeholder=slug_placeholder.clone()
                        prop:value=create_slug
                        on:input=move |ev| create_slug.set(event_target_value(&ev))
                    />
                    <input
                        type="text"
                        class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                        placeholder=name_placeholder.clone()
                        prop:value=create_name
                        on:input=move |ev| create_name.set(event_target_value(&ev))
                    />
                    <button
                        type="submit"
                        class="inline-flex h-10 items-center justify-center rounded-lg bg-primary px-4 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                        disabled=move || create_busy.get()
                    >
                        {move || if create_busy.get() { creating_label.clone() } else { create_label.clone() }}
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
                                            {empty_channels_label.clone()}
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
                                {format!("{}: {err}", load_bootstrap_error.clone())}
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
    let ui_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    view! {
        <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
            <div class="space-y-1">
                <h2 class="text-lg font-semibold text-card-foreground">
                    {t(ui_locale.as_deref(), "channel.runtime.title", "Runtime Context")}
                </h2>
                <p class="text-sm text-muted-foreground">
                    {t(
                        ui_locale.as_deref(),
                        "channel.runtime.subtitle",
                        "Channel resolved by middleware for the current request.",
                    )}
                </p>
            </div>
            {match bootstrap.current_channel {
                Some(current) => view! {
                    <div class="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-5">
                        <InfoPill label=t(ui_locale.as_deref(), "channel.runtime.slug", "Slug") value=current.slug />
                        <InfoPill label=t(ui_locale.as_deref(), "channel.runtime.name", "Name") value=current.name />
                        <InfoPill label=t(ui_locale.as_deref(), "channel.runtime.source", "Source") value=resolution_source_label(&current.resolution_source, ui_locale.as_deref()) />
                        <InfoPill label=t(ui_locale.as_deref(), "channel.runtime.target", "Target") value=current.target_value.unwrap_or_else(|| t(ui_locale.as_deref(), "channel.runtime.na", "n/a")) />
                        <InfoPill label=t(ui_locale.as_deref(), "channel.runtime.type", "Type") value=current.target_type.unwrap_or_else(|| t(ui_locale.as_deref(), "channel.runtime.na", "n/a")) />
                    </div>
                    <div class="mt-4 rounded-xl border border-sky-200 bg-sky-50 px-4 py-3 text-sm text-sky-800">
                        {resolution_source_description(&current.resolution_source, ui_locale.as_deref())}
                    </div>
                }.into_any(),
                None => view! {
                    <div class="mt-4 rounded-xl border border-dashed border-border px-4 py-3 text-sm text-muted-foreground">
                        {t(
                            ui_locale.as_deref(),
                            "channel.runtime.empty",
                            "No channel was resolved for the current request yet.",
                        )}
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
    let ui_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let selected_channel_query = use_route_query_value(AdminQueryKey::ChannelId.as_str());
    let selected_target_query = use_route_query_value(AdminQueryKey::TargetId.as_str());
    let selected_module_query = use_route_query_value(AdminQueryKey::ModuleSlug.as_str());
    let selected_oauth_query = use_route_query_value(AdminQueryKey::OauthAppId.as_str());
    let query_writer = use_route_query_writer();
    let common_cancel_label = t(ui_locale.as_deref(), "common.cancel", "Cancel");
    let targets_cancel_label = common_cancel_label.clone();
    let modules_cancel_label = common_cancel_label.clone();
    let oauth_cancel_label = common_cancel_label.clone();
    let common_edit_label = t(ui_locale.as_deref(), "common.edit", "Edit");
    let common_delete_label = t(ui_locale.as_deref(), "common.delete", "Delete");
    let targets_edit_title = t(
        ui_locale.as_deref(),
        "channel.targets.editTitle",
        "Edit Target",
    );
    let targets_title_label = t(ui_locale.as_deref(), "channel.targets.title", "Targets");
    let targets_empty_title = t(
        ui_locale.as_deref(),
        "channel.targets.emptyTitle",
        "No targets yet.",
    );
    let targets_empty_body = t(
        ui_locale.as_deref(),
        "channel.targets.emptyBody",
        "Add the first target to make this channel discoverable through a concrete delivery surface.",
    );
    let targets_value_placeholder = t(
        ui_locale.as_deref(),
        "channel.targets.valuePlaceholder",
        "example.com or app id",
    );
    let targets_primary_label = t(
        ui_locale.as_deref(),
        "channel.targets.primary",
        "Primary target",
    );
    let targets_save_label = t(ui_locale.as_deref(), "channel.targets.save", "Save Target");
    let targets_add_label = t(ui_locale.as_deref(), "channel.targets.add", "Add Target");
    let _targets_primary_summary_template = t(
        ui_locale.as_deref(),
        "channel.targets.primarySummary",
        "{type} · primary",
    );
    let target_removed_template = t(
        ui_locale.as_deref(),
        "channel.feedback.targetRemoved",
        "Target `{target}` removed from channel `{channel}`.",
    );
    let modules_edit_title = t(
        ui_locale.as_deref(),
        "channel.modules.editTitle",
        "Edit Module Binding",
    );
    let modules_title_label = t(
        ui_locale.as_deref(),
        "channel.modules.title",
        "Module Bindings",
    );
    let modules_empty_title = t(
        ui_locale.as_deref(),
        "channel.modules.emptyTitle",
        "No module bindings yet.",
    );
    let modules_empty_body = t(
        ui_locale.as_deref(),
        "channel.modules.emptyBody",
        "Bindings are optional in v0. Add one when this channel should explicitly enable or disable a module surface.",
    );
    let modules_enabled_label = t(ui_locale.as_deref(), "channel.modules.enabled", "enabled");
    let modules_disabled_label = t(ui_locale.as_deref(), "channel.modules.disabled", "disabled");
    let modules_no_descriptors_label = t(
        ui_locale.as_deref(),
        "channel.modules.noDescriptors",
        "No module descriptors are currently available for binding.",
    );
    let modules_enabled_for_channel_label = t(
        ui_locale.as_deref(),
        "channel.modules.enabledForChannel",
        "Enabled for this channel",
    );
    let modules_update_label = t(
        ui_locale.as_deref(),
        "channel.modules.update",
        "Update Module Binding",
    );
    let modules_save_label = t(
        ui_locale.as_deref(),
        "channel.modules.save",
        "Save Module Binding",
    );
    let module_removed_template = t(
        ui_locale.as_deref(),
        "channel.feedback.moduleRemoved",
        "Module binding `{module}` removed from channel `{channel}`.",
    );
    let oauth_edit_title = t(
        ui_locale.as_deref(),
        "channel.oauth.editTitle",
        "Edit OAuth App Binding",
    );
    let oauth_title_label = t(ui_locale.as_deref(), "channel.oauth.title", "OAuth Apps");
    let oauth_empty_title = t(
        ui_locale.as_deref(),
        "channel.oauth.emptyTitle",
        "No OAuth app bindings yet.",
    );
    let oauth_empty_body = t(
        ui_locale.as_deref(),
        "channel.oauth.emptyBody",
        "Bind an existing OAuth app when this channel needs an integration-level relationship without introducing a second credential subsystem.",
    );
    let oauth_no_role_label = t(ui_locale.as_deref(), "channel.oauth.noRole", "no role");
    let oauth_revoke_label = t(ui_locale.as_deref(), "channel.oauth.revoke", "Revoke");
    let oauth_no_apps_label = t(
        ui_locale.as_deref(),
        "channel.oauth.noApps",
        "No active OAuth apps are available for this tenant yet.",
    );
    let oauth_role_placeholder = t(
        ui_locale.as_deref(),
        "channel.oauth.rolePlaceholder",
        "role (optional)",
    );
    let oauth_update_label = t(
        ui_locale.as_deref(),
        "channel.oauth.update",
        "Update OAuth App Binding",
    );
    let oauth_bind_label = t(ui_locale.as_deref(), "channel.oauth.bind", "Bind OAuth App");
    let oauth_revoked_template = t(
        ui_locale.as_deref(),
        "channel.feedback.oauthRevoked",
        "OAuth app binding `{app}` revoked for channel `{channel}`.",
    );
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
    let channel_targets = channel.targets.clone();
    let channel_module_bindings = channel.module_bindings.clone();
    let channel_oauth_bindings = channel.oauth_apps.clone();
    let is_selected_channel = Signal::derive({
        let channel_id = channel_id.clone();
        move || selected_channel_query.get().as_deref() == Some(channel_id.as_str())
    });
    let cancel_target_query_writer = query_writer.clone();
    let cancel_module_query_writer = query_writer.clone();
    let cancel_oauth_query_writer = query_writer.clone();
    let select_channel_query_writer = query_writer.clone();
    let create_target_query_writer = query_writer.clone();
    let bind_module_query_writer = query_writer.clone();
    let bind_oauth_query_writer = query_writer.clone();
    let target_edit_query_writer = query_writer.clone();
    let target_delete_query_writer = query_writer.clone();
    let module_edit_query_writer = query_writer.clone();
    let module_delete_query_writer = query_writer.clone();
    let oauth_edit_query_writer = query_writer.clone();
    let oauth_delete_query_writer = query_writer.clone();
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
    let select_channel_button_writer = select_channel_query_writer.clone();
    let select_channel_button_id = channel_id_for_default.clone();
    let select_button_locale = ui_locale.clone();
    let target_edit_channel_id = channel_id_for_target.clone();
    let module_edit_channel_id = channel_id_for_module.clone();
    let oauth_edit_channel_id = channel_id_for_app.clone();
    let selection_query_writer = query_writer.clone();
    Effect::new(move |_| {
        if !is_selected_channel.get() {
            editing_target_id.set(None);
            editing_module_slug.set(None);
            editing_oauth_app_id.set(None);
            target_type.set("web_domain".to_string());
            target_value.set(String::new());
            target_primary.set(true);
            bind_module_slug.set(initial_module_slug.get_untracked());
            bind_module_enabled.set(true);
            bind_oauth_app_id.set(initial_oauth_app_id.get_untracked());
            bind_oauth_role.set(String::new());
            return;
        }

        match selected_target_query.get().as_deref().filter(|value| !value.trim().is_empty()) {
            Some(target_id) => {
                if let Some(target) = channel_targets.iter().find(|target| target.id == target_id) {
                    editing_target_id.set(Some(target.id.clone()));
                    target_type.set(target.target_type.clone());
                    target_value.set(target.value.clone());
                    target_primary.set(target.is_primary);
                } else {
                    editing_target_id.set(None);
                    target_type.set("web_domain".to_string());
                    target_value.set(String::new());
                    target_primary.set(true);
                    selection_query_writer.clear_key(AdminQueryKey::TargetId.as_str());
                }
            }
            None => {
                editing_target_id.set(None);
                target_type.set("web_domain".to_string());
                target_value.set(String::new());
                target_primary.set(true);
            }
        }

        match selected_module_query.get().as_deref().filter(|value| !value.trim().is_empty()) {
            Some(module_slug) => {
                if let Some(binding) = channel_module_bindings
                    .iter()
                    .find(|binding| binding.module_slug == module_slug)
                {
                    editing_module_slug.set(Some(binding.module_slug.clone()));
                    bind_module_slug.set(binding.module_slug.clone());
                    bind_module_enabled.set(binding.is_enabled);
                } else {
                    editing_module_slug.set(None);
                    bind_module_slug.set(initial_module_slug.get_untracked());
                    bind_module_enabled.set(true);
                    selection_query_writer.clear_key(AdminQueryKey::ModuleSlug.as_str());
                }
            }
            None => {
                editing_module_slug.set(None);
                bind_module_slug.set(initial_module_slug.get_untracked());
                bind_module_enabled.set(true);
            }
        }

        match selected_oauth_query.get().as_deref().filter(|value| !value.trim().is_empty()) {
            Some(oauth_app_id) => {
                if let Some(binding) = channel_oauth_bindings
                    .iter()
                    .find(|binding| binding.oauth_app_id == oauth_app_id)
                {
                    editing_oauth_app_id.set(Some(binding.oauth_app_id.clone()));
                    bind_oauth_app_id.set(binding.oauth_app_id.clone());
                    bind_oauth_role.set(binding.role.clone().unwrap_or_default());
                } else {
                    editing_oauth_app_id.set(None);
                    bind_oauth_app_id.set(initial_oauth_app_id.get_untracked());
                    bind_oauth_role.set(String::new());
                    selection_query_writer.clear_key(AdminQueryKey::OauthAppId.as_str());
                }
            }
            None => {
                editing_oauth_app_id.set(None);
                bind_oauth_app_id.set(initial_oauth_app_id.get_untracked());
                bind_oauth_role.set(String::new());
            }
        }
    });

    let make_default_locale = ui_locale.clone();
    let make_default = move |_| {
        busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        select_channel_query_writer.replace_value(
            AdminQueryKey::ChannelId.as_str(),
            channel_id_for_default.clone(),
        );
        spawn_local({
            let token = token_for_default.clone();
            let tenant = tenant_for_default.clone();
            let channel_id = channel_id_for_default.clone();
            let ui_locale = make_default_locale.clone();
            async move {
                let result = api::make_default_channel(token, tenant, &channel_id).await;
                match result {
                    Ok(channel) => {
                        set_feedback.set(Some(
                            t(
                                ui_locale.as_deref(),
                                "channel.feedback.default",
                                "Channel `{slug}` is now the tenant default channel.",
                            )
                            .replace("{slug}", channel.slug.as_str()),
                        ));
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_error.set(Some(err.to_string())),
                }
                busy.set(false);
            }
        });
    };

    let create_target_locale = ui_locale.clone();
    let create_target = move |ev: SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        let create_target_query_writer = create_target_query_writer.clone();
        spawn_local({
            let token = token_for_target.clone();
            let tenant = tenant_for_target.clone();
            let channel_id = channel_id_for_target.clone();
            let channel_slug = channel_slug_for_target.clone();
            let editing_target_id_value = editing_target_id.get_untracked();
            let ui_locale = create_target_locale.clone();
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
                        let message = if editing_target_id_value.is_some() {
                            t(
                                ui_locale.as_deref(),
                                "channel.feedback.targetUpdated",
                                "Target `{target}` updated for channel `{channel}`.",
                            )
                        } else {
                            t(
                                ui_locale.as_deref(),
                                "channel.feedback.targetAdded",
                                "Target `{target}` added to channel `{channel}`.",
                            )
                        };
                        set_feedback.set(Some(
                            message
                                .replace("{target}", target.value.as_str())
                                .replace("{channel}", channel_slug.as_str()),
                        ));
                        create_target_query_writer.update(
                            vec![
                                (
                                    AdminQueryKey::ChannelId.as_str().to_string(),
                                    Some(channel_id.clone()),
                                ),
                                (
                                    AdminQueryKey::TargetId.as_str().to_string(),
                                    Some(target.id.clone()),
                                ),
                            ],
                            true,
                        );
                        editing_target_id.set(None);
                        target_type.set("web_domain".to_string());
                        target_value.set(String::new());
                        target_primary.set(true);
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_error.set(Some(err.to_string())),
                }
                busy.set(false);
            }
        });
    };

    let bind_module_locale = ui_locale.clone();
    let bind_module_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        let bind_module_query_writer = bind_module_query_writer.clone();
        spawn_local({
            let token = token_for_module.clone();
            let tenant = tenant_for_module.clone();
            let channel_id = channel_id_for_module.clone();
            let channel_slug = channel_slug_for_module.clone();
            let ui_locale = bind_module_locale.clone();
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
                        let message = if editing_module_slug.get_untracked().is_some() {
                            t(
                                ui_locale.as_deref(),
                                "channel.feedback.moduleUpdated",
                                "Module binding updated for channel `{channel}`.",
                            )
                        } else {
                            t(
                                ui_locale.as_deref(),
                                "channel.feedback.moduleSaved",
                                "Module binding saved for channel `{channel}`.",
                            )
                        };
                        set_feedback.set(Some(message.replace("{channel}", channel_slug.as_str())));
                        bind_module_query_writer.update(
                            vec![
                                (
                                    AdminQueryKey::ChannelId.as_str().to_string(),
                                    Some(channel_id.clone()),
                                ),
                                (
                                    AdminQueryKey::ModuleSlug.as_str().to_string(),
                                    Some(bind_module_slug.get_untracked()),
                                ),
                            ],
                            true,
                        );
                        editing_module_slug.set(None);
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_error.set(Some(err.to_string())),
                }
                busy.set(false);
            }
        });
    };

    let bind_oauth_locale = ui_locale.clone();
    let bind_oauth_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        let bind_oauth_query_writer = bind_oauth_query_writer.clone();
        spawn_local({
            let token = token_for_app.clone();
            let tenant = tenant_for_app.clone();
            let channel_id = channel_id_for_app.clone();
            let channel_slug = channel_slug_for_app.clone();
            let ui_locale = bind_oauth_locale.clone();
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
                        let message = if editing_oauth_app_id.get_untracked().is_some() {
                            t(
                                ui_locale.as_deref(),
                                "channel.feedback.oauthUpdated",
                                "OAuth app binding updated for channel `{channel}`.",
                            )
                        } else {
                            t(
                                ui_locale.as_deref(),
                                "channel.feedback.oauthSaved",
                                "OAuth app binding saved for channel `{channel}`.",
                            )
                        };
                        set_feedback.set(Some(message.replace("{channel}", channel_slug.as_str())));
                        bind_oauth_query_writer.update(
                            vec![
                                (
                                    AdminQueryKey::ChannelId.as_str().to_string(),
                                    Some(channel_id.clone()),
                                ),
                                (
                                    AdminQueryKey::OauthAppId.as_str().to_string(),
                                    Some(bind_oauth_app_id.get_untracked()),
                                ),
                            ],
                            true,
                        );
                        editing_oauth_app_id.set(None);
                        bind_oauth_role.set(String::new());
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_error.set(Some(err.to_string())),
                }
                busy.set(false);
            }
        });
    };

    view! {
        <article
            class="rounded-2xl border border-border bg-card p-6 shadow-sm"
            class=("ring-2 ring-primary/30", move || is_selected_channel.get())
        >
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
                                    {t(ui_locale.as_deref(), "channel.card.default", "Default")}
                                </span>
                            }.into_any()
                        } else {
                            ().into_any()
                        }}
                    </div>
                    <h2 class="text-xl font-semibold text-card-foreground">{channel.channel.name.clone()}</h2>
                    <p class="text-sm text-muted-foreground">
                        {t(
                            ui_locale.as_deref(),
                            "channel.card.summary",
                            "{targets} target(s), {modules} module binding(s), {apps} app binding(s)",
                        )
                        .replace("{targets}", channel.targets.len().to_string().as_str())
                        .replace("{modules}", channel.module_bindings.len().to_string().as_str())
                        .replace("{apps}", channel.oauth_apps.len().to_string().as_str())}
                    </p>
                </div>
                <div class="space-y-3">
                    <button
                        type="button"
                        class="inline-flex h-10 items-center justify-center rounded-lg border border-border bg-background px-4 text-sm font-medium text-card-foreground transition hover:bg-muted disabled:opacity-50"
                        disabled=move || busy.get()
                        on:click={
                            let channel_id = select_channel_button_id.clone();
                            let query_writer = select_channel_button_writer.clone();
                            move |_| {
                                query_writer.replace_value(
                                    AdminQueryKey::ChannelId.as_str(),
                                    channel_id.clone(),
                                );
                            }
                        }
                    >
                        {move || if is_selected_channel.get() {
                            t(select_button_locale.as_deref(), "channel.card.selected", "Selected")
                        } else {
                            t(select_button_locale.as_deref(), "channel.card.select", "Select")
                        }}
                    </button>
                    {if is_default_channel {
                        view! {
                            <div class="rounded-lg border border-sky-200 bg-sky-50 px-4 py-3 text-sm text-sky-800">
                                {t(
                                    ui_locale.as_deref(),
                                    "channel.card.defaultDescription",
                                    "Used as the tenant's explicit default channel when no header, query or host selector matches.",
                                )}
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
                                {t(ui_locale.as_deref(), "channel.card.makeDefault", "Make Default")}
                            </button>
                        }.into_any()
                    }}
                    <div class="grid gap-2 md:grid-cols-2">
                    <InfoPill label=t(ui_locale.as_deref(), "channel.card.id", "ID") value=short_id(&channel.channel.id) />
                    <InfoPill label=t(ui_locale.as_deref(), "channel.card.updated", "Updated") value=channel.channel.updated_at.clone() />
                    </div>
                </div>
            </div>

            <div class="mt-6 grid gap-6 xl:grid-cols-3">
                <section class="space-y-4 rounded-xl border border-border bg-background p-4">
                    <div class="flex items-center justify-between gap-3">
                        <h3 class="text-base font-semibold text-card-foreground">
                            {move || if editing_target_id.get().is_some() {
                                targets_edit_title.clone()
                            } else {
                                targets_title_label.clone()
                            }}
                        </h3>
                        <Show when=move || editing_target_id.get().is_some()>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition hover:bg-muted"
                                on:click={
                                    let query_writer = cancel_target_query_writer.clone();
                                    move |_| {
                                        query_writer.clear_key(AdminQueryKey::TargetId.as_str());
                                        editing_target_id.set(None);
                                        target_type.set("web_domain".to_string());
                                        target_value.set(String::new());
                                        target_primary.set(true);
                                    }
                                }
                            >
                                {targets_cancel_label.clone()}
                            </button>
                        </Show>
                    </div>
                    {if channel.targets.is_empty() {
                        view! {
                            <EmptyState
                                title=targets_empty_title.clone()
                                body=targets_empty_body.clone()
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
                                                    let channel_id = target_edit_channel_id.clone();
                                                    let query_writer = target_edit_query_writer.clone();
                                                    move |_| {
                                                        query_writer.update(
                                                            vec![
                                                                (
                                                                    AdminQueryKey::ChannelId.as_str().to_string(),
                                                                    Some(channel_id.clone()),
                                                                ),
                                                                (
                                                                    AdminQueryKey::TargetId.as_str().to_string(),
                                                                    Some(target.id.clone()),
                                                                ),
                                                            ],
                                                            false,
                                                        );
                                                    }
                                                }
                                            >
                                                {common_edit_label.clone()}
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
                                                    let target_removed_template = target_removed_template.clone();
                                                    let query_writer = target_delete_query_writer.clone();
                                                    move |_| {
                                                        let target_removed_template = target_removed_template.clone();
                                                        let query_writer = query_writer.clone();
                                                        busy.set(true);
                                                        set_feedback.set(None);
                                                        set_error.set(None);
                                                        spawn_local({
                                                            let target = target.clone();
                                                            let token = token.clone();
                                                            let tenant = tenant.clone();
                                                            let channel_id = channel_id.clone();
                                                            let channel_slug = channel_slug.clone();
                                                            let target_removed_template = target_removed_template.clone();
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
                                                                            query_writer.clear_key(AdminQueryKey::TargetId.as_str());
                                                                            editing_target_id.set(None);
                                                                            target_type.set("web_domain".to_string());
                                                                            target_value.set(String::new());
                                                                            target_primary.set(true);
                                                                        }
                                                                        set_feedback.set(Some(
                                                                            target_removed_template
                                                                                .replace("{target}", deleted.value.as_str())
                                                                                .replace("{channel}", channel_slug.as_str()),
                                                                        ));
                                                                        set_refresh_nonce.update(|value| *value += 1);
                                                                    }
                                                                    Err(err) => set_error.set(Some(err.to_string())),
                                                                }
                                                                busy.set(false);
                                                            }
                                                        });
                                                    }
                                                }
                                            >
                                                {common_delete_label.clone()}
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
                        <input type="text" class="w-full rounded-lg border border-input bg-card px-3 py-2 text-sm" placeholder=targets_value_placeholder.clone() prop:value=target_value on:input=move |ev| target_value.set(event_target_value(&ev)) />
                        <label class="flex items-center gap-2 text-sm text-muted-foreground">
                            <input type="checkbox" prop:checked=target_primary on:change=move |ev| target_primary.set(event_target_checked(&ev)) />
                            {targets_primary_label.clone()}
                        </label>
                        <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>
                            {move || if editing_target_id.get().is_some() {
                                targets_save_label.clone()
                            } else {
                                targets_add_label.clone()
                            }}
                        </button>
                    </form>
                </section>

                <section class="space-y-4 rounded-xl border border-border bg-background p-4">
                    <div class="flex items-center justify-between gap-3">
                        <h3 class="text-base font-semibold text-card-foreground">
                            {move || if editing_module_slug.get().is_some() {
                                modules_edit_title.clone()
                            } else {
                                modules_title_label.clone()
                            }}
                        </h3>
                        <Show when=move || editing_module_slug.get().is_some()>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition hover:bg-muted"
                                on:click={
                                    let query_writer = cancel_module_query_writer.clone();
                                    move |_| {
                                        query_writer.clear_key(AdminQueryKey::ModuleSlug.as_str());
                                        editing_module_slug.set(None);
                                        bind_module_slug.set(initial_module_slug.get_untracked());
                                        bind_module_enabled.set(true);
                                    }
                                }
                            >
                                {modules_cancel_label.clone()}
                            </button>
                        </Show>
                    </div>
                    {if channel.module_bindings.is_empty() {
                        view! {
                            <EmptyState
                                title=modules_empty_title.clone()
                                body=modules_empty_body.clone()
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
                                                <div class="mt-1 text-xs text-muted-foreground">
                                                    {if binding.is_enabled {
                                                        modules_enabled_label.clone()
                                                    } else {
                                                        modules_disabled_label.clone()
                                                    }}
                                                </div>
                                            </div>
                                            <button
                                                type="button"
                                                class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition hover:bg-muted"
                                                disabled=move || busy.get()
                                                on:click={
                                                    let binding = binding.clone();
                                                    let channel_id = module_edit_channel_id.clone();
                                                    let query_writer = module_edit_query_writer.clone();
                                                    move |_| {
                                                        query_writer.update(
                                                            vec![
                                                                (
                                                                    AdminQueryKey::ChannelId.as_str().to_string(),
                                                                    Some(channel_id.clone()),
                                                                ),
                                                                (
                                                                    AdminQueryKey::ModuleSlug.as_str().to_string(),
                                                                    Some(binding.module_slug.clone()),
                                                                ),
                                                            ],
                                                            false,
                                                        );
                                                    }
                                                }
                                            >
                                                {common_edit_label.clone()}
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
                                                    let module_removed_template = module_removed_template.clone();
                                                    let query_writer = module_delete_query_writer.clone();
                                                    move |_| {
                                                        let module_removed_template = module_removed_template.clone();
                                                        let query_writer = query_writer.clone();
                                                        busy.set(true);
                                                        set_feedback.set(None);
                                                        set_error.set(None);
                                                        spawn_local({
                                                            let binding = binding.clone();
                                                            let token = token.clone();
                                                            let tenant = tenant.clone();
                                                            let channel_id = channel_id.clone();
                                                            let channel_slug = channel_slug.clone();
                                                            let module_removed_template = module_removed_template.clone();
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
                                                                            query_writer.clear_key(AdminQueryKey::ModuleSlug.as_str());
                                                                            editing_module_slug.set(None);
                                                                            bind_module_slug.set(initial_module_slug.get_untracked());
                                                                            bind_module_enabled.set(true);
                                                                        }
                                                                        set_feedback.set(Some(
                                                                            module_removed_template
                                                                                .replace("{module}", deleted.module_slug.as_str())
                                                                                .replace("{channel}", channel_slug.as_str()),
                                                                        ));
                                                                        set_refresh_nonce.update(|value| *value += 1);
                                                                    }
                                                                    Err(err) => set_error.set(Some(err.to_string())),
                                                                }
                                                                busy.set(false);
                                                            }
                                                        });
                                                    }
                                                }
                                            >
                                                {common_delete_label.clone()}
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
                                    {modules_no_descriptors_label.clone()}
                                </div>
                            }.into_any()
                        }}
                        <label class="flex items-center gap-2 text-sm text-muted-foreground">
                            <input type="checkbox" prop:checked=bind_module_enabled on:change=move |ev| bind_module_enabled.set(event_target_checked(&ev)) />
                            {modules_enabled_for_channel_label.clone()}
                        </label>
                        <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get() || !has_available_modules>
                            {move || if editing_module_slug.get().is_some() {
                                modules_update_label.clone()
                            } else {
                                modules_save_label.clone()
                            }}
                        </button>
                    </form>
                </section>

                <section class="space-y-4 rounded-xl border border-border bg-background p-4">
                    <div class="flex items-center justify-between gap-3">
                        <h3 class="text-base font-semibold text-card-foreground">
                            {move || if editing_oauth_app_id.get().is_some() {
                                oauth_edit_title.clone()
                            } else {
                                oauth_title_label.clone()
                            }}
                        </h3>
                        <Show when=move || editing_oauth_app_id.get().is_some()>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition hover:bg-muted"
                                on:click={
                                    let query_writer = cancel_oauth_query_writer.clone();
                                    move |_| {
                                        query_writer.clear_key(AdminQueryKey::OauthAppId.as_str());
                                        editing_oauth_app_id.set(None);
                                        bind_oauth_app_id.set(initial_oauth_app_id.get_untracked());
                                        bind_oauth_role.set(String::new());
                                    }
                                }
                            >
                                {oauth_cancel_label.clone()}
                            </button>
                        </Show>
                    </div>
                    {if channel.oauth_apps.is_empty() {
                        view! {
                            <EmptyState
                                title=oauth_empty_title.clone()
                                body=oauth_empty_body.clone()
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
                                                <div class="mt-1 text-xs text-muted-foreground">{binding.role.clone().unwrap_or_else(|| oauth_no_role_label.clone())}</div>
                                            </div>
                                            <button
                                                type="button"
                                                class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition hover:bg-muted"
                                                disabled=move || busy.get()
                                                on:click={
                                                    let binding = binding.clone();
                                                    let channel_id = oauth_edit_channel_id.clone();
                                                    let query_writer = oauth_edit_query_writer.clone();
                                                    move |_| {
                                                        query_writer.update(
                                                            vec![
                                                                (
                                                                    AdminQueryKey::ChannelId.as_str().to_string(),
                                                                    Some(channel_id.clone()),
                                                                ),
                                                                (
                                                                    AdminQueryKey::OauthAppId.as_str().to_string(),
                                                                    Some(binding.oauth_app_id.clone()),
                                                                ),
                                                            ],
                                                            false,
                                                        );
                                                    }
                                                }
                                            >
                                                {common_edit_label.clone()}
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
                                                    let oauth_revoked_template = oauth_revoked_template.clone();
                                                    let query_writer = oauth_delete_query_writer.clone();
                                                    move |_| {
                                                        let oauth_revoked_template = oauth_revoked_template.clone();
                                                        let query_writer = query_writer.clone();
                                                        busy.set(true);
                                                        set_feedback.set(None);
                                                        set_error.set(None);
                                                        spawn_local({
                                                            let binding = binding.clone();
                                                            let token = token.clone();
                                                            let tenant = tenant.clone();
                                                            let channel_id = channel_id.clone();
                                                            let channel_slug = channel_slug.clone();
                                                            let oauth_revoked_template = oauth_revoked_template.clone();
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
                                                                            query_writer.clear_key(AdminQueryKey::OauthAppId.as_str());
                                                                            editing_oauth_app_id.set(None);
                                                                            bind_oauth_app_id.set(initial_oauth_app_id.get_untracked());
                                                                            bind_oauth_role.set(String::new());
                                                                        }
                                                                        set_feedback.set(Some(
                                                                            oauth_revoked_template
                                                                                .replace("{app}", deleted.oauth_app_id.as_str())
                                                                                .replace("{channel}", channel_slug.as_str()),
                                                                        ));
                                                                        set_refresh_nonce.update(|value| *value += 1);
                                                                    }
                                                                    Err(err) => set_error.set(Some(err.to_string())),
                                                                }
                                                                busy.set(false);
                                                            }
                                                        });
                                                    }
                                                }
                                            >
                                                {oauth_revoke_label.clone()}
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
                                    {oauth_no_apps_label.clone()}
                                </div>
                            }.into_any()
                        }}
                        <input type="text" class="w-full rounded-lg border border-input bg-card px-3 py-2 text-sm" placeholder=oauth_role_placeholder.clone() prop:value=bind_oauth_role on:input=move |ev| bind_oauth_role.set(event_target_value(&ev)) />
                        <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get() || !has_available_oauth_apps>
                            {move || if editing_oauth_app_id.get().is_some() {
                                oauth_update_label.clone()
                            } else {
                                oauth_bind_label.clone()
                            }}
                        </button>
                    </form>
                </section>
            </div>
        </article>
    }
}

#[component]
fn InfoPill(label: String, value: String) -> impl IntoView {
    view! {
        <div class="rounded-xl border border-border bg-background px-4 py-3">
            <div class="text-xs font-medium uppercase tracking-wide text-muted-foreground">{label}</div>
            <div class="mt-1 text-sm font-medium text-card-foreground">{value}</div>
        </div>
    }
}

#[component]
fn EmptyState(title: String, body: String) -> impl IntoView {
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

fn resolution_source_label(source: &ChannelResolutionSource, locale: Option<&str>) -> String {
    match source {
        ChannelResolutionSource::HeaderId => t(locale, "channel.source.headerId", "Header ID"),
        ChannelResolutionSource::HeaderSlug => {
            t(locale, "channel.source.headerSlug", "Header Slug")
        }
        ChannelResolutionSource::Query => t(locale, "channel.source.query", "Query"),
        ChannelResolutionSource::Host => t(locale, "channel.source.host", "Host"),
        ChannelResolutionSource::Policy => t(locale, "channel.source.policy", "Policy"),
        ChannelResolutionSource::Default => t(locale, "channel.source.default", "Default"),
    }
}

fn resolution_source_description(source: &ChannelResolutionSource, locale: Option<&str>) -> String {
    match source {
        ChannelResolutionSource::HeaderId => t(
            locale,
            "channel.sourceDescription.headerId",
            "The current request explicitly selected this channel through the X-Channel-ID header.",
        ),
        ChannelResolutionSource::HeaderSlug => t(
            locale,
            "channel.sourceDescription.headerSlug",
            "The current request explicitly selected this channel through the X-Channel-Slug header.",
        ),
        ChannelResolutionSource::Query => t(
            locale,
            "channel.sourceDescription.query",
            "The current request selected this channel through the query parameter fallback.",
        ),
        ChannelResolutionSource::Host => t(
            locale,
            "channel.sourceDescription.host",
            "The current request matched this channel through host-based target resolution.",
        ),
        ChannelResolutionSource::Policy => t(
            locale,
            "channel.sourceDescription.policy",
            "The current request matched a tenant-scoped typed channel resolution policy.",
        ),
        ChannelResolutionSource::Default => t(
            locale,
            "channel.sourceDescription.default",
            "No explicit channel selector matched, so the tenant's explicit default channel was used.",
        ),
    }
}
