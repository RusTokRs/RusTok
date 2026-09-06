use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::use_tenant;
use leptos_hook_form::FormState;
use rustok_ui_core::UiRouteContext;

use crate::core::{AuthFormInputError, prepare_password_reset_request};
use crate::i18n::t;
use crate::transport;
use crate::ui::components::{Button, Input};

#[component]
pub fn ResetPassword<F, IV>(language_toggle: F) -> impl IntoView
where
    F: Fn() -> IV + 'static,
    IV: IntoView + 'static,
{
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale_stored = StoredValue::new(route_context.locale);
    let t_local = move |key: &str, fallback: &str| {
        locale_stored.with_value(|l| t(l.as_deref(), key, fallback))
    };

    let tenant_signal = use_tenant();

    let initial_tenant = tenant_signal.get().unwrap_or_default();
    let (tenant, set_tenant) = signal(initial_tenant);
    let (email, set_email) = signal(String::new());
    let (form_state, set_form_state) = signal(FormState::idle());
    let (success_message, set_success_message) = signal(Option::<String>::None);

    let error_required_msg = StoredValue::new(t_local(
        "reset.errorRequired",
        "Tenant and email are required.",
    ));

    let on_request = Callback::new(move |_| {
        let request = match prepare_password_reset_request(tenant.get(), email.get()) {
            Ok(request) => request,
            Err(AuthFormInputError::MissingRequiredFields) => {
                set_form_state.set(FormState::with_form_error(error_required_msg.get_value()));
                return;
            }
        };

        set_form_state.set(FormState::submitting());
        set_success_message.set(None);

        spawn_local(async move {
            match transport::request_password_reset(request.email, request.tenant).await {
                Ok(message) => {
                    set_form_state.set(FormState::idle());
                    set_success_message.set(Some(message));
                }
                Err(e) => {
                    set_form_state.set(FormState::with_form_error(e.to_string()));
                    set_success_message.set(None);
                }
            }
        });
    });

    view! {
        <section class="grid min-h-screen grid-cols-1 lg:grid-cols-[1.2fr_1fr]">
            <aside class="flex flex-col justify-center gap-6 bg-primary p-12 text-primary-foreground lg:p-16">
                <span class="inline-flex w-fit items-center rounded-full bg-primary-foreground/10 px-3 py-1 text-xs font-semibold text-primary-foreground/80">
                    {t_local("reset.badge", "Password recovery")}
                </span>
                <h1 class="text-4xl font-semibold">{t_local("reset.heroTitle", "Recover access safely")}</h1>
                <p class="text-lg text-primary-foreground/80">{t_local("reset.heroSubtitle", "Request a reset email, then finish the update with a secure token.")}</p>
                <div class="grid gap-2">
                    <p class="text-sm font-semibold">
                        {t_local("reset.heroListTitle", "Token-based flow")}
                    </p>
                    <p class="text-sm text-primary-foreground/75">
                        {t_local("reset.heroListSubtitle", "Reset tokens expire automatically for better security.")}
                    </p>
                </div>
            </aside>
            <div class="flex flex-col justify-center gap-7 bg-background p-12 lg:p-20">
                <div class="flex flex-col gap-5 rounded-xl border border-border bg-card p-8 shadow-md">
                    <div>
                        <h2 class="text-2xl font-semibold text-card-foreground">
                            {t_local("reset.title", "Request reset email")}
                        </h2>
                        <p class="text-muted-foreground">
                            {t_local("reset.subtitle", "We'll send a secure reset link to your inbox.")}
                        </p>
                    </div>
                    <div class="flex items-center justify-between gap-3 text-sm text-muted-foreground">
                        <span>{t_local("reset.languageLabel", "Language")}</span>
                        {language_toggle()}
                    </div>
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
                    <Input value=tenant set_value=set_tenant placeholder="demo" label=t_local("reset.tenantLabel", "Tenant Slug") />
                    <Input value=email set_value=set_email placeholder="admin@rustok.io" label=t_local("reset.emailLabel", "Email") />
                    <Button on_click=on_request class="w-full">{t_local("reset.requestSubmit", "Send reset link")}</Button>
                    <div class="flex justify-between gap-3 text-sm">
                        <a class="text-primary hover:underline underline-offset-4" href="/login">
                            {t_local("reset.loginLink", "Back to sign in")}
                        </a>
                        <a class="text-primary hover:underline underline-offset-4" href="/register">
                            {t_local("reset.registerLink", "Create account")}
                        </a>
                    </div>
                </div>
            </div>
        </section>
    }
}
