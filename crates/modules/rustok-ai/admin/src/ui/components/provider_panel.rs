use crate::i18n::t;
use crate::model::{
    AiCredentialRefPayload, AiProviderCatalogEntryPayload, AiProviderProfilePayload,
    AiProviderTargetPayload,
};
use crate::ui::leptos::{Card, TextField, provider_profile_summary};
use leptos::ev::{MouseEvent, SubmitEvent};
use leptos::prelude::*;
use rustok_ui_core::AdminQueryKey;

#[component]
pub fn AiProviderPanel(
    ui_locale: Option<String>,
    provider_catalog: Vec<AiProviderCatalogEntryPayload>,
    provider_targets: Vec<AiProviderTargetPayload>,
    providers: Vec<AiProviderProfilePayload>,
    provider_slug: RwSignal<String>,
    provider_name: RwSignal<String>,
    provider_integration: RwSignal<String>,
    provider_credential_refs: RwSignal<Vec<AiCredentialRefPayload>>,
    provider_model: RwSignal<String>,
    provider_temperature: RwSignal<String>,
    provider_max_tokens: RwSignal<String>,
    provider_capabilities: RwSignal<String>,
    provider_allowed_tasks: RwSignal<String>,
    provider_denied_tasks: RwSignal<String>,
    provider_active: RwSignal<bool>,
    on_create_provider: Callback<SubmitEvent>,
    on_update_provider: Callback<MouseEvent>,
    on_test_provider: Callback<MouseEvent>,
    on_deactivate_provider: Callback<MouseEvent>,
    on_reset: Callback<()>,
    select_provider_query_writer: leptos_ui_routing::RouteQueryWriter,
) -> impl IntoView {
    let ui_locale_providers = ui_locale.clone();
    let select_provider_query_writer = select_provider_query_writer.clone();
    let credentials_catalog = provider_catalog.clone();
    let credentials_targets = provider_targets.clone();

    view! {
        <Card title=t(ui_locale.as_deref(), "ai.card.providers", "Providers")>
            <form class="space-y-3" on:submit=move |ev| on_create_provider.run(ev)>
                <TextField label=t(ui_locale.as_deref(), "ai.field.slug", "Slug") value=provider_slug />
                <TextField label=t(ui_locale.as_deref(), "ai.field.displayName", "Display name") value=provider_name />
                <label class="grid gap-1 text-sm font-medium">
                    <span>{t(ui_locale.as_deref(), "ai.field.providerIntegration", "Deployment target")}</span>
                    <select
                        class="rounded-lg border border-border bg-background px-3 py-2"
                        prop:value=move || provider_integration.get()
                        on:change=move |ev| {
                            provider_integration.set(event_target_value(&ev));
                            provider_credential_refs.set(Vec::new());
                        }
                    >
                        <option value="">{"Select a deployment target"}</option>
                        {provider_targets.into_iter().map(|target| {
                            view! { <option value=target.id>{target.display_name}</option> }
                        }).collect_view()}
                    </select>
                </label>
                <TextField label=t(ui_locale.as_deref(), "ai.field.model", "Model") value=provider_model />
                <For
                    each=move || selected_credential_schema(
                        &credentials_catalog,
                        selected_target_provider_slug(&credentials_targets, &provider_integration.get()).as_str(),
                    )
                    key=|field| field.key.clone()
                    children=move |field| {
                        let key_for_resolver = field.key.clone();
                        let key_for_secret = field.key.clone();
                        let value_key = field.key.clone();
                        let secret_update_key = field.key.clone();
                        let label = field.label;
                        view! {
                            <div class="grid gap-2 rounded-lg border border-border p-3">
                                <div class="text-sm font-medium">{label}</div>
                                <input
                                    class="rounded-lg border border-border bg-background px-3 py-2"
                                    placeholder="Resolver alias"
                                    prop:value=move || credential_resolver(&provider_credential_refs.get(), &key_for_resolver)
                                    on:input=move |ev| update_credential_ref(
                                        provider_credential_refs,
                                        value_key.clone(),
                                        Some(event_target_value(&ev)),
                                        None,
                                    )
                                />
                                <input
                                    class="rounded-lg border border-border bg-background px-3 py-2"
                                    type="password"
                                    placeholder="External secret key"
                                    autocomplete="off"
                                    prop:value=move || credential_key(&provider_credential_refs.get(), &key_for_secret)
                                    on:input=move |ev| update_credential_ref(
                                        provider_credential_refs,
                                        secret_update_key.clone(),
                                        None,
                                        Some(event_target_value(&ev)),
                                    )
                                />
                            </div>
                        }
                    }
                />
                <TextField label=t(ui_locale.as_deref(), "ai.field.temperature", "Temperature") value=provider_temperature />
                <TextField label=t(ui_locale.as_deref(), "ai.field.maxTokens", "Max tokens") value=provider_max_tokens />
                <TextField label=t(ui_locale.as_deref(), "ai.field.capabilitiesCsv", "Capabilities (csv)") value=provider_capabilities />
                <TextField label=t(ui_locale.as_deref(), "ai.field.allowedTasksCsv", "Allowed tasks (csv)") value=provider_allowed_tasks />
                <TextField label=t(ui_locale.as_deref(), "ai.field.deniedTasksCsv", "Denied tasks (csv)") value=provider_denied_tasks />
                <label class="flex items-center gap-2 text-sm text-muted-foreground">
                    <input
                        type="checkbox"
                        prop:checked=provider_active
                        on:change=move |ev| provider_active.set(event_target_checked(&ev))
                    />
                    {t(ui_locale.as_deref(), "ai.field.active", "Active")}
                </label>
                <div class="flex flex-wrap gap-2">
                    <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale.as_deref(), "ai.action.createProvider", "Create provider")}</button>
                    <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=move |ev| on_update_provider.run(ev)>{t(ui_locale.as_deref(), "ai.action.updateSelected", "Update selected")}</button>
                    <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=move |ev| on_test_provider.run(ev)>{t(ui_locale.as_deref(), "ai.action.testSelected", "Test selected")}</button>
                    <button type="button" class="rounded-lg border border-destructive/40 px-4 py-2 text-sm font-medium text-destructive" on:click=move |ev| on_deactivate_provider.run(ev)>{t(ui_locale.as_deref(), "ai.action.deactivate", "Deactivate")}</button>
                    <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=move |_| on_reset.run(())>{t(ui_locale.as_deref(), "ai.action.reset", "Reset")}</button>
                </div>
            </form>
            <div class="mt-4 space-y-2">
                {providers.into_iter().map(|provider| {
                    let provider_slug_value = provider.slug.clone();
                    let provider_query_writer = select_provider_query_writer.clone();
                    view! {
                        <button
                            class="w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted"
                            on:click=move |_| {
                                provider_query_writer.replace_value(
                                    AdminQueryKey::ProviderSlug.as_str(),
                                    provider_slug_value.clone(),
                                );
                            }
                        >
                            <div class="font-medium">{provider.display_name}</div>
                            <div class="text-muted-foreground">
                                {provider_profile_summary(
                                    ui_locale_providers.as_deref(),
                                    provider.provider_slug.as_str(),
                                    provider.model.as_str(),
                                    provider.capabilities.len(),
                                    provider.is_active,
                                )}
                            </div>
                        </button>
                    }
                }).collect_view()}
            </div>
        </Card>
    }
}

fn selected_credential_schema(
    catalog: &[AiProviderCatalogEntryPayload],
    slug: &str,
) -> Vec<crate::model::AiProviderFieldPayload> {
    catalog
        .iter()
        .find(|entry| entry.slug == slug)
        .map(|entry| entry.credential_schema.clone())
        .unwrap_or_default()
}

fn selected_target_provider_slug(targets: &[AiProviderTargetPayload], id: &str) -> String {
    targets
        .iter()
        .find(|target| target.id == id)
        .map(|target| target.provider_slug.clone())
        .unwrap_or_default()
}

fn credential_resolver(values: &[AiCredentialRefPayload], key: &str) -> String {
    values
        .iter()
        .find(|value| value.key == key)
        .map(|value| value.resolver.clone())
        .unwrap_or_default()
}

fn credential_key(values: &[AiCredentialRefPayload], key: &str) -> String {
    values
        .iter()
        .find(|value| value.key == key)
        .map(|value| value.secret_key.clone())
        .unwrap_or_default()
}

fn update_credential_ref(
    refs: RwSignal<Vec<AiCredentialRefPayload>>,
    key: String,
    resolver: Option<String>,
    secret_key: Option<String>,
) {
    refs.update(|values| {
        if let Some(value) = values.iter_mut().find(|value| value.key == key) {
            if let Some(resolver) = resolver {
                value.resolver = resolver;
            }
            if let Some(secret_key) = secret_key {
                value.secret_key = secret_key;
            }
        } else {
            values.push(AiCredentialRefPayload {
                key,
                resolver: resolver.unwrap_or_default(),
                secret_key: secret_key.unwrap_or_default(),
            });
        }
    });
}
