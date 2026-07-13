use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{AssetCatalog, AssetCommand, ComponentPatch, EditorCommand};
use fly_ui::{builtin_style_properties, style_patch, StyleEntry, UiIntent};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::{json, Map, Value};

#[component]
pub fn PropertiesAssetsPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let properties_runtime = runtime.clone();
    let style_runtime = runtime.clone();
    let asset_runtime = runtime.clone();
    let diagnostics_runtime = runtime;
    view! {
        <aside class="space-y-4 overflow-auto rounded-xl border border-border bg-card p-3">
            <PropertiesSection runtime=properties_runtime />
            <StyleSection runtime=style_runtime />
            <AssetSection runtime=asset_runtime />
            <DiagnosticsSection runtime=diagnostics_runtime />
        </aside>
    }
}

#[component]
fn PropertiesSection(runtime: AdminEditorRuntime) -> impl IntoView {
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

#[component]
fn StyleSection(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let styles_label = t(locale.as_deref(), "page_builder.panel.styles", "Styles");
    let apply_label = t(locale.as_deref(), "page_builder.action.apply", "Apply");
    let clear_label = t(locale.as_deref(), "page_builder.action.clear", "Clear");
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
    let style_property = RwSignal::new("padding".to_string());
    let style_value = RwSignal::new(String::new());
    let observed_selection = RwSignal::new(None::<String>);

    Effect::new({
        let runtime = runtime.clone();
        move |_| {
            let selected_id = runtime
                .controller
                .with(|controller| controller.ui().state.selection.component_id.clone());
            if observed_selection.get_untracked() == selected_id {
                return;
            }
            observed_selection.set(selected_id);
            style_value.set(selected_style_value(
                &runtime,
                &style_property.get_untracked(),
            ));
        }
    });

    let change_runtime = runtime.clone();
    let apply_runtime = runtime.clone();
    let clear_runtime = runtime;
    let descriptors = builtin_style_properties();

    view! {
        <section class="space-y-2 border-t border-border pt-3">
            <h2 class="font-semibold">{styles_label}</h2>
            <label class="block text-sm font-medium">{property_label}</label>
            <select
                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                prop:value=move || style_property.get()
                on:change=move |event| {
                    let property = event_target_value(&event);
                    style_property.set(property.clone());
                    style_value.set(selected_style_value(&change_runtime, &property));
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
            <div class="flex gap-2">
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| match style_patch([StyleEntry {
                        property: style_property.get_untracked(),
                        value: style_value.get_untracked(),
                    }]) {
                        Ok(patch) => apply_runtime.dispatch_result(selected_patch(&apply_runtime, patch)),
                        Err(errors) => apply_runtime.fail(
                            errors
                                .into_iter()
                                .map(|error| format!("{}: {}", error.property, error.message))
                                .collect::<Vec<_>>()
                                .join("; "),
                        ),
                    }
                >{apply_label}</button>
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs"
                    on:click=move |_| clear_runtime.dispatch_result(selected_patch(
                        &clear_runtime,
                        ComponentPatch {
                            remove_style_properties: vec![style_property.get_untracked()],
                            ..ComponentPatch::default()
                        },
                    ))
                >{clear_label}</button>
            </div>
        </section>
    }
}

#[component]
fn AssetSection(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let assets_label = t(locale.as_deref(), "page_builder.panel.assets", "Assets");
    let add_label = t(locale.as_deref(), "page_builder.action.add", "Add");
    let remove_label = t(locale.as_deref(), "page_builder.action.remove", "Remove");
    let select_label = t(locale.as_deref(), "page_builder.action.select", "Use");
    let asset_id_label = t(locale.as_deref(), "page_builder.field.assetId", "Asset id");
    let asset_url_label = t(
        locale.as_deref(),
        "page_builder.field.assetUrl",
        "Asset URL",
    );
    let asset_id = RwSignal::new(String::new());
    let asset_url = RwSignal::new(String::new());
    let add_runtime = runtime.clone();
    let list_runtime = runtime;

    view! {
        <section class="space-y-2 border-t border-border pt-3">
            <h2 class="font-semibold">{assets_label}</h2>
            <input
                placeholder=asset_id_label
                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                prop:value=move || asset_id.get()
                on:input=move |event| asset_id.set(event_target_value(&event))
            />
            <input
                placeholder=asset_url_label
                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                prop:value=move || asset_url.get()
                on:input=move |event| asset_url.set(event_target_value(&event))
            />
            <button
                type="button"
                class="rounded border border-border px-2 py-1 text-xs"
                on:click=move |_| {
                    let source = asset_url.get_untracked().trim().to_string();
                    if source.is_empty() {
                        add_runtime.fail("asset URL must not be empty");
                        return;
                    }
                    let id = asset_id.get_untracked().trim().to_string();
                    let asset = if id.is_empty() {
                        json!({ "src": source })
                    } else {
                        json!({ "id": id, "src": source })
                    };
                    add_runtime.dispatch(UiIntent::Execute(EditorCommand::Asset {
                        command: AssetCommand::Upsert { asset },
                    }));
                }
            >{add_label}</button>

            <div class="space-y-1">
                {move || {
                    let catalog = list_runtime.controller.with(|controller| {
                        AssetCatalog::from_document(controller.editor().document())
                    });
                    catalog.assets.into_iter().map(|asset| {
                        let use_runtime = list_runtime.clone();
                        let remove_runtime = list_runtime.clone();
                        let use_id = asset.id.clone();
                        let remove_id = asset.id.clone();
                        let select_label = select_label.clone();
                        let remove_label = remove_label.clone();
                        view! {
                            <div class="rounded border border-border p-2 text-xs">
                                <div class="font-medium">{asset.name.clone().unwrap_or_else(|| asset.id.clone())}</div>
                                <div class="break-all text-muted-foreground">{asset.source}</div>
                                <div class="mt-2 flex gap-2">
                                    <button
                                        type="button"
                                        class="rounded border border-border px-2 py-1"
                                        on:click=move |_| {
                                            let intent = use_runtime.controller.with(|controller| {
                                                controller.apply_asset_to_selected_intent(&use_id, "src")
                                            });
                                            use_runtime.dispatch_result(intent);
                                        }
                                    >{select_label}</button>
                                    <button
                                        type="button"
                                        class="rounded border border-destructive/40 px-2 py-1 text-destructive"
                                        on:click=move |_| remove_runtime.dispatch(UiIntent::Execute(
                                            EditorCommand::Asset {
                                                command: AssetCommand::Remove {
                                                    asset_id: remove_id.clone(),
                                                },
                                            },
                                        ))
                                    >{remove_label}</button>
                                </div>
                            </div>
                        }
                    }).collect_view()
                }}
            </div>
        </section>
    }
}

#[component]
fn DiagnosticsSection(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let diagnostics_label = t(
        locale.as_deref(),
        "page_builder.panel.diagnostics",
        "Diagnostics",
    );
    let project_hash_label = t(
        locale.as_deref(),
        "page_builder.field.projectHash",
        "Project hash",
    );
    let hash_runtime = runtime.clone();
    let diagnostics_runtime = runtime;

    view! {
        <section class="space-y-1 border-t border-border pt-3 text-sm">
            <h2 class="font-semibold">{diagnostics_label}</h2>
            <p class="break-all">{move || hash_runtime.controller.with(|controller| {
                format!(
                    "{project_hash_label}: {}",
                    controller.editor().revision().project_hash.hex(),
                )
            })}</p>
            {move || diagnostics_runtime
                .controller
                .with(|controller| controller.ui().state.diagnostics.clone())
                .into_iter()
                .map(|diagnostic| view! {
                    <div class="rounded bg-muted/50 px-2 py-1 text-xs">
                        <strong>{diagnostic.code}</strong>
                        <div>{diagnostic.message}</div>
                    </div>
                })
                .collect_view()}
        </section>
    }
}

fn selected_patch(runtime: &AdminEditorRuntime, patch: ComponentPatch) -> Result<UiIntent, String> {
    let component_id = runtime
        .controller
        .with(|controller| controller.ui().state.selection.component_id.clone())
        .ok_or_else(|| "select a component before editing properties".to_string())?;
    Ok(UiIntent::Execute(EditorCommand::Patch {
        component_id,
        patch,
    }))
}

fn selected_style_value(runtime: &AdminEditorRuntime, property: &str) -> String {
    runtime.controller.with(|controller| {
        controller
            .selected_component_view()
            .and_then(|selected| selected.style)
            .and_then(|style| style.as_object().cloned())
            .and_then(|style| style.get(property).and_then(scalar_string))
            .unwrap_or_default()
    })
}

fn parse_scalar(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

fn scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}
