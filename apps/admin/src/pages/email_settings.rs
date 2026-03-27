use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::api::queries::{PLATFORM_SETTINGS_QUERY, UPDATE_PLATFORM_SETTINGS_MUTATION};
use crate::shared::api::request;
use crate::shared::ui::{Alert, AlertVariant, Button, Input, PageHeader};
use crate::{t_string, use_i18n};

#[derive(Clone, Debug, Serialize)]
struct PlatformSettingsVariables {
    category: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlatformSettingsResponse {
    #[serde(rename = "platformSettings")]
    platform_settings: PlatformSettingsPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlatformSettingsPayload {
    settings: String,
}

#[derive(Clone, Debug, Serialize)]
struct UpdateSettingsVariables {
    input: UpdateSettingsInput,
}

#[derive(Clone, Debug, Serialize)]
struct UpdateSettingsInput {
    category: String,
    settings: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct UpdateSettingsResponse {
    #[serde(rename = "updatePlatformSettings")]
    update_platform_settings: UpdateSettingsPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct UpdateSettingsPayload {
    success: bool,
}

#[component]
pub fn EmailSettingsPage() -> impl IntoView {
    let i18n = use_i18n();
    let token = use_token();
    let tenant = use_tenant();

    let (smtp_host, set_smtp_host) = signal(String::new());
    let (smtp_port, set_smtp_port) = signal(String::new());
    let (smtp_username, set_smtp_username) = signal(String::new());
    let (from_address, set_from_address) = signal(String::new());
    let (saving, set_saving) = signal(false);
    let (save_result, set_save_result) = signal(Option::<Result<bool, String>>::None);
    let (loaded, set_loaded) = signal(false);

    let settings_resource = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            request::<PlatformSettingsVariables, PlatformSettingsResponse>(
                PLATFORM_SETTINGS_QUERY,
                PlatformSettingsVariables {
                    category: "email".to_string(),
                },
                token_value,
                tenant_value,
            )
            .await
        },
    );

    Effect::new(move |_| {
        if let Some(Ok(response)) = settings_resource.get() {
            if !loaded.get_untracked() {
                if let Ok(val) = serde_json::from_str::<Value>(&response.platform_settings.settings)
                {
                    if let Some(s) = val.get("smtp_host").and_then(|v| v.as_str()) {
                        set_smtp_host.set(s.to_string());
                    }
                    if let Some(p) = val.get("smtp_port").and_then(|v| v.as_u64()) {
                        set_smtp_port.set(p.to_string());
                    }
                    if let Some(u) = val.get("smtp_username").and_then(|v| v.as_str()) {
                        set_smtp_username.set(u.to_string());
                    }
                    if let Some(f) = val.get("from_address").and_then(|v| v.as_str()) {
                        set_from_address.set(f.to_string());
                    }
                }
                set_loaded.set(true);
            }
        }
    });

    let save = {
        move |_| {
            let token_val = token.get();
            let tenant_val = tenant.get();
            let host = smtp_host.get();
            let port = smtp_port.get();
            let username = smtp_username.get();
            let from = from_address.get();

            let port_num: u16 = port.parse().unwrap_or(587);
            let settings = serde_json::json!({
                "smtp_host": host,
                "smtp_port": port_num,
                "smtp_username": username,
                "from_address": from,
            });

            set_saving.set(true);
            set_save_result.set(None);

            spawn_local(async move {
                let result = request::<UpdateSettingsVariables, UpdateSettingsResponse>(
                    UPDATE_PLATFORM_SETTINGS_MUTATION,
                    UpdateSettingsVariables {
                        input: UpdateSettingsInput {
                            category: "email".to_string(),
                            settings: settings.to_string(),
                        },
                    },
                    token_val,
                    tenant_val,
                )
                .await;

                match result {
                    Ok(r) => set_save_result.set(Some(Ok(r.update_platform_settings.success))),
                    Err(e) => set_save_result.set(Some(Err(format!("{:?}", e)))),
                }
                set_saving.set(false);
            });
        }
    };

    view! {
        <section class="px-10 py-8">
            <PageHeader
                title=t_string!(i18n, email.title)
                subtitle=t_string!(i18n, email.subtitle).to_string()
                eyebrow=t_string!(i18n, email.eyebrow).to_string()
                actions=view! { <div /> }.into_any()
            />

            <div class="rounded-2xl bg-card p-6 shadow border border-border max-w-xl">
                <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                    {move || t_string!(i18n, email.smtp.title)}
                </h4>

                <Suspense fallback=move || view! {
                    <div class="space-y-4">
                        {(0..4).map(|_| view! {
                            <div class="h-10 animate-pulse rounded-lg bg-muted" />
                        }).collect_view()}
                    </div>
                }>
                    {move || {
                        let _ = settings_resource.get();
                        view! {
                            <div class="space-y-4">
                                <Input
                                    value=smtp_host
                                    set_value=set_smtp_host
                                    placeholder="smtp.example.com"
                                    label=move || t_string!(i18n, email.smtp.host)
                                />
                                <Input
                                    value=smtp_port
                                    set_value=set_smtp_port
                                    placeholder="587"
                                    label=move || t_string!(i18n, email.smtp.port)
                                />
                                <Input
                                    value=smtp_username
                                    set_value=set_smtp_username
                                    placeholder="noreply@example.com"
                                    label=move || t_string!(i18n, email.smtp.username)
                                />
                                <Input
                                    value=from_address
                                    set_value=set_from_address
                                    placeholder="noreply@example.com"
                                    label=move || t_string!(i18n, email.smtp.fromAddress)
                                />

                                <Show when=move || save_result.get().is_some()>
                                    {move || match save_result.get() {
                                        Some(Ok(true)) => view! {
                                            <Alert variant=AlertVariant::Success>
                                                {t_string!(i18n, email.saved)}
                                            </Alert>
                                        }.into_any(),
                                        Some(Err(e)) => view! {
                                            <Alert variant=AlertVariant::Destructive>
                                                {e}
                                            </Alert>
                                        }.into_any(),
                                        _ => view! { <div /> }.into_any(),
                                    }}
                                </Show>

                                <Button on_click=save disabled=saving.into()>
                                    {move || if saving.get() {
                                        t_string!(i18n, email.saving).to_string()
                                    } else {
                                        t_string!(i18n, email.save).to_string()
                                    }}
                                </Button>
                            </div>
                        }.into_any()
                    }}
                </Suspense>
            </div>
        </section>
    }
}
