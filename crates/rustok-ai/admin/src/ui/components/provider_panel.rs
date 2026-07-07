use crate::i18n::t;
use crate::model::AiProviderProfilePayload;
use crate::ui::leptos::{provider_profile_summary, Card, TextField};
use leptos::ev::{MouseEvent, SubmitEvent};
use leptos::prelude::*;
use rustok_ui_core::AdminQueryKey;

#[component]
pub fn AiProviderPanel(
    ui_locale: Option<String>,
    providers: Vec<AiProviderProfilePayload>,
    provider_slug: RwSignal<String>,
    provider_name: RwSignal<String>,
    provider_kind: RwSignal<String>,
    provider_base_url: RwSignal<String>,
    provider_model: RwSignal<String>,
    provider_api_key: RwSignal<String>,
    provider_temperature: RwSignal<String>,
    provider_max_tokens: RwSignal<String>,
    provider_capabilities: RwSignal<String>,
    provider_allowed_tasks: RwSignal<String>,
    provider_denied_tasks: RwSignal<String>,
    provider_restricted_roles: RwSignal<String>,
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

    view! {
        <Card title=t(ui_locale.as_deref(), "ai.card.providers", "Providers")>
            <form class="space-y-3" on:submit=move |ev| on_create_provider.run(ev)>
                <TextField label=t(ui_locale.as_deref(), "ai.field.slug", "Slug") value=provider_slug />
                <TextField label=t(ui_locale.as_deref(), "ai.field.displayName", "Display name") value=provider_name />
                <TextField label=t(ui_locale.as_deref(), "ai.field.providerKind", "Provider kind") value=provider_kind />
                <TextField label=t(ui_locale.as_deref(), "ai.field.baseUrl", "Base URL") value=provider_base_url />
                <TextField label=t(ui_locale.as_deref(), "ai.field.model", "Model") value=provider_model />
                <TextField label=t(ui_locale.as_deref(), "ai.field.apiKey", "API key") value=provider_api_key />
                <TextField label=t(ui_locale.as_deref(), "ai.field.temperature", "Temperature") value=provider_temperature />
                <TextField label=t(ui_locale.as_deref(), "ai.field.maxTokens", "Max tokens") value=provider_max_tokens />
                <TextField label=t(ui_locale.as_deref(), "ai.field.capabilitiesCsv", "Capabilities (csv)") value=provider_capabilities />
                <TextField label=t(ui_locale.as_deref(), "ai.field.allowedTasksCsv", "Allowed tasks (csv)") value=provider_allowed_tasks />
                <TextField label=t(ui_locale.as_deref(), "ai.field.deniedTasksCsv", "Denied tasks (csv)") value=provider_denied_tasks />
                <TextField label=t(ui_locale.as_deref(), "ai.field.restrictedRolesCsv", "Restricted roles (csv)") value=provider_restricted_roles />
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
                                    provider.provider_kind.as_str(),
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
