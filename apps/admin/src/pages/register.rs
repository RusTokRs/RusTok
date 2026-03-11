use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::use_auth;
use leptos_hook_form::FormState;
use leptos_router::hooks::use_navigate;

use crate::shared::ui::{Button, Input, LanguageToggle};
use crate::{t_string, use_i18n};

#[component]
pub fn Register() -> impl IntoView {
    let i18n = use_i18n();
    let auth = use_auth();
    let navigate = use_navigate();

    let (tenant, set_tenant) = signal(String::new());
    let (email, set_email) = signal(String::new());
    let (name, set_name) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (form_state, set_form_state) = signal(FormState::idle());
    let (_error, set_error) = signal::<Option<String>>(None);
    let (_status, set_status) = signal::<Option<String>>(None);

    let on_submit = move |_| {
        if tenant.get().is_empty() || email.get().is_empty() || password.get().is_empty() {
            set_error.set(Some(t_string!(i18n, register.errorRequired).to_string()));
            set_status.set(None);
            return;
        }

        let tenant_value = tenant.get().trim().to_string();
        let email_value = email.get().trim().to_string();
        let password_value = password.get();
        let name_value = name.get().trim().to_string();
        let name_opt = if name_value.is_empty() {
            None
        } else {
            Some(name_value)
        };
        let auth = auth.clone();
        let navigate = navigate.clone();

        set_form_state.set(FormState::submitting());

        spawn_local(async move {
            match auth
                .sign_up(email_value, password_value, name_opt, tenant_value)
                .await
            {
                Ok(()) => {
                    set_error.set(None);
                    set_status.set(Some(t_string!(i18n, register.success).to_string()));
                    navigate("/dashboard", Default::default());
                }
                Err(e) => {
                    set_form_state.set(FormState::with_form_error(format!("{}", e)));
                }
            }
        });
    };

    view! {
        <section class="grid min-h-screen grid-cols-1 lg:grid-cols-[1.2fr_1fr]">
            <aside class="flex flex-col justify-center gap-6 bg-primary p-12 text-primary-foreground lg:p-16">
                <span class="inline-flex w-fit items-center rounded-full bg-primary-foreground/10 px-3 py-1 text-xs font-semibold text-primary-foreground/80">
                    {move || t_string!(i18n, register.badge)}
                </span>
                <h1 class="text-4xl font-semibold">{move || t_string!(i18n, register.heroTitle)}</h1>
                <p class="text-lg text-primary-foreground/80">{move || t_string!(i18n, register.heroSubtitle)}</p>
                <div class="grid gap-2">
                    <p class="text-sm font-semibold">
                        {move || t_string!(i18n, register.heroListTitle)}
                    </p>
                    <p class="text-sm text-primary-foreground/75">
                        {move || t_string!(i18n, register.heroListSubtitle)}
                    </p>
                </div>
            </aside>
            <div class="flex flex-col justify-center gap-7 bg-background p-12 lg:p-20">
                <div class="flex flex-col gap-5 rounded-xl border border-border bg-card p-8 shadow-md">
                    <div>
                        <h2 class="text-2xl font-semibold text-card-foreground">
                            {move || t_string!(i18n, register.title)}
                        </h2>
                        <p class="text-muted-foreground">
                            {move || t_string!(i18n, register.subtitle)}
                        </p>
                    </div>
                    <div class="flex items-center justify-between gap-3 text-sm text-muted-foreground">
                        <span>{move || t_string!(i18n, register.languageLabel)}</span>
                        <LanguageToggle />
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
                        label=move || t_string!(i18n, register.tenantLabel)
                    />
                    <Input
                        value=email
                        set_value=set_email
                        placeholder="admin@rustok.io"
                        label=move || t_string!(i18n, register.emailLabel)
                    />
                    <Input
                        value=name
                        set_value=set_name
                        placeholder="Alex Morgan"
                        label=move || t_string!(i18n, register.nameLabel)
                    />
                    <Input
                        value=password
                        set_value=set_password
                        placeholder="••••••••"
                        type_="password"
                        label=move || t_string!(i18n, register.passwordLabel)
                    />
                    <p class="text-sm text-muted-foreground">
                        {move || t_string!(i18n, register.passwordHint)}
                    </p>
                    <Button on_click=on_submit class="w-full">
                        {move || t_string!(i18n, register.submit)}
                    </Button>
                    <div class="flex justify-between gap-3 text-sm">
                        <a class="text-primary hover:underline underline-offset-4" href="/login">
                            {move || t_string!(i18n, register.loginLink)}
                        </a>
                        <a class="text-primary hover:underline underline-offset-4" href="/reset">
                            {move || t_string!(i18n, register.resetLink)}
                        </a>
                    </div>
                </div>
            </div>
        </section>
    }
}
