use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    BindingCatalog, BindingCommand, BindingTarget, BindingTransform, EditorCommand,
    RuntimeBinding,
};
use fly_ui::UiIntent;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::{Map, Value};

#[component]
pub fn BindingPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.bindings",
        "Data bindings",
    );
    let apply_label = t(locale.as_deref(), "page_builder.action.apply", "Apply");
    let remove_label = t(locale.as_deref(), "page_builder.action.remove", "Remove");
    let empty_label = t(
        locale.as_deref(),
        "page_builder.bindings.noSelection",
        "Select a component to configure data bindings.",
    );

    let path = RwSignal::new(String::new());
    let target_kind = RwSignal::new("field".to_string());
    let target_name = RwSignal::new("content".to_string());
    let transform = RwSignal::new("identity".to_string());
    let fallback = RwSignal::new(String::new());
    let apply_runtime = runtime.clone();
    let list_runtime = runtime;

    view! {
        <section class="space-y-3 rounded-xl border border-border bg-card p-3">
            <h2 class="font-semibold">{title}</h2>
            {move || {
                let selected_id = list_runtime.controller.with(|controller| {
                    controller.ui().state.selection.component_id.clone()
                });
                let Some(component_id) = selected_id else {
                    return view! {
                        <p class="text-sm text-muted-foreground">{empty_label.clone()}</p>
                    }
                    .into_any();
                };
                let bindings = list_runtime.controller.with(|controller| {
                    BindingCatalog::from_document(controller.editor().document())
                        .bindings
                        .into_iter()
                        .filter(|binding| binding.component_id == component_id)
                        .collect::<Vec<_>>()
                });
                let command_runtime = apply_runtime.clone();
                let command_component_id = component_id.clone();

                view! {
                    <div class="space-y-2">
                        <p class="break-all text-xs text-muted-foreground">
                            {format!("Selected: {component_id}")}
                        </p>
                        <input
                            class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                            placeholder="page.title"
                            prop:value=move || path.get()
                            on:input=move |event| path.set(event_target_value(&event))
                        />
                        <div class="grid grid-cols-2 gap-2">
                            <select
                                class="rounded border border-input bg-background px-2 py-1 text-sm"
                                prop:value=move || target_kind.get()
                                on:change=move |event| {
                                    let kind = event_target_value(&event);
                                    let default_name = match kind.as_str() {
                                        "attribute" => "title",
                                        "style" => "color",
                                        _ => "content",
                                    };
                                    target_kind.set(kind);
                                    target_name.set(default_name.to_string());
                                }
                            >
                                <option value="field">"Field"</option>
                                <option value="attribute">"Attribute"</option>
                                <option value="style">"Style"</option>
                            </select>
                            <input
                                class="rounded border border-input bg-background px-2 py-1 text-sm"
                                placeholder="target name"
                                prop:value=move || target_name.get()
                                on:input=move |event| target_name.set(event_target_value(&event))
                            />
                        </div>
                        <select
                            class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                            prop:value=move || transform.get()
                            on:change=move |event| transform.set(event_target_value(&event))
                        >
                            <option value="identity">"Identity"</option>
                            <option value="string">"String"</option>
                            <option value="number">"Number"</option>
                            <option value="boolean">"Boolean"</option>
                            <option value="uppercase">"Uppercase"</option>
                            <option value="lowercase">"Lowercase"</option>
                            <option value="trim">"Trim"</option>
                            <option value="json">"JSON string"</option>
                        </select>
                        <input
                            class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                            placeholder="Fallback JSON or text"
                            prop:value=move || fallback.get()
                            on:input=move |event| fallback.set(event_target_value(&event))
                        />
                        <button
                            type="button"
                            class="rounded border border-border px-2 py-1 text-xs"
                            on:click=move |_| {
                                let binding_path = path.get_untracked().trim().to_string();
                                let name = target_name.get_untracked().trim().to_string();
                                if binding_path.is_empty() || name.is_empty() {
                                    command_runtime.fail(
                                        "Binding path and target name must not be empty"
                                    );
                                    return;
                                }
                                let kind = target_kind.get_untracked();
                                let target = match kind.as_str() {
                                    "attribute" => BindingTarget::Attribute { name: name.clone() },
                                    "style" => BindingTarget::Style { name: name.clone() },
                                    _ => BindingTarget::Field { name: name.clone() },
                                };
                                let id = format!(
                                    "fly.binding.{}.{}.{}",
                                    stable_suffix(&command_component_id),
                                    stable_suffix(&kind),
                                    stable_suffix(&name),
                                );
                                command_runtime.dispatch(UiIntent::Execute(
                                    EditorCommand::Binding {
                                        command: BindingCommand::Upsert {
                                            binding: RuntimeBinding {
                                                id,
                                                component_id: command_component_id.clone(),
                                                path: binding_path,
                                                target,
                                                fallback: parse_optional_value(
                                                    &fallback.get_untracked()
                                                ),
                                                transform: parse_transform(
                                                    &transform.get_untracked()
                                                ),
                                                extensions: Map::new(),
                                            },
                                        },
                                    },
                                ));
                            }
                        >{apply_label.clone()}</button>

                        <div class="space-y-1 border-t border-border pt-2">
                            {bindings.into_iter().map(|binding| {
                                let remove_runtime = list_runtime.clone();
                                let binding_id = binding.id.clone();
                                let remove_label = remove_label.clone();
                                let target = target_text(&binding.target);
                                view! {
                                    <div class="rounded bg-muted/50 px-2 py-1 text-xs">
                                        <div class="flex items-start justify-between gap-2">
                                            <div class="min-w-0">
                                                <strong class="block truncate">{target}</strong>
                                                <span class="block truncate text-muted-foreground">
                                                    {format!("{} · {:?}", binding.path, binding.transform)}
                                                </span>
                                            </div>
                                            <button
                                                type="button"
                                                class="shrink-0 text-destructive"
                                                on:click=move |_| remove_runtime.dispatch(
                                                    UiIntent::Execute(EditorCommand::Binding {
                                                        command: BindingCommand::Remove {
                                                            binding_id: binding_id.clone(),
                                                        },
                                                    })
                                                )
                                            >{remove_label}</button>
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    </div>
                }
                .into_any()
            }}
        </section>
    }
}

fn target_text(target: &BindingTarget) -> String {
    match target {
        BindingTarget::Attribute { name } => format!("attribute:{name}"),
        BindingTarget::Field { name } => format!("field:{name}"),
        BindingTarget::Style { name } => format!("style:{name}"),
    }
}

fn parse_transform(value: &str) -> BindingTransform {
    match value {
        "string" => BindingTransform::String,
        "number" => BindingTransform::Number,
        "boolean" => BindingTransform::Boolean,
        "uppercase" => BindingTransform::Uppercase,
        "lowercase" => BindingTransform::Lowercase,
        "trim" => BindingTransform::Trim,
        "json" => BindingTransform::Json,
        _ => BindingTransform::Identity,
    }
}

fn parse_optional_value(value: &str) -> Option<Value> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    serde_json::from_str(value)
        .ok()
        .or_else(|| Some(Value::String(value.to_string())))
}

fn stable_suffix(value: &str) -> String {
    let suffix = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let suffix = suffix.trim_matches('-');
    if suffix.is_empty() {
        "value".to_string()
    } else {
        suffix.to_string()
    }
}
