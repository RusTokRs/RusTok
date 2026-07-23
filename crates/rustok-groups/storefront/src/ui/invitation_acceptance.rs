use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::use_route_query_writer;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    GROUP_INVITATION_TOKEN_QUERY_KEY, GROUP_TARGETED_INVITATION_QUERY_KEY,
    GroupsStorefrontInvitationInputError, groups_storefront_error, prepare_accept_group_invitation,
    prepare_accept_targeted_group_invitation,
};
use crate::i18n::t;
use crate::model::{
    AcceptGroupInvitationCommand, AcceptTargetedGroupInvitationCommand,
    GroupsStorefrontAcceptInvitationResult,
};
use crate::transport::{
    GroupsStorefrontTransportContext, accept_groups_storefront_invitation,
    accept_groups_storefront_targeted_invitation,
};

#[derive(Clone)]
struct InvitationAcceptanceCopy {
    title: String,
    body: String,
    targeted_body: String,
    token_label: String,
    token_hint: String,
    targeted_hint: String,
    accept: String,
    busy: String,
    error: String,
    success: String,
    group: String,
    role: String,
    status: String,
    missing_token: String,
    invalid_token: String,
    invalid_invitation_id: String,
}

enum PendingInvitationAcceptance {
    Token(AcceptGroupInvitationCommand),
    Targeted(AcceptTargetedGroupInvitationCommand),
}

#[component]
pub fn GroupsInvitationAcceptance(transport: GroupsStorefrontTransportContext) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let copy = invitation_acceptance_copy(route_context.locale.as_deref());
    let initial_token = route_context
        .query_value(GROUP_INVITATION_TOKEN_QUERY_KEY)
        .unwrap_or_default()
        .to_string();
    let initial_targeted_invitation = route_context
        .query_value(GROUP_TARGETED_INVITATION_QUERY_KEY)
        .unwrap_or_default()
        .to_string();
    let query_writer = use_route_query_writer();

    let (token, set_token) = signal(initial_token);
    let (targeted_invitation_id, set_targeted_invitation_id) = signal(initial_targeted_invitation);
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (result, set_result) = signal(Option::<GroupsStorefrontAcceptInvitationResult>::None);

    let submit_transport = transport.clone();
    let submit_copy = copy.clone();
    let on_submit = move |event: SubmitEvent| {
        event.prevent_default();
        let targeted_value = targeted_invitation_id.get_untracked();
        let command = if targeted_value.trim().is_empty() {
            match prepare_accept_group_invitation(&token.get_untracked()) {
                Ok(command) => PendingInvitationAcceptance::Token(command),
                Err(input_error) => {
                    set_error.set(Some(invitation_input_error_message(
                        input_error,
                        &submit_copy,
                    )));
                    set_result.set(None);
                    return;
                }
            }
        } else {
            match prepare_accept_targeted_group_invitation(&targeted_value) {
                Ok(command) => PendingInvitationAcceptance::Targeted(command),
                Err(input_error) => {
                    set_error.set(Some(invitation_input_error_message(
                        input_error,
                        &submit_copy,
                    )));
                    set_result.set(None);
                    return;
                }
            }
        };

        let context = submit_transport.clone();
        let copy = submit_copy.clone();
        let query_writer = query_writer.clone();
        let targeted = matches!(&command, PendingInvitationAcceptance::Targeted(_));
        set_busy.set(true);
        set_error.set(None);
        set_result.set(None);
        if targeted {
            query_writer.clear_key(GROUP_TARGETED_INVITATION_QUERY_KEY);
        } else {
            query_writer.clear_key(GROUP_INVITATION_TOKEN_QUERY_KEY);
        }
        spawn_local(async move {
            let accepted = match command {
                PendingInvitationAcceptance::Token(command) => {
                    accept_groups_storefront_invitation(context, command).await
                }
                PendingInvitationAcceptance::Targeted(command) => {
                    accept_groups_storefront_targeted_invitation(context, command).await
                }
            };
            match accepted {
                Ok(accepted) => {
                    set_token.set(String::new());
                    set_targeted_invitation_id.set(String::new());
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
        targeted_body,
        token_label,
        token_hint,
        targeted_hint,
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
            <p class="mt-2 max-w-3xl text-sm text-muted-foreground">
                {move || if targeted_invitation_id.get().trim().is_empty() {
                    body.clone()
                } else {
                    targeted_body.clone()
                }}
            </p>

            <form class="mt-5 grid gap-3 md:grid-cols-[1fr_auto]" on:submit=on_submit>
                <Show
                    when=move || targeted_invitation_id.get().trim().is_empty()
                    fallback=move || view! {
                        <div class="rounded-xl border border-border bg-muted px-4 py-3 text-sm text-muted-foreground">
                            {targeted_hint.clone()}
                        </div>
                    }
                >
                    {
                        let token_label = token_label.clone();
                        let token_hint = token_hint.clone();
                        view! {
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
                        }
                    }
                </Show>
                <button
                    class="self-end rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:cursor-not-allowed disabled:opacity-60"
                    type="submit"
                    disabled=move || busy.get()
                >
                    {accept.clone()}
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
                {
                    let success = success.clone();
                    let group_label = group_label.clone();
                    let role_label = role_label.clone();
                    let status_label = status_label.clone();
                    move || result.get().map(|accepted| {
                        let membership = accepted.membership;
                        let success = success.clone();
                        let group_label = group_label.clone();
                        let role_label = role_label.clone();
                        let status_label = status_label.clone();
                        view! {
                            <div class="mt-4 rounded-xl border border-primary/30 bg-primary/5 px-4 py-3" role="status">
                                <p class="font-medium text-foreground">{success}</p>
                                <dl class="mt-3 grid gap-2 text-sm md:grid-cols-3">
                                    <div>
                                        <dt class="text-xs text-muted-foreground">{group_label}</dt>
                                        <dd class="break-all text-foreground">{accepted.group_id}</dd>
                                    </div>
                                    <div>
                                        <dt class="text-xs text-muted-foreground">{role_label}</dt>
                                        <dd class="text-foreground">{membership.role}</dd>
                                    </div>
                                    <div>
                                        <dt class="text-xs text-muted-foreground">{status_label}</dt>
                                        <dd class="text-foreground">{membership.status}</dd>
                                    </div>
                                </dl>
                            </div>
                        }
                    })
                }
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
        GroupsStorefrontInvitationInputError::InvalidInvitationId => {
            copy.invalid_invitation_id.clone()
        }
    }
}

fn invitation_acceptance_copy(locale: Option<&str>) -> InvitationAcceptanceCopy {
    InvitationAcceptanceCopy {
        title: t(
            locale,
            "groups.storefront.invitation.title",
            "Accept a group invitation",
        ),
        body: t(
            locale,
            "groups.storefront.invitation.body",
            "Sign in, review the invitation token, and join the group. The token is removed from the URL when the command is submitted.",
        ),
        targeted_body: t(
            locale,
            "groups.storefront.invitation.targetedBody",
            "This invitation was addressed to your account. Sign in and accept it without exposing an invitation token.",
        ),
        token_label: t(
            locale,
            "groups.storefront.invitation.tokenLabel",
            "Invitation token",
        ),
        token_hint: t(
            locale,
            "groups.storefront.invitation.tokenHint",
            "Paste the opaque token from the invitation. It is never displayed after acceptance.",
        ),
        targeted_hint: t(
            locale,
            "groups.storefront.invitation.targetedHint",
            "The invitation identifier came from an authorized notification route and can only be accepted by the addressed account.",
        ),
        accept: t(
            locale,
            "groups.storefront.invitation.accept",
            "Accept invitation",
        ),
        busy: t(
            locale,
            "groups.storefront.invitation.busy",
            "Accepting invitation...",
        ),
        error: t(
            locale,
            "groups.storefront.invitation.error",
            "Invitation could not be accepted",
        ),
        success: t(
            locale,
            "groups.storefront.invitation.success",
            "Invitation accepted. You are now a group member.",
        ),
        group: t(locale, "groups.storefront.invitation.group", "Group"),
        role: t(locale, "groups.storefront.invitation.role", "Role"),
        status: t(
            locale,
            "groups.storefront.invitation.status",
            "Membership status",
        ),
        missing_token: t(
            locale,
            "groups.storefront.invitation.missingToken",
            "Enter an invitation token.",
        ),
        invalid_token: t(
            locale,
            "groups.storefront.invitation.invalidToken",
            "The invitation token has an invalid length.",
        ),
        invalid_invitation_id: t(
            locale,
            "groups.storefront.invitation.invalidInvitationId",
            "The targeted invitation identifier is invalid.",
        ),
    }
}
