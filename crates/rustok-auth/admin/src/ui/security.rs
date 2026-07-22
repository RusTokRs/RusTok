use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_auth, use_tenant, use_token};
use leptos_hook_form::FormState;
use rustok_ui_core::UiRouteContext;

use crate::core::{ChangePasswordInputError, prepare_change_password_request};
use crate::i18n::{auth_transport_error_message, t};
use crate::transport::change_password;
use crate::ui::components::{Button, Input, PageHeader};

#[component]
pub fn Security() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale_stored = StoredValue::new(route_context.locale);
    let t_local = move |key: &str, fallback: &str| {
        locale_stored.with_value(|l| t(l.as_deref(), key, fallback))
    };

    let auth = use_auth();
    let token = use_token();
    let tenant = use_tenant();

    let (current_password, set_current_password) = signal(String::new());
    let (new_password, set_new_password) = signal(String::new());
    let (form_state, set_form_state) = signal(FormState::idle());
    let (success_message, set_success_message) = signal(Option::<String>::None);

    let on_change_password = Callback::new(move |_| {
        let request = match prepare_change_password_request(
            token.get(),
            tenant.get(),
            current_password.get(),
            new_password.get(),
        ) {
            Ok(request) => request,
            Err(ChangePasswordInputError::MissingPasswords) => {
                set_form_state.set(FormState::with_form_error(t_local(
                    "security.passwordRequired",
                    "Enter current and new passwords.",
                )));
                return;
            }
            Err(ChangePasswordInputError::Unauthorized) => {
                set_form_state.set(FormState::with_form_error(t_local(
                    "errors.auth.unauthorized",
                    "You are not authorized to perform this action.",
                )));
                return;
            }
        };

        set_form_state.set(FormState::submitting());
        set_success_message.set(None);

        spawn_local(async move {
            let result = change_password(
                request.token,
                request.tenant,
                request.current_password,
                request.new_password,
            )
            .await;

            match result {
                Ok(_) => {
                    set_form_state.set(FormState::idle());
                    set_success_message.set(Some(t_local(
                        "security.passwordUpdated",
                        "Password updated successfully.",
                    )));
                    set_current_password.set(String::new());
                    set_new_password.set(String::new());
                }
                Err(err_str) => {
                    let message = locale_stored.with_value(|locale| {
                        auth_transport_error_message(locale.as_deref(), &err_str)
                    });
                    set_form_state.set(FormState::with_form_error(message));
                    set_success_message.set(None);
                }
            }
        });
    });

    let on_sign_out_all = Callback::new(move |_| {
        let auth = auth.clone();
        spawn_local(async move {
            let _ = auth.sign_out().await;
        });
    });

    view! {
        <section class="flex flex-1 flex-col p-4 md:px-6">
            <PageHeader
                title=t_local("security.title", "Security & sessions")
                subtitle=t_local("security.subtitle", "Monitor active sessions and keep credentials secure.")
                eyebrow=t_local("security.badge", "Security")
                actions=view! {
                    <Button
                        on_click=on_sign_out_all
                        class="border border-border bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                    >
                        {t_local("security.signOutAll", "Sign out all sessions")}
                    </Button>
                }
                .into_any()
            />

            <div class="grid gap-6 lg:grid-cols-2">
                <div class="grid gap-4 rounded-xl border border-border bg-card p-6 shadow-sm">
                    <h3 class="text-lg font-semibold text-card-foreground">
                        {t_local("security.passwordTitle", "Change password")}
                    </h3>
                    <p class="text-sm text-muted-foreground">
                        {t_local("security.passwordSubtitle", "Use a strong password unique to this admin account.")}
                    </p>
                     <Input
                        value=current_password
                        set_value=set_current_password
                        placeholder="••••••••"
                        type_="password"
                        label=t_local("security.currentPasswordLabel", "Current password")
                    />
                    <Input
                        value=new_password
                        set_value=set_new_password
                        placeholder="••••••••"
                        type_="password"
                        label=t_local("security.newPasswordLabel", "New password")
                    />
                    <p class="text-sm text-muted-foreground">
                        {t_local("security.passwordHint", "Use 12+ characters with mixed case and symbols.")}
                    </p>
                    <Button on_click=on_change_password class="w-full">
                        {t_local("security.passwordSubmit", "Update password")}
                    </Button>
                    <Show when=move || form_state.get().form_error.is_some()>
                        <div class="rounded-md bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                            {move || form_state.get().form_error.unwrap_or_default()}
                        </div>
                    </Show>
                    <Show when=move || success_message.get().is_some()>
                        <div class="rounded-md bg-emerald-100 border border-emerald-200 px-4 py-2 text-sm text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400">
                            {move || success_message.get().unwrap_or_default()}
                        </div>
                    </Show>
                </div>

                <div class="grid gap-4 rounded-xl border border-border bg-card p-6 shadow-sm">
                    <h3 class="text-lg font-semibold text-card-foreground">
                        {t_local("security.sessionsTitle", "Active sessions")}
                    </h3>
                    <p class="text-sm text-muted-foreground">
                        {t_local("security.sessionsSubtitle", "Review devices that are currently signed in.")}
                    </p>
                    <div class="rounded-lg bg-muted px-4 py-8 text-center text-sm text-muted-foreground">
                        "Session management via GraphQL — coming soon"
                    </div>
                </div>
            </div>
        </section>
    }
}
