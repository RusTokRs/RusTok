use super::*;
use rustok_ui_leptos::{Button, ButtonVariant, Checkbox, Input, Select, SelectOption, Size};

#[component]
pub(super) fn PolicySetCard(
    policy_set: ChannelResolutionPolicySetDetail,
    channels: Vec<ChannelDetail>,
    oauth_apps: Vec<crate::model::AvailableOauthAppItem>,
    token: Option<String>,
    tenant: Option<String>,
    set_feedback: WriteSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
    set_refresh_nonce: WriteSignal<u64>,
) -> impl IntoView {
    let ui_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let selected_policy_set_query = use_route_query_value(AdminQueryKey::PolicySetId.as_str());
    let selected_policy_rule_query = use_route_query_value(AdminQueryKey::PolicyRuleId.as_str());
    let query_writer = use_route_query_writer();
    let policy_selection_query_writer = query_writer.clone();
    let has_channels = !channels.is_empty();
    let busy = RwSignal::new(false);
    let editing_rule_id = RwSignal::new(Option::<String>::None);
    let policy_rules = policy_set.rules.clone();
    let rule_order = policy_rules
        .iter()
        .map(|rule| rule.id.clone())
        .collect::<Vec<_>>();
    let create_form_state = policy_rule_create_form_state(&policy_rules, &channels);
    let policy_rules_for_selection = policy_rules.clone();
    let channels_for_selection = channels.clone();
    let create_form_state_for_selection = create_form_state.clone();
    let priority = RwSignal::new(create_form_state.priority);
    let is_active = RwSignal::new(create_form_state.is_active);
    let action_channel_id = RwSignal::new(create_form_state.action_channel_id.clone());
    let host_equals = RwSignal::new(create_form_state.host_equals.clone());
    let host_suffix = RwSignal::new(create_form_state.host_suffix.clone());
    let locale = RwSignal::new(create_form_state.locale.clone());
    let surface = RwSignal::new(create_form_state.surface.clone());
    let oauth_app_id = RwSignal::new(create_form_state.oauth_app_id.clone());
    let form_signals = PolicyRuleFormSignals {
        priority,
        is_active,
        action_channel_id,
        host_equals,
        host_suffix,
        locale,
        surface,
        oauth_app_id,
    };
    let policy_set_id = policy_set.policy_set.id.clone();
    let policy_set_slug = policy_set.policy_set.slug.clone();
    let activate_ctx = StoredValue::new((
        token.clone(),
        tenant.clone(),
        policy_set_id.clone(),
        policy_set_slug.clone(),
        ui_locale.clone(),
    ));
    let submit_rule_ctx = StoredValue::new((
        token.clone(),
        tenant.clone(),
        policy_set_id.clone(),
        policy_set_slug.clone(),
        ui_locale.clone(),
    ));

    let active_badge_label = t(
        ui_locale.as_deref(),
        "channel.policies.activeBadge",
        "Active",
    );
    let schema_label = t(ui_locale.as_deref(), "channel.policies.schema", "Schema");
    let activate_label = StoredValue::new(t(
        ui_locale.as_deref(),
        "channel.policies.activate",
        "Activate",
    ));
    let empty_rules_title = t(
        ui_locale.as_deref(),
        "channel.policies.rules.emptyTitle",
        "No rules yet.",
    );
    let empty_rules_body = t(
        ui_locale.as_deref(),
        "channel.policies.rules.emptyBody",
        "Add the first rule to connect request facts to a specific channel.",
    );
    let edit_rule_title = t(
        ui_locale.as_deref(),
        "channel.policies.editRuleTitle",
        "Edit Rule",
    );
    let add_rule_title = t(
        ui_locale.as_deref(),
        "channel.policies.addRuleTitle",
        "Add Rule",
    );
    let save_rule_label = t(
        ui_locale.as_deref(),
        "channel.policies.saveRule",
        "Save Rule",
    );
    let add_rule_label = t(ui_locale.as_deref(), "channel.policies.addRule", "Add Rule");
    let edit_rule_label =
        StoredValue::new(t(ui_locale.as_deref(), "channel.policies.editRule", "Edit"));
    let delete_rule_label = StoredValue::new(t(
        ui_locale.as_deref(),
        "channel.policies.deleteRule",
        "Delete",
    ));
    let move_up_label = StoredValue::new(t(
        ui_locale.as_deref(),
        "channel.policies.moveUp",
        "Move Up",
    ));
    let move_down_label = StoredValue::new(t(
        ui_locale.as_deref(),
        "channel.policies.moveDown",
        "Move Down",
    ));
    let enable_rule_label = StoredValue::new(t(
        ui_locale.as_deref(),
        "channel.policies.enableRule",
        "Enable",
    ));
    let disable_rule_label = StoredValue::new(t(
        ui_locale.as_deref(),
        "channel.policies.disableRule",
        "Disable",
    ));
    let inactive_rule_badge = t(
        ui_locale.as_deref(),
        "channel.policies.inactiveBadge",
        "Inactive",
    );
    let cancel_label = StoredValue::new(t(ui_locale.as_deref(), "common.cancel", "Cancel"));
    let any_surface_label = t(
        ui_locale.as_deref(),
        "channel.policies.surfaceAny",
        "any surface",
    );
    let priority_label = t(
        ui_locale.as_deref(),
        "channel.policies.priority",
        "Rule priority",
    );
    let action_channel_label = t(
        ui_locale.as_deref(),
        "channel.policies.actionChannel",
        "Action channel",
    );
    let surface_label = t(ui_locale.as_deref(), "channel.policies.surface", "Surface");
    let oauth_app_label = t(
        ui_locale.as_deref(),
        "channel.policies.oauthApp",
        "OAuth app filter",
    );
    // The rule form feeds three `Select`s, and a `Select` takes `SelectOption` rows: the lists are
    // mapped once here instead of being rebuilt inside the view on every render.
    let channel_options = channels
        .iter()
        .map(|channel| {
            SelectOption::new(
                channel.channel.id.clone(),
                format!("{} ({})", channel.channel.name, channel.channel.slug),
            )
        })
        .collect::<Vec<_>>();
    let surface_options = vec![
        SelectOption::new("", any_surface_label.clone()),
        SelectOption::new("http", "http"),
    ];
    let oauth_app_options = {
        let any_app_label = t(
            ui_locale.as_deref(),
            "channel.policies.oauthAny",
            "any OAuth app",
        );
        let mut options = vec![SelectOption::new("", any_app_label)];
        for app in &oauth_apps {
            let label = format!("{} ({})", app.name, app.slug);
            options.push(SelectOption::new(app.id.clone(), label));
        }
        options
    };
    let rules_ui_locale = ui_locale.clone();

    Effect::new(move |_| {
        let selected_policy_set_id = selected_policy_set_query
            .get()
            .filter(|value| !value.trim().is_empty());
        let selected_policy_rule_id = selected_policy_rule_query
            .get()
            .filter(|value| !value.trim().is_empty());

        if selected_policy_set_id.as_deref() != Some(policy_set_id.as_str()) {
            editing_rule_id.set(None);
            form_signals.apply(&create_form_state_for_selection);
            return;
        }

        match selected_policy_rule_id {
            Some(rule_id) => {
                if let Some(rule) = policy_rules_for_selection
                    .iter()
                    .find(|rule| rule.id == rule_id)
                {
                    editing_rule_id.set(Some(rule.id.clone()));
                    let edit_form_state =
                        policy_rule_edit_form_state(rule, &channels_for_selection);
                    form_signals.apply(&edit_form_state);
                } else {
                    policy_selection_query_writer.clear_key(AdminQueryKey::PolicyRuleId.as_str());
                    editing_rule_id.set(None);
                    form_signals.apply(&create_form_state_for_selection);
                }
            }
            None => {
                editing_rule_id.set(None);
                form_signals.apply(&create_form_state_for_selection);
            }
        }
    });

    let submit_query_writer = query_writer.clone();
    let submit_create_form_state = create_form_state.clone();
    let on_submit_rule = move |ev: SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        set_feedback.set(None);
        set_error.set(None);

        spawn_local({
            let (token, tenant, policy_set_id, policy_set_slug, ui_locale) =
                submit_rule_ctx.get_value();
            let editing_rule_id_value = editing_rule_id.get_untracked();
            let query_writer = submit_query_writer.clone();
            let create_form_state = submit_create_form_state.clone();
            async move {
                match editing_rule_id_value {
                    Some(rule_id) => {
                        let payload = PolicyRuleFormState {
                            priority: priority.get_untracked(),
                            is_active: is_active.get_untracked(),
                            action_channel_id: action_channel_id.get_untracked(),
                            host_equals: host_equals.get_untracked(),
                            host_suffix: host_suffix.get_untracked(),
                            oauth_app_id: oauth_app_id.get_untracked(),
                            surface: surface.get_untracked(),
                            locale: locale.get_untracked(),
                        }
                        .update_payload();
                        let result = transport::update_resolution_rule(
                            token,
                            tenant,
                            policy_set_id.as_str(),
                            rule_id.as_str(),
                            &payload,
                        )
                        .await;

                        match result {
                            Ok(rule) => {
                                set_feedback.set(Some(
                                    crate::i18n::format(
                                        ui_locale.as_deref(),
                                        "channel.policies.feedback.ruleUpdated",
                                        Some(&rustok_ui_i18n::fluent_args!("rule" => short_id(rule.id.as_str()).to_string(), "slug" => policy_set_slug.to_string())),
                                        "Rule `{rule}` updated in policy set `{slug}`.",
                                    ),
                                ));
                                query_writer.clear_key(AdminQueryKey::PolicyRuleId.as_str());
                                editing_rule_id.set(None);
                                form_signals.apply(&create_form_state);
                                set_refresh_nonce.update(|value| *value += 1);
                            }
                            Err(err) => set_error.set(Some(err.to_string())),
                        }
                    }
                    None => {
                        let payload = PolicyRuleFormState {
                            priority: priority.get_untracked(),
                            is_active: is_active.get_untracked(),
                            action_channel_id: action_channel_id.get_untracked(),
                            host_equals: host_equals.get_untracked(),
                            host_suffix: host_suffix.get_untracked(),
                            oauth_app_id: oauth_app_id.get_untracked(),
                            surface: surface.get_untracked(),
                            locale: locale.get_untracked(),
                        }
                        .create_payload();
                        let result = transport::create_resolution_rule(
                            token,
                            tenant,
                            policy_set_id.as_str(),
                            &payload,
                        )
                        .await;

                        match result {
                            Ok(rule) => {
                                set_feedback.set(Some(
                                    crate::i18n::format(
                                        ui_locale.as_deref(),
                                        "channel.policies.feedback.ruleCreated",
                                        Some(&rustok_ui_i18n::fluent_args!("rule" => short_id(rule.id.as_str()).to_string(), "slug" => policy_set_slug.to_string())),
                                        "Rule `{rule}` added to policy set `{slug}`.",
                                    ),
                                ));
                                host_equals.set(String::new());
                                host_suffix.set(String::new());
                                locale.set(String::new());
                                oauth_app_id.set(String::new());
                                priority.update(|value| *value += 10);
                                query_writer.replace_value(
                                    AdminQueryKey::PolicySetId.as_str(),
                                    policy_set_id.clone(),
                                );
                                set_refresh_nonce.update(|value| *value += 1);
                            }
                            Err(err) => set_error.set(Some(err.to_string())),
                        }
                    }
                }

                busy.set(false);
            }
        });
    };

    view! {
        <section class="space-y-4 rounded-xl border border-border bg-background p-4">
            <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
                <div>
                    <div class="flex items-center gap-2">
                        <h3 class="text-base font-semibold text-card-foreground">
                            {policy_set.policy_set.name.clone()}
                        </h3>
                        <span class="rounded-full border border-border px-2 py-0.5 text-xs text-muted-foreground">
                            {policy_set.policy_set.slug.clone()}
                        </span>
                        <Show when=move || policy_set.policy_set.is_active>
                            <span class="rounded-full border border-emerald-300 dark:border-emerald-700 bg-emerald-50 dark:bg-emerald-950 px-2 py-0.5 text-xs font-medium text-emerald-700 dark:text-emerald-300">
                                {active_badge_label.clone()}
                            </span>
                        </Show>
                    </div>
                    <p class="mt-1 text-xs text-muted-foreground">
                        {format!("{} {}", schema_label.clone(), policy_set.policy_set.schema_version)}
                    </p>
                </div>
                <Show when=move || !policy_set.policy_set.is_active>
                    <Button
                        variant=ButtonVariant::Outline
                        disabled=move || busy.get()
                        on_click=Box::new(move || {
                            busy.set(true);
                            set_feedback.set(None);
                            set_error.set(None);
                            let (token, tenant, policy_set_id, policy_set_slug, ui_locale) =
                                activate_ctx.get_value();

                            spawn_local(async move {
                                let result = transport::activate_resolution_policy_set(
                                    token,
                                    tenant,
                                    policy_set_id.as_str(),
                                )
                                .await;
                                match result {
                                    Ok(_) => {
                                        set_feedback.set(Some(
                                            crate::i18n::format(
                                                ui_locale.as_deref(),
                                                "channel.policies.feedback.activated",
                                                Some(&rustok_ui_i18n::fluent_args!("slug" => policy_set_slug.to_string())),
                                                "Policy set `{slug}` is now active.",
                                            ),
                                        ));
                                        set_refresh_nonce.update(|value| *value += 1);
                                    }
                                    Err(err) => set_error.set(Some(err.to_string())),
                                }
                                busy.set(false);
                            });
                        })
                    >
                        {activate_label.get_value()}
                    </Button>
                </Show>
            </div>

            {if policy_rules.is_empty() {
                view! {
                    <EmptyState
                        title=empty_rules_title.clone()
                        body=empty_rules_body.clone()
                    />
                }.into_any()
            } else {
                view! {
                    <div class="space-y-2">
                        {policy_rules.into_iter().enumerate().map(|(index, rule)| {
                            let summary = policy_rule_summary(&rule, &channels);
                            let rule_is_active = rule.is_active;
                            let is_editing_rule = Signal::derive({
                                let rule_id = rule.id.clone();
                                move || editing_rule_id.get().as_deref() == Some(rule_id.as_str())
                            });
                            let rule_ids_for_reorder_up = rule_order.clone();
                            let rule_ids_for_reorder_down = rule_order.clone();
                            let inactive_rule_badge = inactive_rule_badge.clone();
                            let can_move_up = index > 0;
                            let can_move_down = index + 1 < rule_order.len();
                            let token_for_up = token.clone();
                            let tenant_for_up = tenant.clone();
                            let token_for_down = token.clone();
                            let tenant_for_down = tenant.clone();
                            let token_for_toggle = token.clone();
                            let tenant_for_toggle = tenant.clone();
                            let token_for_delete = token.clone();
                            let tenant_for_delete = tenant.clone();
                            let policy_set_id_for_up = policy_set.policy_set.id.clone();
                            let policy_set_id_for_down = policy_set.policy_set.id.clone();
                            let policy_set_id_for_toggle = policy_set.policy_set.id.clone();
                            let policy_set_id_for_delete = policy_set.policy_set.id.clone();
                            let policy_set_slug_for_up = policy_set.policy_set.slug.clone();
                            let policy_set_slug_for_down = policy_set.policy_set.slug.clone();
                            let ui_locale_for_up = rules_ui_locale.clone();
                            let ui_locale_for_down = rules_ui_locale.clone();
                            let ui_locale_for_toggle = rules_ui_locale.clone();
                            let ui_locale_for_delete = rules_ui_locale.clone();
                            let query_writer_for_edit = query_writer.clone();
                            let query_writer_for_delete = query_writer.clone();
                            let rule_id_for_toggle = rule.id.clone();
                            let rule_id_for_delete = rule.id.clone();
                            let rule_id_for_feedback = rule.id.clone();
                            view! {
                                <div
                                    class="rounded-lg border border-border px-3 py-3 text-sm"
                                    class=("ring-2 ring-primary/30", move || is_editing_rule.get())
                                >
                                    <div class="flex flex-wrap items-start justify-between gap-3">
                                        <div class="space-y-1">
                                            <div class="flex items-center gap-2 font-medium text-card-foreground">
                                                <span>{format!("#{} · {}", rule.priority, short_id(rule.id.as_str()))}</span>
                                                <Show when=move || !rule_is_active>
                                                    <span class="rounded-full border border-amber-300 dark:border-amber-700 bg-amber-50 dark:bg-amber-950 px-2 py-0.5 text-xs font-medium text-amber-700 dark:text-amber-300">
                                                        {inactive_rule_badge.clone()}
                                                    </span>
                                                </Show>
                                            </div>
                                            <div class="text-muted-foreground">{summary}</div>
                                        </div>
                                        <div class="flex flex-wrap items-center gap-2">
                                            <Button
                                                variant=ButtonVariant::Outline
                                                size=Size::Xs
                                                disabled=move || busy.get()
                                                on_click=Box::new({
                                                    let policy_set_id = policy_set.policy_set.id.clone();
                                                    let rule_id = rule.id.clone();
                                                    let query_writer = query_writer_for_edit.clone();
                                                    move || {
                                                        query_writer.update(
                                                            vec![
                                                                (
                                                                    AdminQueryKey::PolicySetId.as_str().to_string(),
                                                                    Some(policy_set_id.clone()),
                                                                ),
                                                                (
                                                                    AdminQueryKey::PolicyRuleId.as_str().to_string(),
                                                                    Some(rule_id.clone()),
                                                                ),
                                                            ],
                                                            false,
                                                        );
                                                    }
                                                })
                                            >
                                                {edit_rule_label.get_value()}
                                            </Button>
                                            <Button
                                                variant=ButtonVariant::Outline
                                                size=Size::Xs
                                                disabled=move || busy.get() || !can_move_up
                                                on_click=Box::new(move || {
                                                    busy.set(true);
                                                    set_feedback.set(None);
                                                    set_error.set(None);
                                                    spawn_local({
                                                        let token = token_for_up.clone();
                                                        let tenant = tenant_for_up.clone();
                                                        let policy_set_id = policy_set_id_for_up.clone();
                                                        let policy_set_slug = policy_set_slug_for_up.clone();
                                                        let ui_locale = ui_locale_for_up.clone();
                                                        let rule_ids_for_reorder = rule_ids_for_reorder_up.clone();
                                                        async move {
                                                            let Some(rule_ids) = reorder_policy_rule_ids(rule_ids_for_reorder.as_slice(), index, true) else {
                                                                busy.set(false);
                                                                return;
                                                            };
                                                            let result = transport::reorder_resolution_rules(
                                                                token,
                                                                tenant,
                                                                policy_set_id.as_str(),
                                                                &ReorderResolutionRulesPayload { rule_ids },
                                                            )
                                                            .await;
                                                            match result {
                                                                Ok(_) => {
                                                                    set_feedback.set(Some(
                                                                        crate::i18n::format(
                                                                            ui_locale.as_deref(),
                                                                            "channel.policies.feedback.ruleReordered",
                                                                            Some(&rustok_ui_i18n::fluent_args!("slug" => policy_set_slug.to_string())),
                                                                            "Rule order updated for policy set `{slug}`.",
                                                                        ),
                                                                    ));
                                                                    set_refresh_nonce.update(|value| *value += 1);
                                                                }
                                                                Err(err) => set_error.set(Some(err.to_string())),
                                                            }
                                                            busy.set(false);
                                                        }
                                                    });
                                                })
                                            >
                                                {move_up_label.get_value()}
                                            </Button>
                                            <Button
                                                variant=ButtonVariant::Outline
                                                size=Size::Xs
                                                disabled=move || busy.get() || !can_move_down
                                                on_click=Box::new(move || {
                                                    busy.set(true);
                                                    set_feedback.set(None);
                                                    set_error.set(None);
                                                    spawn_local({
                                                        let token = token_for_down.clone();
                                                        let tenant = tenant_for_down.clone();
                                                        let policy_set_id = policy_set_id_for_down.clone();
                                                        let policy_set_slug = policy_set_slug_for_down.clone();
                                                        let ui_locale = ui_locale_for_down.clone();
                                                        let rule_ids_for_reorder = rule_ids_for_reorder_down.clone();
                                                        async move {
                                                            let Some(rule_ids) = reorder_policy_rule_ids(rule_ids_for_reorder.as_slice(), index, false) else {
                                                                busy.set(false);
                                                                return;
                                                            };
                                                            let result = transport::reorder_resolution_rules(
                                                                token,
                                                                tenant,
                                                                policy_set_id.as_str(),
                                                                &ReorderResolutionRulesPayload { rule_ids },
                                                            )
                                                            .await;
                                                            match result {
                                                                Ok(_) => {
                                                                    set_feedback.set(Some(
                                                                        crate::i18n::format(
                                                                            ui_locale.as_deref(),
                                                                            "channel.policies.feedback.ruleReordered",
                                                                            Some(&rustok_ui_i18n::fluent_args!("slug" => policy_set_slug.to_string())),
                                                                            "Rule order updated for policy set `{slug}`.",
                                                                        ),
                                                                    ));
                                                                    set_refresh_nonce.update(|value| *value += 1);
                                                                }
                                                                Err(err) => set_error.set(Some(err.to_string())),
                                                            }
                                                            busy.set(false);
                                                        }
                                                    });
                                                })
                                            >
                                                {move_down_label.get_value()}
                                            </Button>
                                            <Button
                                                variant=ButtonVariant::Outline
                                                size=Size::Xs
                                                disabled=move || busy.get()
                                                on_click=Box::new(move || {
                                                    busy.set(true);
                                                    set_feedback.set(None);
                                                    set_error.set(None);
                                                    spawn_local({
                                                        let token = token_for_toggle.clone();
                                                        let tenant = tenant_for_toggle.clone();
                                                        let policy_set_id = policy_set_id_for_toggle.clone();
                                                        let rule_id = rule_id_for_toggle.clone();
                                                        let rule_id_for_feedback = rule_id_for_feedback.clone();
                                                        let ui_locale = ui_locale_for_toggle.clone();
                                                        async move {
                                                            let next_is_active = !rule_is_active;
                                                            let payload = policy_rule_active_update_payload(next_is_active);
                                                            let result = transport::update_resolution_rule(
                                                                token,
                                                                tenant,
                                                                policy_set_id.as_str(),
                                                                rule_id.as_str(),
                                                                &payload,
                                                            )
                                                            .await;
                                                            match result {
                                                                Ok(_) => {
                                                                    let message = if next_is_active {
                                                                        t(
                                                                            ui_locale.as_deref(),
                                                                            "channel.policies.feedback.ruleEnabled",
                                                                            "Rule `{rule}` enabled.",
                                                                        )
                                                                    } else {
                                                                        t(
                                                                            ui_locale.as_deref(),
                                                                            "channel.policies.feedback.ruleDisabled",
                                                                            "Rule `{rule}` disabled.",
                                                                        )
                                                                    };
                                                                    set_feedback.set(Some(
                                                                        message.replace(
                                                                            "{rule}",
                                                                            short_id(rule_id_for_feedback.as_str()).as_str(),
                                                                        ),
                                                                    ));
                                                                    set_refresh_nonce.update(|value| *value += 1);
                                                                }
                                                                Err(err) => set_error.set(Some(err.to_string())),
                                                            }
                                                            busy.set(false);
                                                        }
                                                    });
                                                })
                                            >
                                                {if rule_is_active {
                                                    disable_rule_label.get_value()
                                                } else {
                                                    enable_rule_label.get_value()
                                                }}
                                            </Button>
                                            <Button
                                                variant=ButtonVariant::Outline
                                                size=Size::Xs
                                                class="text-destructive"
                                                disabled=move || busy.get()
                                                on_click=Box::new(move || {
                                                    busy.set(true);
                                                    set_feedback.set(None);
                                                    set_error.set(None);
                                                    spawn_local({
                                                        let token = token_for_delete.clone();
                                                        let tenant = tenant_for_delete.clone();
                                                        let policy_set_id = policy_set_id_for_delete.clone();
                                                        let rule_id = rule_id_for_delete.clone();
                                                        let ui_locale = ui_locale_for_delete.clone();
                                                        let query_writer = query_writer_for_delete.clone();
                                                        async move {
                                                            let result = transport::delete_resolution_rule(
                                                                token,
                                                                tenant,
                                                                policy_set_id.as_str(),
                                                                rule_id.as_str(),
                                                            )
                                                            .await;
                                                            match result {
                                                                Ok(_) => {
                                                                    if editing_rule_id
                                                                        .get_untracked()
                                                                        .as_deref()
                                                                        == Some(rule_id.as_str())
                                                                    {
                                                                        query_writer.clear_key(AdminQueryKey::PolicyRuleId.as_str());
                                                                    }
                                                                    set_feedback.set(Some(
                                                                        crate::i18n::format(
                                                                            ui_locale.as_deref(),
                                                                            "channel.policies.feedback.ruleDeleted",
                                                                            Some(&rustok_ui_i18n::fluent_args!("rule" => short_id(rule_id.as_str()).to_string())),
                                                                            "Rule `{rule}` removed.",
                                                                        ),
                                                                    ));
                                                                    set_refresh_nonce.update(|value| *value += 1);
                                                                }
                                                                Err(err) => set_error.set(Some(err.to_string())),
                                                            }
                                                            busy.set(false);
                                                        }
                                                    });
                                                })
                                            >
                                                {delete_rule_label.get_value()}
                                            </Button>
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                }.into_any()
            }}

            <form class="grid gap-3 rounded-lg border border-border bg-card p-4 lg:grid-cols-2" on:submit=on_submit_rule>
                <div class="flex items-center justify-between gap-3 lg:col-span-2">
                    <h4 class="text-sm font-semibold text-card-foreground">
                        {move || if editing_rule_id.get().is_some() {
                            edit_rule_title.clone()
                        } else {
                            add_rule_title.clone()
                        }}
                    </h4>
                    <Show when=move || editing_rule_id.get().is_some()>
                        <Button
                            variant=ButtonVariant::Outline
                            size=Size::Xs
                            on_click=Box::new({
                                let query_writer = query_writer.clone();
                                let create_form_state = create_form_state.clone();
                                move || {
                                    query_writer.clear_key(AdminQueryKey::PolicyRuleId.as_str());
                                    editing_rule_id.set(None);
                                    form_signals.apply(&create_form_state);
                                }
                            })
                        >
                            {cancel_label.get_value()}
                        </Button>
                    </Show>
                </div>
                <Input
                    r#type="number"
                    aria_label=priority_label.clone()
                    derived_value=Signal::derive(move || priority.get().to_string())
                    on_change=Callback::new(move |raw: String| {
                        if let Ok(value) = raw.parse::<i32>() {
                            priority.set(value);
                        }
                    })
                />
                <Select
                    aria_label=action_channel_label.clone()
                    value=action_channel_id.read_only()
                    set_value=action_channel_id.write_only()
                    options=Signal::derive(move || channel_options.clone())
                />
                <Input
                    aria_label=t(ui_locale.as_deref(), "channel.policies.hostEquals", "host equals")
                    placeholder=t(ui_locale.as_deref(), "channel.policies.hostEquals", "host equals")
                    value=host_equals.read_only()
                    set_value=host_equals.write_only()
                />
                <Input
                    aria_label=t(ui_locale.as_deref(), "channel.policies.hostSuffix", "host suffix")
                    placeholder=t(ui_locale.as_deref(), "channel.policies.hostSuffix", "host suffix")
                    value=host_suffix.read_only()
                    set_value=host_suffix.write_only()
                />
                <Input
                    aria_label=t(ui_locale.as_deref(), "channel.policies.locale", "locale")
                    placeholder=t(ui_locale.as_deref(), "channel.policies.locale", "locale")
                    value=locale.read_only()
                    set_value=locale.write_only()
                />
                <Select
                    aria_label=surface_label.clone()
                    value=surface.read_only()
                    set_value=surface.write_only()
                    options=Signal::derive(move || surface_options.clone())
                />
                <Select
                    aria_label=oauth_app_label.clone()
                    class="lg:col-span-2"
                    value=oauth_app_id.read_only()
                    set_value=oauth_app_id.write_only()
                    options=Signal::derive(move || oauth_app_options.clone())
                />
                <label class="flex items-center gap-2 text-sm text-muted-foreground lg:col-span-2">
                    <Checkbox checked=is_active.read_only() set_checked=is_active.write_only() />
                    {t(ui_locale.as_deref(), "channel.policies.ruleActive", "Rule is active")}
                </label>
                <Button
                    r#type="submit"
                    class="lg:col-span-2"
                    disabled=move || busy.get() || !has_channels
                >
                    {move || if editing_rule_id.get().is_some() {
                        save_rule_label.clone()
                    } else {
                        add_rule_label.clone()
                    }}
                </Button>
            </form>
        </section>
    }
}
