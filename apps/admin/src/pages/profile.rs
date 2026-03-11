use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_current_user, use_tenant, use_token};
use leptos_hook_form::FormState;
use leptos_ui::{Select, SelectOption};
use serde::{Deserialize, Serialize};

use crate::shared::api::{request, ApiError};
use crate::shared::ui::{Button, Input, LanguageToggle};
use crate::{t_string, use_i18n};

const UPDATE_PROFILE_MUTATION: &str = r#"
mutation UpdateProfile($input: UpdateProfileInput!) {
    updateProfile(input: $input) {
        id
        email
        name
        role
    }
}
"#;

#[derive(Serialize)]
struct UpdateProfileInput {
    input: ProfileData,
}

#[derive(Serialize)]
struct ProfileData {
    name: Option<String>,
}

#[derive(Deserialize)]
struct UpdateProfileResponse {
    #[serde(rename = "updateProfile")]
    update_profile: ProfileUser,
}

#[derive(Deserialize)]
struct ProfileUser {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    email: String,
    name: Option<String>,
    #[allow(dead_code)]
    role: String,
}

#[component]
pub fn Profile() -> impl IntoView {
    let i18n = use_i18n();
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
    let (error, set_error) = signal::<Option<String>>(None);
    let (status, set_status) = signal::<Option<String>>(None);
    let (success_message, set_success_message) = signal(Option::<String>::None);

    let on_save = move |_| {
        let token_value = token.get();
        let tenant_value = tenant.get();
        if token_value.is_none() {
            set_error.set(Some(t_string!(i18n, errors.auth.unauthorized).to_string()));
            set_status.set(None);
            return;
        }

        let name_value = name.get().trim().to_string();

        set_form_state.set(FormState::submitting());
        set_success_message.set(None);

        spawn_local(async move {
            let result = request::<UpdateProfileInput, UpdateProfileResponse>(
                UPDATE_PROFILE_MUTATION,
                UpdateProfileInput {
                    input: ProfileData {
                        name: if name_value.is_empty() {
                            None
                        } else {
                            Some(name_value)
                        },
                    },
                },
                token_value,
                tenant_value,
            )
            .await;

            match result {
                Ok(response) => {
                    if let Some(new_name) = response.update_profile.name {
                        set_name.set(new_name);
                    }
                    set_error.set(None);
                    set_status.set(Some(t_string!(i18n, profile.saved).to_string()));
                }
                Err(err) => {
                    let message = match err {
                        ApiError::Unauthorized => {
                            t_string!(i18n, errors.auth.unauthorized).to_string()
                        }
                        ApiError::Http(_) => t_string!(i18n, errors.http).to_string(),
                        ApiError::Network => t_string!(i18n, errors.network).to_string(),
                        ApiError::Graphql(_) => t_string!(i18n, errors.unknown).to_string(),
                    };
                    set_form_state.set(FormState::with_form_error(message));
                    set_success_message.set(None);
                }
            }
        });
    };

    view! {
        <section class="px-10 py-8">
            <header class="mb-6 flex flex-wrap items-start justify-between gap-4">
                <div>
                    <span class="inline-flex items-center rounded-full bg-secondary px-3 py-1 text-xs font-semibold text-secondary-foreground">
                        {move || t_string!(i18n, profile.badge)}
                    </span>
                    <h1 class="mt-2 text-2xl font-semibold text-foreground">
                        {move || t_string!(i18n, profile.title)}
                    </h1>
                    <p class="mt-2 text-sm text-muted-foreground">
                        {move || t_string!(i18n, profile.subtitle)}
                    </p>
                </div>
                <div class="flex flex-wrap items-center gap-3">
                    <LanguageToggle />
                    <Button on_click=on_save>{move || t_string!(i18n, profile.save)}</Button>
                </div>
            </header>

            <div class="grid gap-6 lg:grid-cols-2">
                <div class="grid gap-4 rounded-2xl bg-card p-6 shadow border border-border">
                    <h3 class="text-lg font-semibold text-card-foreground">
                        {move || t_string!(i18n, profile.sectionTitle)}
                    </h3>
                    <p class="text-sm text-muted-foreground">
                        {move || t_string!(i18n, profile.sectionSubtitle)}
                    </p>
                    <Input
                        value=name
                        set_value=set_name
                        placeholder="Alex Morgan"
                        label=move || t_string!(i18n, profile.nameLabel)
                    />
                    <div class="flex flex-col gap-2">
                        <label class="text-sm text-muted-foreground">
                            {move || t_string!(i18n, profile.emailLabel)}
                        </label>
                        <p class="rounded-xl border border-input bg-muted px-4 py-3 text-sm text-muted-foreground">
                            {move || email.get()}
                        </p>
                    </div>
                    <Input
                        value=avatar
                        set_value=set_avatar
                        placeholder="https://cdn.rustok.io/avatar.png"
                        label=move || t_string!(i18n, profile.avatarLabel)
                    />
                    <div class="flex flex-col gap-2">
                        <label class="text-sm text-muted-foreground">
                            {move || t_string!(i18n, profile.timezoneLabel)}
                        </label>
                        <Select
                            options=vec![
                                SelectOption::new("Europe/Moscow", "Europe/Moscow"),
                                SelectOption::new("Europe/Berlin", "Europe/Berlin"),
                                SelectOption::new("America/New_York", "America/New_York"),
                                SelectOption::new("Asia/Dubai", "Asia/Dubai"),
                            ]
                            value=Some(timezone)
                            set_value=Some(set_timezone)
                        />
                    </div>
                    <div class="flex flex-col gap-2">
                        <label class="text-sm text-muted-foreground">
                            {move || t_string!(i18n, profile.userLocaleLabel)}
                        </label>
                        <select
                            class="rounded-xl border border-input bg-background px-4 py-3 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-ring"
                            on:change=move |ev| set_preferred_locale.set(event_target_value(&ev))
                            prop:value=preferred_locale
                        >
                            <option value="ru">{move || t_string!(i18n, profile.localeRu)}</option>
                            <option value="en">{move || t_string!(i18n, profile.localeEn)}</option>
                        </select>
                        <p class="text-sm text-muted-foreground">
                            {move || t_string!(i18n, profile.localeHint)}
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

                <div class="grid gap-4 rounded-2xl bg-card p-6 shadow border border-border">
                    <h3 class="text-lg font-semibold text-card-foreground">
                        {move || t_string!(i18n, profile.preferencesTitle)}
                    </h3>
                    <p class="text-sm text-muted-foreground">
                        {move || t_string!(i18n, profile.preferencesSubtitle)}
                    </p>
                    <div class="flex items-center justify-between gap-4 border-b border-border py-3 last:border-b-0">
                        <div>
                            <strong class="text-foreground">{move || t_string!(i18n, profile.uiLocaleLabel)}</strong>
                            <p class="text-sm text-muted-foreground">
                                {move || t_string!(i18n, profile.uiLocaleHint)}
                            </p>
                        </div>
                        <LanguageToggle />
                    </div>
                    <div class="flex items-center justify-between gap-4 border-b border-border py-3 last:border-b-0">
                        <div>
                            <strong class="text-foreground">{move || t_string!(i18n, profile.notificationsTitle)}</strong>
                            <p class="text-sm text-muted-foreground">
                                {move || t_string!(i18n, profile.notificationsHint)}
                            </p>
                        </div>
                        <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-1 text-xs text-secondary-foreground">
                            {move || t_string!(i18n, profile.notificationsStatus)}
                        </span>
                    </div>
                    <div class="flex items-center justify-between gap-4 border-b border-border py-3 last:border-b-0">
                        <div>
                            <strong class="text-foreground">{move || t_string!(i18n, profile.auditTitle)}</strong>
                            <p class="text-sm text-muted-foreground">
                                {move || t_string!(i18n, profile.auditHint)}
                            </p>
                        </div>
                        <Button
                            on_click=move |_| {}
                            class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                        >
                            {move || t_string!(i18n, profile.auditAction)}
                        </Button>
                    </div>
                </div>
            </div>
        </section>
    }
}
