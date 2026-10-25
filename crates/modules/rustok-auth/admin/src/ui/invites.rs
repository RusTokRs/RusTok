use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui::{Badge, BadgeVariant};
use rustok_ui_core::UiRouteContext;

use crate::i18n::{auth_transport_error_message, t};
use crate::model::{IssueUserInviteInput, IssuedUserInvite};
use crate::transport::{fetch_user_invites, issue_user_invite, revoke_user_invite};
use crate::ui::components::{Button, Input};

/// Absolute origin for building shareable invite links (browser only).
fn current_origin() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        String::new()
    }
}

fn invite_link(issued: &IssuedUserInvite, tenant: &str) -> String {
    format!(
        "{}/accept-invite?token={}&tenant={}",
        current_origin(),
        issued.token,
        tenant
    )
}

/// Tenant invite management: issue one-time invite links, list
/// outstanding invites, and revoke them. The raw invite token is
/// returned by the server exactly once (at issuance), so the issued
/// link is shown in a copy-it-now state and never persisted here.
#[component]
pub fn UserInvites() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = StoredValue::new(route_context.locale);
    let t_local =
        move |key: &str, fallback: &str| locale.with_value(|l| t(l.as_deref(), key, fallback));

    let token = use_token();
    let tenant = use_tenant();

    let (refresh_counter, set_refresh_counter) = signal(0u32);
    let invites_resource = LocalResource::new(move || {
        let token_value = token.get();
        let tenant_value = tenant.get();
        let _ = refresh_counter.get();
        async move { fetch_user_invites(token_value, tenant_value).await }
    });

    let (show_invite_modal, set_show_invite_modal) = signal(false);
    let (invite_email, set_invite_email) = signal(String::new());
    let (invite_role, set_invite_role) = signal(String::new());
    let (invite_error, set_invite_error) = signal(Option::<String>::None);
    let (is_issuing, set_is_issuing) = signal(false);
    let (issued_link, set_issued_link) = signal(Option::<String>::None);
    let (revoke_error, set_revoke_error) = signal(Option::<String>::None);

    let open_invite_modal = Callback::new(move |_| {
        set_invite_email.set(String::new());
        set_invite_role.set(String::new());
        set_invite_error.set(None);
        set_issued_link.set(None);
        set_show_invite_modal.set(true);
    });

    let close_invite_modal = Callback::new(move |_| {
        set_show_invite_modal.set(false);
        set_issued_link.set(None);
    });

    let required_msg = StoredValue::new(t_local(
        "users.invites.errorRequired",
        "Email is required.",
    ));

    let issue_action = Callback::new(move |_| {
        let email_val = invite_email.get().trim().to_string();
        if email_val.is_empty() {
            set_invite_error.set(Some(required_msg.get_value()));
            return;
        }
        let role_val = invite_role.get().trim().to_string();
        let token_val = token.get();
        let tenant_val = tenant.get();
        let tenant_for_link = tenant_val.clone().unwrap_or_default();

        set_is_issuing.set(true);
        set_invite_error.set(None);

        spawn_local(async move {
            let input = IssueUserInviteInput {
                email: email_val,
                role: if role_val.is_empty() {
                    None
                } else {
                    Some(role_val)
                },
            };
            match issue_user_invite(token_val, tenant_val, input).await {
                Ok(issued) => {
                    set_is_issuing.set(false);
                    set_issued_link.set(Some(invite_link(&issued, &tenant_for_link)));
                    set_refresh_counter.update(|value| *value += 1);
                }
                Err(error) => {
                    set_is_issuing.set(false);
                    set_invite_error.set(Some(locale.with_value(|locale| {
                        auth_transport_error_message(locale.as_deref(), &error.to_string())
                    })));
                }
            }
        });
    });

    let revoke_action = Callback::new(move |invite_id: String| {
        let token_val = token.get();
        let tenant_val = tenant.get();
        set_revoke_error.set(None);
        spawn_local(async move {
            match revoke_user_invite(token_val, tenant_val, invite_id).await {
                Ok(_) => set_refresh_counter.update(|value| *value += 1),
                Err(error) => {
                    set_revoke_error.set(Some(locale.with_value(|locale| {
                        auth_transport_error_message(locale.as_deref(), &error.to_string())
                    })));
                }
            }
        });
    });

    view! {
        <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
            <div class="mb-4 flex items-center justify-between gap-3">
                <h4 class="text-lg font-semibold text-card-foreground">
                    {t_local("users.invites.title", "Invitations")}
                </h4>
                <Button on_click=open_invite_modal>
                    {t_local("users.invites.issueButton", "Invite user")}
                </Button>
            </div>
            <p class="mb-4 text-sm text-muted-foreground">
                {t_local(
                    "users.invites.subtitle",
                    "Invite links are single-use. Anyone with the link can create the invited account, so share it through a trusted channel.",
                )}
            </p>

            <Show when=move || revoke_error.get().is_some()>
                <div class="mb-4 rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                    {move || revoke_error.get().unwrap_or_default()}
                </div>
            </Show>

            <Suspense fallback=move || {
                view! { <div class="h-10 animate-pulse rounded-lg bg-muted"></div> }
            }>
                {move || match invites_resource.get() {
                    None => {
                        view! { <div class="h-10 animate-pulse rounded-lg bg-muted"></div> }
                            .into_any()
                    }
                    Some(Ok(invites)) if invites.is_empty() => {
                        view! {
                            <p class="text-sm text-muted-foreground">
                                {t_local("users.invites.empty", "No invitations yet")}
                            </p>
                        }
                            .into_any()
                    }
                    Some(Ok(invites)) => {
                        view! {
                            <div class="space-y-2">
                                {invites
                                    .into_iter()
                                    .map(|invite| {
                                        let revoked = invite.revoked_at.is_some();
                                        let invite_id = invite.id.clone();
                                        view! {
                                            <div class="flex flex-wrap items-center gap-3 rounded-lg border border-border px-4 py-3">
                                                <span class="font-medium text-foreground">
                                                    {invite.email}
                                                </span>
                                                <span class="text-xs font-mono uppercase bg-muted text-muted-foreground px-2 py-0.5 rounded">
                                                    {invite.role}
                                                </span>
                                                <Badge variant=if revoked {
                                                    BadgeVariant::Default
                                                } else {
                                                    BadgeVariant::Success
                                                }>
                                                    {if revoked {
                                                        t_local("users.invites.revoked", "Revoked")
                                                    } else {
                                                        t_local("users.invites.pending", "Pending")
                                                    }}
                                                </Badge>
                                                <span class="text-xs text-muted-foreground font-mono">
                                                    {format!(
                                                        "{} {}",
                                                        t_local("users.invites.expires", "Expires"),
                                                        invite.expires_at,
                                                    )}
                                                </span>
                                                <span class="flex-1"></span>
                                                {if revoked {
                                                    ().into_any()
                                                } else {
                                                    view! {
                                                        <button
                                                            type="button"
                                                            class="text-xs text-destructive hover:underline font-medium px-2 py-1 rounded hover:bg-muted/50"
                                                            on:click=move |_| revoke_action.run(invite_id.clone())
                                                        >
                                                            {t_local("users.invites.revoke", "Revoke")}
                                                        </button>
                                                    }
                                                        .into_any()
                                                }}
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                            .into_any()
                    }
                    Some(Err(err)) => {
                        view! {
                            <div class="rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {format!(
                                    "{}: {}",
                                    t_local(
                                        "users.invites.loadError",
                                        "Failed to load invitations",
                                    ),
                                    err,
                                )}
                            </div>
                        }
                            .into_any()
                    }
                }}
            </Suspense>

            <Show when=move || show_invite_modal.get()>
                <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
                    <div class="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-xl">
                        <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                            {t_local("users.invites.issueTitle", "Invite a user")}
                        </h3>

                        <Show when=move || invite_error.get().is_some()>
                            <div class="mb-4 rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {move || invite_error.get().unwrap_or_default()}
                            </div>
                        </Show>

                        {move || match issued_link.get() {
                            Some(link) => {
                                view! {
                                    <div class="space-y-4">
                                        <p class="text-sm text-muted-foreground">
                                            {t_local(
                                                "users.invites.issuedNotice",
                                                "Copy this link now — it is shown only once and works a single time:",
                                            )}
                                        </p>
                                        <code class="block break-all rounded-lg bg-muted px-3 py-2 text-xs">
                                            {link}
                                        </code>
                                        <Button on_click=close_invite_modal class="w-full">
                                            {t_local("users.invites.done", "Done")}
                                        </Button>
                                    </div>
                                }
                                    .into_any()
                            }
                            None => {
                                view! {
                                    <div class="space-y-4">
                                        <Input
                                            value=invite_email
                                            set_value=set_invite_email
                                            placeholder="teammate@example.com"
                                            label=t_local("users.invites.emailLabel", "Email")
                                        />
                                        <Input
                                            value=invite_role
                                            set_value=set_invite_role
                                            placeholder="customer, manager, admin, or a custom role slug"
                                            label=t_local("users.invites.roleLabel", "Role (optional)")
                                        />
                                        <div class="mt-6 flex gap-3">
                                            <Button
                                                on_click=issue_action
                                                disabled=is_issuing.into()
                                                class="flex-1"
                                            >
                                                {move || if is_issuing.get() {
                                                    t_local("users.invites.issuing", "Issuing...")
                                                } else {
                                                    t_local("users.invites.issueSubmit", "Create invite link")
                                                }}
                                            </Button>
                                            <Button
                                                on_click=close_invite_modal
                                                class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                                            >
                                                {t_local("users.invites.cancel", "Cancel")}
                                            </Button>
                                        </div>
                                    </div>
                                }
                                    .into_any()
                            }
                        }}
                    </div>
                </div>
            </Show>
        </div>
    }
}
