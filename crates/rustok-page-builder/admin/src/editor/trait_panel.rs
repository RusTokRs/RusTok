use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{EditorCommand, TraitSchema, TraitSnapshot, TraitValueKind, trait_snapshots};
use fly_ui::UiIntent;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::Value;

#[component]
pub fn TraitPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(locale.as_deref(), "page_builder.panel.traits", "Traits");
    let empty = t(
        locale.as_deref(),
        "page_builder.traits.empty",
        "Select a component to edit its traits.",
    );
    let panel_runtime = runtime;

    view! {
        <section class="space-y-3 rounded-xl border border-border bg-card p-3">
            <div class="flex items-center justify-between gap-2">
                <h2 class="font-semibold">{title}</h2>
                <span class="text-xs text-muted-foreground">
                    {format!("{} schemas", panel_runtime.trait_schemas.len())}
                </span>
            </div>
            {move || {
                let selection = panel_runtime.controller.with(|controller| {
                    let selected = controller.selected_component_view()?;
                    let component = controller.editor().document().component(&selected.id)?;
                    Some((
                        selected.id,
                        trait_snapshots(
                            component,
                            panel_runtime
                                .trait_schemas
                                .for_component(component.component_type()),
                        ),
                    ))
                });
                match selection {
                    Some((component_id, snapshots)) if !snapshots.is_empty() => snapshots
                        .into_iter()
                        .map(|snapshot| view! {
                            <TraitEditorRow
                                runtime=panel_runtime.clone()
                                component_id=component_id.clone()
                                snapshot
                            />
                        })
                        .collect_view()
                        .into_any(),
                    _ => view! {
                        <p class="text-sm text-muted-foreground">{empty.clone()}</p>
                    }
                    .into_any(),
                }
            }}
        </section>
    }
}

#[component]
fn TraitEditorRow(
    runtime: AdminEditorRuntime,
    component_id: String,
    snapshot: TraitSnapshot,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let apply_label = t(locale.as_deref(), "page_builder.action.apply", "Apply");
    let clear_label = t(locale.as_deref(), "page_builder.action.clear", "Clear");
    let initial = value_text(snapshot.value.as_ref());
    let value = RwSignal::new(initial);
    let schema = snapshot.schema;
    let input_schema = schema.clone();
    let apply_schema = schema.clone();
    let clear_schema = schema.clone();
    let apply_runtime = runtime.clone();
    let clear_runtime = runtime.clone();
    let apply_component_id = component_id.clone();
    let clear_component_id = component_id.clone();

    view! {
        <div class="space-y-2 border-t border-border pt-3 first:border-t-0 first:pt-0">
            <label class="block text-sm font-medium">{schema.label.clone()}</label>
            {match schema.value_type {
                TraitValueKind::Boolean => {
                    let checked = snapshot.value.as_ref().and_then(Value::as_bool).unwrap_or(false);
                    let runtime = runtime.clone();
                    let component_id = component_id.clone();
                    let schema = input_schema.clone();
                    view! {
                        <label class="flex items-center gap-2 text-sm">
                            <input
                                type="checkbox"
                                prop:checked=checked
                                on:change=move |event| apply_trait_value(
                                    &runtime,
                                    &component_id,
                                    &schema,
                                    if event_target_checked(&event) { "true" } else { "false" },
                                )
                            />
                            <span>{if checked { "true" } else { "false" }}</span>
                        </label>
                    }
                    .into_any()
                }
                TraitValueKind::Select => {
                    let runtime = runtime.clone();
                    let component_id = component_id.clone();
                    let schema = input_schema.clone();
                    let options = schema.options.clone();
                    view! {
                        <select
                            class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                            prop:value=move || value.get()
                            on:change=move |event| {
                                let selected = event_target_value(&event);
                                value.set(selected.clone());
                                apply_trait_value(&runtime, &component_id, &schema, &selected);
                            }
                        >
                            <option value="">"—"</option>
                            {options.into_iter().map(|option| view! {
                                <option value=option.value>{option.label}</option>
                            }).collect_view()}
                        </select>
                    }
                    .into_any()
                }
                TraitValueKind::Multiline => view! {
                    <textarea
                        class="min-h-20 w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        placeholder=schema.placeholder.clone().unwrap_or_default()
                        prop:value=move || value.get()
                        on:input=move |event| value.set(event_target_value(&event))
                    ></textarea>
                }
                .into_any(),
                TraitValueKind::Number => view! {
                    <input
                        type="number"
                        class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        placeholder=schema.placeholder.clone().unwrap_or_default()
                        prop:value=move || value.get()
                        on:input=move |event| value.set(event_target_value(&event))
                    />
                }
                .into_any(),
                TraitValueKind::Text | TraitValueKind::Url => view! {
                    <input
                        type=if matches!(schema.value_type, TraitValueKind::Url) { "url" } else { "text" }
                        class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        placeholder=schema.placeholder.clone().unwrap_or_default()
                        prop:value=move || value.get()
                        on:input=move |event| value.set(event_target_value(&event))
                    />
                }
                .into_any(),
            }}
            <div class="flex gap-2" class:hidden=matches!(schema.value_type, TraitValueKind::Boolean | TraitValueKind::Select)>
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| apply_trait_value(
                        &apply_runtime,
                        &apply_component_id,
                        &apply_schema,
                        &value.get_untracked(),
                    )
                >{apply_label}</button>
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| {
                        value.set(String::new());
                        clear_runtime.dispatch(UiIntent::execute(EditorCommand::Patch {
                            component_id: clear_component_id.clone(),
                            patch: clear_schema.remove_patch(),
                        }));
                    }
                >{clear_label}</button>
            </div>
        </div>
    }
}

fn apply_trait_value(
    runtime: &AdminEditorRuntime,
    component_id: &str,
    schema: &TraitSchema,
    value: &str,
) {
    match schema.patch_from_text(value) {
        Ok(patch) => runtime.dispatch(UiIntent::execute(EditorCommand::Patch {
            component_id: component_id.to_string(),
            patch,
        })),
        Err(error) => runtime.fail(error.to_string()),
    }
}

fn value_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Number(value)) => value.to_string(),
        _ => String::new(),
    }
}
