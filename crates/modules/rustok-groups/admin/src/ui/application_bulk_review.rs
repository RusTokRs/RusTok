use std::collections::BTreeSet;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;

use crate::application_bulk_core::{
    GroupsAdminBulkReviewInputError, prepare_bulk_review_group_membership_application_query,
    prepare_bulk_review_group_membership_applications,
};
use crate::application_bulk_transport::bulk_review_group_admin_membership_applications;
use crate::application_model::{
    GroupsAdminApplicationReviewDecision, GroupsAdminBulkReviewApplicationItemResult,
    GroupsAdminMembershipApplication,
};
use crate::core::{GroupsAdminTransportProfile, groups_admin_error, selected_transport_profile};
use crate::i18n::t;
use crate::transport::{GroupsAdminTransportContext, load_group_admin_membership_applications};

const MAX_BULK_REVIEW_ITEMS: usize = 50;

#[derive(Clone)]
struct BulkReviewCopy {
    title: String,
    body: String,
    group_id: String,
    load: String,
    empty: String,
    select_all: String,
    select_application: String,
    clear: String,
    selected: String,
    decision: String,
    approve: String,
    reject: String,
    note: String,
    revision: String,
    confirm: String,
    submit: String,
    busy: String,
    loaded: String,
    completed: String,
    error: String,
    invalid_group_id: String,
    empty_selection: String,
    too_many: String,
    duplicate: String,
    invalid_application_id: String,
    confirmation_required: String,
    review_note_too_long: String,
    succeeded: String,
    failed: String,
    replayed: String,
}

#[component]
pub fn GroupsApplicationsBulkReviewAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let transport = transport_context(profile);
    let copy = bulk_review_copy(locale.as_deref());

    let (group_id, set_group_id) = signal(String::new());
    let (applications, set_applications) = signal(Vec::<GroupsAdminMembershipApplication>::new());
    let (selected_ids, set_selected_ids) = signal(BTreeSet::<String>::new());
    let (decision, set_decision) = signal(GroupsAdminApplicationReviewDecision::Approve);
    let (review_note, set_review_note) = signal(String::new());
    let (confirmed, set_confirmed) = signal(false);
    let (results, set_results) = signal(Vec::<GroupsAdminBulkReviewApplicationItemResult>::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (success, set_success) = signal(Option::<String>::None);

    let load_transport = transport.clone();
    let load_copy = copy.clone();
    let on_load = move |event: SubmitEvent| {
        event.prevent_default();
        let query =
            match prepare_bulk_review_group_membership_application_query(&group_id.get_untracked())
            {
                Ok(query) => query,
                Err(_) => {
                    set_error.set(Some(load_copy.invalid_group_id.clone()));
                    set_success.set(None);
                    return;
                }
            };
        let context = load_transport.clone();
        let copy = load_copy.clone();
        set_busy.set(true);
        set_error.set(None);
        set_success.set(None);
        set_results.set(Vec::new());
        set_selected_ids.set(BTreeSet::new());
        set_confirmed.set(false);
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

    let select_all_copy = copy.clone();
    let on_select_all = move |_| {
        let ids = applications
            .get_untracked()
            .into_iter()
            .filter(|application| application.status == "pending")
            .map(|application| application.id)
            .take(MAX_BULK_REVIEW_ITEMS + 1)
            .collect::<Vec<_>>();
        if ids.len() > MAX_BULK_REVIEW_ITEMS {
            set_error.set(Some(select_all_copy.too_many.clone()));
            return;
        }
        set_selected_ids.set(ids.into_iter().collect());
        set_confirmed.set(false);
        set_error.set(None);
    };

    let submit_transport = transport.clone();
    let submit_copy = copy.clone();
    let on_submit = move |event: SubmitEvent| {
        event.prevent_default();
        let command = match prepare_bulk_review_group_membership_applications(
            selected_ids.get_untracked().into_iter().collect(),
            decision.get_untracked(),
            Some(review_note.get_untracked()),
            confirmed.get_untracked(),
        ) {
            Ok(command) => command,
            Err(input_error) => {
                set_error.set(Some(bulk_input_error_message(input_error, &submit_copy)));
                set_success.set(None);
                return;
            }
        };
        let context = submit_transport.clone();
        let copy = submit_copy.clone();
        set_busy.set(true);
        set_error.set(None);
        set_success.set(None);
        set_results.set(Vec::new());
        spawn_local(async move {
            match bulk_review_group_admin_membership_applications(context, command).await {
                Ok(result) => {
                    let succeeded_count = result.succeeded;
                    let failed_count = result.failed;
                    let item_results = result.items;
                    let successful_ids = item_results
                        .iter()
                        .filter_map(|item| {
                            item.result.as_ref().map(|_| item.application_id.clone())
                        })
                        .collect::<BTreeSet<_>>();
                    set_applications.update(|items| {
                        items.retain(|item| !successful_ids.contains(&item.id));
                    });
                    set_selected_ids.update(|ids| {
                        ids.retain(|id| !successful_ids.contains(id));
                    });
                    set_confirmed.set(false);
                    set_results.set(item_results);
                    set_success.set(Some(format!(
                        "{} · {}: {} · {}: {}",
                        copy.completed, copy.succeeded, succeeded_count, copy.failed, failed_count
                    )));
                }
                Err(submit_error) => set_error.set(Some(groups_admin_error(
                    &copy.error,
                    &submit_error.to_string(),
                ))),
            }
            set_busy.set(false);
        });
    };

    let selection_limit_error = copy.too_many.clone();
    let BulkReviewCopy {
        title,
        body,
        group_id: group_id_label,
        load,
        empty,
        select_all,
        select_application,
        clear,
        selected,
        decision: decision_label,
        approve,
        reject,
        note,
        revision,
        confirm,
        submit,
        busy: busy_label,
        succeeded,
        failed,
        replayed,
        ..
    } = copy;

    view! {
        <section class="groups-admin-application-bulk-review rounded-3xl border border-border bg-card p-6 shadow-sm">
            <h2 class="text-xl font-semibold text-card-foreground">{title}</h2>
            <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{body}</p>

            <form class="mt-6 flex flex-wrap gap-3" on:submit=on_load>
                <input
                    class="min-w-72 flex-1 rounded-xl border border-border bg-background px-3 py-2 text-sm"
                    placeholder=group_id_label
                    prop:value=move || group_id.get()
                    on:input=move |event| set_group_id.set(event_target_value(&event))
                />
                <button class="rounded-xl border border-border bg-background px-4 py-2 text-sm font-medium" type="submit" disabled=move || busy.get()>{load}</button>
            </form>

            <Show when=move || error.get().is_some()>
                <p class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">{move || error.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || success.get().is_some()>
                <p class="mt-4 rounded-xl border border-border bg-muted px-4 py-3 text-sm text-foreground" role="status" aria-live="polite">{move || success.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || busy.get()>
                <p class="mt-4 text-sm text-muted-foreground" aria-live="polite">{busy_label.clone()}</p>
            </Show>

            <div class="mt-6 flex flex-wrap items-center gap-3">
                <button class="rounded-xl border border-border bg-background px-3 py-2 text-sm" type="button" on:click=on_select_all disabled=move || busy.get()>{select_all}</button>
                <button class="rounded-xl border border-border bg-background px-3 py-2 text-sm" type="button" on:click=move |_| { set_selected_ids.set(BTreeSet::new()); set_confirmed.set(false); } disabled=move || busy.get()>{clear}</button>
                <span class="text-sm text-muted-foreground" aria-live="polite">{move || format!("{}: {}/{}", selected, selected_ids.get().len(), MAX_BULK_REVIEW_ITEMS)}</span>
            </div>

            <div class="mt-4 grid gap-2">
                {move || {
                    let items = applications.get();
                    if items.is_empty() {
                        view! { <p class="text-sm text-muted-foreground">{empty.clone()}</p> }.into_any()
                    } else {
                        let select_application = select_application.clone();
                        let revision = revision.clone();
                        let limit_error = selection_limit_error.clone();
                        view! {
                            <ul class="grid gap-2">
                                {items.into_iter().map(|item| {
                                    let item_id = item.id.clone();
                                    let item_id_for_checked = item.id.clone();
                                    let item_id_for_change = item.id.clone();
                                    let item_select_label = select_application.clone();
                                    let item_revision_label = revision.clone();
                                    let item_limit_error = limit_error.clone();
                                    view! {
                                        <li class="flex items-center gap-3 rounded-xl border border-border px-4 py-3">
                                            <input
                                                type="checkbox"
                                                aria-label=format!("{} {}", item_select_label, item_id)
                                                prop:checked=move || selected_ids.get().contains(&item_id_for_checked)
                                                on:change=move |event| {
                                                    let checked = event_target_checked(&event);
                                                    if checked && selected_ids.get_untracked().len() >= MAX_BULK_REVIEW_ITEMS {
                                                        set_error.set(Some(item_limit_error.clone()));
                                                        return;
                                                    }
                                                    set_selected_ids.update(|ids| {
                                                        if checked {
                                                            ids.insert(item_id_for_change.clone());
                                                        } else {
                                                            ids.remove(&item_id_for_change);
                                                        }
                                                    });
                                                    set_confirmed.set(false);
                                                    set_error.set(None);
                                                }
                                            />
                                            <div class="min-w-0">
                                                <p class="break-all font-mono text-xs text-card-foreground">{item.id}</p>
                                                <p class="mt-1 text-xs text-muted-foreground">{format!("{} · {} · {} {}", item.user_id, item.policy_locale, item_revision_label, item.policy_revision)}</p>
                                            </div>
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                        }.into_any()
                    }
                }}
            </div>

            <form class="mt-6 grid gap-4 rounded-2xl border border-border p-5 md:grid-cols-2" on:submit=on_submit>
                <label class="text-sm text-muted-foreground">
                    <span class="mb-2 block">{decision_label}</span>
                    <select class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm" on:change=move |event| {
                        set_decision.set(if event_target_value(&event) == "reject" { GroupsAdminApplicationReviewDecision::Reject } else { GroupsAdminApplicationReviewDecision::Approve });
                        set_confirmed.set(false);
                    }>
                        <option value="approve">{approve}</option>
                        <option value="reject">{reject}</option>
                    </select>
                </label>
                <label class="text-sm text-muted-foreground">
                    <span class="mb-2 block">{note}</span>
                    <textarea class="min-h-24 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm" prop:value=move || review_note.get() on:input=move |event| { set_review_note.set(event_target_value(&event)); set_confirmed.set(false); }></textarea>
                </label>
                <label class="flex items-start gap-3 rounded-xl bg-muted px-4 py-3 text-sm md:col-span-2">
                    <input type="checkbox" prop:checked=move || confirmed.get() on:change=move |event| set_confirmed.set(event_target_checked(&event)) />
                    <span>{move || format!("{} ({})", confirm, selected_ids.get().len())}</span>
                </label>
                <button class="rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground md:col-span-2 disabled:opacity-50" type="submit" disabled=move || busy.get() || selected_ids.get().is_empty() || !confirmed.get()>{submit}</button>
            </form>

            <Show when=move || !results.get().is_empty()>
                {
                    let succeeded = succeeded.clone();
                    let replayed = replayed.clone();
                    let failed = failed.clone();
                    move || {
                        let succeeded = succeeded.clone();
                        let replayed = replayed.clone();
                        let failed = failed.clone();
                        view! {
                            <ul class="mt-6 grid gap-2" aria-live="polite">
                                {results.get().into_iter().map(|item| {
                                    let GroupsAdminBulkReviewApplicationItemResult {
                                        application_id,
                                        result,
                                        error,
                                    } = item;
                                    let succeeded = succeeded.clone();
                                    let replayed = replayed.clone();
                                    let failed = failed.clone();
                                    let message = if let Some(result) = result {
                                        format!("{} · {}{}", application_id, succeeded, if result.replayed { format!(" · {replayed}") } else { String::new() })
                                    } else if let Some(error) = error {
                                        format!("{} · {} · {}: {}", application_id, failed, error.code, error.message)
                                    } else {
                                        format!("{} · {}", application_id, failed)
                                    };
                                    view! { <li class="rounded-xl border border-border bg-muted px-4 py-2 text-xs font-mono">{message}</li> }
                                }).collect_view()}
                            </ul>
                        }
                    }
                }
            </Show>
        </section>
    }
}

fn bulk_input_error_message(
    error: GroupsAdminBulkReviewInputError,
    copy: &BulkReviewCopy,
) -> String {
    match error {
        GroupsAdminBulkReviewInputError::EmptySelection => copy.empty_selection.clone(),
        GroupsAdminBulkReviewInputError::TooManyApplications => copy.too_many.clone(),
        GroupsAdminBulkReviewInputError::DuplicateApplication => copy.duplicate.clone(),
        GroupsAdminBulkReviewInputError::InvalidGroupId => copy.invalid_group_id.clone(),
        GroupsAdminBulkReviewInputError::InvalidApplicationId => {
            copy.invalid_application_id.clone()
        }
        GroupsAdminBulkReviewInputError::ConfirmationRequired => copy.confirmation_required.clone(),
        GroupsAdminBulkReviewInputError::ReviewNoteTooLong => copy.review_note_too_long.clone(),
    }
}

fn bulk_review_copy(locale: Option<&str>) -> BulkReviewCopy {
    BulkReviewCopy {
        title: t(
            locale,
            "groups.admin.applications.bulk.title",
            "Bulk membership application review",
        ),
        body: t(
            locale,
            "groups.admin.applications.bulk.body",
            "Load pending applications, select up to 50, confirm one decision, and inspect every item result.",
        ),
        group_id: t(locale, "groups.admin.applications.groupId", "Group UUID"),
        load: t(
            locale,
            "groups.admin.applications.bulk.load",
            "Load pending applications",
        ),
        empty: t(
            locale,
            "groups.admin.applications.bulk.empty",
            "No pending applications loaded.",
        ),
        select_all: t(
            locale,
            "groups.admin.applications.bulk.selectAll",
            "Select all loaded",
        ),
        select_application: t(
            locale,
            "groups.admin.applications.bulk.selectApplication",
            "Select application",
        ),
        clear: t(
            locale,
            "groups.admin.applications.bulk.clear",
            "Clear selection",
        ),
        selected: t(
            locale,
            "groups.admin.applications.bulk.selected",
            "Selected",
        ),
        decision: t(locale, "groups.admin.applications.decision", "Decision"),
        approve: t(locale, "groups.admin.applications.approve", "Approve"),
        reject: t(locale, "groups.admin.applications.reject", "Reject"),
        note: t(
            locale,
            "groups.admin.applications.note",
            "Review note (optional)",
        ),
        revision: t(locale, "groups.admin.policyEditor.revision", "revision"),
        confirm: t(
            locale,
            "groups.admin.applications.bulk.confirm",
            "I confirm this bulk decision for the selected applications",
        ),
        submit: t(
            locale,
            "groups.admin.applications.bulk.submit",
            "Apply bulk review",
        ),
        busy: t(
            locale,
            "groups.admin.applications.bulk.busy",
            "Applying bulk review...",
        ),
        loaded: t(
            locale,
            "groups.admin.applications.loaded",
            "Applications loaded",
        ),
        completed: t(
            locale,
            "groups.admin.applications.bulk.completed",
            "Bulk review completed",
        ),
        error: t(
            locale,
            "groups.admin.applications.error",
            "Membership application command failed",
        ),
        invalid_group_id: t(
            locale,
            "groups.admin.applications.invalidGroupId",
            "Enter a valid group UUID.",
        ),
        empty_selection: t(
            locale,
            "groups.admin.applications.bulk.emptySelection",
            "Select at least one application.",
        ),
        too_many: t(
            locale,
            "groups.admin.applications.bulk.tooMany",
            "Select no more than 50 applications.",
        ),
        duplicate: t(
            locale,
            "groups.admin.applications.bulk.duplicate",
            "The selection contains duplicate applications.",
        ),
        invalid_application_id: t(
            locale,
            "groups.admin.applications.invalidApplicationId",
            "Enter a valid application UUID.",
        ),
        confirmation_required: t(
            locale,
            "groups.admin.applications.bulk.confirmationRequired",
            "Confirm the bulk decision before submitting.",
        ),
        review_note_too_long: t(
            locale,
            "groups.admin.applications.reviewNoteTooLong",
            "The review note must not exceed 2000 characters.",
        ),
        succeeded: t(
            locale,
            "groups.admin.applications.bulk.succeeded",
            "Succeeded",
        ),
        failed: t(locale, "groups.admin.applications.bulk.failed", "Failed"),
        replayed: t(
            locale,
            "groups.admin.applications.bulk.replayed",
            "replayed",
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
