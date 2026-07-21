use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    groups_admin_error, prepare_create_group_invitation, prepare_group_invitation_query,
    prepare_revoke_group_invitation, selected_transport_profile,
    GroupsAdminInvitationInputError, GroupsAdminTransportProfile,
};
use crate::i18n::t;
use crate::model::GroupsAdminInvitation;
use crate::transport::{
    create_group_admin_invitation, load_group_admin_invitations,
    revoke_group_admin_invitation, GroupsAdminTransportContext,
};

#[derive(Clone)]
struct InvitationCopy {
    title: String,
    body: String,
    group_id: String,
    target_user_id: String,
    target_hint: String,
    expiry_seconds: String,
    max_uses: String,
    invitation_id: String,
    include_inactive: String,
    load: String,
    create: String,
    revoke: String,
    empty: String,
    busy: String,
    error: String,
    loaded: String,
    created: String,
    revoked: String,
    token_once: String,
    version: String,
    invalid_group_id: String,
    invalid_invitation_id: String,
    invalid_target_user_id: String,
    invalid_expiry: String,
    invalid_max_uses: String,
    targeted_single_use: String,
}

#[component]
pub fn GroupsInvitationsAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let transport = transport_context(profile);
    let copy = invitation_copy(locale.as_deref());

    let (group_id, set_group_id) = signal(String::new());
    let (target_user_id, set_target_user_id) = signal(String::new());
    let (expiry_seconds, set_expiry_seconds) = signal("86400".to_string());
    let (max_uses, set_max_uses) = signal("1".to_string());
    let (revoke_invitation_id, set_revoke_invitation_id) = signal(String::new());
    let (include_inactive, set_include_inactive) = signal(false);
    let (invitations, set_invitations) = signal(Vec::<GroupsAdminInvitation>::new());
    let (one_time_token, set_one_time_token) = signal(Option::<String>::None);
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (success, set_success) = signal(Option::<String>::None);

    let load_transport = transport.clone();
    let load_copy = copy.clone();
    let on_load_submit = move |event: SubmitEvent| {
        event.prevent_default();
        let query = match prepare_group_invitation_query(
            &group_id.get_untracked(),
            include_inactive.get_untracked(),
        ) {
            Ok(query) => query,
            Err(input_error) => {
                set_error.set(Some(invitation_input_error_message(input_error, &load_copy)));
                set_success.set(None);
                return;
            }
        };
        let context = load_transport.clone();
        let copy = load_copy.clone();
        set_busy.set(true);
        set_error.set(None);
        set_success.set(None);
        set_one_time_token.set(None);
        spawn_local(async move {
            match load_group_admin_invitations(context, query).await {
                Ok(connection) => {
                    let count = connection.items.len();
                    set_invitations.set(connection.items);
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

    let create_transport = transport.clone();
    let create_copy = copy.clone();
    let on_create_submit = move |event: SubmitEvent| {
        event.prevent_default();
        let expires = match expiry_seconds.get_untracked().trim().parse::<u64>() {
            Ok(value) => value,
            Err(_) => {
                set_error.set(Some(create_copy.invalid_expiry.clone()));
                set_success.set(None);
                return;
            }
        };
        let uses = match max_uses.get_untracked().trim().parse::<u32>() {
            Ok(value) => value,
            Err(_) => {
                set_error.set(Some(create_copy.invalid_max_uses.clone()));
                set_success.set(None);
                return;
            }
        };
        let target = target_user_id.get_untracked();
        let command = match prepare_create_group_invitation(
            &group_id.get_untracked(),
            Some(target.as_str()),
            expires,
            uses,
        ) {
            Ok(command) => command,
            Err(input_error) => {
                set_error.set(Some(invitation_input_error_message(input_error, &create_copy)));
                set_success.set(None);
                return;
            }
        };
        let context = create_transport.clone();
        let copy = create_copy.clone();
        set_busy.set(true);
        set_error.set(None);
        set_success.set(None);
        set_one_time_token.set(None);
        spawn_local(async move {
            match create_group_admin_invitation(context, command).await {
                Ok(result) => {
                    let invitation = result.invitation.clone();
                    set_invitations.update(|items| {
                        items.retain(|item| item.id != invitation.id);
                        items.insert(0, invitation);
                    });
                    set_one_time_token.set(result.token);
                    set_success.set(Some(format!(
                        "{} · {} {}",
                        copy.created, copy.version, result.group_version
                    )));
                }
                Err(create_error) => set_error.set(Some(groups_admin_error(
                    &copy.error,
                    &create_error.to_string(),
                ))),
            }
            set_busy.set(false);
        });
    };

    let revoke_transport = transport.clone();
    let revoke_copy = copy.clone();
    let on_revoke_submit = move |event: SubmitEvent| {
        event.prevent_default();
        let command = match prepare_revoke_group_invitation(
            &revoke_invitation_id.get_untracked(),
        ) {
            Ok(command) => command,
            Err(input_error) => {
                set_error.set(Some(invitation_input_error_message(input_error, &revoke_copy)));
                set_success.set(None);
                return;
            }
        };
        let context = revoke_transport.clone();
        let copy = revoke_copy.clone();
        set_busy.set(true);
        set_error.set(None);
        set_success.set(None);
        set_one_time_token.set(None);
        spawn_local(async move {
            match revoke_group_admin_invitation(context, command).await {
                Ok(result) => {
                    let revoked = result.invitation.clone();
                    set_invitations.update(|items| {
                        if let Some(item) = items.iter_mut().find(|item| item.id == revoked.id) {
                            *item = revoked;
                        }
                    });
                    set_success.set(Some(format!(
                        "{} · {} {}",
                        copy.revoked, copy.version, result.group_version
                    )));
                }
                Err(revoke_error) => set_error.set(Some(groups_admin_error(
                    &copy.error,
                    &revoke_error.to_string(),
                ))),
            }
            set_busy.set(false);
        });
    };

    let InvitationCopy {
        title: workspace_title,
        body: workspace_body,
        group_id: group_id_label,
        target_user_id: target_user_id_label,
        target_hint,
        expiry_seconds: expiry_label,
        max_uses: max_uses_label,
        invitation_id: invitation_id_label,
        include_inactive: include_inactive_label,
        load: load_label,
        create: create_label,
        revoke: revoke_label,
        empty: empty_label,
        busy: busy_label,
        token_once: token_once_label,
        ..
    } = copy;
    let create_heading = create_label.clone();
    let revoke_heading = revoke_label.clone();

    view! {
        <section class="groups-admin-invitations rounded-3xl border border-border bg-card p-6 shadow-sm">
            <h2 class="text-xl font-semibold text-card-foreground">{workspace_title}</h2>
            <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{workspace_body}</p>

            <form class="mt-6 grid gap-3 md:grid-cols-[1fr_auto_auto]" on:submit=on_load_submit>
                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=group_id_label.clone() prop:value=move || group_id.get() on:input=move |event| set_group_id.set(event_target_value(&event)) />
                <label class="flex items-center gap-2 text-sm text-muted-foreground">
                    <input type="checkbox" prop:checked=move || include_inactive.get() on:change=move |event| set_include_inactive.set(event_target_checked(&event)) />
                    <span>{include_inactive_label}</span>
                </label>
                <button class="rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground" type="submit">{load_label}</button>
            </form>

            <Show when=move || error.get().is_some()>
                <p class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || error.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || success.get().is_some()>
                <p class="mt-4 rounded-xl border border-border bg-muted px-4 py-3 text-sm text-foreground">{move || success.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || one_time_token.get().is_some()>
                <div class="mt-4 rounded-xl border border-primary/30 bg-primary/5 px-4 py-3">
                    <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">{token_once_label.clone()}</p>
                    <code class="mt-2 block break-all text-sm text-foreground">{move || one_time_token.get().unwrap_or_default()}</code>
                </div>
            </Show>
            <Show when=move || busy.get()>
                <p class="mt-4 text-sm text-muted-foreground">{busy_label}</p>
            </Show>

            <div class="mt-6 grid gap-6 xl:grid-cols-2">
                <form class="space-y-3 rounded-2xl border border-border p-5" on:submit=on_create_submit>
                    <h3 class="font-semibold text-card-foreground">{create_heading}</h3>
                    <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=target_user_id_label prop:value=move || target_user_id.get() on:input=move |event| set_target_user_id.set(event_target_value(&event)) />
                    <p class="text-xs text-muted-foreground">{target_hint}</p>
                    <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=expiry_label prop:value=move || expiry_seconds.get() on:input=move |event| set_expiry_seconds.set(event_target_value(&event)) />
                    <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=max_uses_label prop:value=move || max_uses.get() on:input=move |event| set_max_uses.set(event_target_value(&event)) />
                    <button class="rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground" type="submit">{create_label}</button>
                </form>

                <form class="space-y-3 rounded-2xl border border-border p-5" on:submit=on_revoke_submit>
                    <h3 class="font-semibold text-card-foreground">{revoke_heading}</h3>
                    <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm" placeholder=invitation_id_label prop:value=move || revoke_invitation_id.get() on:input=move |event| set_revoke_invitation_id.set(event_target_value(&event)) />
                    <button class="rounded-xl border border-destructive px-4 py-2 text-sm font-medium text-destructive" type="submit">{revoke_label}</button>
                </form>
            </div>

            <div class="mt-6">
                {move || {
                    let items = invitations.get();
                    if items.is_empty() {
                        view! { <p class="text-sm text-muted-foreground">{empty_label.clone()}</p> }.into_any()
                    } else {
                        view! {
                            <ul class="grid gap-3 md:grid-cols-2">
                                {items.into_iter().map(|item| view! {
                                    <li class="rounded-2xl border border-border p-4">
                                        <div class="flex items-center justify-between gap-3">
                                            <strong class="text-sm text-card-foreground">{item.status}</strong>
                                            <code class="text-xs text-muted-foreground">{item.id}</code>
                                        </div>
                                        <p class="mt-2 text-sm text-foreground">{format!("{} / {}", item.use_count, item.max_uses)}</p>
                                        <small class="mt-2 block text-xs text-muted-foreground">{item.expires_at}</small>
                                        <small class="mt-1 block text-xs text-muted-foreground">{item.target_user_id.unwrap_or_else(|| "shareable".to_string())}</small>
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

fn invitation_input_error_message(
    error: GroupsAdminInvitationInputError,
    copy: &InvitationCopy,
) -> String {
    match error {
        GroupsAdminInvitationInputError::InvalidGroupId => copy.invalid_group_id.clone(),
        GroupsAdminInvitationInputError::InvalidInvitationId => copy.invalid_invitation_id.clone(),
        GroupsAdminInvitationInputError::InvalidTargetUserId => copy.invalid_target_user_id.clone(),
        GroupsAdminInvitationInputError::InvalidExpiry => copy.invalid_expiry.clone(),
        GroupsAdminInvitationInputError::InvalidMaxUses => copy.invalid_max_uses.clone(),
        GroupsAdminInvitationInputError::TargetedInviteMustBeSingleUse => {
            copy.targeted_single_use.clone()
        }
    }
}

fn invitation_copy(locale: Option<&str>) -> InvitationCopy {
    InvitationCopy {
        title: t(locale, "groups.admin.invitations.title", "Group invitations"),
        body: t(locale, "groups.admin.invitations.body", "Create targeted invitations or bounded shareable links. Plaintext tokens are displayed once and are never stored by Groups."),
        group_id: t(locale, "groups.admin.invitations.groupId", "Group UUID"),
        target_user_id: t(locale, "groups.admin.invitations.targetUserId", "Target user UUID (optional)"),
        target_hint: t(locale, "groups.admin.invitations.targetHint", "Leave the target empty for a shareable link. Targeted invitations must be single-use."),
        expiry_seconds: t(locale, "groups.admin.invitations.expirySeconds", "Expiry in seconds"),
        max_uses: t(locale, "groups.admin.invitations.maxUses", "Maximum uses"),
        invitation_id: t(locale, "groups.admin.invitations.invitationId", "Invitation UUID"),
        include_inactive: t(locale, "groups.admin.invitations.includeInactive", "Include expired/revoked"),
        load: t(locale, "groups.admin.invitations.load", "Load invitations"),
        create: t(locale, "groups.admin.invitations.create", "Create invitation"),
        revoke: t(locale, "groups.admin.invitations.revoke", "Revoke invitation"),
        empty: t(locale, "groups.admin.invitations.empty", "No invitations loaded."),
        busy: t(locale, "groups.admin.invitations.busy", "Applying invitation command..."),
        error: t(locale, "groups.admin.invitations.error", "Invitation command failed"),
        loaded: t(locale, "groups.admin.invitations.loaded", "Invitations loaded"),
        created: t(locale, "groups.admin.invitations.created", "Invitation created"),
        revoked: t(locale, "groups.admin.invitations.revoked", "Invitation revoked"),
        token_once: t(locale, "groups.admin.invitations.tokenOnce", "Copy this token now. It will not be shown again."),
        version: t(locale, "groups.admin.invitations.version", "group version"),
        invalid_group_id: t(locale, "groups.admin.invitations.invalidGroupId", "Enter a valid group UUID."),
        invalid_invitation_id: t(locale, "groups.admin.invitations.invalidInvitationId", "Enter a valid invitation UUID."),
        invalid_target_user_id: t(locale, "groups.admin.invitations.invalidTargetUserId", "Enter a valid target user UUID or leave it empty."),
        invalid_expiry: t(locale, "groups.admin.invitations.invalidExpiry", "Expiry must be between 300 and 2592000 seconds."),
        invalid_max_uses: t(locale, "groups.admin.invitations.invalidMaxUses", "Maximum uses must be between 1 and 100."),
        targeted_single_use: t(locale, "groups.admin.invitations.targetedSingleUse", "A targeted invitation must have exactly one use."),
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
