use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{EditorCommand, StyleRuleCatalog, StyleRuleCommand, StyleRuleScope};
use fly_ui::{
    StyleEntry, UiIntent, builtin_responsive_breakpoints, builtin_style_properties,
    responsive_breakpoint, style_patch, viewport_preset,
};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::Value;

#[component]
pub fn ResponsiveStylePanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.responsiveStyles",
        "Responsive styles",
    );
    let breakpoint_label = t(
        locale.as_deref(),
        "page_builder.field.breakpoint",
        "Breakpoint",
    );
    let property_label = t(
        locale.as_deref(),
        "page_builder.field.styleProperty",
        "Style property",
    );
    let value_label = t(
        locale.as_deref(),
        "page_builder.field.styleValue",
        "Style value",
    );
    let apply_label = t(locale.as_deref(), "page_builder.action.apply", "Apply");
    let clear_label = t(locale.as_deref(), "page_builder.action.clear", "Clear");
    let preview_label = t(
        locale.as_deref(),
        "page_builder.action.previewBreakpoint",
        "Preview breakpoint",
    );

    let breakpoints = builtin_responsive_breakpoints();
    let descriptors = builtin_style_properties();
    let breakpoint_id = RwSignal::new("mobile-down".to_string());
    let style_property = RwSignal::new("padding".to_string());
    let style_value = RwSignal::new(String::new());
    let observed_key = RwSignal::new(None::<String>);

    Effect::new({
        let runtime = runtime.clone();
        move |_| {
            let selected_id = runtime
                .controller
                .with(|controller| controller.ui().state.selection.component_id.clone());
            let key = selected_id.as_ref().map(|selected_id| {
                format!(
                    "{}|{}|{}",
                    selected_id,
                    breakpoint_id.get(),
                    style_property.get()
                )
            });
            if observed_key.get_untracked() == key {
                return;
            }
            observed_key.set(key);
            style_value.set(current_rule_value(
                &runtime,
                &breakpoint_id.get_untracked(),
                &style_property.get_untracked(),
            ));
        }
    });

    let breakpoint_runtime = runtime.clone();
    let property_runtime = runtime.clone();
    let apply_runtime = runtime.clone();
    let clear_runtime = runtime.clone();
    let preview_runtime = runtime.clone();
    let summary_runtime = runtime;

    view! {
        <section class="space-y-2 rounded-xl border border-border bg-card p-3">
            <h2 class="font-semibold">{title}</h2>
            <label class="block text-sm font-medium">{breakpoint_label}</label>
            <select
                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                prop:value=move || breakpoint_id.get()
                on:change=move |event| {
                    breakpoint_id.set(event_target_value(&event));
                    style_value.set(current_rule_value(
                        &breakpoint_runtime,
                        &breakpoint_id.get_untracked(),
                        &style_property.get_untracked(),
                    ));
                }
            >
                {breakpoints.into_iter().map(|breakpoint| view! {
                    <option value=breakpoint.id>{format!("{} · {}", breakpoint.label, breakpoint.media_query)}</option>
                }).collect_view()}
            </select>

            <label class="block text-sm font-medium">{property_label}</label>
            <select
                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                prop:value=move || style_property.get()
                on:change=move |event| {
                    style_property.set(event_target_value(&event));
                    style_value.set(current_rule_value(
                        &property_runtime,
                        &breakpoint_id.get_untracked(),
                        &style_property.get_untracked(),
                    ));
                }
            >
                {descriptors.into_iter().map(|descriptor| view! {
                    <option value=descriptor.property>{format!("{:?} · {}", descriptor.group, descriptor.label)}</option>
                }).collect_view()}
            </select>

            <label class="block text-sm font-medium">{value_label}</label>
            <input
                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                prop:value=move || style_value.get()
                on:input=move |event| style_value.set(event_target_value(&event))
            />

            <div class="flex flex-wrap gap-2">
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| dispatch_rule_patch(
                        &apply_runtime,
                        &breakpoint_id.get_untracked(),
                        StyleEntry {
                            property: style_property.get_untracked(),
                            value: style_value.get_untracked(),
                        },
                    )
                >{apply_label}</button>
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| dispatch_rule_patch(
                        &clear_runtime,
                        &breakpoint_id.get_untracked(),
                        StyleEntry {
                            property: style_property.get_untracked(),
                            value: String::new(),
                        },
                    )
                >{clear_label}</button>
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| {
                        let Some(breakpoint) = responsive_breakpoint(&breakpoint_id.get_untracked()) else {
                            preview_runtime.fail("responsive breakpoint is not registered");
                            return;
                        };
                        let Some(preset) = viewport_preset(&breakpoint.preview_preset_id) else {
                            preview_runtime.fail("responsive preview preset is not registered");
                            return;
                        };
                        let viewport = preview_runtime.controller.with(|controller| {
                            preset.apply(controller.ui().state.viewport)
                        });
                        preview_runtime.dispatch(UiIntent::SetViewport(viewport));
                    }
                >{preview_label}</button>
            </div>

            <div class="space-y-1 pt-1 text-xs text-muted-foreground">
                {move || selected_rule_summary(
                    &summary_runtime,
                    &breakpoint_id.get(),
                )}
            </div>
        </section>
    }
}

fn dispatch_rule_patch(runtime: &AdminEditorRuntime, breakpoint_id: &str, entry: StyleEntry) {
    let Some(component_id) = runtime
        .controller
        .with(|controller| controller.ui().state.selection.component_id.clone())
    else {
        runtime.fail("select a component before editing responsive styles");
        return;
    };
    let Some(breakpoint) = responsive_breakpoint(breakpoint_id) else {
        runtime.fail("responsive breakpoint is not registered");
        return;
    };
    let patch = match style_patch([entry]) {
        Ok(patch) => patch,
        Err(errors) => {
            runtime.fail(
                errors
                    .into_iter()
                    .map(|error| format!("{}: {}", error.property, error.message))
                    .collect::<Vec<_>>()
                    .join("; "),
            );
            return;
        }
    };
    let declarations = patch
        .style
        .and_then(|style| style.as_object().cloned())
        .unwrap_or_default();
    runtime.dispatch(UiIntent::execute(EditorCommand::StyleRule {
        command: StyleRuleCommand::UpsertComponentRule {
            component_id,
            scope: StyleRuleScope::Media {
                query: breakpoint.media_query,
            },
            declarations,
            remove_properties: patch.remove_style_properties,
        },
    }));
}

fn current_rule_value(runtime: &AdminEditorRuntime, breakpoint_id: &str, property: &str) -> String {
    let Some(component_id) = runtime
        .controller
        .with(|controller| controller.ui().state.selection.component_id.clone())
    else {
        return String::new();
    };
    let Some(breakpoint) = responsive_breakpoint(breakpoint_id) else {
        return String::new();
    };
    let scope = StyleRuleScope::Media {
        query: breakpoint.media_query,
    };
    runtime.controller.with(|controller| {
        StyleRuleCatalog::from_document(controller.editor().document())
            .component_rule(&component_id, &scope)
            .and_then(|rule| rule.declarations.get(property))
            .and_then(scalar_string)
            .unwrap_or_default()
    })
}

fn selected_rule_summary(runtime: &AdminEditorRuntime, breakpoint_id: &str) -> String {
    let Some(component_id) = runtime
        .controller
        .with(|controller| controller.ui().state.selection.component_id.clone())
    else {
        return "No component selected".to_string();
    };
    let Some(breakpoint) = responsive_breakpoint(breakpoint_id) else {
        return "Unknown breakpoint".to_string();
    };
    let scope = StyleRuleScope::Media {
        query: breakpoint.media_query.clone(),
    };
    runtime.controller.with(|controller| {
        let catalog = StyleRuleCatalog::from_document(controller.editor().document());
        match catalog.component_rule(&component_id, &scope) {
            Some(rule) if !rule.declarations.is_empty() => format!(
                "{} · {} declaration(s)",
                breakpoint.media_query,
                rule.declarations.len()
            ),
            _ => format!("{} · no overrides", breakpoint.media_query),
        }
    })
}

fn scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}
