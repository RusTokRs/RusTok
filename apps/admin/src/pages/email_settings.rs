use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_auth_ui::ui::leptos::hooks::{use_tenant, use_token};
use serde_json::Value;

use crate::features::email::transport;
use crate::shared::ui::{Alert, AlertVariant, Button, Input, PageHeader};
use crate::use_admin_locale;

/// The settings the backend holds, decoded once so the form can render them.
#[derive(Clone, Debug, Default)]
struct ResolvedEmailSettings {
    smtp_host: String,
    smtp_port: String,
    smtp_username: String,
    from_address: String,
}

impl ResolvedEmailSettings {
    /// Reads the stored `settings` document. Absent keys stay empty strings: the form shows what the
    /// backend has, not a default that would be saved back on the next edit.
    fn from_payload(settings: &str, schema_version: i32) -> (Self, i32) {
        let value: Value = serde_json::from_str(settings).unwrap_or(Value::Null);
        let text = |key: &str| {
            value
                .get(key)
                .and_then(|value| match value {
                    Value::String(text) => Some(text.clone()),
                    Value::Number(number) => Some(number.to_string()),
                    _ => None,
                })
                .unwrap_or_default()
        };
        (
            Self {
                smtp_host: text("smtp_host"),
                smtp_port: text("smtp_port"),
                smtp_username: text("smtp_username"),
                from_address: text("from_address"),
            },
            schema_version,
        )
    }
}

/// The fields the user has changed. `None` means "unchanged", so the resolved value is shown.
#[derive(Clone, Debug, Default)]
struct EmailSettingsEdits {
    smtp_host: Option<String>,
    smtp_port: Option<String>,
    smtp_username: Option<String>,
    from_address: Option<String>,
}

#[component]
pub fn EmailSettingsPage() -> impl IntoView {
    let i18n = use_admin_locale();
    let token = use_token();
    let tenant = use_tenant();

    // A `Resource`, not a `LocalResource`: the page is served `Async` inside the admin layout, and
    // a `LocalResource` is `pending()` on the server, so the form arrived empty.
    let settings_resource = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            transport::fetch_email_settings(token_value, tenant_value).await
        },
    );

    // The form is the resolved settings plus the fields the user has changed, not a set of cells
    // that an `Effect` fills after the first render. An `Effect` runs on a later tick and is not
    // driven before the server produces the response, so a form built that way is always empty in
    // the HTML, and a field the user has not touched keeps a value no reader ever set.
    let edits = RwSignal::new(EmailSettingsEdits::default());
    let (saving, set_saving) = signal(false);
    let (save_result, set_save_result) = signal(Option::<Result<bool, String>>::None);
    let (schema_version, set_schema_version) = signal(0_i32);

    // The backend's values are derived from the resolved response, not written into cells.
    let resolved = move || {
        settings_resource
            .get()
            .and_then(Result::ok)
            .map(|response| {
                ResolvedEmailSettings::from_payload(
                    &response.platform_settings.settings,
                    response.platform_settings.schema_version,
                )
                .0
            })
    };
    // `Copy`, not `move`: the four inputs each capture it, and a moved closure would be `FnOnce`.
    let field = |read: fn(&EmailSettingsEdits) -> Option<String>,
                 backend: fn(&ResolvedEmailSettings) -> String| {
        let edited = edits.get();
        read(&edited).or_else(|| resolved().map(|settings| backend(&settings)))
    };

    let save = move || {
        let token_val = token.get();
        let tenant_val = tenant.get();
        let edited = edits.get();
        let backend = resolved();
        let effective = |read: fn(&EmailSettingsEdits) -> Option<String>,
                         decode: fn(&ResolvedEmailSettings) -> String| {
            read(&edited).or_else(|| backend.as_ref().map(decode))
        };
        let host = effective(|e| e.smtp_host.clone(), |s| s.smtp_host.clone());
        let port = effective(|e| e.smtp_port.clone(), |s| s.smtp_port.clone());
        let username = effective(|e| e.smtp_username.clone(), |s| s.smtp_username.clone());
        let from = effective(|e| e.from_address.clone(), |s| s.from_address.clone());

        let port_num: u16 = port.parse().unwrap_or(587);
        let settings = serde_json::json!({
            "smtp_host": host,
            "smtp_port": port_num,
            "smtp_username": username,
            "from_address": from,
        });

        let expected_schema_version = schema_version.get();
        set_saving.set(true);
        set_save_result.set(None);

        spawn_local(async move {
            let result = transport::update_email_settings(
                token_val,
                tenant_val,
                settings.to_string(),
                expected_schema_version,
            )
            .await;

            match result {
                Ok(response) => {
                    set_schema_version.set(response.update_platform_settings.schema_version);
                    set_save_result.set(Some(Ok(response.update_platform_settings.success)));
                }
                Err(error) => set_save_result.set(Some(Err(error))),
            }
            set_saving.set(false);
        });
    };

    view! {
        <section class="flex flex-1 flex-col p-4 md:px-6">
            <PageHeader
                title=i18n.translate("email.title")
                subtitle=i18n.translate("email.subtitle").to_string()
                eyebrow=i18n.translate("email.eyebrow").to_string()
            />

            <div class="rounded-xl border border-border bg-card p-6 shadow-sm max-w-xl">
                <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                    {move || i18n.translate("email.smtp.title")}
                </h4>

                <Suspense fallback=move || view! {
                    <div class="space-y-4">
                        {(0..4).map(|_| view! {
                            <div class="h-10 animate-pulse rounded-lg bg-muted" />
                        }).collect_view()}
                    </div>
                }>
                    {move || {
                        // The render reads the resource, so the form is part of the response; a
                        // failed load says so instead of showing an editable empty form.
                        match settings_resource.get() {
                            None => view! { <div /> }.into_any(),
                            Some(Err(error)) => view! {
                                <Alert variant=AlertVariant::Destructive>{error}</Alert>
                            }
                            .into_any(),
                            Some(Ok(response)) => {
                                let (_, backend_schema_version) =
                                    ResolvedEmailSettings::from_payload(
                                        &response.platform_settings.settings,
                                        response.platform_settings.schema_version,
                                    );
                                // The save sends the revision it read, so a concurrent change is
                                // rejected instead of overwritten.
                                if schema_version.get_untracked() != backend_schema_version {
                                    set_schema_version.set(backend_schema_version);
                                }
                                view! {
                            <div class="space-y-4">
                                <Input
                                    value=Signal::derive(move || {
                                        field(
                                            |edits: &EmailSettingsEdits| edits.smtp_host.clone(),
                                            |settings: &ResolvedEmailSettings| settings.smtp_host.clone(),
                                        )
                                        .unwrap_or_default()
                                    })
                                    set_value=SignalSetter::map(move |value: String| {
                                        edits.update(|edits| edits.smtp_host = Some(value));
                                    })
                                    placeholder="smtp.example.com"
                                    label=move || i18n.translate("email.smtp.host")
                                />
                                <Input
                                    value=Signal::derive(move || {
                                        field(
                                            |edits: &EmailSettingsEdits| edits.smtp_port.clone(),
                                            |settings: &ResolvedEmailSettings| settings.smtp_port.clone(),
                                        )
                                        .unwrap_or_default()
                                    })
                                    set_value=SignalSetter::map(move |value: String| {
                                        edits.update(|edits| edits.smtp_port = Some(value));
                                    })
                                    placeholder="587"
                                    label=move || i18n.translate("email.smtp.port")
                                />
                                <Input
                                    value=Signal::derive(move || {
                                        field(
                                            |edits: &EmailSettingsEdits| edits.smtp_username.clone(),
                                            |settings: &ResolvedEmailSettings| settings.smtp_username.clone(),
                                        )
                                        .unwrap_or_default()
                                    })
                                    set_value=SignalSetter::map(move |value: String| {
                                        edits.update(|edits| edits.smtp_username = Some(value));
                                    })
                                    placeholder="noreply@example.com"
                                    label=move || i18n.translate("email.smtp.username")
                                />
                                <Input
                                    value=Signal::derive(move || {
                                        field(
                                            |edits: &EmailSettingsEdits| edits.from_address.clone(),
                                            |settings: &ResolvedEmailSettings| settings.from_address.clone(),
                                        )
                                        .unwrap_or_default()
                                    })
                                    set_value=SignalSetter::map(move |value: String| {
                                        edits.update(|edits| edits.from_address = Some(value));
                                    })
                                    placeholder="noreply@example.com"
                                    label=move || i18n.translate("email.smtp.fromAddress")
                                />

                                <Show when=move || save_result.get().is_some()>
                                    {move || match save_result.get() {
                                        Some(Ok(true)) => view! {
                                            <Alert variant=AlertVariant::Success>
                                                {i18n.translate("email.saved")}
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

                                <Button on_click=Callback::new(move |_| save()) disabled=Signal::derive(move || saving.get())>
                                    {move || if saving.get() {
                                        i18n.translate("email.saving").to_string()
                                    } else {
                                        i18n.translate("email.save").to_string()
                                    }}
                                </Button>
                            </div>
                            }
                            .into_any()
                            }
                        }
                    }}
                </Suspense>
            </div>
        </section>
    }
}
