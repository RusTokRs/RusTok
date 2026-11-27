use super::*;
use rustok_ui_leptos::{Button, ButtonVariant, Checkbox, Input, Select, SelectOption, Size};

#[component]
pub(super) fn ChannelCard(
    channel: ChannelDetail,
    available_modules: Vec<crate::model::AvailableModuleItem>,
    oauth_apps: Vec<crate::model::AvailableOauthAppItem>,
    token: Option<String>,
    tenant: Option<String>,
    set_feedback: WriteSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
    set_refresh_nonce: WriteSignal<u64>,
    /// Which workspace the module root is rendering. A focused workspace
    /// (`/targets`, `/apps`) shows only the card sections it owns; the landing
    /// workspace shows all of them.
    section: crate::core::ChannelAdminSection,
) -> impl IntoView {
    let shows_targets = section.shows_targets();
    let shows_modules = section.shows_modules();
    let shows_oauth_apps = section.shows_oauth_apps();
    // Three side-by-side panes on the landing workspace; a focused workspace
    // (`/targets`, `/apps`) renders one pane, and a three-column grid would leave
    // two empty columns beside it.
    let card_sections_class = if shows_modules {
        "mt-6 grid gap-6 xl:grid-cols-3"
    } else {
        "mt-6 grid gap-6"
    };
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
    let targets_primary_summary_template = t(
        ui_locale.as_deref(),
        "channel.targets.primarySummary",
        "{type} · primary",
    );
    let targets_summary_locale = ui_locale.clone();
    let targets_type_label = t(ui_locale.as_deref(), "channel.targets.type", "Target type");
    let targets_value_label = t(
        ui_locale.as_deref(),
        "channel.targets.value",
        "Target value",
    );
    // One descriptor list feeds both the dropdown and the rendered summary, so a
    // wire token ("web_domain") never reaches the operator and the two cannot drift.
    let target_type_options = StoredValue::new(vec![
        (
            "web_domain".to_string(),
            t(
                ui_locale.as_deref(),
                "channel.targets.typeWebDomain",
                "Web domain",
            ),
        ),
        (
            "mobile_app".to_string(),
            t(
                ui_locale.as_deref(),
                "channel.targets.typeMobileApp",
                "Mobile app",
            ),
        ),
        (
            "api_client".to_string(),
            t(
                ui_locale.as_deref(),
                "channel.targets.typeApiClient",
                "API client",
            ),
        ),
        (
            "embedded".to_string(),
            t(
                ui_locale.as_deref(),
                "channel.targets.typeEmbedded",
                "Embedded",
            ),
        ),
        (
            "external".to_string(),
            t(
                ui_locale.as_deref(),
                "channel.targets.typeExternal",
                "External",
            ),
        ),
    ]);
    let target_removed_locale = ui_locale.clone();
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
    let module_removed_locale = ui_locale.clone();
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
    let oauth_role_label = t(
        ui_locale.as_deref(),
        "channel.oauth.roleLabel",
        "OAuth role",
    );
    let oauth_update_label = t(
        ui_locale.as_deref(),
        "channel.oauth.update",
        "Update OAuth App Binding",
    );
    let oauth_bind_label = t(ui_locale.as_deref(), "channel.oauth.bind", "Bind OAuth App");
    let oauth_revoked_locale = ui_locale.clone();
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
    // The design-system `Select` takes `SelectOption` rows, so the two descriptor lists are mapped
    // once here instead of being re-derived inside the view; the signal hands out a clone per
    // render, which is what a `Signal<Vec<_>>` prop expects.
    let module_options = available_modules
        .iter()
        .map(|item| {
            let label = format!("{} ({})", item.name, item.kind);
            SelectOption::new(item.slug.clone(), label)
        })
        .collect::<Vec<_>>();
    let oauth_app_options = oauth_apps
        .iter()
        .map(|item| {
            let label = format!("{} ({})", item.name, item.app_type);
            SelectOption::new(item.id.clone(), label)
        })
        .collect::<Vec<_>>();
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

        match selected_target_query
            .get()
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
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

        match selected_module_query
            .get()
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
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

        match selected_oauth_query
            .get()
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
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
    let make_default = move || {
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
                let result = transport::make_default_channel(token, tenant, &channel_id).await;
                match result {
                    Ok(channel) => {
                        set_feedback.set(Some(crate::i18n::format(
                            ui_locale.as_deref(),
                            "channel.feedback.default",
                            Some(&rustok_ui_i18n::fluent_args!("slug" => channel.slug.to_string())),
                            "Channel `{slug}` is now the tenant default channel.",
                        )));
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_error.set(Some(err.to_string())),
                }
                busy.set(false);
            }
        });
    };

    // Channel lifecycle: activation and deletion are owner commands, so the card
    // exposes them as explicit, confirmed actions instead of leaving the flags
    // reachable only through the database. Renaming is inline because the slug is
    // immutable and the name is the only editable identity field.
    let lifecycle_active = channel.channel.is_active;
    let lifecycle_toggle = channel_lifecycle_toggle(lifecycle_active);
    let status_badge = channel_status_badge(lifecycle_active);
    let channel_status_label = t(
        ui_locale.as_deref(),
        status_badge.label_key,
        status_badge.label_fallback,
    );
    let lifecycle_token_toggle = token.clone();
    let lifecycle_tenant_toggle = tenant.clone();
    let lifecycle_channel_toggle = channel_id.clone();
    let lifecycle_slug_toggle = channel_slug.clone();
    let lifecycle_locale_toggle = ui_locale.clone();
    let lifecycle_locale_toggle_error = ui_locale.clone();
    let toggle_active = move || {
        let token = lifecycle_token_toggle.clone();
        let tenant = lifecycle_tenant_toggle.clone();
        let channel_id = lifecycle_channel_toggle.clone();
        let channel_slug = lifecycle_slug_toggle.clone();
        let ui_locale = lifecycle_locale_toggle.clone();
        let ui_locale_error = lifecycle_locale_toggle_error.clone();
        busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        spawn_local(async move {
            let next_is_active = lifecycle_toggle.next_is_active;
            match transport::set_channel_active(token, tenant, &channel_id, next_is_active).await {
                Ok(_) => {
                    set_feedback.set(Some(crate::i18n::format(
                        ui_locale.as_deref(),
                        lifecycle_toggle.feedback_key,
                        Some(&rustok_ui_i18n::fluent_args!("slug" => channel_slug.clone())),
                        lifecycle_toggle.feedback_fallback,
                    )));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(format!(
                    "{}: {err}",
                    t(
                        ui_locale_error.as_deref(),
                        "channel.error.lifecycle",
                        "Channel update failed",
                    )
                ))),
            }
            busy.set(false);
        });
    };

    let lifecycle_token_rename = token.clone();
    let lifecycle_tenant_rename = tenant.clone();
    let lifecycle_channel_rename = channel_id.clone();
    let lifecycle_slug_rename = channel_slug.clone();
    let lifecycle_locale_rename = ui_locale.clone();
    let rename_name = RwSignal::new(channel.channel.name.clone());
    let renaming = RwSignal::new(false);
    let submit_rename = move |ev: SubmitEvent| {
        ev.prevent_default();
        let token = lifecycle_token_rename.clone();
        let tenant = lifecycle_tenant_rename.clone();
        let channel_id = lifecycle_channel_rename.clone();
        let channel_slug = lifecycle_slug_rename.clone();
        let ui_locale = lifecycle_locale_rename.clone();
        let command = match channel_rename_command(&rename_name.get_untracked()) {
            Ok(command) => command,
            Err(message) => {
                set_error.set(Some(t(ui_locale.as_deref(), message.key, message.fallback)));
                return;
            }
        };
        busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        spawn_local(async move {
            match transport::update_channel(token, tenant, &channel_id, &command.payload).await {
                Ok(_) => {
                    set_feedback.set(Some(crate::i18n::format(
                        ui_locale.as_deref(),
                        command.feedback_key,
                        Some(&rustok_ui_i18n::fluent_args!("slug" => channel_slug.clone())),
                        command.feedback_fallback,
                    )));
                    renaming.set(false);
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
            busy.set(false);
        });
    };

    let lifecycle_clear_query = query_writer.clone();
    let lifecycle_token_delete = token.clone();
    let lifecycle_tenant_delete = tenant.clone();
    let lifecycle_channel_delete = channel_id.clone();
    let lifecycle_slug_delete = channel_slug.clone();
    let lifecycle_locale_delete = ui_locale.clone();
    let confirming_delete = RwSignal::new(false);
    let delete_channel_action = move || {
        let token = lifecycle_token_delete.clone();
        let tenant = lifecycle_tenant_delete.clone();
        let channel_id = lifecycle_channel_delete.clone();
        let channel_slug = lifecycle_slug_delete.clone();
        let ui_locale = lifecycle_locale_delete.clone();
        busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        spawn_local(async move {
            match transport::delete_channel(token, tenant, &channel_id).await {
                Ok(_) => {
                    set_feedback.set(Some(crate::i18n::format(
                        ui_locale.as_deref(),
                        CHANNEL_DELETE_FEEDBACK.feedback_key,
                        Some(&rustok_ui_i18n::fluent_args!("slug" => channel_slug.clone())),
                        CHANNEL_DELETE_FEEDBACK.feedback_fallback,
                    )));
                    confirming_delete.set(false);
                    lifecycle_clear_query.clear_key(AdminQueryKey::ChannelId.as_str());
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
            busy.set(false);
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
                        transport::update_target(token, tenant, &channel_id, target_id, &payload)
                            .await
                    }
                    None => transport::create_target(token, tenant, &channel_id, &payload).await,
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
                let result = transport::bind_module(
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
                let result = transport::bind_oauth_app(
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

    let lifecycle_toggle_label = t(
        ui_locale.as_deref(),
        lifecycle_toggle.label_key,
        lifecycle_toggle.label_fallback,
    );
    let lifecycle_rename_label = t(ui_locale.as_deref(), "channel.card.rename", "Rename");
    let rename_field_label = t(
        ui_locale.as_deref(),
        "channel.card.nameLabel",
        "Channel name",
    );
    let lifecycle_save_label = t(ui_locale.as_deref(), "channel.card.saveName", "Save name");
    let lifecycle_cancel_label = t(ui_locale.as_deref(), "common.cancel", "Cancel");
    let lifecycle_cancel_label_confirm = lifecycle_cancel_label.clone();
    let lifecycle_delete_label = t(
        ui_locale.as_deref(),
        "channel.card.delete",
        "Delete channel",
    );
    let lifecycle_delete_confirm_label = t(
        ui_locale.as_deref(),
        "channel.card.confirmDelete",
        "Confirm delete",
    );
    let lifecycle_delete_hint = t(
        ui_locale.as_deref(),
        "channel.card.deleteHint",
        "This also removes its targets, module bindings, app bindings and policy rules.",
    );

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
                            {move || channel_status_label.clone()}
                        </span>
                        {if is_default_channel {
                            view! {
                                <span class="inline-flex items-center rounded-full border border-sky-300 dark:border-sky-700 bg-sky-50 dark:bg-sky-950 px-3 py-1 text-xs font-medium text-sky-700 dark:text-sky-300">
                                    {t(ui_locale.as_deref(), "channel.card.default", "Default")}
                                </span>
                            }.into_any()
                        } else {
                            ().into_any()
                        }}
                    </div>
                    <h2 class="text-xl font-semibold text-card-foreground">{channel.channel.name.clone()}</h2>
                    <p class="text-sm text-muted-foreground">
                        {crate::i18n::format(
                            ui_locale.as_deref(),
                            "channel.card.summary",
                            Some(&rustok_ui_i18n::fluent_args!("targets" => channel.targets.len(), "modules" => channel.module_bindings.len(), "apps" => channel.oauth_apps.len())),
                            "{targets} target(s), {modules} module binding(s), {apps} app binding(s)",
                        )}
                    </p>
                </div>
                <div class="space-y-3">
                    <Button
                        variant=ButtonVariant::Outline
                        disabled=move || busy.get()
                        on_click=Box::new(move || {
                            let channel_id = select_channel_button_id.clone();
                            let query_writer = select_channel_button_writer.clone();
                            query_writer
                                .replace_value(AdminQueryKey::ChannelId.as_str(), channel_id);
                        })
                    >
                        {move || if is_selected_channel.get() {
                            t(select_button_locale.as_deref(), "channel.card.selected", "Selected")
                        } else {
                            t(select_button_locale.as_deref(), "channel.card.select", "Select")
                        }}
                    </Button>
                    {if is_default_channel {
                        view! {
                            <div class="rounded-lg border border-sky-200 dark:border-sky-800 bg-sky-50 dark:bg-sky-950 px-4 py-3 text-sm text-sky-800 dark:text-sky-200">
                                {t(
                                    ui_locale.as_deref(),
                                    "channel.card.defaultDescription",
                                    "Used as the tenant's explicit default channel when no header, query or host selector matches.",
                                )}
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <Button
                                variant=ButtonVariant::Outline
                                disabled=move || busy.get()
                                on_click=Box::new(make_default)
                            >
                                {t(ui_locale.as_deref(), "channel.card.makeDefault", "Make Default")}
                            </Button>
                        }.into_any()
                    }}
                    <div class="flex flex-wrap items-center gap-2">
                        <Button
                            variant=ButtonVariant::Outline
                            disabled=move || busy.get()
                            on_click=Box::new(toggle_active)
                        >
                            {move || lifecycle_toggle_label.clone()}
                        </Button>
                        <Show when=move || !renaming.get()>
                            <Button
                                variant=ButtonVariant::Outline
                                disabled=move || busy.get()
                                on_click=Box::new(move || renaming.set(true))
                            >
                                {move || lifecycle_rename_label.clone()}
                            </Button>
                        </Show>
                        <Show when=move || renaming.get()>
                            <form class="flex flex-wrap items-center gap-2" on:submit=submit_rename>
                                // `Input` spans `w-full`, so it sits in a flexible column of its
                                // own: next to the buttons it would otherwise shrink them to the
                                // edge of the card.
                                <div class="min-w-[10rem] flex-1">
                                    <Input
                                        value=rename_name.read_only()
                                        set_value=rename_name.write_only()
                                        aria_label=rename_field_label.clone()
                                    />
                                </div>
                                <Button r#type="submit" disabled=move || busy.get()>
                                    {move || lifecycle_save_label.clone()}
                                </Button>
                                <Button
                                    variant=ButtonVariant::Outline
                                    class="text-muted-foreground"
                                    on_click=Box::new(move || renaming.set(false))
                                >
                                    {move || lifecycle_cancel_label.clone()}
                                </Button>
                            </form>
                        </Show>
                        <Show when=move || !confirming_delete.get()>
                            <Button
                                variant=ButtonVariant::Outline
                                class="text-destructive"
                                disabled=move || busy.get()
                                on_click=Box::new(move || confirming_delete.set(true))
                            >
                                {move || lifecycle_delete_label.clone()}
                            </Button>
                        </Show>
                        <Show when=move || confirming_delete.get()>
                            <div class="flex flex-wrap items-center gap-2 rounded-lg border border-destructive/40 bg-destructive/5 px-3 py-2">
                                <span class="text-xs text-destructive">
                                    {move || lifecycle_delete_hint.clone()}
                                </span>
                                <Button
                                    variant=ButtonVariant::Destructive
                                    disabled=move || busy.get()
                                    on_click=Box::new(delete_channel_action)
                                >
                                    {move || lifecycle_delete_confirm_label.clone()}
                                </Button>
                                <Button
                                    variant=ButtonVariant::Outline
                                    class="text-muted-foreground"
                                    on_click=Box::new(move || confirming_delete.set(false))
                                >
                                    {move || lifecycle_cancel_label_confirm.clone()}
                                </Button>
                            </div>
                        </Show>
                    </div>
                    <div class="grid gap-2 md:grid-cols-2">
                    <InfoPill label=t(ui_locale.as_deref(), "channel.card.id", "ID") value=short_id(&channel.channel.id) />
                    <InfoPill label=t(ui_locale.as_deref(), "channel.card.updated", "Updated") value=channel.channel.updated_at.clone() />
                    </div>
                </div>
            </div>

            <div class=card_sections_class>
                {if shows_targets {
                    view! {
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
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=Size::Xs
                                        on_click=Box::new({
                                            let query_writer = cancel_target_query_writer.clone();
                                            move || {
                                                query_writer.clear_key(AdminQueryKey::TargetId.as_str());
                                                editing_target_id.set(None);
                                                target_type.set("web_domain".to_string());
                                                target_value.set(String::new());
                                                target_primary.set(true);
                                            }
                                        })
                                    >
                                        {targets_cancel_label.clone()}
                                    </Button>
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
                                                            {{
                                                                let label = target_type_options
                                                                    .get_value()
                                                                    .into_iter()
                                                                    .find(|(value, _)| *value == target.target_type)
                                                                    .map(|(_, label)| label)
                                                                    .unwrap_or_else(|| target.target_type.clone());
                                                                if target.is_primary {
                                                                    crate::i18n::format(
                                                                        targets_summary_locale.as_deref(),
                                                                        "channel.targets.primarySummary",
                                                                        Some(&rustok_ui_i18n::fluent_args!("type" => label)),
                                                                        &targets_primary_summary_template,
                                                                    )
                                                                } else {
                                                                    label
                                                                }
                                                            }}
                                                        </div>
                                                    </div>
                                                    <Button
                                                        variant=ButtonVariant::Outline
                                                        size=Size::Xs
                                                        disabled=move || busy.get()
                                                        on_click=Box::new({
                                                            let target = target.clone();
                                                            let channel_id = target_edit_channel_id.clone();
                                                            let query_writer = target_edit_query_writer.clone();
                                                            move || {
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
                                                        })
                                                    >
                                                        {common_edit_label.clone()}
                                                    </Button>
                                                    <Button
                                                        variant=ButtonVariant::Outline
                                                        size=Size::Xs
                                                        class="text-destructive"
                                                        disabled=move || busy.get()
                                                        on_click=Box::new({
                                                            let target = target.clone();
                                                            let token = token_for_target_delete.clone();
                                                            let tenant = tenant_for_target_delete.clone();
                                                            let channel_id = channel_id_for_target_delete.clone();
                                                            let channel_slug = channel_slug_for_target_delete.clone();
                                                            let target_removed_locale = target_removed_locale.clone();
                                                            let query_writer = target_delete_query_writer.clone();
                                                            move || {
                                                                let target_removed_locale = target_removed_locale.clone();
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
                                                                    let target_removed_locale = target_removed_locale.clone();
                                                                    async move {
                                                                        let result = transport::delete_target(
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
                                                                                    crate::i18n::format(
                                                                                        target_removed_locale.as_deref(),
                                                                                        "channel.feedback.targetRemoved",
                                                                                        Some(&rustok_ui_i18n::fluent_args!("target" => deleted.value.to_string(), "channel" => channel_slug.to_string())),
                                                                                        "Target `{target}` removed from channel `{channel}`.",
                                                                                    ),
                                                                                ));
                                                                                set_refresh_nonce.update(|value| *value += 1);
                                                                            }
                                                                            Err(err) => set_error.set(Some(err.to_string())),
                                                                        }
                                                                        busy.set(false);
                                                                    }
                                                                });
                                                            }
                                                        })
                                                    >
                                                        {common_delete_label.clone()}
                                                    </Button>
                                                </div>
                                            </div>
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }}
                            <form class="space-y-3" on:submit=create_target>
                                // `prop:value` keeps the control honest when an existing target is
                                // edited: without it the select always showed the browser default and
                                // saving overwrote the stored target type.
                                // `Select` keeps `prop:value` and the server-rendered `selected`
                                // option in one place, so editing a target still shows the stored
                                // type before hydration.
                                <Select
                                    aria_label=targets_type_label.clone()
                                    value=target_type.read_only()
                                    set_value=target_type.write_only()
                                    options=Signal::derive(move || {
                                        target_type_options
                                            .get_value()
                                            .into_iter()
                                            .map(|(value, label)| SelectOption::new(value, label))
                                            .collect()
                                    })
                                />
                                <Input
                                    aria_label=targets_value_label.clone()
                                    placeholder=targets_value_placeholder.clone()
                                    value=target_value.read_only()
                                    set_value=target_value.write_only()
                                />
                                <label class="flex items-center gap-2 text-sm text-muted-foreground">
                                    <Checkbox
                                        checked=target_primary.read_only()
                                        set_checked=target_primary.write_only()
                                    />
                                    {targets_primary_label.clone()}
                                </label>
                                <Button r#type="submit" class="w-full" disabled=move || busy.get()>
                                    {move || if editing_target_id.get().is_some() {
                                        targets_save_label.clone()
                                    } else {
                                        targets_add_label.clone()
                                    }}
                                </Button>
                            </form>
                        </section>
                    }.into_any()
                } else {
                    ().into_any()
                }}

                {if shows_modules {
                    view! {
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
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=Size::Xs
                                        on_click=Box::new({
                                            let query_writer = cancel_module_query_writer.clone();
                                            move || {
                                                query_writer.clear_key(AdminQueryKey::ModuleSlug.as_str());
                                                editing_module_slug.set(None);
                                                bind_module_slug.set(initial_module_slug.get_untracked());
                                                bind_module_enabled.set(true);
                                            }
                                        })
                                    >
                                        {modules_cancel_label.clone()}
                                    </Button>
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
                                                    <Button
                                                        variant=ButtonVariant::Outline
                                                        size=Size::Xs
                                                        disabled=move || busy.get()
                                                        on_click=Box::new({
                                                            let binding = binding.clone();
                                                            let channel_id = module_edit_channel_id.clone();
                                                            let query_writer = module_edit_query_writer.clone();
                                                            move || {
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
                                                        })
                                                    >
                                                        {common_edit_label.clone()}
                                                    </Button>
                                                    <Button
                                                        variant=ButtonVariant::Outline
                                                        size=Size::Xs
                                                        class="text-destructive"
                                                        disabled=move || busy.get()
                                                        on_click=Box::new({
                                                            let binding = binding.clone();
                                                            let token = token_for_module_delete.clone();
                                                            let tenant = tenant_for_module_delete.clone();
                                                            let channel_id = channel_id_for_module_delete.clone();
                                                            let channel_slug = channel_slug_for_module_delete.clone();
                                                            let module_removed_locale = module_removed_locale.clone();
                                                            let query_writer = module_delete_query_writer.clone();
                                                            move || {
                                                                let module_removed_locale = module_removed_locale.clone();
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
                                                                    let module_removed_locale = module_removed_locale.clone();
                                                                    async move {
                                                                        let result = transport::delete_module_binding(
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
                                                                                    crate::i18n::format(
                                                                                        module_removed_locale.as_deref(),
                                                                                        "channel.feedback.moduleRemoved",
                                                                                        Some(&rustok_ui_i18n::fluent_args!("module" => deleted.module_slug.to_string(), "channel" => channel_slug.to_string())),
                                                                                        "Module binding `{module}` removed from channel `{channel}`.",
                                                                                    ),
                                                                                ));
                                                                                set_refresh_nonce.update(|value| *value += 1);
                                                                            }
                                                                            Err(err) => set_error.set(Some(err.to_string())),
                                                                        }
                                                                        busy.set(false);
                                                                    }
                                                                });
                                                            }
                                                        })
                                                    >
                                                        {common_delete_label.clone()}
                                                    </Button>
                                                </div>
                                            </div>
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }}
                            <form class="space-y-3" on:submit=bind_module_submit>
                                {if has_available_modules {
                                    view! {
                                        <Select
                                            value=bind_module_slug.read_only()
                                            set_value=bind_module_slug.write_only()
                                            options=Signal::derive(move || module_options.clone())
                                        />
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="rounded-lg border border-dashed border-border px-3 py-2 text-sm text-muted-foreground">
                                            {modules_no_descriptors_label.clone()}
                                        </div>
                                    }.into_any()
                                }}
                                <label class="flex items-center gap-2 text-sm text-muted-foreground">
                                    <Checkbox
                                        checked=bind_module_enabled.read_only()
                                        set_checked=bind_module_enabled.write_only()
                                    />
                                    {modules_enabled_for_channel_label.clone()}
                                </label>
                                <Button
                                    r#type="submit"
                                    class="w-full"
                                    disabled=move || busy.get() || !has_available_modules
                                >
                                    {move || if editing_module_slug.get().is_some() {
                                        modules_update_label.clone()
                                    } else {
                                        modules_save_label.clone()
                                    }}
                                </Button>
                            </form>
                        </section>
                    }.into_any()
                } else {
                    ().into_any()
                }}

                {if shows_oauth_apps {
                    view! {
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
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=Size::Xs
                                        on_click=Box::new({
                                            let query_writer = cancel_oauth_query_writer.clone();
                                            move || {
                                                query_writer.clear_key(AdminQueryKey::OauthAppId.as_str());
                                                editing_oauth_app_id.set(None);
                                                bind_oauth_app_id.set(initial_oauth_app_id.get_untracked());
                                                bind_oauth_role.set(String::new());
                                            }
                                        })
                                    >
                                        {oauth_cancel_label.clone()}
                                    </Button>
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
                                                    <Button
                                                        variant=ButtonVariant::Outline
                                                        size=Size::Xs
                                                        disabled=move || busy.get()
                                                        on_click=Box::new({
                                                            let binding = binding.clone();
                                                            let channel_id = oauth_edit_channel_id.clone();
                                                            let query_writer = oauth_edit_query_writer.clone();
                                                            move || {
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
                                                        })
                                                    >
                                                        {common_edit_label.clone()}
                                                    </Button>
                                                    <Button
                                                        variant=ButtonVariant::Outline
                                                        size=Size::Xs
                                                        class="text-destructive"
                                                        disabled=move || busy.get()
                                                        on_click=Box::new({
                                                            let binding = binding.clone();
                                                            let token = token_for_app_delete.clone();
                                                            let tenant = tenant_for_app_delete.clone();
                                                            let channel_id = channel_id_for_app_delete.clone();
                                                            let channel_slug = channel_slug_for_app_delete.clone();
                                                            let oauth_revoked_locale = oauth_revoked_locale.clone();
                                                            let query_writer = oauth_delete_query_writer.clone();
                                                            move || {
                                                                let oauth_revoked_locale = oauth_revoked_locale.clone();
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
                                                                    let oauth_revoked_locale = oauth_revoked_locale.clone();
                                                                    async move {
                                                                        let result = transport::delete_oauth_app_binding(
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
                                                                                    crate::i18n::format(
                                                                                        oauth_revoked_locale.as_deref(),
                                                                                        "channel.feedback.oauthRevoked",
                                                                                        Some(&rustok_ui_i18n::fluent_args!("app" => deleted.oauth_app_id.to_string(), "channel" => channel_slug.to_string())),
                                                                                        "OAuth app binding `{app}` revoked for channel `{channel}`.",
                                                                                    ),
                                                                                ));
                                                                                set_refresh_nonce.update(|value| *value += 1);
                                                                            }
                                                                            Err(err) => set_error.set(Some(err.to_string())),
                                                                        }
                                                                        busy.set(false);
                                                                    }
                                                                });
                                                            }
                                                        })
                                                    >
                                                        {oauth_revoke_label.clone()}
                                                    </Button>
                                                </div>
                                            </div>
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }}
                            <form class="space-y-3" on:submit=bind_oauth_submit>
                                {if has_available_oauth_apps {
                                    view! {
                                        <Select
                                            value=bind_oauth_app_id.read_only()
                                            set_value=bind_oauth_app_id.write_only()
                                            options=Signal::derive(move || oauth_app_options.clone())
                                        />
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="rounded-lg border border-dashed border-border px-3 py-2 text-sm text-muted-foreground">
                                            {oauth_no_apps_label.clone()}
                                        </div>
                                    }.into_any()
                                }}
                                <Input
                                    aria_label=oauth_role_label.clone()
                                    placeholder=oauth_role_placeholder.clone()
                                    value=bind_oauth_role.read_only()
                                    set_value=bind_oauth_role.write_only()
                                />
                                <Button
                                    r#type="submit"
                                    class="w-full"
                                    disabled=move || busy.get() || !has_available_oauth_apps
                                >
                                    {move || if editing_oauth_app_id.get().is_some() {
                                        oauth_update_label.clone()
                                    } else {
                                        oauth_bind_label.clone()
                                    }}
                                </Button>
                            </form>
                        </section>
                    }.into_any()
                } else {
                    ().into_any()
                }}
            </div>
        </article>
    }
}
