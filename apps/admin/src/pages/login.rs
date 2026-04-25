use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::use_auth;
use leptos_hook_form::FormState;
use leptos_router::hooks::use_navigate;

use crate::shared::ui::{Button, Input, LanguageToggle};
use crate::{t_string, use_i18n};

#[component]
pub fn Login() -> impl IntoView {
    let i18n = use_i18n();
    let auth = use_auth();
    let navigate = use_navigate();

    let (tenant, set_tenant) = signal(String::from("demo"));
    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (form_state, set_form_state) = signal(FormState::idle());

    let on_submit = move |_| {
        if tenant.get().is_empty() || email.get().is_empty() || password.get().is_empty() {
            set_form_state.set(FormState::with_form_error(
                t_string!(i18n, auth.errorRequired).to_string(),
            ));
            return;
        }

        let tenant_value = tenant.get().trim().to_string();
        let email_value = email.get().trim().to_string();
        let password_value = password.get();
        let auth = auth.clone();
        let navigate = navigate.clone();

        set_form_state.set(FormState::submitting());

        spawn_local(async move {
            match auth
                .sign_in(email_value, password_value, tenant_value)
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
    };

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
                        "\"RusTok - a modern multi-tenant CMS built with Rust and WebAssembly.\""
                    </p>
                    <p class="text-sm text-white/70">"RusTok Team"</p>
                </div>
            </aside>

            <div class="flex h-full items-center justify-center p-4 lg:p-8">
                <div class="flex w-full max-w-md flex-col items-center justify-center space-y-6">
                    <div class="flex flex-col space-y-2 text-center">
                        <h1 class="text-2xl font-semibold tracking-tight">
                            {move || t_string!(i18n, auth.title)}
                        </h1>
                        <p class="text-sm text-muted-foreground">
                            {move || t_string!(i18n, auth.subtitle)}
                        </p>
                    </div>

                    <div class="flex w-full items-center justify-center gap-3 text-sm text-muted-foreground">
                        <span>{move || t_string!(i18n, auth.languageLabel)}</span>
                        <LanguageToggle />
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
                            label=move || t_string!(i18n, auth.tenantLabel)
                        />
                        <Input
                            value=email
                            set_value=set_email
                            placeholder="admin@rustok.io"
                            label=move || t_string!(i18n, auth.emailLabel)
                        />
                        <Input
                            value=password
                            set_value=set_password
                            placeholder="********"
                            type_="password"
                            label=move || t_string!(i18n, auth.passwordLabel)
                        />
                        <Button on_click=on_submit class="w-full">
                            {move || t_string!(i18n, auth.submit)}
                        </Button>
                    </div>

                    <p class="px-8 text-center text-sm text-muted-foreground">
                        <a class="hover:text-primary underline underline-offset-4" href="/register">
                            {move || t_string!(i18n, auth.registerLink)}
                        </a>
                        " · "
                        <a class="hover:text-primary underline underline-offset-4" href="/reset">
                            {move || t_string!(i18n, auth.resetLink)}
                        </a>
                    </p>
                </div>
            </div>
        </section>
    }
}
