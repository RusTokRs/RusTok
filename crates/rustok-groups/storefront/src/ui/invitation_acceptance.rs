use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;
use rustok_ui_routing::use_route_query_writer;

use crate::core::{
    groups_storefront_error, prepare_accept_group_invitation,
    GroupsStorefrontInvitationInputError, GROUP_INVITATION_TOKEN_QUERY_KEY,
};
use crate::i18n::t;
use crate::model::GroupsStorefrontAcceptInvitationResult;
use crate::transport::{
    accept_groups_storefront_invitation, GroupsStorefrontTransportContext,
};

#[derive(Clone)]
struct InvitationAcceptanceCopy {
    title: String,
    body: String,
    token_label: String,
    token_hint: String,
    accept: String,
    busy: String,
    error: String,
    success: String,
    group: String,
    role: String,
    status: String,
    missing_token: String,
    invalid_token: String,
}

#[component]
pub fn GroupsInvitationAcceptance(
    transport: GroupsStorefrontTransportContext,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let copy = invitation_acceptance_copy(route_context.locale.as_deref());
    let initial_token = route_context
        .query_value(GROUP_INVITATION_TOKEN_QUERY_KEY)
        .unwrap_or_default()
        .to_string();
    let query_writer = use_route_query_writer();

    let (token, set_token) = signal(initial_token);
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (result, set_result) = signal(Option::<GroupsStorefrontAcceptInvitationResult>::None);

    let submit_transport = transport.clone();
    let submit_copy = copy.clone();
    let on_submit = move |event: SubmitEvent| {
        event.prevent_default();
        let command = match prepare_accept_group_invitation(&token.get_untracked()) {
            Ok(command) => command,
            Err(input_error) => {
                set_error.set(Some(invitation_input_error_message(
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
        query_writer.clear_key(GROUP_INVITATION_TOKEN_QUERY_KEY);
        spawn_local(async move {
            match accept_groups_storefront_invitation(context, command).await {
                Ok(accepted) => {
                    set_token.set(String::new());
                    set_result.set(Some(accepted));
                }
                Err(accept_error) => set_error.set(Some(groups_storefront_error(
                    &copy.error,
                    &accept_error.to_string(),
                ))),
            }
            set_busy.set(false);
        });
    };

    let InvitationAcceptanceCopy {
        title,
        body,
        token_label,
        token_hint,
        accept,
        busy: busy_label,
        success,
        group: group_label,
        role: role_label,
        status: status_label,
        ..
    } = copy;

    view! {
        <section class="groups-storefront__invitation rounded-3xl border border-border bg-card p-6 shadow-sm">
            <h2 class="text-xl font-semibold text-card-foreground">{title}</h2>
            <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{body}</p>

            <form class="mt-5 grid gap-3 md:grid-cols-[1fr_auto]" on:submit=on_submit>
                <div>
                    <label class="mb-2 block text-sm font-medium text-card-foreground" for="groups-invitation-token">{token_label}</label>
                    <input
                        id="groups-invitation-token"
                        class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm"
                        type="password"
                        autocomplete="off"
                        spellcheck="false"
                        prop:value=move || token.get()
                        on:input=move |event| set_token.set(event_target_value(&event))
                    />
                    <p class="mt-2 text-xs text-muted-foreground">{token_hint}</p>
                </div>
                <button
                    class="self-end rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:cursor-not-allowed disabled:opacity-60"
                    type="submit"
                    disabled=move || busy.get()
                >
                    {accept}
                </button>
            </form>

            <Show when=move || busy.get()>
                <p class="mt-4 text-sm text-muted-foreground" aria-live="polite">{busy_label.clone()}</p>
            </Show>
            <Show when=move || error.get().is_some()>
                <p class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">
                    {move || error.get().unwrap_or_default()}
                </p>
            </Show>
            <Show when=move || result.get().is_some()>
                {move || result.get().map(|accepted| {
                    let membership = accepted.membership;
                    view! {
                        <div class="mt-4 rounded-xl border border-primary/30 bg-primary/5 px-4 py-3" role="status">
                            <p class="font-medium text-foreground">{success.clone()}</p>
                            <dl class="mt-3 grid gap-2 text-sm md:grid-cols-3">
                                <div>
                                    <dt class="text-xs text-muted-foreground">{group_label.clone()}</dt>
                                    <dd class="break-all text-foreground">{accepted.group_id}</dd>
                                </div>
                                <div>
                                    <dt class="text-xs text-muted-foreground">{role_label.clone()}</dt>
                                    <dd class="text-foreground">{membership.role}</dd>
                                </div>
                                <div>
                                    <dt class="text-xs text-muted-foreground">{status_label.clone()}</dt>
                                    <dd class="text-foreground">{membership.status}</dd>
                                </div>
                            </dl>
                        </div>
                    }
                })}
            </Show>
        </section>
    }
}

fn invitation_input_error_message(
    error: GroupsStorefrontInvitationInputError,
    copy: &InvitationAcceptanceCopy,
) -> String {
    match error {
        GroupsStorefrontInvitationInputError::MissingToken => copy.missing_token.clone(),
        GroupsStorefrontInvitationInputError::InvalidTokenLength => copy.invalid_token.clone(),
    }
}

fn invitation_acceptance_copy(locale: Option<&str>) -> InvitationAcceptanceCopy {
    InvitationAcceptanceCopy {
        title: t(locale, "groups.storefront.invitation.title", "Accept a group invitation"),
        body: t(locale, "groups.storefront.invitation.body", "Sign in, review the invitation token, and join the group. The token is removed from the URL when the command is submitted."),
        token_label: t(locale, "groups.storefront.invitation.tokenLabel", "Invitation token"),
        token_hint: t(locale, "groups.storefront.invitation.tokenHint", "Paste the opaque token from the invitation. It is never displayed after acceptance."),
        accept: t(locale, "groups.storefront.invitation.accept", "Accept invitation"),
        busy: t(locale, "groups.storefront.invitation.busy", "Accepting invitation..."),
        error: t(locale, "groups.storefront.invitation.error", "Invitation could not be accepted"),
        success: t(locale, "groups.storefront.invitation.success", "Invitation accepted. You are now a group member."),
        group: t(locale, "groups.storefront.invitation.group", "Group"),
        role: t(locale, "groups.storefront.invitation.role", "Role"),
        status: t(locale, "groups.storefront.invitation.status", "Membership status"),
        missing_token: t(locale, "groups.storefront.invitation.missingToken", "Enter an invitation token."),
        invalid_token: t(locale, "groups.storefront.invitation.invalidToken", "The invitation token has an invalid length."),
    }
}
