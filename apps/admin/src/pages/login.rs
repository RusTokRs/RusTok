use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::use_auth;
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
    let (error, set_error) = signal(Option::<String>::None);

    let on_submit = move |_| {
        if tenant.get().is_empty() || email.get().is_empty() || password.get().is_empty() {
            set_error.set(Some(t_string!(i18n, auth.errorRequired).to_string()));
            return;
        }

        let tenant_value = tenant.get().trim().to_string();
        let email_value = email.get().trim().to_string();
        let password_value = password.get();
        let auth = auth.clone();
        let navigate = navigate.clone();

        spawn_local(async move {
            match auth
                .sign_in(email_value, password_value, tenant_value)
                .await
            {
                Ok(()) => {
                    set_error.set(None);
                    navigate("/dashboard", Default::default());
                }
                Err(e) => {
                    set_error.set(Some(format!("{}", e)));
                }
            }
        });
    };

    view! {
        <section class="grid min-h-screen grid-cols-1 lg:grid-cols-[1.2fr_1fr]">
            <aside class="flex flex-col justify-center gap-6 bg-primary p-12 text-primary-foreground lg:p-16">
                <span class="inline-flex w-fit items-center rounded-full bg-primary-foreground/10 px-3 py-1 text-xs font-semibold text-primary-foreground/80">
                    {move || t_string!(i18n, auth.badge)}
                </span>
                <h1 class="text-4xl font-semibold">{move || t_string!(i18n, auth.heroTitle)}</h1>
                <p class="text-lg text-primary-foreground/80">{move || t_string!(i18n, auth.heroSubtitle)}</p>
                <div class="grid gap-2">
                    <p class="text-sm font-semibold">
                        {move || t_string!(i18n, auth.heroListTitle)}
                    </p>
                    <p class="text-sm text-primary-foreground/75">
                        {move || t_string!(i18n, auth.heroListSubtitle)}
                    </p>
                </div>
            </aside>
            <div class="flex flex-col justify-center gap-7 bg-background p-12 lg:p-20">
                <div class="flex flex-col gap-5 rounded-xl border border-border bg-card p-8 shadow-md">
                    <div>
                        <h2 class="text-2xl font-semibold text-card-foreground">
                            {move || t_string!(i18n, auth.title)}
                        </h2>
                        <p class="text-muted-foreground">
                            {move || t_string!(i18n, auth.subtitle)}
                        </p>
                    </div>
                    <div class="flex items-center justify-between gap-3 text-sm text-muted-foreground">
                        <span>{move || t_string!(i18n, auth.languageLabel)}</span>
                        <LanguageToggle />
                    </div>
                    <Show when=move || error.get().is_some()>
                        <div class="rounded-md bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                            {move || error.get().unwrap_or_default()}
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
                        placeholder="••••••••"
                        type_="password"
                        label=move || t_string!(i18n, auth.passwordLabel)
                    />
                    <Button on_click=on_submit class="w-full">
                        {move || t_string!(i18n, auth.submit)}
                    </Button>
                    <div class="flex justify-between gap-3 text-sm">
                        <a class="text-primary hover:underline underline-offset-4" href="/register">
                            {move || t_string!(i18n, auth.registerLink)}
                        </a>
                        <a class="text-primary hover:underline underline-offset-4" href="/reset">
                            {move || t_string!(i18n, auth.resetLink)}
                        </a>
                    </div>
                </div>
            </div>
        </section>
    }
}
