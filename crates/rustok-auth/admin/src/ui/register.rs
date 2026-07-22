use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::use_auth;
use leptos_hook_form::FormState;
use leptos_router::hooks::use_navigate;
use rustok_ui_core::UiRouteContext;

use crate::core::{AuthFormInputError, prepare_register_request};
use crate::i18n::t;
use crate::ui::components::{Button, Input};

#[component]
pub fn Register<F, IV>(language_toggle: F) -> impl IntoView
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

    let (tenant, set_tenant) = signal(String::new());
    let (email, set_email) = signal(String::new());
    let (name, set_name) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (form_state, set_form_state) = signal(FormState::idle());

    let error_required_msg = StoredValue::new(t_local(
        "register.errorRequired",
        "Fill in tenant, email, and password.",
    ));

    let on_submit = Callback::new(move |_| {
        let request =
            match prepare_register_request(tenant.get(), email.get(), password.get(), name.get()) {
                Ok(request) => request,
                Err(AuthFormInputError::MissingRequiredFields) => {
                    set_form_state.set(FormState::with_form_error(error_required_msg.get_value()));
                    return;
                }
            };
        let auth = auth.clone();
        let navigate = navigate.clone();

        set_form_state.set(FormState::submitting());

        spawn_local(async move {
            match auth
                .sign_up(
                    request.email,
                    request.password,
                    request.name,
                    request.tenant,
                )
                .await
            {
                Ok(()) => {
                    set_form_state.set(FormState::idle());
                    navigate("/dashboard", Default::default());
                }
                Err(e) => {
                    set_form_state.set(FormState::with_form_error(format!("{}", e)));
                }
            }
        });
    });

    view! {
        <section class="grid min-h-screen grid-cols-1 lg:grid-cols-[1.2fr_1fr]">
            <aside class="flex flex-col justify-center gap-6 bg-primary p-12 text-primary-foreground lg:p-16">
                <span class="inline-flex w-fit items-center rounded-full bg-primary-foreground/10 px-3 py-1 text-xs font-semibold text-primary-foreground/80">
                    {t_local("register.badge", "Admin Onboarding")}
                </span>
                <h1 class="text-4xl font-semibold">{t_local("register.heroTitle", "Launch your admin workspace")}</h1>
                <p class="text-lg text-primary-foreground/80">{t_local("register.heroSubtitle", "Create a secure admin account, accept invitations, and verify email access in one streamlined flow.")}</p>
                <div class="grid gap-2">
                    <p class="text-sm font-semibold">
                        {t_local("register.heroListTitle", "Parallel-ready onboarding")}
                    </p>
                    <p class="text-sm text-primary-foreground/75">
                        {t_local("register.heroListSubtitle", "Sign-up, invites, and verification can ship independently without blocking login.")}
                    </p>
                </div>
            </aside>
            <div class="flex flex-col justify-center gap-7 bg-background p-12 lg:p-20">
                <div class="flex flex-col gap-5 rounded-xl border border-border bg-card p-8 shadow-md">
                    <div>
                        <h2 class="text-2xl font-semibold text-card-foreground">
                            {t_local("register.title", "Create admin account")}
                        </h2>
                        <p class="text-muted-foreground">
                            {t_local("register.subtitle", "Invite your team or register with a tenant slug.")}
                        </p>
                    </div>
                    <div class="flex items-center justify-between gap-3 text-sm text-muted-foreground">
                        <span>{t_local("register.languageLabel", "Language")}</span>
                        {language_toggle()}
                    </div>
                    <Show when=move || form_state.get().form_error.is_some()>
                        <div class="rounded-md bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                            {move || form_state.get().form_error.unwrap_or_default()}
                        </div>
                    </Show>
                    <Input
                        value=tenant
                        set_value=set_tenant
                        placeholder="demo"
                        label=t_local("register.tenantLabel", "Tenant Slug")
                    />
                    <Input
                        value=email
                        set_value=set_email
                        placeholder="admin@rustok.io"
                        label=t_local("register.emailLabel", "Work email")
                    />
                    <Input
                        value=name
                        set_value=set_name
                        placeholder="Alex Morgan"
                        label=t_local("register.nameLabel", "Full name")
                    />
                    <Input
                        value=password
                        set_value=set_password
                        placeholder="••••••••"
                        type_="password"
                        label=t_local("register.passwordLabel", "Password")
                    />
                    <p class="text-sm text-muted-foreground">
                        {t_local("register.passwordHint", "Use 10+ characters with upper/lowercase, numbers, and symbols.")}
                    </p>
                    <Button on_click=on_submit class="w-full">
                        {t_local("register.submit", "Create account")}
                    </Button>
                    <div class="flex justify-between gap-3 text-sm">
                        <a class="text-primary hover:underline underline-offset-4" href="/login">
                            {t_local("register.loginLink", "Back to sign in")}
                        </a>
                        <a class="text-primary hover:underline underline-offset-4" href="/reset">
                            {t_local("register.resetLink", "Reset password")}
                        </a>
                    </div>
                </div>
            </div>
        </section>
    }
}
