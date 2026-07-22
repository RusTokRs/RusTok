use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    default_groups_admin_filters, groups_admin_error, groups_admin_header,
    prepare_change_group_role, prepare_transfer_group_ownership, selected_transport_profile,
    GroupsAdminGovernanceInputError, GroupsAdminTransportProfile,
};
use crate::i18n::t;
use crate::model::{
    GroupsAdminAssignableRole, GroupsAdminDirectory, GroupsAdminGovernanceResult,
};
use crate::transport::{
    change_group_admin_role, load_groups_admin_directory, transfer_group_admin_ownership,
    GroupsAdminTransportContext,
};

#[component]
pub fn GroupsAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let transport = transport_context(profile);
    let directory_transport = transport.clone();
    let filters = default_groups_admin_filters();
    let directory = LocalResource::new(move || {
        let context = directory_transport.clone();
        let request = filters.clone();
        async move { load_groups_admin_directory(context, request).await }
    });

    let (role_group_id, set_role_group_id) = signal(String::new());
    let (target_user_id, set_target_user_id) = signal(String::new());
    let (assignable_role, set_assignable_role) = signal(GroupsAdminAssignableRole::Member);
    let (ownership_group_id, set_ownership_group_id) = signal(String::new());
    let (new_owner_user_id, set_new_owner_user_id) = signal(String::new());
    let (governance_busy, set_governance_busy) = signal(false);
    let (governance_error, set_governance_error) = signal(Option::<String>::None);
    let (governance_success, set_governance_success) = signal(Option::<String>::None);

    let header = groups_admin_header(
        t(locale.as_deref(), "groups.admin.title", "Groups"),
        t(
            locale.as_deref(),
            "groups.admin.body",
            "Manage group privacy, memberships, local roles, and modular feature bindings.",
        ),
        t(
            locale.as_deref(),
            "groups.admin.badge",
            "community control room",
        ),
    );
    let loading = t(
        locale.as_deref(),
        "groups.admin.loading",
        "Loading groups...",
    );
    let load_error = t(
        locale.as_deref(),
        "groups.admin.loadError",
        "Failed to load groups",
    );
    let empty = t(
        locale.as_deref(),
        "groups.admin.empty",
        "No groups are available for this tenant.",
    );
    let total_label = t(locale.as_deref(), "groups.admin.total", "Total");

    let governance_title = t(
        locale.as_deref(),
        "groups.admin.governance.title",
        "Group governance",
    );
    let governance_body = t(
        locale.as_deref(),
        "groups.admin.governance.body",
        "Delegate local roles or transfer ownership through the Groups owner service. Every command is idempotent and audited.",
    );
    let group_id_label = t(
        locale.as_deref(),
        "groups.admin.governance.groupId",
        "Group UUID",
    );
    let target_user_id_label = t(
        locale.as_deref(),
        "groups.admin.governance.targetUserId",
        "Member UUID",
    );
    let new_owner_user_id_label = t(
        locale.as_deref(),
        "groups.admin.governance.newOwnerUserId",
        "New owner UUID",
    );
    let role_label = t(
        locale.as_deref(),
        "groups.admin.governance.role",
        "New local role",
    );
    let role_admin_label = t(
        locale.as_deref(),
        "groups.admin.governance.roleAdmin",
        "Administrator",
    );
    let role_moderator_label = t(
        locale.as_deref(),
        "groups.admin.governance.roleModerator",
        "Moderator",
    );
    let role_member_label = t(
        locale.as_deref(),
        "groups.admin.governance.roleMember",
        "Member",
    );
    let change_role_label = t(
        locale.as_deref(),
        "groups.admin.governance.changeRole",
        "Change role",
    );
    let transfer_ownership_label = t(
        locale.as_deref(),
        "groups.admin.governance.transferOwnership",
        "Transfer ownership",
    );
    let transfer_warning = t(
        locale.as_deref(),
        "groups.admin.governance.transferWarning",
        "Ownership transfer demotes the current owner to administrator and cannot be replaced by a role change.",
    );
    let governance_error_label = t(
        locale.as_deref(),
        "groups.admin.governance.error",
        "Governance command failed",
    );
    let invalid_group_id_label = t(
        locale.as_deref(),
        "groups.admin.governance.invalidGroupId",
        "Enter a valid group UUID.",
    );
    let invalid_target_user_id_label = t(
        locale.as_deref(),
        "groups.admin.governance.invalidTargetUserId",
        "Enter a valid member UUID.",
    );
    let invalid_new_owner_user_id_label = t(
        locale.as_deref(),
        "groups.admin.governance.invalidNewOwnerUserId",
        "Enter a valid new-owner UUID.",
    );
    let role_changed_label = t(
        locale.as_deref(),
        "groups.admin.governance.roleChanged",
        "Role changed",
    );
    let ownership_transferred_label = t(
        locale.as_deref(),
        "groups.admin.governance.ownershipTransferred",
        "Ownership transferred",
    );
    let version_label = t(
        locale.as_deref(),
        "groups.admin.governance.version",
        "group version",
    );
    let replayed_label = t(
        locale.as_deref(),
        "groups.admin.governance.replayed",
        "idempotent replay",
    );
    let busy_label = t(
        locale.as_deref(),
        "groups.admin.governance.busy",
        "Applying governance command...",
    );

    let role_transport = transport.clone();
    let role_error_label = governance_error_label.clone();
    let role_invalid_group = invalid_group_id_label.clone();
    let role_invalid_target = invalid_target_user_id_label.clone();
    let role_invalid_owner = invalid_new_owner_user_id_label.clone();
    let role_success_label = role_changed_label.clone();
    let role_version_label = version_label.clone();
    let role_replayed_label = replayed_label.clone();
    let on_role_submit = move |event: SubmitEvent| {
        event.prevent_default();
        let command = match prepare_change_group_role(
            &role_group_id.get_untracked(),
            &target_user_id.get_untracked(),
            assignable_role.get_untracked(),
        ) {
            Ok(command) => command,
            Err(error) => {
                set_governance_error.set(Some(governance_input_error_message(
                    error,
                    &role_invalid_group,
                    &role_invalid_target,
                    &role_invalid_owner,
                )));
                set_governance_success.set(None);
                return;
            }
        };
        let context = role_transport.clone();
        let error_label = role_error_label.clone();
        let success_label = role_success_label.clone();
        let version_label = role_version_label.clone();
        let replayed_label = role_replayed_label.clone();
        set_governance_busy.set(true);
        set_governance_error.set(None);
        set_governance_success.set(None);
        spawn_local(async move {
            match change_group_admin_role(context, command).await {
                Ok(result) => set_governance_success.set(Some(governance_success_message(
                    &success_label,
                    &version_label,
                    &replayed_label,
                    &result,
                ))),
                Err(error) => set_governance_error.set(Some(groups_admin_error(
                    &error_label,
                    &error.to_string(),
                ))),
            }
            set_governance_busy.set(false);
        });
    };

    let ownership_transport = transport.clone();
    let ownership_error_label = governance_error_label.clone();
    let ownership_invalid_group = invalid_group_id_label.clone();
    let ownership_invalid_target = invalid_target_user_id_label.clone();
    let ownership_invalid_owner = invalid_new_owner_user_id_label.clone();
    let ownership_success_label = ownership_transferred_label.clone();
    let ownership_version_label = version_label.clone();
    let ownership_replayed_label = replayed_label.clone();
    let on_ownership_submit = move |event: SubmitEvent| {
        event.prevent_default();
        let command = match prepare_transfer_group_ownership(
            &ownership_group_id.get_untracked(),
            &new_owner_user_id.get_untracked(),
        ) {
            Ok(command) => command,
            Err(error) => {
                set_governance_error.set(Some(governance_input_error_message(
                    error,
                    &ownership_invalid_group,
                    &ownership_invalid_target,
                    &ownership_invalid_owner,
                )));
                set_governance_success.set(None);
                return;
            }
        };
        let context = ownership_transport.clone();
        let error_label = ownership_error_label.clone();
        let success_label = ownership_success_label.clone();
        let version_label = ownership_version_label.clone();
        let replayed_label = ownership_replayed_label.clone();
        set_governance_busy.set(true);
        set_governance_error.set(None);
        set_governance_success.set(None);
        spawn_local(async move {
            match transfer_group_admin_ownership(context, command).await {
                Ok(result) => set_governance_success.set(Some(governance_success_message(
                    &success_label,
                    &version_label,
                    &replayed_label,
                    &result,
                ))),
                Err(error) => set_governance_error.set(Some(groups_admin_error(
                    &error_label,
                    &error.to_string(),
                ))),
            }
            set_governance_busy.set(false);
        });
    };

    view! {
        <section class="groups-admin space-y-6">
            <header class="groups-admin__header rounded-3xl border border-border bg-card p-6 shadow-sm">
                <span class="inline-flex rounded-full border border-border px-3 py-1 text-xs uppercase tracking-[0.18em] text-muted-foreground">{header.badge}</span>
                <h1 class="mt-3 text-2xl font-semibold text-card-foreground">{header.title}</h1>
                <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{header.body}</p>
                <small class="mt-3 block text-xs text-muted-foreground">{format!("transport: {}", profile.as_str())}</small>
            </header>

            <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                <Suspense fallback=move || view! { <p class="text-sm text-muted-foreground">{loading.clone()}</p> }>
                    {move || directory.get().map(|result| match result {
                        Ok(directory) => render_directory(directory, &total_label, &empty).into_any(),
                        Err(error) => view! {
                            <p class="groups-admin__error text-sm text-destructive">{groups_admin_error(&load_error, &error.to_string())}</p>
                        }.into_any(),
                    })}
                </Suspense>
            </section>

            <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                <div>
                    <h2 class="text-xl font-semibold text-card-foreground">{governance_title}</h2>
                    <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{governance_body}</p>
                </div>

                <Show when=move || governance_error.get().is_some()>
                    <p class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                        {move || governance_error.get().unwrap_or_default()}
                    </p>
                </Show>
                <Show when=move || governance_success.get().is_some()>
                    <p class="mt-4 rounded-xl border border-border bg-muted px-4 py-3 text-sm text-foreground">
                        {move || governance_success.get().unwrap_or_default()}
                    </p>
                </Show>
                <Show when=move || governance_busy.get()>
                    <p class="mt-4 text-sm text-muted-foreground">{busy_label.clone()}</p>
                </Show>

                <div class="mt-6 grid gap-6 xl:grid-cols-2">
                    <form class="space-y-4 rounded-2xl border border-border p-5" on:submit=on_role_submit>
                        <h3 class="text-base font-semibold text-card-foreground">{change_role_label.clone()}</h3>
                        <input
                            class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            placeholder=group_id_label.clone()
                            prop:value=move || role_group_id.get()
                            on:input=move |event| set_role_group_id.set(event_target_value(&event))
                        />
                        <input
                            class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            placeholder=target_user_id_label.clone()
                            prop:value=move || target_user_id.get()
                            on:input=move |event| set_target_user_id.set(event_target_value(&event))
                        />
                        <label class="block text-sm text-muted-foreground">
                            <span>{role_label}</span>
                            <select
                                class="mt-2 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                prop:value=move || assignable_role.get().as_str()
                                on:change=move |event| {
                                    let role = match event_target_value(&event).as_str() {
                                        "admin" => GroupsAdminAssignableRole::Admin,
                                        "moderator" => GroupsAdminAssignableRole::Moderator,
                                        _ => GroupsAdminAssignableRole::Member,
                                    };
                                    set_assignable_role.set(role);
                                }
                            >
                                <option value="admin">{role_admin_label}</option>
                                <option value="moderator">{role_moderator_label}</option>
                                <option value="member">{role_member_label}</option>
                            </select>
                        </label>
                        <button
                            type="submit"
                            class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:opacity-90 disabled:opacity-50"
                            disabled=move || governance_busy.get()
                        >
                            {change_role_label}
                        </button>
                    </form>

                    <form class="space-y-4 rounded-2xl border border-border p-5" on:submit=on_ownership_submit>
                        <h3 class="text-base font-semibold text-card-foreground">{transfer_ownership_label.clone()}</h3>
                        <p class="text-sm text-muted-foreground">{transfer_warning}</p>
                        <input
                            class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            placeholder=group_id_label
                            prop:value=move || ownership_group_id.get()
                            on:input=move |event| set_ownership_group_id.set(event_target_value(&event))
                        />
                        <input
                            class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            placeholder=new_owner_user_id_label
                            prop:value=move || new_owner_user_id.get()
                            on:input=move |event| set_new_owner_user_id.set(event_target_value(&event))
                        />
                        <button
                            type="submit"
                            class="inline-flex rounded-xl border border-destructive/40 px-4 py-2 text-sm font-medium text-destructive transition hover:bg-destructive/10 disabled:opacity-50"
                            disabled=move || governance_busy.get()
                        >
                            {transfer_ownership_label}
                        </button>
                    </form>
                </div>
            </section>
        </section>
    }
}

fn render_directory(
    directory: GroupsAdminDirectory,
    total_label: &str,
    empty: &str,
) -> impl IntoView {
    if directory.items.is_empty() {
        return view! { <p class="groups-admin__empty text-sm text-muted-foreground">{empty.to_string()}</p> }.into_any();
    }

    view! {
        <div class="groups-admin__directory space-y-4">
            <p class="text-sm text-muted-foreground">{format!("{total_label}: {}", directory.total)}</p>
            <ul class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                {directory.items.into_iter().map(|group| view! {
                    <li>
                        <article class="rounded-2xl border border-border p-4">
                            <h2 class="font-semibold text-card-foreground">{group.title}</h2>
                            <p class="mt-2 text-sm text-muted-foreground">{format!("@{} · {} · {}", group.handle, group.visibility, group.status)}</p>
                            <small class="mt-2 block text-xs text-muted-foreground">{format!("{} members · {}", group.member_count, group.effective_locale)}</small>
                        </article>
                    </li>
                }).collect_view()}
            </ul>
        </div>
    }
    .into_any()
}

fn governance_input_error_message(
    error: GroupsAdminGovernanceInputError,
    invalid_group_id: &str,
    invalid_target_user_id: &str,
    invalid_new_owner_user_id: &str,
) -> String {
    match error {
        GroupsAdminGovernanceInputError::InvalidGroupId => invalid_group_id.to_string(),
        GroupsAdminGovernanceInputError::InvalidTargetUserId => {
            invalid_target_user_id.to_string()
        }
        GroupsAdminGovernanceInputError::InvalidNewOwnerUserId => {
            invalid_new_owner_user_id.to_string()
        }
    }
}

fn governance_success_message(
    action: &str,
    version_label: &str,
    replayed_label: &str,
    result: &GroupsAdminGovernanceResult,
) -> String {
    let replayed = if result.replayed {
        format!(" · {replayed_label}")
    } else {
        String::new()
    };
    format!(
        "{action}: {} → {} · {version_label} {}{replayed}",
        result.previous_role, result.current_role, result.group_version
    )
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
