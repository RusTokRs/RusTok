use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    groups_admin_error, prepare_delete_group_translation, prepare_group_translation_query,
    prepare_upsert_group_translation, selected_transport_profile,
    GroupsAdminLocalizationInputError, GroupsAdminTransportProfile,
};
use crate::i18n::t;
use crate::model::GroupsAdminTranslation;
use crate::transport::{
    delete_group_admin_translation, load_group_admin_translations,
    upsert_group_admin_translation, GroupsAdminTransportContext,
};

#[component]
pub fn GroupsLocalizationAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let transport = transport_context(profile);

    let (group_id, set_group_id) = signal(String::new());
    let (translation_locale, set_translation_locale) = signal(String::new());
    let (title, set_title) = signal(String::new());
    let (summary, set_summary) = signal(String::new());
    let (body, set_body) = signal(String::new());
    let (delete_locale, set_delete_locale) = signal(String::new());
    let (translations, set_translations) = signal(Vec::<GroupsAdminTranslation>::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (success, set_success) = signal(Option::<String>::None);

    let workspace_title = t(
        locale.as_deref(),
        "groups.admin.localization.title",
        "Localized presentation",
    );
    let workspace_body = t(
        locale.as_deref(),
        "groups.admin.localization.body",
        "Manage exact locale rows. Locale fallback remains a host/runtime responsibility.",
    );
    let group_id_label = t(
        locale.as_deref(),
        "groups.admin.localization.groupId",
        "Group UUID",
    );
    let locale_label = t(
        locale.as_deref(),
        "groups.admin.localization.locale",
        "Locale",
    );
    let title_label = t(
        locale.as_deref(),
        "groups.admin.localization.translationTitle",
        "Title",
    );
    let summary_label = t(
        locale.as_deref(),
        "groups.admin.localization.summary",
        "Summary",
    );
    let body_label = t(
        locale.as_deref(),
        "groups.admin.localization.translationBody",
        "Body",
    );
    let load_label = t(
        locale.as_deref(),
        "groups.admin.localization.load",
        "Load translations",
    );
    let save_label = t(
        locale.as_deref(),
        "groups.admin.localization.save",
        "Save translation",
    );
    let delete_label = t(
        locale.as_deref(),
        "groups.admin.localization.delete",
        "Delete translation",
    );
    let empty_label = t(
        locale.as_deref(),
        "groups.admin.localization.empty",
        "No translations loaded.",
    );
    let busy_label = t(
        locale.as_deref(),
        "groups.admin.localization.busy",
        "Applying localization command...",
    );
    let error_label = t(
        locale.as_deref(),
        "groups.admin.localization.error",
        "Localization command failed",
    );
    let loaded_label = t(
        locale.as_deref(),
        "groups.admin.localization.loaded",
        "Translations loaded",
    );
    let saved_label = t(
        locale.as_deref(),
        "groups.admin.localization.saved",
        "Translation saved",
    );
    let deleted_label = t(
        locale.as_deref(),
        "groups.admin.localization.deleted",
        "Translation deleted",
    );
    let version_label = t(
        locale.as_deref(),
        "groups.admin.localization.version",
        "group version",
    );
    let invalid_group_label = t(
        locale.as_deref(),
        "groups.admin.localization.invalidGroupId",
        "Enter a valid group UUID.",
    );
    let invalid_locale_label = t(
        locale.as_deref(),
        "groups.admin.localization.invalidLocale",
        "Enter a valid locale tag.",
    );
    let missing_title_label = t(
        locale.as_deref(),
        "groups.admin.localization.missingTitle",
        "Title is required.",
    );
    let title_too_long_label = t(
        locale.as_deref(),
        "groups.admin.localization.titleTooLong",
        "Title must not exceed 240 characters.",
    );
    let summary_too_long_label = t(
        locale.as_deref(),
        "groups.admin.localization.summaryTooLong",
        "Summary must not exceed 500 characters.",
    );

    let load_transport = transport.clone();
    let load_error_label = error_label.clone();
    let load_invalid_group = invalid_group_label.clone();
    let load_success_label = loaded_label.clone();
    let on_load_submit = move |event: SubmitEvent| {
        event.prevent_default();
        let query = match prepare_group_translation_query(&group_id.get_untracked()) {
            Ok(query) => query,
            Err(error) => {
                set_error.set(Some(localization_input_error_message(
                    error,
                    &load_invalid_group,
                    &invalid_locale_label,
                    &missing_title_label,
                    &title_too_long_label,
                    &summary_too_long_label,
                )));
                return;
            }
        };
        let context = load_transport.clone();
        let error_label = load_error_label.clone();
        let success_label = load_success_label.clone();
        set_busy.set(true);
        set_error.set(None);
        set_success.set(None);
        spawn_local(async move {
            match load_group_admin_translations(context, query).await {
                Ok(items) => {
                    let count = items.len();
                    set_translations.set(items);
                    set_success.set(Some(format!("{success_label}: {count}")));
                }
                Err(load_error) => set_error.set(Some(groups_admin_error(
                    &error_label,
                    &load_error.to_string(),
                ))),
            }
            set_busy.set(false);
        });
    };

    let save_transport = transport.clone();
    let save_error_label = error_label.clone();
    let save_invalid_group = invalid_group_label.clone();
    let save_invalid_locale = invalid_locale_label.clone();
    let save_missing_title = missing_title_label.clone();
    let save_title_too_long = title_too_long_label.clone();
    let save_summary_too_long = summary_too_long_label.clone();
    let save_success_label = saved_label.clone();
    let save_version_label = version_label.clone();
    let on_save_submit = move |event: SubmitEvent| {
        event.prevent_default();
        let command = match prepare_upsert_group_translation(
            &group_id.get_untracked(),
            &translation_locale.get_untracked(),
            &title.get_untracked(),
            Some(summary.get_untracked()),
            Some(body.get_untracked()),
        ) {
            Ok(command) => command,
            Err(input_error) => {
                set_error.set(Some(localization_input_error_message(
                    input_error,
                    &save_invalid_group,
                    &save_invalid_locale,
                    &save_missing_title,
                    &save_title_too_long,
                    &save_summary_too_long,
                )));
                return;
            }
        };
        let context = save_transport.clone();
        let error_label = save_error_label.clone();
        let success_label = save_success_label.clone();
        let version_label = save_version_label.clone();
        set_busy.set(true);
        set_error.set(None);
        set_success.set(None);
        spawn_local(async move {
            match upsert_group_admin_translation(context, command).await {
                Ok(result) => {
                    let updated = result.translation.clone();
                    set_translations.update(|items| {
                        items.retain(|item| item.locale != updated.locale);
                        items.push(updated);
                        items.sort_by(|left, right| left.locale.cmp(&right.locale));
                    });
                    set_success.set(Some(format!(
                        "{success_label}: {} · {version_label} {}",
                        result.translation.locale, result.group_version
                    )));
                }
                Err(save_error) => set_error.set(Some(groups_admin_error(
                    &error_label,
                    &save_error.to_string(),
                ))),
            }
            set_busy.set(false);
        });
    };

    let delete_transport = transport.clone();
    let delete_error_label = error_label.clone();
    let delete_invalid_group = invalid_group_label.clone();
    let delete_invalid_locale = invalid_locale_label.clone();
    let delete_success_label = deleted_label.clone();
    let delete_version_label = version_label.clone();
    let on_delete_submit = move |event: SubmitEvent| {
        event.prevent_default();
        let command = match prepare_delete_group_translation(
            &group_id.get_untracked(),
            &delete_locale.get_untracked(),
        ) {
            Ok(command) => command,
            Err(input_error) => {
                set_error.set(Some(localization_input_error_message(
                    input_error,
                    &delete_invalid_group,
                    &delete_invalid_locale,
                    &missing_title_label,
                    &title_too_long_label,
                    &summary_too_long_label,
                )));
                return;
            }
        };
        let context = delete_transport.clone();
        let error_label = delete_error_label.clone();
        let success_label = delete_success_label.clone();
        let version_label = delete_version_label.clone();
        set_busy.set(true);
        set_error.set(None);
        set_success.set(None);
        spawn_local(async move {
            match delete_group_admin_translation(context, command).await {
                Ok(result) => {
                    set_translations.update(|items| {
                        items.retain(|item| item.locale != result.locale)
                    });
                    set_success.set(Some(format!(
                        "{success_label}: {} · {version_label} {}",
                        result.locale, result.group_version
                    )));
                }
                Err(delete_error) => set_error.set(Some(groups_admin_error(
                    &error_label,
                    &delete_error.to_string(),
                ))),
            }
            set_busy.set(false);
        });
    };

    view! {
        <section class="groups-admin-localization rounded-3xl border border-border bg-card p-6 shadow-sm">
            <h2 class="text-xl font-semibold text-card-foreground">{workspace_title}</h2>
            <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{workspace_body}</p>

            <form class="mt-6 flex flex-col gap-3 md:flex-row" on:submit=on_load_submit>
                <input
                    class="flex-1 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground"
                    placeholder=group_id_label.clone()
                    prop:value=move || group_id.get()
                    on:input=move |event| set_group_id.set(event_target_value(&event))
                />
                <button class="rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground" type="submit">
                    {load_label}
                </button>
            </form>

            <Show when=move || error.get().is_some()>
                <p class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                    {move || error.get().unwrap_or_default()}
                </p>
            </Show>
            <Show when=move || success.get().is_some()>
                <p class="mt-4 rounded-xl border border-border bg-muted px-4 py-3 text-sm text-foreground">
                    {move || success.get().unwrap_or_default()}
                </p>
            </Show>
            <Show when=move || busy.get()>
                <p class="mt-4 text-sm text-muted-foreground">{busy_label.clone()}</p>
            </Show>

            <div class="mt-6 grid gap-6 xl:grid-cols-2">
                <form class="space-y-3 rounded-2xl border border-border p-5" on:submit=on_save_submit>
                    <h3 class="font-semibold text-card-foreground">{save_label.clone()}</h3>
                    <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=locale_label.clone() prop:value=move || translation_locale.get() on:input=move |event| set_translation_locale.set(event_target_value(&event)) />
                    <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=title_label prop:value=move || title.get() on:input=move |event| set_title.set(event_target_value(&event)) />
                    <textarea class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=summary_label prop:value=move || summary.get() on:input=move |event| set_summary.set(event_target_value(&event)) />
                    <textarea class="min-h-32 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=body_label prop:value=move || body.get() on:input=move |event| set_body.set(event_target_value(&event)) />
                    <button class="rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground" type="submit">{save_label}</button>
                </form>

                <form class="space-y-3 rounded-2xl border border-border p-5" on:submit=on_delete_submit>
                    <h3 class="font-semibold text-card-foreground">{delete_label.clone()}</h3>
                    <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=locale_label prop:value=move || delete_locale.get() on:input=move |event| set_delete_locale.set(event_target_value(&event)) />
                    <p class="text-sm text-muted-foreground">{"The owner service rejects deletion of the last translation row."}</p>
                    <button class="rounded-xl border border-destructive px-4 py-2 text-sm font-medium text-destructive" type="submit">{delete_label}</button>
                </form>
            </div>

            <div class="mt-6 space-y-3">
                {move || {
                    let items = translations.get();
                    if items.is_empty() {
                        view! { <p class="text-sm text-muted-foreground">{empty_label.clone()}</p> }.into_any()
                    } else {
                        view! {
                            <ul class="grid gap-3 md:grid-cols-2">
                                {items.into_iter().map(|item| view! {
                                    <li class="rounded-2xl border border-border p-4">
                                        <strong class="text-sm text-card-foreground">{item.locale}</strong>
                                        <p class="mt-2 text-sm text-foreground">{item.title}</p>
                                        <small class="mt-2 block text-xs text-muted-foreground">{item.summary.unwrap_or_default()}</small>
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

fn localization_input_error_message(
    error: GroupsAdminLocalizationInputError,
    invalid_group_id: &str,
    invalid_locale: &str,
    missing_title: &str,
    title_too_long: &str,
    summary_too_long: &str,
) -> String {
    match error {
        GroupsAdminLocalizationInputError::InvalidGroupId => invalid_group_id.to_string(),
        GroupsAdminLocalizationInputError::InvalidLocale => invalid_locale.to_string(),
        GroupsAdminLocalizationInputError::MissingTitle => missing_title.to_string(),
        GroupsAdminLocalizationInputError::TitleTooLong => title_too_long.to_string(),
        GroupsAdminLocalizationInputError::SummaryTooLong => summary_too_long.to_string(),
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
