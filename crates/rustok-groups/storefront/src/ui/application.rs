use std::collections::{BTreeMap, BTreeSet};

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::use_route_query_writer;
use rustok_ui_core::UiRouteContext;

use crate::application_core::{
    prepare_group_application_policy_query, prepare_submit_group_membership_application,
    GroupsStorefrontApplicationInputError, GROUP_APPLICATION_QUERY_KEY,
};
use crate::application_model::{
    GroupsStorefrontApplicationPolicy, GroupsStorefrontSubmitApplicationResult,
};
use crate::core::groups_storefront_error;
use crate::i18n::t;
use crate::transport::{
    load_groups_storefront_application_policy, submit_groups_storefront_membership_application,
    GroupsStorefrontTransportContext,
};

#[derive(Clone)]
struct ApplicationCopy {
    title: String,
    body: String,
    loading: String,
    unavailable: String,
    required: String,
    optional: String,
    rules: String,
    acknowledge: String,
    submit: String,
    busy: String,
    error: String,
    success: String,
    pending: String,
    invalid_group_id: String,
    unknown_question: String,
    missing_answer: String,
    answer_too_long: String,
    unknown_rule: String,
    missing_rule: String,
}

#[component]
pub fn GroupsMembershipApplication(
    transport: GroupsStorefrontTransportContext,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let copy = application_copy(locale.as_deref());
    let application_group_id = route_context
        .query_value(GROUP_APPLICATION_QUERY_KEY)
        .unwrap_or_default()
        .to_string();
    let query_writer = use_route_query_writer();

    let (answers, set_answers) = signal(BTreeMap::<String, String>::new());
    let (acknowledged_rules, set_acknowledged_rules) = signal(BTreeSet::<String>::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (result, set_result) = signal(Option::<GroupsStorefrontSubmitApplicationResult>::None);

    let load_transport = transport.clone();
    let group_id_for_load = application_group_id.clone();
    let policy = LocalResource::new(move || {
        let context = load_transport.clone();
        let group_id = group_id_for_load.clone();
        async move {
            if group_id.trim().is_empty() {
                return Ok(None);
            }
            let query = prepare_group_application_policy_query(&group_id)
                .map_err(|_| "invalid application group UUID".to_string())?;
            load_groups_storefront_application_policy(context, query)
                .await
                .map(Some)
                .map_err(|error| error.to_string())
        }
    });

    let submit_transport = transport.clone();
    let submit_copy = copy.clone();
    let on_submit = Callback::new(move |event: SubmitEvent| {
        event.prevent_default();
        let Some(loaded_policy) = policy.get().and_then(Result::ok).flatten() else {
            set_error.set(Some(submit_copy.unavailable.clone()));
            set_result.set(None);
            return;
        };
        let command = match prepare_submit_group_membership_application(
            &loaded_policy,
            answers.get_untracked(),
            acknowledged_rules.get_untracked(),
        ) {
            Ok(command) => command,
            Err(input_error) => {
                set_error.set(Some(application_input_error_message(
                    input_error,
                    &submit_copy,
                )));
                set_result.set(None);
                return;
            }
        };
        let context = submit_transport.clone();
        let copy = submit_copy.clone();
        let query_writer = query_writer.clone();
        set_busy.set(true);
        set_error.set(None);
        set_result.set(None);
        spawn_local(async move {
            match submit_groups_storefront_membership_application(context, command).await {
                Ok(submitted) => {
                    set_result.set(Some(submitted));
                    query_writer.clear_key(GROUP_APPLICATION_QUERY_KEY);
                }
                Err(submit_error) => set_error.set(Some(groups_storefront_error(
                    &copy.error,
                    &submit_error.to_string(),
                ))),
            }
            set_busy.set(false);
        });
    });

    if application_group_id.trim().is_empty() {
        return view! { <></> }.into_any();
    }

    let ApplicationCopy {
        title,
        body,
        loading,
        unavailable,
        required,
        optional,
        rules: rules_label,
        acknowledge,
        submit,
        busy: busy_label,
        success,
        pending,
        ..
    } = copy;

    view! {
        <section class="groups-storefront__application rounded-3xl border border-border bg-card p-6 shadow-sm">
            <h2 class="text-xl font-semibold text-card-foreground">{title}</h2>
            <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{body}</p>

            <Suspense fallback=move || view! { <p class="mt-4 text-sm text-muted-foreground">{loading.clone()}</p> }>
                {move || policy.get().map(|loaded| match loaded {
                    Ok(Some(policy)) if policy.enabled => render_policy_form(
                        policy,
                        answers,
                        set_answers,
                        acknowledged_rules,
                        set_acknowledged_rules,
                        on_submit,
                        &required,
                        &optional,
                        &rules_label,
                        &acknowledge,
                        &submit,
                    ).into_any(),
                    Ok(Some(_)) | Ok(None) => view! { <p class="mt-4 text-sm text-muted-foreground">{unavailable.clone()}</p> }.into_any(),
                    Err(load_error) => view! {
                        <p class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">{groups_storefront_error(&unavailable, &load_error)}</p>
                    }.into_any(),
                })}
            </Suspense>

            <Show when=move || busy.get()>
                <p class="mt-4 text-sm text-muted-foreground" aria-live="polite">{busy_label.clone()}</p>
            </Show>
            <Show when=move || error.get().is_some()>
                <p class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">{move || error.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || result.get().is_some()>
                {move || result.get().map(|submitted| view! {
                    <div class="mt-4 rounded-xl border border-primary/30 bg-primary/5 px-4 py-3" role="status">
                        <p class="font-medium text-foreground">{success.clone()}</p>
                        <p class="mt-1 text-sm text-muted-foreground">{format!("{} · {}", pending.clone(), submitted.application.id)}</p>
                    </div>
                })}
            </Show>
        </section>
    }
    .into_any()
}

#[allow(clippy::too_many_arguments)]
fn render_policy_form(
    policy: GroupsStorefrontApplicationPolicy,
    answers: ReadSignal<BTreeMap<String, String>>,
    set_answers: WriteSignal<BTreeMap<String, String>>,
    acknowledged_rules: ReadSignal<BTreeSet<String>>,
    set_acknowledged_rules: WriteSignal<BTreeSet<String>>,
    on_submit: Callback<SubmitEvent>,
    required: &str,
    optional: &str,
    rules_label: &str,
    acknowledge: &str,
    submit: &str,
) -> impl IntoView {
    let questions = policy.questions;
    let rules = policy.rules;
    let has_rules = !rules.is_empty();
    view! {
        <form class="mt-6 space-y-6" on:submit=move |event| on_submit.run(event)>
            <div class="space-y-4">
                {questions.into_iter().map(|question| {
                    let answer_key = question.key.clone();
                    let value_key = question.key.clone();
                    let requirement = if question.required { required.to_string() } else { optional.to_string() };
                    view! {
                        <label class="block rounded-2xl border border-border p-4">
                            <span class="flex flex-wrap items-center justify-between gap-2 text-sm font-medium text-card-foreground">
                                <span>{question.prompt}</span>
                                <small class="text-xs font-normal text-muted-foreground">{requirement}</small>
                            </span>
                            {question.help_text.map(|text| view! { <small class="mt-1 block text-xs text-muted-foreground">{text}</small> })}
                            <textarea
                                class="mt-3 min-h-28 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm"
                                maxlength=question.max_answer_chars
                                prop:value=move || answers.get().get(&value_key).cloned().unwrap_or_default()
                                on:input=move |event| {
                                    let value = event_target_value(&event);
                                    set_answers.update(|items| {
                                        items.insert(answer_key.clone(), value);
                                    });
                                }
                            ></textarea>
                        </label>
                    }
                }).collect_view()}
            </div>

            <Show when=move || has_rules>
                <fieldset class="space-y-3 rounded-2xl border border-border p-4">
                    <legend class="px-2 text-sm font-semibold text-card-foreground">{rules_label.to_string()}</legend>
                    {rules.into_iter().map(|rule| {
                        let rule_key = rule.key.clone();
                        let checked_key = rule.key.clone();
                        view! {
                            <label class="flex items-start gap-3 rounded-xl bg-muted p-3">
                                <input
                                    class="mt-1"
                                    type="checkbox"
                                    prop:checked=move || acknowledged_rules.get().contains(&checked_key)
                                    on:change=move |event| {
                                        let checked = event_target_checked(&event);
                                        set_acknowledged_rules.update(|items| {
                                            if checked {
                                                items.insert(rule_key.clone());
                                            } else {
                                                items.remove(&rule_key);
                                            }
                                        });
                                    }
                                />
                                <span>
                                    <strong class="text-sm text-card-foreground">{rule.title}</strong>
                                    <p class="mt-1 whitespace-pre-wrap text-sm text-muted-foreground">{rule.body}</p>
                                    <small class="mt-1 block text-xs text-muted-foreground">{acknowledge.to_string()}</small>
                                </span>
                            </label>
                        }
                    }).collect_view()}
                </fieldset>
            </Show>

            <button class="rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground" type="submit">{submit.to_string()}</button>
        </form>
    }
}

fn application_input_error_message(
    error: GroupsStorefrontApplicationInputError,
    copy: &ApplicationCopy,
) -> String {
    match error {
        GroupsStorefrontApplicationInputError::InvalidGroupId => copy.invalid_group_id.clone(),
        GroupsStorefrontApplicationInputError::UnknownQuestion => copy.unknown_question.clone(),
        GroupsStorefrontApplicationInputError::MissingRequiredAnswer => copy.missing_answer.clone(),
        GroupsStorefrontApplicationInputError::AnswerTooLong => copy.answer_too_long.clone(),
        GroupsStorefrontApplicationInputError::UnknownRule => copy.unknown_rule.clone(),
        GroupsStorefrontApplicationInputError::MissingRequiredRule => copy.missing_rule.clone(),
    }
}

fn application_copy(locale: Option<&str>) -> ApplicationCopy {
    ApplicationCopy {
        title: t(locale, "groups.storefront.application.title", "Apply to join this group"),
        body: t(locale, "groups.storefront.application.body", "Answer the current membership questions and acknowledge required rules. Your submission keeps an immutable snapshot for review."),
        loading: t(locale, "groups.storefront.application.loading", "Loading membership policy..."),
        unavailable: t(locale, "groups.storefront.application.unavailable", "Membership applications are unavailable for this group or locale."),
        required: t(locale, "groups.storefront.application.required", "Required"),
        optional: t(locale, "groups.storefront.application.optional", "Optional"),
        rules: t(locale, "groups.storefront.application.rules", "Group rules"),
        acknowledge: t(locale, "groups.storefront.application.acknowledge", "I acknowledge this rule"),
        submit: t(locale, "groups.storefront.application.submit", "Submit application"),
        busy: t(locale, "groups.storefront.application.busy", "Submitting application..."),
        error: t(locale, "groups.storefront.application.error", "Membership application could not be submitted"),
        success: t(locale, "groups.storefront.application.success", "Application submitted for review."),
        pending: t(locale, "groups.storefront.application.pending", "Pending"),
        invalid_group_id: t(locale, "groups.storefront.application.invalidGroupId", "The application link contains an invalid group UUID."),
        unknown_question: t(locale, "groups.storefront.application.unknownQuestion", "The application contains an unknown question."),
        missing_answer: t(locale, "groups.storefront.application.missingAnswer", "Answer every required question."),
        answer_too_long: t(locale, "groups.storefront.application.answerTooLong", "One or more answers exceed their character limit."),
        unknown_rule: t(locale, "groups.storefront.application.unknownRule", "The application contains an unknown rule acknowledgement."),
        missing_rule: t(locale, "groups.storefront.application.missingRule", "Acknowledge every required group rule."),
    }
}
