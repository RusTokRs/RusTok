use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;

use crate::application_core::{
    prepare_group_application_policy_locale_catalog_query,
    prepare_group_application_policy_query, prepare_upsert_group_application_policy,
};
use crate::application_model::{
    GroupsAdminApplicationPolicyPrecondition, GroupsAdminApplicationPolicyRevision,
    GroupsAdminApplicationPolicyRevisionQuery, GroupsAdminApplicationQuestion,
    GroupsAdminApplicationRule,
};
use crate::core::{GroupsAdminTransportProfile, groups_admin_error, selected_transport_profile};
use crate::i18n::t;
use crate::transport::{
    load_group_admin_application_policy_for_management,
    load_group_admin_application_policy_locale_catalog,
    load_group_admin_application_policy_revisions, upsert_group_admin_application_policy,
    GroupsAdminTransportContext,
};

const GROUP_APPLICATION_POLICY_CHANGED_CODE: &str = "groups.application_policy_changed";

#[derive(Clone)]
struct PolicyEditorCopy {
    title: String,
    body: String,
    group_id: String,
    locale: String,
    available_locales: String,
    existing_translation: String,
    new_translation: String,
    enabled: String,
    load: String,
    save: String,
    add_question: String,
    add_rule: String,
    question_key: String,
    question_prompt: String,
    question_help: String,
    answer_limit: String,
    rule_key: String,
    rule_title: String,
    rule_body: String,
    required: String,
    move_up: String,
    move_down: String,
    remove: String,
    history: String,
    empty_history: String,
    revision: String,
    changed_by: String,
    busy: String,
    loaded: String,
    saved: String,
    stale: String,
    error: String,
    invalid: String,
}

#[component]
pub fn GroupsPolicyEditorAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let transport = transport_context(profile);
    let copy = policy_editor_copy(ui_locale.as_deref());

    let group_id = RwSignal::new(String::new());
    let locale = RwSignal::new(ui_locale.unwrap_or_else(|| "en".to_string()));
    let locale_options = RwSignal::new(Vec::<String>::new());
    let translation_exists = RwSignal::new(None::<bool>);
    let management_loaded = RwSignal::new(false);
    let enabled = RwSignal::new(true);
    let questions = RwSignal::new(Vec::<GroupsAdminApplicationQuestion>::new());
    let rules = RwSignal::new(Vec::<GroupsAdminApplicationRule>::new());
    let loaded_policy = RwSignal::new(None::<GroupsAdminApplicationPolicyPrecondition>);
    let history = RwSignal::new(Vec::<GroupsAdminApplicationPolicyRevision>::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let success = RwSignal::new(None::<String>);

    let load_transport = transport.clone();
    let load_copy = copy.clone();
    let on_load = Callback::new(move |_: ()| {
        let catalog_query = match prepare_group_application_policy_locale_catalog_query(
            &group_id.get_untracked(),
        ) {
            Ok(query) => query,
            Err(_) => {
                error.set(Some(load_copy.invalid.clone()));
                success.set(None);
                return;
            }
        };
        let policy_query = match prepare_group_application_policy_query(
            &group_id.get_untracked(),
            &locale.get_untracked(),
        ) {
            Ok(query) => query,
            Err(_) => {
                error.set(Some(load_copy.invalid.clone()));
                success.set(None);
                return;
            }
        };
        let revision_query = GroupsAdminApplicationPolicyRevisionQuery {
            group_id: policy_query.group_id.clone(),
            page: 1,
            per_page: 20,
        };
        let catalog_context = load_transport.clone();
        let policy_context = load_transport.clone();
        let history_context = load_transport.clone();
        let copy = load_copy.clone();
        busy.set(true);
        error.set(None);
        success.set(None);
        spawn_local(async move {
            match load_group_admin_application_policy_locale_catalog(
                catalog_context,
                catalog_query,
            )
            .await
            {
                Ok(catalog) => locale_options.set(catalog.locales),
                Err(load_error) => {
                    management_loaded.set(false);
                    error.set(Some(groups_admin_error(
                        &copy.error,
                        &load_error.to_string(),
                    )));
                    busy.set(false);
                    return;
                }
            }
            match load_group_admin_application_policy_for_management(
                policy_context,
                policy_query,
            )
            .await
            {
                Ok(policy) => {
                    let revision = policy.revision;
                    let exists = policy.translation_exists;
                    let expected = policy.precondition();
                    locale.set(policy.locale);
                    enabled.set(policy.enabled);
                    questions.set(policy.questions);
                    rules.set(policy.rules);
                    loaded_policy.set(expected);
                    translation_exists.set(Some(exists));
                    management_loaded.set(true);
                    let translation_state = if exists {
                        copy.existing_translation.clone()
                    } else {
                        copy.new_translation.clone()
                    };
                    success.set(Some(match revision {
                        Some(revision) => format!(
                            "{} · {} {} · {}",
                            copy.loaded, copy.revision, revision, translation_state
                        ),
                        None => format!("{} · {}", copy.loaded, translation_state),
                    }));
                }
                Err(load_error) => {
                    management_loaded.set(false);
                    loaded_policy.set(None);
                    translation_exists.set(None);
                    error.set(Some(groups_admin_error(
                        &copy.error,
                        &load_error.to_string(),
                    )));
                }
            }
            match load_group_admin_application_policy_revisions(history_context, revision_query)
                .await
            {
                Ok(connection) => history.set(connection.items),
                Err(_) => history.set(Vec::new()),
            }
            busy.set(false);
        });
    });

    let save_transport = transport.clone();
    let save_copy = copy.clone();
    let on_save = move |event: SubmitEvent| {
        event.prevent_default();
        if !management_loaded.get_untracked() {
            error.set(Some(save_copy.invalid.clone()));
            success.set(None);
            return;
        }
        let command = match prepare_upsert_group_application_policy(
            &group_id.get_untracked(),
            &locale.get_untracked(),
            loaded_policy.get_untracked(),
            enabled.get_untracked(),
            questions.get_untracked(),
            rules.get_untracked(),
        ) {
            Ok(command) => command,
            Err(_) => {
                error.set(Some(save_copy.invalid.clone()));
                success.set(None);
                return;
            }
        };

        let revision_query = GroupsAdminApplicationPolicyRevisionQuery {
            group_id: command.group_id.clone(),
            page: 1,
            per_page: 20,
        };
        let save_context = save_transport.clone();
        let history_context = save_transport.clone();
        let copy = save_copy.clone();
        busy.set(true);
        error.set(None);
        success.set(None);
        spawn_local(async move {
            match upsert_group_admin_application_policy(save_context, command).await {
                Ok(result) => {
                    let revision = result.policy.revision;
                    let saved_locale = result.policy.locale.clone();
                    let expected = GroupsAdminApplicationPolicyPrecondition::from(&result.policy);
                    locale.set(saved_locale.clone());
                    enabled.set(result.policy.enabled);
                    questions.set(result.policy.questions);
                    rules.set(result.policy.rules);
                    loaded_policy.set(Some(expected));
                    translation_exists.set(Some(true));
                    management_loaded.set(true);
                    locale_options.update(|items| {
                        if !items.contains(&saved_locale) {
                            items.push(saved_locale);
                            items.sort();
                        }
                    });
                    success.set(Some(format!(
                        "{} · {} {}",
                        copy.saved, copy.revision, revision
                    )));
                    if let Ok(connection) = load_group_admin_application_policy_revisions(
                        history_context,
                        revision_query,
                    )
                    .await
                    {
                        history.set(connection.items);
                    }
                }
                Err(save_error) => {
                    let details = save_error.to_string();
                    if details.contains(GROUP_APPLICATION_POLICY_CHANGED_CODE) {
                        management_loaded.set(false);
                        error.set(Some(copy.stale.clone()));
                    } else {
                        error.set(Some(groups_admin_error(&copy.error, &details)));
                    }
                }
            }
            busy.set(false);
        });
    };

    let add_question = move |_| {
        questions.update(|items| {
            if items.len() < 20 {
                items.push(GroupsAdminApplicationQuestion {
                    key: format!("question_{}", items.len() + 1),
                    prompt: String::new(),
                    help_text: None,
                    required: true,
                    max_answer_chars: 500,
                });
            }
        });
    };
    let add_rule = move |_| {
        rules.update(|items| {
            if items.len() < 20 {
                items.push(GroupsAdminApplicationRule {
                    key: format!("rule_{}", items.len() + 1),
                    title: String::new(),
                    body: String::new(),
                    required: true,
                });
            }
        });
    };

    let PolicyEditorCopy {
        title,
        body,
        group_id: group_id_label,
        locale: locale_label,
        available_locales,
        existing_translation,
        new_translation,
        enabled: enabled_label,
        load: load_label,
        save: save_label,
        add_question: add_question_label,
        add_rule: add_rule_label,
        question_key,
        question_prompt,
        question_help,
        answer_limit,
        rule_key,
        rule_title,
        rule_body,
        required,
        move_up,
        move_down,
        remove,
        history: history_label,
        empty_history,
        revision: revision_label,
        changed_by,
        busy: busy_label,
        ..
    } = copy;
    let question_required = required.clone();
    let question_move_up = move_up.clone();
    let question_move_down = move_down.clone();
    let question_remove = remove.clone();
    let rule_required = required;
    let rule_move_up = move_up;
    let rule_move_down = move_down;
    let rule_remove = remove;

    view! {
        <section class="groups-admin-policy-editor rounded-3xl border border-border bg-card p-6 shadow-sm">
            <h2 class="text-xl font-semibold text-card-foreground">{title}</h2>
            <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{body}</p>

            <form class="mt-6 space-y-6" on:submit=on_save>
                <div class="grid gap-3 md:grid-cols-[1fr_12rem_auto_auto]">
                    <input
                        class="rounded-xl border border-border bg-background px-3 py-2 text-sm"
                        placeholder=group_id_label
                        prop:value=move || group_id.get()
                        on:input=move |event| {
                            group_id.set(event_target_value(&event));
                            management_loaded.set(false);
                            loaded_policy.set(None);
                            translation_exists.set(None);
                        }
                    />
                    <div>
                        <input
                            class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm"
                            placeholder=locale_label
                            list="groups-policy-locales"
                            prop:value=move || locale.get()
                            on:input=move |event| {
                                locale.set(event_target_value(&event));
                                management_loaded.set(false);
                                loaded_policy.set(None);
                                translation_exists.set(None);
                                questions.set(Vec::new());
                                rules.set(Vec::new());
                            }
                        />
                        <datalist id="groups-policy-locales">
                            {move || locale_options.get().into_iter().map(|item| view! { <option value=item></option> }).collect_view()}
                        </datalist>
                    </div>
                    <label class="flex items-center gap-2 rounded-xl border border-border px-3 py-2 text-sm">
                        <input type="checkbox" prop:checked=move || enabled.get() on:change=move |event| enabled.set(event_target_checked(&event)) />
                        <span>{enabled_label}</span>
                    </label>
                    <button class="rounded-xl border border-border px-4 py-2 text-sm font-medium" type="button" on:click=move |_| on_load.run(())>{load_label}</button>
                </div>

                <Show when=move || !locale_options.get().is_empty()>
                    <p class="text-xs text-muted-foreground">{move || format!("{}: {}", available_locales.clone(), locale_options.get().join(", "))}</p>
                </Show>
                <Show when=move || translation_exists.get().is_some()>
                    <p class="text-xs text-muted-foreground">{move || if translation_exists.get().unwrap_or(false) { existing_translation.clone() } else { new_translation.clone() }}</p>
                </Show>

                <div class="space-y-3">
                    <div class="flex items-center justify-between gap-3">
                        <h3 class="font-semibold text-card-foreground">{add_question_label.clone()}</h3>
                        <button class="rounded-xl border border-border px-3 py-2 text-sm" type="button" on:click=add_question>{add_question_label}</button>
                    </div>
                    {move || questions.get().into_iter().enumerate().map(|(index, question)| {
                        let key_value = question.key.clone();
                        let prompt_value = question.prompt.clone();
                        let help_value = question.help_text.clone().unwrap_or_default();
                        view! {
                            <div class="space-y-3 rounded-2xl border border-border p-4">
                                <div class="grid gap-3 md:grid-cols-2">
                                    <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=question_key.clone() prop:value=key_value on:input=move |event| questions.update(|items| if let Some(item) = items.get_mut(index) { item.key = event_target_value(&event); }) />
                                    <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=question_prompt.clone() prop:value=prompt_value on:input=move |event| questions.update(|items| if let Some(item) = items.get_mut(index) { item.prompt = event_target_value(&event); }) />
                                </div>
                                <textarea class="min-h-20 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=question_help.clone() prop:value=help_value on:input=move |event| questions.update(|items| if let Some(item) = items.get_mut(index) { let value = event_target_value(&event); item.help_text = (!value.trim().is_empty()).then_some(value); })></textarea>
                                <div class="flex flex-wrap items-center gap-3">
                                    <label class="flex items-center gap-2 text-sm"><input type="checkbox" prop:checked=question.required on:change=move |event| questions.update(|items| if let Some(item) = items.get_mut(index) { item.required = event_target_checked(&event); }) /><span>{question_required.clone()}</span></label>
                                    <input
                                        class="w-40 rounded-xl border border-border bg-background px-3 py-2 text-sm"
                                        aria-label=answer_limit.clone()
                                        prop:value=question.max_answer_chars.to_string()
                                        on:input=move |event| {
                                            if let Ok(value) = event_target_value(&event).parse::<u32>() {
                                                questions.update(|items| {
                                                    if let Some(item) = items.get_mut(index) {
                                                        item.max_answer_chars = value;
                                                    }
                                                });
                                            }
                                        }
                                    />
                                    <button type="button" class="rounded-lg border border-border px-2 py-1 text-xs" on:click=move |_| move_item(questions, index, -1)>{question_move_up.clone()}</button>
                                    <button type="button" class="rounded-lg border border-border px-2 py-1 text-xs" on:click=move |_| move_item(questions, index, 1)>{question_move_down.clone()}</button>
                                    <button type="button" class="rounded-lg border border-destructive px-2 py-1 text-xs text-destructive" on:click=move |_| questions.update(|items| { if index < items.len() { items.remove(index); } })>{question_remove.clone()}</button>
                                </div>
                            </div>
                        }
                    }).collect_view()}
                </div>

                <div class="space-y-3">
                    <div class="flex items-center justify-between gap-3">
                        <h3 class="font-semibold text-card-foreground">{add_rule_label.clone()}</h3>
                        <button class="rounded-xl border border-border px-3 py-2 text-sm" type="button" on:click=add_rule>{add_rule_label}</button>
                    </div>
                    {move || rules.get().into_iter().enumerate().map(|(index, rule)| {
                        let key_value = rule.key.clone();
                        let title_value = rule.title.clone();
                        let body_value = rule.body.clone();
                        view! {
                            <div class="space-y-3 rounded-2xl border border-border p-4">
                                <div class="grid gap-3 md:grid-cols-2">
                                    <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=rule_key.clone() prop:value=key_value on:input=move |event| rules.update(|items| if let Some(item) = items.get_mut(index) { item.key = event_target_value(&event); }) />
                                    <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=rule_title.clone() prop:value=title_value on:input=move |event| rules.update(|items| if let Some(item) = items.get_mut(index) { item.title = event_target_value(&event); }) />
                                </div>
                                <textarea class="min-h-28 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=rule_body.clone() prop:value=body_value on:input=move |event| rules.update(|items| if let Some(item) = items.get_mut(index) { item.body = event_target_value(&event); })></textarea>
                                <div class="flex flex-wrap items-center gap-3">
                                    <label class="flex items-center gap-2 text-sm"><input type="checkbox" prop:checked=rule.required on:change=move |event| rules.update(|items| if let Some(item) = items.get_mut(index) { item.required = event_target_checked(&event); }) /><span>{rule_required.clone()}</span></label>
                                    <button type="button" class="rounded-lg border border-border px-2 py-1 text-xs" on:click=move |_| move_item(rules, index, -1)>{rule_move_up.clone()}</button>
                                    <button type="button" class="rounded-lg border border-border px-2 py-1 text-xs" on:click=move |_| move_item(rules, index, 1)>{rule_move_down.clone()}</button>
                                    <button type="button" class="rounded-lg border border-destructive px-2 py-1 text-xs text-destructive" on:click=move |_| rules.update(|items| { if index < items.len() { items.remove(index); } })>{rule_remove.clone()}</button>
                                </div>
                            </div>
                        }
                    }).collect_view()}
                </div>

                <button class="rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:cursor-not-allowed disabled:opacity-50" type="submit" disabled=move || !management_loaded.get()>{save_label}</button>
            </form>

            <Show when=move || busy.get()><p class="mt-4 text-sm text-muted-foreground">{busy_label.clone()}</p></Show>
            <Show when=move || error.get().is_some()><p class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">{move || error.get().unwrap_or_default()}</p></Show>
            <Show when=move || success.get().is_some()><p class="mt-4 rounded-xl border border-border bg-muted px-4 py-3 text-sm" role="status">{move || success.get().unwrap_or_default()}</p></Show>

            <div class="mt-8 space-y-3">
                <h3 class="font-semibold text-card-foreground">{history_label}</h3>
                {move || {
                    let items = history.get();
                    if items.is_empty() {
                        view! { <p class="text-sm text-muted-foreground">{empty_history.clone()}</p> }.into_any()
                    } else {
                        view! {
                            <ul class="grid gap-3 md:grid-cols-2">
                                {items.into_iter().map(|item| view! {
                                    <li class="rounded-2xl border border-border p-4">
                                        <strong class="text-sm text-card-foreground">{format!("{} {} · {}", revision_label, item.revision, item.locale)}</strong>
                                        <p class="mt-2 text-xs text-muted-foreground">{format!("{} {} · {}", changed_by, item.created_by_user_id, item.created_at)}</p>
                                        <p class="mt-2 text-sm text-muted-foreground">{format!("{} questions · {} rules · enabled={}", item.questions.len(), item.rules.len(), item.enabled)}</p>
                                    </li>
                                }).collect_view()}
                            </ul>
                        }.into_any()
                    }
                }}
            </div>
        </section>
    }
}

fn move_item<T: Send + Sync + 'static>(signal: RwSignal<Vec<T>>, index: usize, direction: isize) {
    signal.update(|items| {
        let target = index as isize + direction;
        if index < items.len() && target >= 0 && (target as usize) < items.len() {
            items.swap(index, target as usize);
        }
    });
}

fn policy_editor_copy(locale: Option<&str>) -> PolicyEditorCopy {
    PolicyEditorCopy {
        title: t(locale, "groups.admin.policyEditor.title", "Membership policy editor"),
        body: t(locale, "groups.admin.policyEditor.body", "Select an exact policy locale, edit its questions and rules, and save through owner CAS without changing the host UI locale."),
        group_id: t(locale, "groups.admin.policyEditor.groupId", "Group UUID"),
        locale: t(locale, "groups.admin.policyEditor.locale", "Policy locale"),
        available_locales: t(locale, "groups.admin.policyEditor.availableLocales", "Available locales"),
        existing_translation: t(locale, "groups.admin.policyEditor.existingTranslation", "Editing an existing exact-locale translation."),
        new_translation: t(locale, "groups.admin.policyEditor.newTranslation", "This locale has no translation yet. Saving creates it with the current policy CAS precondition."),
        enabled: t(locale, "groups.admin.policyEditor.enabled", "Applications enabled"),
        load: t(locale, "groups.admin.policyEditor.load", "Load selected locale"),
        save: t(locale, "groups.admin.policyEditor.save", "Save policy"),
        add_question: t(
            locale,
            "groups.admin.policyEditor.addQuestion",
            "Add question",
        ),
        add_rule: t(locale, "groups.admin.policyEditor.addRule", "Add rule"),
        question_key: t(
            locale,
            "groups.admin.policyEditor.questionKey",
            "Question key",
        ),
        question_prompt: t(
            locale,
            "groups.admin.policyEditor.questionPrompt",
            "Question prompt",
        ),
        question_help: t(
            locale,
            "groups.admin.policyEditor.questionHelp",
            "Help text (optional)",
        ),
        answer_limit: t(
            locale,
            "groups.admin.policyEditor.answerLimit",
            "Maximum answer characters",
        ),
        rule_key: t(locale, "groups.admin.policyEditor.ruleKey", "Rule key"),
        rule_title: t(locale, "groups.admin.policyEditor.ruleTitle", "Rule title"),
        rule_body: t(locale, "groups.admin.policyEditor.ruleBody", "Rule body"),
        required: t(locale, "groups.admin.policyEditor.required", "Required"),
        move_up: t(locale, "groups.admin.policyEditor.moveUp", "Move up"),
        move_down: t(locale, "groups.admin.policyEditor.moveDown", "Move down"),
        remove: t(locale, "groups.admin.policyEditor.remove", "Remove"),
        history: t(
            locale,
            "groups.admin.policyEditor.history",
            "Policy revision history",
        ),
        empty_history: t(
            locale,
            "groups.admin.policyEditor.emptyHistory",
            "No policy revisions loaded.",
        ),
        revision: t(locale, "groups.admin.policyEditor.revision", "revision"),
        changed_by: t(locale, "groups.admin.policyEditor.changedBy", "changed by"),
        busy: t(locale, "groups.admin.policyEditor.busy", "Applying policy operation..."),
        loaded: t(locale, "groups.admin.policyEditor.loaded", "Policy locale loaded"),
        saved: t(locale, "groups.admin.policyEditor.saved", "Policy saved"),
        stale: t(locale, "groups.admin.policyEditor.stale", "The owner service rejected this stale policy atomically. Reload the selected locale before saving again."),
        error: t(locale, "groups.admin.policyEditor.error", "Policy operation failed"),
        invalid: t(locale, "groups.admin.policyEditor.invalid", "Load a valid group and exact locale before saving, then check the expected revision, question keys, text limits, and rule fields."),
    }
}

fn transport_context(profile: GroupsAdminTransportProfile) -> GroupsAdminTransportContext {
    match profile {
        GroupsAdminTransportProfile::Native => GroupsAdminTransportContext::native(),
        GroupsAdminTransportProfile::Graphql => GroupsAdminTransportContext::graphql(
            None,
            option_env!("RUSTOK_TENANT_SLUG").map(str::to_string),
        ),
    }
}
