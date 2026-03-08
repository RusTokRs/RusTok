use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_auth, use_tenant, use_token};
use serde::{Deserialize, Serialize};

use crate::shared::api::{request, ApiError};
use crate::shared::ui::{Button, Input};
use crate::{t_string, use_i18n};

const CHANGE_PASSWORD_MUTATION: &str = r#"
mutation ChangePassword($input: ChangePasswordInput!) {
    changePassword(input: $input) {
        success
    }
}
"#;

#[derive(Serialize)]
struct ChangePasswordVariables {
    input: ChangePasswordInput,
}

#[derive(Serialize)]
struct ChangePasswordInput {
    #[serde(rename = "currentPassword")]
    current_password: String,
    #[serde(rename = "newPassword")]
    new_password: String,
}

#[derive(Deserialize)]
struct ChangePasswordResponse {
    #[serde(rename = "changePassword")]
    _change_password: SuccessPayload,
}

#[derive(Deserialize)]
struct SuccessPayload {
    #[allow(dead_code)]
    success: bool,
}

#[component]
pub fn Security() -> impl IntoView {
    let i18n = use_i18n();
    let auth = use_auth();
    let token = use_token();
    let tenant = use_tenant();

    let (current_password, set_current_password) = signal(String::new());
    let (new_password, set_new_password) = signal(String::new());
    let (status, set_status) = signal(Option::<String>::None);
    let (error, set_error) = signal(Option::<String>::None);

    let on_change_password = move |_| {
        if current_password.get().is_empty() || new_password.get().is_empty() {
            set_error.set(Some(t_string!(i18n, security.passwordRequired).to_string()));
            set_status.set(None);
            return;
        }

        let token_value = token.get();
        let tenant_value = tenant.get();
        if token_value.is_none() {
            set_error.set(Some(t_string!(i18n, errors.auth.unauthorized).to_string()));
            set_status.set(None);
            return;
        }

        let current_password_value = current_password.get();
        let new_password_value = new_password.get();

        spawn_local(async move {
            let result = request::<ChangePasswordVariables, ChangePasswordResponse>(
                CHANGE_PASSWORD_MUTATION,
                ChangePasswordVariables {
                    input: ChangePasswordInput {
                        current_password: current_password_value,
                        new_password: new_password_value,
                    },
                },
                token_value,
                tenant_value,
            )
            .await;

            match result {
                Ok(_) => {
                    set_error.set(None);
                    set_status.set(Some(t_string!(i18n, security.passwordUpdated).to_string()));
                    set_current_password.set(String::new());
                    set_new_password.set(String::new());
                }
                Err(err) => {
                    let message = match err {
                        ApiError::Unauthorized => t_string!(i18n, errors.auth.unauthorized).to_string(),
                        ApiError::Http(_) => t_string!(i18n, errors.http).to_string(),
                        ApiError::Network => t_string!(i18n, errors.network).to_string(),
                        ApiError::Graphql(_) => t_string!(i18n, errors.unknown).to_string(),
                    };
                    set_error.set(Some(message));
                    set_status.set(None);
                }
            }
        });
    };

    let on_sign_out_all = move |_: leptos::ev::MouseEvent| {
        let auth = auth.clone();
        spawn_local(async move {
            let _ = auth.sign_out().await;
        });
    };

    view! {
        <section class="px-10 py-8">
            <header class="mb-6 flex flex-wrap items-start justify-between gap-4">
                <div>
                    <span class="inline-flex items-center rounded-full border bg-secondary px-3 py-1 text-xs font-semibold text-secondary-foreground">
                        {move || t_string!(i18n, security.badge)}
                    </span>
                    <h1 class="mt-2 text-2xl font-semibold text-foreground">
                        {move || t_string!(i18n, security.title)}
                    </h1>
                    <p class="mt-2 text-sm text-muted-foreground">
                        {move || t_string!(i18n, security.subtitle)}
                    </p>
                </div>
                <div class="flex flex-wrap items-center gap-3">
                    <Button
                        on_click=on_sign_out_all
                        class="border border-border bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                    >
                        {move || t_string!(i18n, security.signOutAll)}
                    </Button>
                </div>
            </header>

            <div class="grid gap-6 lg:grid-cols-2">
                <div class="grid gap-4 rounded-xl border border-border bg-card p-6 shadow-sm">
                    <h3 class="text-lg font-semibold text-card-foreground">
                        {move || t_string!(i18n, security.passwordTitle)}
                    </h3>
                    <p class="text-sm text-muted-foreground">
                        {move || t_string!(i18n, security.passwordSubtitle)}
                    </p>
                    <Input
                        value=current_password
                        set_value=set_current_password
                        placeholder="••••••••"
                        type_="password"
                        label=move || t_string!(i18n, security.currentPasswordLabel)
                    />
                    <Input
                        value=new_password
                        set_value=set_new_password
                        placeholder="••••••••"
                        type_="password"
                        label=move || t_string!(i18n, security.newPasswordLabel)
                    />
                    <p class="text-sm text-muted-foreground">
                        {move || t_string!(i18n, security.passwordHint)}
                    </p>
                    <Button on_click=on_change_password class="w-full">
                        {move || t_string!(i18n, security.passwordSubmit)}
                    </Button>
                    <Show when=move || error.get().is_some()>
                        <div class="rounded-md bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                            {move || error.get().unwrap_or_default()}
                        </div>
                    </Show>
                    <Show when=move || status.get().is_some()>
                        <div class="rounded-md bg-emerald-100 border border-emerald-200 px-4 py-2 text-sm text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400">
                            {move || status.get().unwrap_or_default()}
                        </div>
                    </Show>
                </div>

                <div class="grid gap-4 rounded-xl border border-border bg-card p-6 shadow-sm">
                    <h3 class="text-lg font-semibold text-card-foreground">
                        {move || t_string!(i18n, security.sessionsTitle)}
                    </h3>
                    <p class="text-sm text-muted-foreground">
                        {move || t_string!(i18n, security.sessionsSubtitle)}
                    </p>
                    <div class="rounded-lg bg-muted px-4 py-8 text-center text-sm text-muted-foreground">
                        "Session management via GraphQL — coming soon"
                    </div>
                </div>
            </div>
        </section>
    }
}
