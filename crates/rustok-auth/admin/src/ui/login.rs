use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::use_auth;
use leptos_hook_form::FormState;
use leptos_router::hooks::use_navigate;
use rustok_ui_core::UiRouteContext;

use crate::core::{AuthFormInputError, prepare_login_request};
use crate::i18n::t;
use crate::ui::components::{Button, Input};

#[component]
pub fn Login<F, IV>(language_toggle: F) -> impl IntoView
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

    let (tenant, set_tenant) = signal(String::from("demo"));
    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (form_state, set_form_state) = signal(FormState::idle());

    let error_required_msg =
        StoredValue::new(t_local("auth.errorRequired", "Please fill in all fields"));

    let on_submit = Callback::new(move |_| {
        let request = match prepare_login_request(tenant.get(), email.get(), password.get()) {
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
                .sign_in(request.email, request.password, request.tenant)
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
        <section class="relative h-screen flex-col items-center justify-center bg-background md:grid lg:max-w-none lg:grid-cols-2 lg:px-0">
            <aside class="relative hidden h-full flex-col overflow-hidden bg-zinc-900 p-10 text-white lg:flex">
                <div class="relative z-20 flex items-center text-lg font-medium">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        class="mr-2 h-6 w-6"
                    >
                        <path d="M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3" />
                    </svg>
                    "RusTok Admin"
                </div>
                <div class="absolute inset-0 opacity-40 [background-image:linear-gradient(to_right,rgba(255,255,255,.12)_1px,transparent_1px),linear-gradient(to_bottom,rgba(255,255,255,.12)_1px,transparent_1px)] [background-size:34px_34px] [mask-image:radial-gradient(420px_circle_at_center,white,transparent)]" />
                <div class="relative z-20 mt-auto space-y-2">
                    <p class="text-lg">
                        "\"RusTok - a multi-tenant language agnostic highload platform built with Rust and WebAssembly.\""
                    </p>
                    <p class="text-sm text-white/70">"RusTok Team"</p>
                </div>
            </aside>

            <div class="flex h-full items-center justify-center p-4 lg:p-8">
                <div class="flex w-full max-w-md flex-col items-center justify-center space-y-6">
                    <div class="flex flex-col space-y-2 text-center">
                        <h1 class="text-2xl font-semibold tracking-tight">
                            {t_local("auth.title", "Sign in to admin")}
                        </h1>
                        <p class="text-sm text-muted-foreground">
                            {t_local("auth.subtitle", "Enter your credentials to access the control panel.")}
                        </p>
                    </div>

                    <div class="flex w-full items-center justify-center gap-3 text-sm text-muted-foreground">
                        <span>{t_local("auth.languageLabel", "Language")}</span>
                        {language_toggle()}
                    </div>

                    <div class="w-full space-y-4">
                        <Show when=move || form_state.get().form_error.is_some()>
                            <div class="rounded-md bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {move || form_state.get().form_error.unwrap_or_default()}
                            </div>
                        </Show>
                        <Input
                            value=tenant
                            set_value=set_tenant
                            placeholder="demo"
                            label=t_local("auth.tenantLabel", "Tenant Slug")
                        />
                        <Input
                            value=email
                            set_value=set_email
                            placeholder="admin@rustok.io"
                            label=t_local("auth.emailLabel", "Email")
                        />
                        <Input
                            value=password
                            set_value=set_password
                            placeholder="********"
                            type_="password"
                            label=t_local("auth.passwordLabel", "Password")
                        />
                        <Button on_click=on_submit class="w-full">
                            {t_local("auth.submit", "Continue")}
                        </Button>
                    </div>

                    <p class="px-8 text-center text-sm text-muted-foreground">
                        <a class="hover:text-primary underline underline-offset-4" href="/register">
                            {t_local("auth.registerLink", "Create account")}
                        </a>
                        " · "
                        <a class="hover:text-primary underline underline-offset-4" href="/reset">
                            {t_local("auth.resetLink", "Forgot password?")}
                        </a>
                    </p>
                </div>
            </div>
        </section>
    }
}
