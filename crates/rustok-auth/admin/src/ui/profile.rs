use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_current_user, use_tenant, use_token};
use leptos_hook_form::FormState;
use leptos_ui::{Select, SelectOption};
use rustok_api::UiRouteContext;

use crate::core::prepare_profile_name;
use crate::i18n::{auth_transport_error_message, t};
use crate::transport::update_profile;
use crate::ui::components::{Button, Input, PageHeader};

#[component]
pub fn Profile<F, IV>(language_toggle: F) -> impl IntoView
where
    F: Fn() -> IV + 'static,
    IV: IntoView + 'static,
{
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale_stored = StoredValue::new(route_context.locale);
    let t_local = move |key: &str, fallback: &str| {
        locale_stored.with_value(|l| t(l.as_deref(), key, fallback))
    };

    let current_user = use_current_user();
    let token = use_token();
    let tenant = use_tenant();

    let initial_name = current_user
        .get()
        .and_then(|user| user.name)
        .unwrap_or_default();
    let initial_email = current_user
        .get()
        .map(|user| user.email)
        .unwrap_or_default();

    let (name, set_name) = signal(initial_name);
    let (email, _set_email) = signal(initial_email);
    let (avatar, set_avatar) = signal(String::new());
    let (timezone, set_timezone) = signal(String::from("Europe/Moscow"));
    let (preferred_locale, set_preferred_locale) = signal(String::from("ru"));
    let (form_state, set_form_state) = signal(FormState::idle());
    let (success_message, set_success_message) = signal(Option::<String>::None);

    let on_save = Callback::new(move |_| {
        let token_value = token.get();
        let tenant_value = tenant.get();
        if token_value.is_none() {
            set_form_state.set(FormState::with_form_error(t_local(
                "errors.auth.unauthorized",
                "You are not authorized to perform this action.",
            )));
            return;
        }

        let name_value = prepare_profile_name(name.get());

        set_form_state.set(FormState::submitting());
        set_success_message.set(None);

        spawn_local(async move {
            let result = update_profile(
                token_value.clone().unwrap_or_default(),
                tenant_value.clone().unwrap_or_default(),
                name_value,
            )
            .await;

            match result {
                Ok(user) => {
                    if let Some(new_name) = user.name {
                        set_name.set(new_name);
                    }
                    set_form_state.set(FormState::idle());
                    set_success_message.set(Some(t_local("profile.saved", "Profile updated.")));
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

    view! {
        <section class="flex flex-1 flex-col p-4 md:px-6">
            <PageHeader
                title=t_local("profile.title", "Profile & preferences")
                subtitle=t_local("profile.subtitle", "Update your personal details and language preferences.")
                eyebrow=t_local("profile.badge", "Profile")
                actions=view! {
                    <Button on_click=on_save>{t_local("profile.save", "Save changes")}</Button>
                }
                .into_any()
            />

            <div class="grid gap-6 lg:grid-cols-2">
                <div class="grid gap-4 rounded-xl border border-border bg-card p-6 shadow-sm">
                    <h3 class="text-lg font-semibold text-card-foreground">
                        {t_local("profile.sectionTitle", "Profile details")}
                    </h3>
                    <p class="text-sm text-muted-foreground">
                        {t_local("profile.sectionSubtitle", "Keep your admin identity current across tenants.")}
                    </p>
                    <Input
                        value=name
                        set_value=set_name
                        placeholder="Alex Morgan"
                        label=t_local("profile.nameLabel", "Full name")
                    />
                    <div class="flex flex-col gap-2">
                        <label class="text-sm text-muted-foreground">
                            {t_local("profile.emailLabel", "Email")}
                        </label>
                        <p class="rounded-xl border border-input bg-muted px-4 py-3 text-sm text-muted-foreground">
                            {move || email.get()}
                        </p>
                    </div>
                    <Input
                        value=avatar
                        set_value=set_avatar
                        placeholder="https://cdn.rustok.io/avatar.png"
                        label=t_local("profile.avatarLabel", "Avatar URL")
                    />
                    <div class="flex flex-col gap-2">
                        <label class="text-sm text-muted-foreground">
                            {t_local("profile.timezoneLabel", "Timezone")}
                        </label>
                        <Select
                            options=vec![
                                SelectOption::new("Europe/Moscow", "Europe/Moscow"),
                                SelectOption::new("Europe/Berlin", "Europe/Berlin"),
                                SelectOption::new("America/New_York", "America/New_York"),
                                SelectOption::new("Asia/Dubai", "Asia/Dubai"),
                            ]
                            value=timezone
                            set_value=set_timezone
                        />
                    </div>
                    <div class="flex flex-col gap-2">
                        <label class="text-sm text-muted-foreground">
                            {t_local("profile.userLocaleLabel", "User-facing language")}
                        </label>
                        <select
                            class="rounded-xl border border-input bg-background px-4 py-3 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-ring"
                            on:change=move |ev| set_preferred_locale.set(event_target_value(&ev))
                            prop:value=preferred_locale
                        >
                            <option value="ru">{t_local("profile.localeRu", "Russian")}</option>
                            <option value="en">{t_local("profile.localeEn", "English")}</option>
                        </select>
                        <p class="text-sm text-muted-foreground">
                            {t_local("profile.localeHint", "This affects notifications and emails sent to you.")}
                        </p>
                    </div>
                    <Show when=move || form_state.get().form_error.is_some()>
                        <div class="rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                            {move || form_state.get().form_error.unwrap_or_default()}
                        </div>
                    </Show>
                    <Show when=move || success_message.get().is_some()>
                        <div class="rounded-xl bg-emerald-100 border border-emerald-200 px-4 py-2 text-sm text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400">
                            {move || success_message.get().unwrap_or_default()}
                        </div>
                    </Show>
                </div>

                <div class="grid gap-4 rounded-xl border border-border bg-card p-6 shadow-sm">
                    <h3 class="text-lg font-semibold text-card-foreground">
                        {t_local("profile.preferencesTitle", "Security preferences")}
                    </h3>
                    <p class="text-sm text-muted-foreground">
                        {t_local("profile.preferencesSubtitle", "Configure admin-only behavior separately.")}
                    </p>
                    <div class="flex items-center justify-between gap-4 border-b border-border py-3 last:border-b-0">
                        <div>
                            <strong class="text-foreground">{t_local("profile.uiLocaleLabel", "Admin UI language")}</strong>
                            <p class="text-sm text-muted-foreground">
                                {t_local("profile.uiLocaleHint", "Applies to the admin control center interface.")}
                            </p>
                        </div>
                        {language_toggle()}
                    </div>
                    <div class="flex items-center justify-between gap-4 border-b border-border py-3 last:border-b-0">
                        <div>
                            <strong class="text-foreground">{t_local("profile.notificationsTitle", "Security notifications")}</strong>
                            <p class="text-sm text-muted-foreground">
                                {t_local("profile.notificationsHint", "Receive alerts about new logins and password changes.")}
                            </p>
                        </div>
                        <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-1 text-xs text-secondary-foreground">
                            {t_local("profile.notificationsStatus", "Enabled")}
                        </span>
                    </div>
                    <div class="flex items-center justify-between gap-4 border-b border-border py-3 last:border-b-0">
                        <div>
                            <strong class="text-foreground">{t_local("profile.auditTitle", "Audit trail")}</strong>
                            <p class="text-sm text-muted-foreground">
                                {t_local("profile.auditHint", "Download a report of recent authentication activity.")}
                            </p>
                        </div>
                        <Button
                            on_click=Callback::new(move |_| {})
                            class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                        >
                            {t_local("profile.auditAction", "Export log")}
                        </Button>
                    </div>
                </div>
            </div>
        </section>
    }
}
