use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::use_auth;
use leptos_router::hooks::{use_navigate, use_query_map};
use rustok_forms::FormState;
use rustok_ui_core::UiRouteContext;

use crate::i18n::t;
use crate::ui::components::{Button, Input};

/// Public invite-acceptance screen (`/accept-invite?token=…&tenant=…`).
///
/// Accepts the single-use invite with a chosen password, then signs
/// in through the normal cookie-session flow so session handling
/// stays identical to a regular login.
#[component]
pub fn AcceptInvite<F, IV>(language_toggle: F) -> impl IntoView
where
    F: Fn() -> IV + 'static,
    IV: IntoView + 'static,
{
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale_stored = StoredValue::new(route_context.locale);
    let t_local = move |key: &str, fallback: &str| {
        locale_stored.with_value(|l| t(l.as_deref(), key, fallback))
    };

    let auth = use_auth();
    let navigate = use_navigate();
    let query = use_query_map();

    let invite_token = query.get_untracked().get("token").unwrap_or_default();
    let initial_tenant = query.get_untracked().get("tenant").unwrap_or_default();
    let has_token = !invite_token.trim().is_empty();
    let invite_token = StoredValue::new(invite_token);

    let (tenant, set_tenant) = signal(initial_tenant);
    let (name, set_name) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (confirm_password, set_confirm_password) = signal(String::new());
    let (form_state, set_form_state) = signal(FormState::idle());
    let (accepted_notice, set_accepted_notice) = signal(Option::<String>::None);

    let required_msg = StoredValue::new(t_local(
        "acceptInvite.errorRequired",
        "Fill in the tenant and choose a password.",
    ));
    let mismatch_msg = StoredValue::new(t_local(
        "acceptInvite.errorMismatch",
        "Passwords do not match.",
    ));
    let accepted_msg = StoredValue::new(t_local(
        "acceptInvite.acceptedNotice",
        "Your account is ready. Sign in with the password you just chose.",
    ));

    let submit_navigate = navigate.clone();
    let on_submit = Callback::new(move |_| {
        let tenant_val = tenant.get().trim().to_string();
        let password_val = password.get();
        if tenant_val.is_empty() || password_val.is_empty() {
            set_form_state.set(FormState::with_form_error(required_msg.get_value()));
            return;
        }
        if password_val != confirm_password.get() {
            set_form_state.set(FormState::with_form_error(mismatch_msg.get_value()));
            return;
        }
        let name_val = name.get().trim().to_string();
        let token_val = invite_token.get_value();
        let auth = auth.clone();
        let navigate = submit_navigate.clone();

        set_form_state.set(FormState::submitting());

        spawn_local(async move {
            let email = match leptos_auth::transport::accept_invite(
                token_val,
                password_val.clone(),
                if name_val.is_empty() {
                    None
                } else {
                    Some(name_val)
                },
                tenant_val.clone(),
            )
            .await
            {
                Ok(email) => email,
                Err(error) => {
                    set_form_state.set(FormState::with_form_error(format!("{error}")));
                    return;
                }
            };

            match auth.sign_in(email, password_val, tenant_val).await {
                Ok(()) => {
                    set_form_state.set(FormState::idle());
                    navigate("/dashboard", Default::default());
                }
                Err(_) => {
                    // The account exists; only the automatic sign-in
                    // failed (for example, email verification is
                    // required first). Send the user to the login
                    // screen with a clear notice.
                    set_form_state.set(FormState::idle());
                    set_accepted_notice.set(Some(accepted_msg.get_value()));
                }
            }
        });
    });

    let go_login = {
        let navigate = navigate.clone();
        Callback::new(move |_| navigate("/login", Default::default()))
    };

    view! {
        <section class="grid min-h-screen grid-cols-1 lg:grid-cols-[1.2fr_1fr]">
            <aside class="flex flex-col justify-center gap-6 bg-primary p-12 text-primary-foreground lg:p-16">
                <span class="inline-flex w-fit items-center rounded-full bg-primary-foreground/10 px-3 py-1 text-xs font-semibold text-primary-foreground/80">
                    {t_local("acceptInvite.badge", "Invitation")}
                </span>
                <h1 class="text-4xl font-semibold">
                    {t_local("acceptInvite.heroTitle", "Join your workspace")}
                </h1>
                <p class="text-lg text-primary-foreground/80">
                    {t_local(
                        "acceptInvite.heroSubtitle",
                        "You were invited to a RusTok workspace. Choose a password to activate your account.",
                    )}
                </p>
            </aside>
            <div class="flex flex-col justify-center gap-7 bg-background p-12 lg:p-20">
                <div class="flex flex-col gap-5 rounded-xl border border-border bg-card p-8 shadow-md">
                    <div>
                        <h2 class="text-2xl font-semibold text-card-foreground">
                            {t_local("acceptInvite.title", "Accept invitation")}
                        </h2>
                        <p class="text-muted-foreground">
                            {t_local(
                                "acceptInvite.subtitle",
                                "This invite link works once. Set your password to continue.",
                            )}
                        </p>
                    </div>
                    <div class="flex items-center justify-between gap-3 text-sm text-muted-foreground">
                        <span>{t_local("acceptInvite.languageLabel", "Language")}</span>
                        {language_toggle()}
                    </div>

                    {if has_token {
                        view! {
                            <Show when=move || form_state.get().form_error.is_some()>
                                <div class="rounded-md bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                    {move || form_state.get().form_error.unwrap_or_default()}
                                </div>
                            </Show>
                            <Show when=move || accepted_notice.get().is_some()>
                                <div class="rounded-md bg-primary/10 border border-primary/20 px-4 py-2 text-sm text-foreground">
                                    {move || accepted_notice.get().unwrap_or_default()}
                                </div>
                                <Button on_click=go_login>
                                    {t_local("acceptInvite.goLogin", "Go to sign in")}
                                </Button>
                            </Show>
                            <Show when=move || accepted_notice.get().is_none()>
                                <Input
                                    value=tenant
                                    set_value=set_tenant
                                    placeholder="workspace-slug"
                                    label=t_local("acceptInvite.tenantLabel", "Tenant")
                                />
                                <Input
                                    value=name
                                    set_value=set_name
                                    placeholder="Jane Doe"
                                    label=t_local("acceptInvite.nameLabel", "Full name (optional)")
                                />
                                <Input
                                    value=password
                                    set_value=set_password
                                    placeholder="••••••••"
                                    type_="password"
                                    label=t_local("acceptInvite.passwordLabel", "Password")
                                />
                                <Input
                                    value=confirm_password
                                    set_value=set_confirm_password
                                    placeholder="••••••••"
                                    type_="password"
                                    label=t_local("acceptInvite.confirmLabel", "Confirm password")
                                />
                                <Button
                                    on_click=on_submit
                                    disabled=Signal::derive(move || form_state.get().is_submitting)
                                >
                                    {move || if form_state.get().is_submitting {
                                        t_local("acceptInvite.submitting", "Activating...")
                                    } else {
                                        t_local("acceptInvite.submit", "Activate account")
                                    }}
                                </Button>
                            </Show>
                        }
                            .into_any()
                    } else {
                        view! {
                            <div class="rounded-md bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {t_local(
                                    "acceptInvite.missingToken",
                                    "This invitation link is incomplete. Ask your administrator for a new link.",
                                )}
                            </div>
                            <Button on_click=go_login>
                                {t_local("acceptInvite.goLogin", "Go to sign in")}
                            </Button>
                        }
                            .into_any()
                    }}
                </div>
            </div>
        </section>
    }
}
