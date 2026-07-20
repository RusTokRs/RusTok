use crate::i18n::t;
use crate::model::AiTaskProfilePayload;
use crate::ui::leptos::{Card, TextField, task_profile_summary};
use leptos::ev::{MouseEvent, SubmitEvent};
use leptos::prelude::*;
use rustok_ui_core::AdminQueryKey;

#[component]
pub fn AiTaskPanel(
    ui_locale: Option<String>,
    task_profiles: Vec<AiTaskProfilePayload>,
    task_slug: RwSignal<String>,
    task_name: RwSignal<String>,
    task_description: RwSignal<String>,
    task_capability: RwSignal<String>,
    task_system_prompt: RwSignal<String>,
    task_allowed_providers: RwSignal<String>,
    task_preferred_providers: RwSignal<String>,
    task_execution_mode: RwSignal<String>,
    task_active: RwSignal<bool>,
    on_create_task_profile: Callback<SubmitEvent>,
    on_update_task_profile: Callback<MouseEvent>,
    on_reset: Callback<()>,
    select_task_query_writer: leptos_ui_routing::RouteQueryWriter,
) -> impl IntoView {
    let ui_locale_tasks = ui_locale.clone();
    let select_task_query_writer = select_task_query_writer.clone();

    view! {
        <Card title=t(ui_locale.as_deref(), "ai.card.taskProfiles", "Task Profiles")>
            <form class="space-y-3" on:submit=move |ev| on_create_task_profile.run(ev)>
                <TextField label=t(ui_locale.as_deref(), "ai.field.slug", "Slug") value=task_slug />
                <TextField label=t(ui_locale.as_deref(), "ai.field.displayName", "Display name") value=task_name />
                <TextField label=t(ui_locale.as_deref(), "ai.field.description", "Description") value=task_description />
                <TextField label=t(ui_locale.as_deref(), "ai.field.targetCapability", "Target capability") value=task_capability />
                <div class="flex flex-col gap-2">
                    <label class="text-sm font-medium leading-none">{t(ui_locale.as_deref(), "ai.field.systemPrompt", "System prompt")}</label>
                    <textarea
                        class="min-h-20 w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm transition focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                        prop:value=task_system_prompt
                        on:input=move |ev| task_system_prompt.set(event_target_value(&ev))
                    ></textarea>
                </div>
                <TextField label=t(ui_locale.as_deref(), "ai.field.allowedProvidersCsv", "Allowed providers (csv)") value=task_allowed_providers />
                <TextField label=t(ui_locale.as_deref(), "ai.field.preferredProvidersCsv", "Preferred providers (csv)") value=task_preferred_providers />
                <TextField label=t(ui_locale.as_deref(), "ai.field.defaultExecutionMode", "Default execution mode") value=task_execution_mode />
                <label class="flex items-center gap-2 text-sm text-muted-foreground">
                    <input
                        type="checkbox"
                        prop:checked=task_active
                        on:change=move |ev| task_active.set(event_target_checked(&ev))
                    />
                    {t(ui_locale.as_deref(), "ai.field.active", "Active")}
                </label>
                <div class="flex flex-wrap gap-2">
                    <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale.as_deref(), "ai.action.createTaskProfile", "Create task profile")}</button>
                    <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=move |ev| on_update_task_profile.run(ev)>{t(ui_locale.as_deref(), "ai.action.updateSelected", "Update selected")}</button>
                    <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=move |_| on_reset.run(())>{t(ui_locale.as_deref(), "ai.action.reset", "Reset")}</button>
                </div>
            </form>
            <div class="mt-4 space-y-2">
                {task_profiles.into_iter().map(|profile| {
                    let profile_slug_value = profile.slug.clone();
                    let task_query_writer = select_task_query_writer.clone();
                    view! {
                        <button
                            class="w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted"
                            on:click=move |_| {
                                task_query_writer.replace_value(
                                    AdminQueryKey::TaskProfileSlug.as_str(),
                                    profile_slug_value.clone(),
                                );
                            }
                        >
                            <div class="font-medium">{profile.display_name}</div>
                            <div class="text-muted-foreground">
                                {task_profile_summary(
                                    ui_locale_tasks.as_deref(),
                                    profile.target_capability.as_str(),
                                    profile.default_execution_mode.as_str(),
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
