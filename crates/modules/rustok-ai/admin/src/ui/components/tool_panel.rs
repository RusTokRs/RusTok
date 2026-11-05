use crate::i18n::t;
use crate::model::AiToolProfilePayload;
use crate::ui::leptos::{Card, TextField, tool_profile_summary};
use leptos::ev::{MouseEvent, SubmitEvent};
use leptos::prelude::*;
use rustok_ui_core::AdminQueryKey;

#[component]
pub fn AiToolPanel(
    ui_locale: Option<String>,
    tool_profiles: Vec<AiToolProfilePayload>,
    tool_slug: RwSignal<String>,
    tool_name: RwSignal<String>,
    tool_description: RwSignal<String>,
    tool_allowed: RwSignal<String>,
    tool_denied: RwSignal<String>,
    tool_sensitive: RwSignal<String>,
    tool_active: RwSignal<bool>,
    on_create_tool_profile: Callback<SubmitEvent>,
    on_update_tool_profile: Callback<MouseEvent>,
    on_reset: Callback<()>,
    select_tool_query_writer: leptos_ui_routing::RouteQueryWriter,
) -> impl IntoView {
    let ui_locale_tools = ui_locale.clone();
    let select_tool_query_writer = select_tool_query_writer.clone();

    view! {
        <Card title=t(ui_locale.as_deref(), "ai.card.toolProfiles", "Tool Profiles")>
            <form class="space-y-3" on:submit=move |ev| on_create_tool_profile.run(ev)>
                <TextField label=t(ui_locale.as_deref(), "ai.field.slug", "Slug") value=tool_slug />
                <TextField label=t(ui_locale.as_deref(), "ai.field.displayName", "Display name") value=tool_name />
                <TextField label=t(ui_locale.as_deref(), "ai.field.description", "Description") value=tool_description />
                <TextField label=t(ui_locale.as_deref(), "ai.field.allowedToolsCsv", "Allowed tools (csv)") value=tool_allowed />
                <TextField label=t(ui_locale.as_deref(), "ai.field.deniedToolsCsv", "Denied tools (csv)") value=tool_denied />
                <TextField label=t(ui_locale.as_deref(), "ai.field.sensitiveToolsCsv", "Sensitive tools (csv)") value=tool_sensitive />
                <label class="flex items-center gap-2 text-sm text-muted-foreground">
                    <input
                        type="checkbox"
                        prop:checked=tool_active
                        on:change=move |ev| tool_active.set(event_target_checked(&ev))
                    />
                    {t(ui_locale.as_deref(), "ai.field.active", "Active")}
                </label>
                <div class="flex flex-wrap gap-2">
                    <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale.as_deref(), "ai.action.createToolProfile", "Create tool profile")}</button>
                    <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=move |ev| on_update_tool_profile.run(ev)>{t(ui_locale.as_deref(), "ai.action.updateSelected", "Update selected")}</button>
                    <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=move |_| on_reset.run(())>{t(ui_locale.as_deref(), "ai.action.reset", "Reset")}</button>
                </div>
            </form>
            <div class="mt-4 space-y-2">
                {tool_profiles.into_iter().map(|profile| {
                    let profile_slug_value = profile.slug.clone();
                    let tool_query_writer = select_tool_query_writer.clone();
                    view! {
                        <button
                            class="w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted"
                            on:click=move |_| {
                                tool_query_writer.replace_value(
                                    AdminQueryKey::ToolProfileSlug.as_str(),
                                    profile_slug_value.clone(),
                                );
                            }
                        >
                            <div class="font-medium">{profile.display_name}</div>
                            <div class="text-muted-foreground">
                                {tool_profile_summary(
                                    ui_locale_tools.as_deref(),
                                    profile.allowed_tools.len(),
                                    profile.sensitive_tools.len(),
                                    profile.is_active,
                                )}
                            </div>
                        </button>
                    }
                }).collect_view()}
            </div>
        </Card>
    }
}
