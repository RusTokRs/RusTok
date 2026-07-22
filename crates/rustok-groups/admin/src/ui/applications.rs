use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;

use crate::application_core::{
    prepare_group_membership_application_query, prepare_reopen_group_membership_application,
    prepare_review_group_membership_application, GroupsAdminApplicationInputError,
};
use crate::application_model::{
    GroupsAdminApplicationReviewDecision, GroupsAdminMembershipApplication,
};
use crate::core::{GroupsAdminTransportProfile, groups_admin_error, selected_transport_profile};
use crate::i18n::t;
use crate::transport::{
    load_group_admin_membership_applications, reopen_group_admin_membership_application,
    review_group_admin_membership_application, GroupsAdminTransportContext,
};

#[derive(Clone)]
struct ApplicationCopy {
    title: String,
    body: String,
    group_id: String,
    status: String,
    pending: String,
    approved: String,
    rejected: String,
    cancelled: String,
    load: String,
    empty: String,
    application_id: String,
    decision: String,
    approve: String,
    reject: String,
    note: String,
    review: String,
    reopen: String,
    busy: String,
    error: String,
    loaded: String,
    reviewed: String,
    reopened: String,
    user: String,
    policy: String,
    answers: String,
    acknowledgements: String,
    invalid_group_id: String,
    invalid_application_id: String,
    invalid_status: String,
    review_note_too_long: String,
}

#[component]
pub fn GroupsApplicationsAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let transport = transport_context(profile);
    let copy = application_copy(locale.as_deref());

    let (group_id, set_group_id) = signal(String::new());
    let (status, set_status) = signal("pending".to_string());
    let (application_id, set_application_id) = signal(String::new());
    let (review_note, set_review_note) = signal(String::new());
    let (decision, set_decision) = signal(GroupsAdminApplicationReviewDecision::Approve);
    let (applications, set_applications) = signal(Vec::<GroupsAdminMembershipApplication>::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (success, set_success) = signal(Option::<String>::None);

    let load_transport = transport.clone();
    let load_copy = copy.clone();
    let on_load = move |event: SubmitEvent| {
        event.prevent_default();
        let selected_status = status.get_untracked();
        let query = match prepare_group_membership_application_query(
            &group_id.get_untracked(),
            Some(&selected_status),
        ) {
            Ok(query) => query,
            Err(input_error) => {
                set_error.set(Some(application_input_error_message(
                    input_error,
                    &load_copy,
                )));
                set_success.set(None);
                return;
            }
        };
        let context = load_transport.clone();
        let copy = load_copy.clone();
        set_busy.set(true);
        set_error.set(None);
        set_success.set(None);
        spawn_local(async move {
            match load_group_admin_membership_applications(context, query).await {
                Ok(connection) => {
                    let count = connection.items.len();
                    set_applications.set(connection.items);
                    set_success.set(Some(format!("{}: {count}", copy.loaded)));
                }
                Err(load_error) => set_error.set(Some(groups_admin_error(
                    &copy.error,
                    &load_error.to_string(),
                ))),
            }
            set_busy.set(false);
        });
    };

    let review_transport = transport.clone();
    let review_copy = copy.clone();
    let on_review = move |event: SubmitEvent| {
        event.prevent_default();
        let command = match prepare_review_group_membership_application(
            &application_id.get_untracked(),
            decision.get_untracked(),
            Some(review_note.get_untracked()),
        ) {
            Ok(command) => command,
            Err(input_error) => {
                set_error.set(Some(application_input_error_message(
                    input_error,
                    &review_copy,
                )));
                set_success.set(None);
                return;
            }
        };
        let context = review_transport.clone();
        let copy = review_copy.clone();
        set_busy.set(true);
        set_error.set(None);
        set_success.set(None);
        spawn_local(async move {
            match review_group_admin_membership_application(context, command).await {
                Ok(result) => {
                    let reviewed_id = result.application.id.clone();
                    set_applications.update(|items| items.retain(|item| item.id != reviewed_id));
                    set_application_id.set(String::new());
                    set_review_note.set(String::new());
                    set_success.set(Some(format!(
                        "{} · group version {}",
                        copy.reviewed, result.group_version
                    )));
                }
                Err(review_error) => set_error.set(Some(groups_admin_error(
                    &copy.error,
                    &review_error.to_string(),
                ))),
            }
            set_busy.set(false);
        });
    };

    let reopen_transport = transport.clone();
    let reopen_copy = copy.clone();
    let on_reopen = Callback::new(move |selected_application_id: String| {
        let command = match prepare_reopen_group_membership_application(&selected_application_id) {
            Ok(command) => command,
            Err(input_error) => {
                set_error.set(Some(application_input_error_message(
                    input_error,
                    &reopen_copy,
                )));
                set_success.set(None);
                return;
            }
        };
        let context = reopen_transport.clone();
        let copy = reopen_copy.clone();
        set_busy.set(true);
        set_error.set(None);
        set_success.set(None);
        spawn_local(async move {
            match reopen_group_admin_membership_application(context, command).await {
                Ok(result) => {
                    let reopened_id = result.application.id.clone();
                    set_applications.update(|items| items.retain(|item| item.id != reopened_id));
                    set_application_id.set(String::new());
                    set_success.set(Some(format!(
                        "{} · group version {}",
                        copy.reopened, result.group_version
                    )));
                }
                Err(reopen_error) => set_error.set(Some(groups_admin_error(
                    &copy.error,
                    &reopen_error.to_string(),
                ))),
            }
            set_busy.set(false);
        });
    });

    let ApplicationCopy {
        title,
        body,
        group_id: group_id_label,
        status: status_label,
        pending,
        approved,
        rejected,
        cancelled,
        load,
        empty,
        application_id: application_id_label,
        decision: decision_label,
        approve,
        reject,
        note,
        review,
        reopen,
        busy: busy_label,
        user: user_label,
        policy: policy_label,
        answers: answers_label,
        acknowledgements: acknowledgements_label,
        ..
    } = copy;

    view! {
        <section class="groups-admin-applications rounded-3xl border border-border bg-card p-6 shadow-sm">
            <h2 class="text-xl font-semibold text-card-foreground">{title}</h2>
            <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{body}</p>

            <form class="mt-6 grid gap-3 md:grid-cols-[1fr_14rem_auto]" on:submit=on_load>
                <input
                    class="rounded-xl border border-border bg-background px-3 py-2 text-sm"
                    placeholder=group_id_label
                    prop:value=move || group_id.get()
                    on:input=move |event| set_group_id.set(event_target_value(&event))
                />
                <label class="sr-only">{status_label.clone()}</label>
                <select
                    class="rounded-xl border border-border bg-background px-3 py-2 text-sm"
                    aria-label=status_label
                    on:change=move |event| set_status.set(event_target_value(&event))
                >
                    <option value="pending">{pending}</option>
                    <option value="rejected">{rejected}</option>
                    <option value="cancelled">{cancelled}</option>
                    <option value="approved">{approved}</option>
                </select>
                <button class="rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground" type="submit">{load}</button>
            </form>

            <Show when=move || error.get().is_some()>
                <p class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">{move || error.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || success.get().is_some()>
                <p class="mt-4 rounded-xl border border-border bg-muted px-4 py-3 text-sm text-foreground" role="status">{move || success.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || busy.get()>
                <p class="mt-4 text-sm text-muted-foreground" aria-live="polite">{busy_label}</p>
            </Show>

            <div class="mt-6">
                {move || {
                    let items = applications.get();
                    if items.is_empty() {
                        view! { <p class="text-sm text-muted-foreground">{empty.clone()}</p> }.into_any()
                    } else {
                        view! {
                            <ul class="grid gap-4">
                                {items.into_iter().map(|item| {
                                    let application_id_for_pick = item.id.clone();
                                    let application_id_for_reopen = item.id.clone();
                                    let reopenable = matches!(item.status.as_str(), "rejected" | "cancelled");
                                    let reopen_callback = on_reopen.clone();
                                    let answers = item.answers.clone();
                                    let acknowledgements = item.acknowledged_rule_keys.join(", ");
                                    view! {
                                        <li class="rounded-2xl border border-border p-5">
                                            <div class="flex flex-wrap items-center justify-between gap-3">
                                                <button
                                                    class="text-left font-mono text-xs text-primary underline-offset-4 hover:underline"
                                                    type="button"
                                                    on:click=move |_| set_application_id.set(application_id_for_pick.clone())
                                                >
                                                    {item.id.clone()}
                                                </button>
                                                <div class="flex items-center gap-2">
                                                    <span class="rounded-full bg-muted px-3 py-1 text-xs text-muted-foreground">{item.status.clone()}</span>
                                                    <Show when=move || reopenable>
                                                        <button
                                                            class="rounded-lg border border-border bg-background px-3 py-1 text-xs font-medium"
                                                            type="button"
                                                            on:click=move |_| reopen_callback.run(application_id_for_reopen.clone())
                                                        >
                                                            {reopen.clone()}
                                                        </button>
                                                    </Show>
                                                </div>
                                            </div>
                                            <dl class="mt-4 grid gap-3 text-sm md:grid-cols-2">
                                                <div><dt class="text-xs text-muted-foreground">{user_label.clone()}</dt><dd class="break-all">{item.user_id}</dd></div>
                                                <div><dt class="text-xs text-muted-foreground">{policy_label.clone()}</dt><dd>{format!("{} · rev {} · {}", item.policy_id, item.policy_revision, item.policy_locale)}</dd></div>
                                            </dl>
                                            <div class="mt-4">
                                                <h3 class="text-sm font-medium text-card-foreground">{answers_label.clone()}</h3>
                                                <ul class="mt-2 space-y-2">
                                                    {answers.into_iter().map(|answer| view! {
                                                        <li class="rounded-xl bg-muted px-3 py-2 text-sm"><strong>{answer.key}</strong><p class="mt-1 whitespace-pre-wrap">{answer.value}</p></li>
                                                    }).collect_view()}
                                                </ul>
                                            </div>
                                            <p class="mt-4 text-xs text-muted-foreground">{format!("{}: {}", acknowledgements_label.clone(), acknowledgements)}</p>
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                        }.into_any()
                    }
                }}
            </div>

            <form class="mt-6 grid gap-3 rounded-2xl border border-border p-5 md:grid-cols-2" on:submit=on_review>
                <input
                    class="rounded-xl border border-border bg-background px-3 py-2 text-sm md:col-span-2"
                    placeholder=application_id_label
                    prop:value=move || application_id.get()
                    on:input=move |event| set_application_id.set(event_target_value(&event))
                />
                <label class="text-sm text-muted-foreground">
                    <span class="mb-2 block">{decision_label}</span>
                    <select
                        class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm"
                        on:change=move |event| {
                            let value = event_target_value(&event);
                            set_decision.set(if value == "reject" {
                                GroupsAdminApplicationReviewDecision::Reject
                            } else {
                                GroupsAdminApplicationReviewDecision::Approve
                            });
                        }
                    >
                        <option value="approve">{approve}</option>
                        <option value="reject">{reject}</option>
                    </select>
                </label>
                <label class="text-sm text-muted-foreground">
                    <span class="mb-2 block">{note}</span>
                    <textarea
                        class="min-h-24 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm"
                        prop:value=move || review_note.get()
                        on:input=move |event| set_review_note.set(event_target_value(&event))
                    ></textarea>
                </label>
                <button class="rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground md:col-span-2" type="submit">{review}</button>
            </form>
        </section>
    }
}

fn application_input_error_message(
    error: GroupsAdminApplicationInputError,
    copy: &ApplicationCopy,
) -> String {
    match error {
        GroupsAdminApplicationInputError::InvalidGroupId => copy.invalid_group_id.clone(),
        GroupsAdminApplicationInputError::InvalidApplicationId => {
            copy.invalid_application_id.clone()
        }
        GroupsAdminApplicationInputError::InvalidStatus => copy.invalid_status.clone(),
        GroupsAdminApplicationInputError::ReviewNoteTooLong => copy.review_note_too_long.clone(),
        GroupsAdminApplicationInputError::InvalidLocale
        | GroupsAdminApplicationInputError::InvalidExpectedPolicy
        | GroupsAdminApplicationInputError::TooManyQuestions
        | GroupsAdminApplicationInputError::TooManyRules
        | GroupsAdminApplicationInputError::InvalidQuestion
        | GroupsAdminApplicationInputError::InvalidRule => copy.error.clone(),
    }
}

fn application_copy(locale: Option<&str>) -> ApplicationCopy {
    ApplicationCopy {
        title: t(locale, "groups.admin.applications.title", "Membership applications"),
        body: t(locale, "groups.admin.applications.body", "Review pending applications or reopen rejected and cancelled applications while preserving their policy snapshot."),
        group_id: t(locale, "groups.admin.applications.groupId", "Group UUID"),
        status: t(locale, "groups.admin.applications.status", "Application status"),
        pending: t(locale, "groups.admin.applications.pending", "Pending"),
        approved: t(locale, "groups.admin.applications.approved", "Approved"),
        rejected: t(locale, "groups.admin.applications.rejected", "Rejected"),
        cancelled: t(locale, "groups.admin.applications.cancelled", "Cancelled"),
        load: t(locale, "groups.admin.applications.load", "Load applications"),
        empty: t(locale, "groups.admin.applications.empty", "No applications loaded."),
        application_id: t(locale, "groups.admin.applications.applicationId", "Application UUID"),
        decision: t(locale, "groups.admin.applications.decision", "Decision"),
        approve: t(locale, "groups.admin.applications.approve", "Approve"),
        reject: t(locale, "groups.admin.applications.reject", "Reject"),
        note: t(
            locale,
            "groups.admin.applications.note",
            "Review note (optional)",
        ),
        review: t(locale, "groups.admin.applications.review", "Apply review"),
        reopen: t(locale, "groups.admin.applications.reopen", "Reopen"),
        busy: t(locale, "groups.admin.applications.busy", "Applying membership application command..."),
        error: t(locale, "groups.admin.applications.error", "Membership application command failed"),
        loaded: t(locale, "groups.admin.applications.loaded", "Applications loaded"),
        reviewed: t(locale, "groups.admin.applications.reviewed", "Application reviewed"),
        reopened: t(locale, "groups.admin.applications.reopened", "Application reopened"),
        user: t(locale, "groups.admin.applications.user", "Candidate"),
        policy: t(
            locale,
            "groups.admin.applications.policy",
            "Policy snapshot",
        ),
        answers: t(locale, "groups.admin.applications.answers", "Answers"),
        acknowledgements: t(
            locale,
            "groups.admin.applications.acknowledgements",
            "Acknowledged rules",
        ),
        invalid_group_id: t(
            locale,
            "groups.admin.applications.invalidGroupId",
            "Enter a valid group UUID.",
        ),
        invalid_application_id: t(
            locale,
            "groups.admin.applications.invalidApplicationId",
            "Enter a valid application UUID.",
        ),
        invalid_status: t(
            locale,
            "groups.admin.applications.invalidStatus",
            "Select a supported application status.",
        ),
        review_note_too_long: t(
            locale,
            "groups.admin.applications.reviewNoteTooLong",
            "The review note must not exceed 2000 characters.",
        ),
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
