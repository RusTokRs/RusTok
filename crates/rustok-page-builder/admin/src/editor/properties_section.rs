use crate::editor::AdminEditorRuntime;
use crate::editor::property_helpers::{parse_scalar, selected_patch};
use crate::i18n::t;
use fly::ComponentPatch;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::{Map, Value};

#[component]
pub(crate) fn PropertiesSection(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let properties_label = t(
        locale.as_deref(),
        "page_builder.panel.properties",
        "Properties",
    );
    let apply_label = t(locale.as_deref(), "page_builder.action.apply", "Apply");
    let clear_label = t(locale.as_deref(), "page_builder.action.clear", "Clear");
    let selected_label = t(
        locale.as_deref(),
        "page_builder.field.selectedComponent",
        "Selected component",
    );
    let none_label = t(locale.as_deref(), "page_builder.field.none", "None");
    let tag_label = t(locale.as_deref(), "page_builder.field.tagName", "Tag name");
    let content_label = t(locale.as_deref(), "page_builder.field.content", "Content");
    let attribute_name_label = t(
        locale.as_deref(),
        "page_builder.field.attributeName",
        "Attribute name",
    );
    let attribute_value_label = t(
        locale.as_deref(),
        "page_builder.field.attributeValue",
        "Attribute value or JSON",
    );

    let tag_name = RwSignal::new(String::new());
    let content_value = RwSignal::new(String::new());
    let attribute_name = RwSignal::new(String::new());
    let attribute_value = RwSignal::new(String::new());
    let observed_selection = RwSignal::new(None::<String>);

    Effect::new({
        let runtime = runtime.clone();
        move |_| {
            let selected = runtime
                .controller
                .with(|controller| controller.selected_component_view());
            let selected_id = selected.as_ref().map(|selected| selected.id.clone());
            if observed_selection.get_untracked() == selected_id {
                return;
            }
            observed_selection.set(selected_id);
            match selected {
                Some(selected) => {
                    tag_name.set(selected.tag_name.unwrap_or_default());
                    content_value.set(
                        selected
                            .fields
                            .get("content")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    );
                }
                None => {
                    tag_name.set(String::new());
                    content_value.set(String::new());
                }
            }
        }
    });

    let summary_runtime = runtime.clone();
    let type_runtime = runtime.clone();
    let tag_runtime = runtime.clone();
    let content_runtime = runtime.clone();
    let clear_content_runtime = runtime.clone();
    let attribute_runtime = runtime.clone();
    let attributes_runtime = runtime;

    view! {
        <section class="space-y-3">
            <h2 class="font-semibold">{properties_label}</h2>
            <dl class="grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 text-sm">
                <dt class="text-muted-foreground">{selected_label}</dt>
                <dd class="break-all">{move || summary_runtime.controller.with(|controller| {
                    controller
                        .ui()
                        .state
                        .selection
                        .component_id
                        .clone()
                        .unwrap_or_else(|| none_label.clone())
                })}</dd>
                <dt class="text-muted-foreground">"Type"</dt>
                <dd>{move || type_runtime.controller.with(|controller| {
                    controller
                        .selected_component_view()
                        .map(|selected| selected.component_type)
                        .unwrap_or_default()
                })}</dd>
            </dl>

            <label class="block text-sm font-medium">{tag_label}</label>
            <div class="flex gap-2">
                <input
                    class="min-w-0 flex-1 rounded border border-input bg-background px-2 py-1 text-sm"
                    prop:value=move || tag_name.get()
                    on:input=move |event| tag_name.set(event_target_value(&event))
                />
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| {
                        let value = tag_name.get_untracked();
                        let patch = if value.trim().is_empty() {
                            ComponentPatch {
                                remove_fields: vec!["tagName".to_string()],
                                ..ComponentPatch::default()
                            }
                        } else {
                            ComponentPatch {
                                fields: Map::from_iter([(
                                    "tagName".to_string(),
                                    Value::String(value),
                                )]),
                                ..ComponentPatch::default()
                            }
                        };
                        tag_runtime.dispatch_result(selected_patch(&tag_runtime, patch));
                    }
                >{apply_label.clone()}</button>
            </div>

            <label class="block text-sm font-medium">{content_label}</label>
            <textarea
                class="min-h-24 w-full rounded border border-input bg-background px-2 py-1 text-sm"
                prop:value=move || content_value.get()
                on:input=move |event| content_value.set(event_target_value(&event))
            ></textarea>
            <div class="flex gap-2">
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| content_runtime.dispatch_result(selected_patch(
                        &content_runtime,
                        ComponentPatch {
                            fields: Map::from_iter([(
                                "content".to_string(),
                                Value::String(content_value.get_untracked()),
                            )]),
                            ..ComponentPatch::default()
                        },
                    ))
                >{apply_label.clone()}</button>
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| clear_content_runtime.dispatch_result(selected_patch(
                        &clear_content_runtime,
                        ComponentPatch {
                            remove_fields: vec!["content".to_string()],
                            ..ComponentPatch::default()
                        },
                    ))
                >{clear_label.clone()}</button>
            </div>

            <div class="border-t border-border pt-3 text-sm font-medium">"Attributes"</div>
            <input
                aria-label=attribute_name_label.clone()
                placeholder=attribute_name_label
                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                prop:value=move || attribute_name.get()
                on:input=move |event| attribute_name.set(event_target_value(&event))
            />
            <input
                aria-label=attribute_value_label.clone()
                placeholder=attribute_value_label
                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                prop:value=move || attribute_value.get()
                on:input=move |event| attribute_value.set(event_target_value(&event))
            />
            <button
                type="button"
                class="rounded border border-border px-2 py-1 text-xs"
                on:click=move |_| {
                    let name = attribute_name.get_untracked().trim().to_string();
                    if name.is_empty() {
                        attribute_runtime.fail("attribute name must not be empty");
                        return;
                    }
                    attribute_runtime.dispatch_result(selected_patch(
                        &attribute_runtime,
                        ComponentPatch {
                            attributes: Map::from_iter([(
                                name,
                                parse_scalar(&attribute_value.get_untracked()),
                            )]),
                            ..ComponentPatch::default()
                        },
                    ));
                }
            >{apply_label}</button>

            <div class="space-y-1">
                {move || attributes_runtime
                    .controller
                    .with(|controller| controller.selected_component_view())
                    .map(|selected| selected.attributes.into_iter().map(|(name, value)| {
                        let runtime = attributes_runtime.clone();
                        let remove_name = name.clone();
                        let clear_label = clear_label.clone();
                        view! {
                            <div class="flex items-start gap-2 rounded bg-muted/50 px-2 py-1 text-xs">
                                <code class="min-w-0 flex-1 break-all">{format!("{name}={value}")}</code>
                                <button
                                    type="button"
                                    class="text-destructive"
                                    on:click=move |_| runtime.dispatch_result(selected_patch(
                                        &runtime,
                                        ComponentPatch {
                                            remove_attributes: vec![remove_name.clone()],
                                            ..ComponentPatch::default()
                                        },
                                    ))
                                >{clear_label}</button>
                            </div>
                        }
                    }).collect_view())}
            </div>
        </section>
    }
}
