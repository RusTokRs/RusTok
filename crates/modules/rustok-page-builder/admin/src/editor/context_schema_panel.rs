use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    ComputedContextValue, ContextCommand, ContextExpression, ContextFieldDefinition,
    ContextSchemaCatalog, ContextValueKind, EditorCommand,
    materialize_project_with_runtime_context,
};
use fly_ui::UiIntent;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::{Map, Value};

#[component]
pub fn ContextSchemaPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.contextSchema",
        "Runtime context schema",
    );
    let apply_label = t(locale.as_deref(), "page_builder.action.apply", "Apply");
    let remove_label = t(locale.as_deref(), "page_builder.action.remove", "Remove");

    let field_path = RwSignal::new(String::new());
    let field_kind = RwSignal::new("string".to_string());
    let field_required = RwSignal::new(false);
    let field_default = RwSignal::new(String::new());
    let item_kind = RwSignal::new(String::new());

    let computed_path = RwSignal::new(String::new());
    let expression_json = RwSignal::new(
        serde_json::to_string_pretty(&serde_json::json!({
            "op": "format",
            "template": "{{page.title}}"
        }))
        .unwrap_or_default(),
    );
    let computed_fallback = RwSignal::new(String::new());

    let field_runtime = runtime.clone();
    let computed_runtime = runtime.clone();
    let list_runtime = runtime.clone();
    let report_runtime = runtime;

    view! {
        <section class="space-y-3 rounded-xl border border-border bg-card p-3">
            <h2 class="font-semibold">{title}</h2>

            <details open>
                <summary class="cursor-pointer text-sm font-medium">"Typed field"</summary>
                <div class="mt-2 space-y-2">
                    <input
                        class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        placeholder="page.title"
                        prop:value=move || field_path.get()
                        on:input=move |event| field_path.set(event_target_value(&event))
                    />
                    <div class="grid grid-cols-2 gap-2">
                        <select
                            class="rounded border border-input bg-background px-2 py-1 text-sm"
                            prop:value=move || field_kind.get()
                            on:change=move |event| field_kind.set(event_target_value(&event))
                        >
                            <option value="any">"Any"</option>
                            <option value="null">"Null"</option>
                            <option value="boolean">"Boolean"</option>
                            <option value="number">"Number"</option>
                            <option value="string">"String"</option>
                            <option value="object">"Object"</option>
                            <option value="array">"Array"</option>
                        </select>
                        <select
                            class="rounded border border-input bg-background px-2 py-1 text-sm"
                            prop:value=move || item_kind.get()
                            on:change=move |event| item_kind.set(event_target_value(&event))
                            disabled=move || field_kind.get() != "array"
                        >
                            <option value="">"Any array items"</option>
                            <option value="null">"Null items"</option>
                            <option value="boolean">"Boolean items"</option>
                            <option value="number">"Number items"</option>
                            <option value="string">"String items"</option>
                            <option value="object">"Object items"</option>
                            <option value="array">"Array items"</option>
                        </select>
                    </div>
                    <input
                        class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        placeholder="Default JSON or text"
                        prop:value=move || field_default.get()
                        on:input=move |event| field_default.set(event_target_value(&event))
                    />
                    <label class="flex items-center gap-2 text-xs">
                        <input
                            type="checkbox"
                            prop:checked=move || field_required.get()
                            on:change=move |event| field_required.set(event_target_checked(&event))
                        />
                        <span>"Required in runtime context"</span>
                    </label>
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        on:click=move |_| {
                            let path = field_path.get_untracked().trim().to_string();
                            if path.is_empty() {
                                field_runtime.fail("Context field path must not be empty");
                                return;
                            }
                            let kind = parse_kind(&field_kind.get_untracked());
                            let item = if kind == ContextValueKind::Array {
                                parse_optional_kind(&item_kind.get_untracked())
                            } else {
                                None
                            };
                            let id = field_runtime.controller.with(|controller| {
                                ContextSchemaCatalog::from_document(controller.editor().document())
                                    .fields
                                    .into_iter()
                                    .find(|field| field.path == path)
                                    .map(|field| field.id)
                                    .unwrap_or_else(|| format!(
                                        "fly.context.{}",
                                        stable_suffix(&path)
                                    ))
                            });
                            field_runtime.dispatch(UiIntent::execute(EditorCommand::Context {
                                command: ContextCommand::UpsertField {
                                    field: ContextFieldDefinition {
                                        id,
                                        path,
                                        kind,
                                        required: field_required.get_untracked(),
                                        default: parse_optional_value(
                                            &field_default.get_untracked()
                                        ),
                                        item_kind: item,
                                        extensions: Map::new(),
                                    },
                                },
                            }));
                        }
                    >{apply_label.clone()}</button>
                </div>
            </details>

            <details>
                <summary class="cursor-pointer text-sm font-medium">"Computed value"</summary>
                <div class="mt-2 space-y-2">
                    <input
                        class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        placeholder="page.displayTitle"
                        prop:value=move || computed_path.get()
                        on:input=move |event| computed_path.set(event_target_value(&event))
                    />
                    <textarea
                        class="min-h-40 w-full rounded border border-input bg-background px-2 py-1 font-mono text-xs"
                        prop:value=move || expression_json.get()
                        on:input=move |event| expression_json.set(event_target_value(&event))
                    ></textarea>
                    <input
                        class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        placeholder="Fallback JSON or text"
                        prop:value=move || computed_fallback.get()
                        on:input=move |event| computed_fallback.set(event_target_value(&event))
                    />
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        on:click=move |_| {
                            let path = computed_path.get_untracked().trim().to_string();
                            if path.is_empty() {
                                computed_runtime.fail("Computed context path must not be empty");
                                return;
                            }
                            let expression = match serde_json::from_str::<ContextExpression>(
                                &expression_json.get_untracked()
                            ) {
                                Ok(expression) => expression,
                                Err(error) => {
                                    computed_runtime.fail(format!(
                                        "Invalid computed expression JSON: {error}"
                                    ));
                                    return;
                                }
                            };
                            let id = computed_runtime.controller.with(|controller| {
                                ContextSchemaCatalog::from_document(controller.editor().document())
                                    .computed
                                    .into_iter()
                                    .find(|computed| computed.path == path)
                                    .map(|computed| computed.id)
                                    .unwrap_or_else(|| format!(
                                        "fly.computed.{}",
                                        stable_suffix(&path)
                                    ))
                            });
                            computed_runtime.dispatch(UiIntent::execute(EditorCommand::Context {
                                command: ContextCommand::UpsertComputed {
                                    computed: ComputedContextValue {
                                        id,
                                        path,
                                        expression,
                                        fallback: parse_optional_value(
                                            &computed_fallback.get_untracked()
                                        ),
                                        extensions: Map::new(),
                                    },
                                },
                            }));
                        }
                    >{apply_label.clone()}</button>
                </div>
            </details>

            {move || {
                let catalog = list_runtime.controller.with(|controller| {
                    ContextSchemaCatalog::from_document(controller.editor().document())
                });
                view! {
                    <div class="space-y-2 border-t border-border pt-2">
                        {catalog.fields.into_iter().map(|field| {
                            let remove_runtime = list_runtime.clone();
                            let id = field.id.clone();
                            let remove_label = remove_label.clone();
                            view! {
                                <div class="flex items-start justify-between gap-2 rounded bg-muted/50 px-2 py-1 text-xs">
                                    <div class="min-w-0">
                                        <strong class="block truncate">{field.path}</strong>
                                        <span class="block truncate text-muted-foreground">
                                            {format!("{}{}", field.kind.as_str(), if field.required { " · required" } else { "" })}
                                        </span>
                                    </div>
                                    <button
                                        type="button"
                                        class="shrink-0 text-destructive"
                                        on:click=move |_| remove_runtime.dispatch(
                                            UiIntent::execute(EditorCommand::Context {
                                                command: ContextCommand::RemoveField {
                                                    field_id: id.clone(),
                                                },
                                            })
                                        )
                                    >{remove_label}</button>
                                </div>
                            }
                        }).collect_view()}
                        {catalog.computed.into_iter().map(|computed| {
                            let remove_runtime = list_runtime.clone();
                            let id = computed.id.clone();
                            let remove_label = remove_label.clone();
                            view! {
                                <div class="flex items-start justify-between gap-2 rounded bg-muted/50 px-2 py-1 text-xs">
                                    <div class="min-w-0">
                                        <strong class="block truncate">{computed.path}</strong>
                                        <span class="block truncate text-muted-foreground">"Computed expression"</span>
                                    </div>
                                    <button
                                        type="button"
                                        class="shrink-0 text-destructive"
                                        on:click=move |_| remove_runtime.dispatch(
                                            UiIntent::execute(EditorCommand::Context {
                                                command: ContextCommand::RemoveComputed {
                                                    computed_id: id.clone(),
                                                },
                                            })
                                        )
                                    >{remove_label}</button>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                }
            }}

            {move || {
                let materialized = report_runtime.controller.with(|controller| {
                    materialize_project_with_runtime_context(
                        controller.editor().document(),
                        &report_runtime.runtime_context.get(),
                    )
                });
                view! {
                    <div class="space-y-1 border-t border-border pt-2 text-xs text-muted-foreground">
                        <p>{format!(
                            "{} defaults · {} computed · {} fallbacks · {} unresolved · {} type mismatches",
                            materialized.defaults_applied,
                            materialized.computed_applied,
                            materialized.computed_fallbacks,
                            materialized.unresolved_computed,
                            materialized.context_type_mismatches,
                        )}</p>
                        {materialized.diagnostics.into_iter().filter(|diagnostic| {
                            diagnostic.code.starts_with("runtime_context")
                                || diagnostic.code.starts_with("runtime_computed")
                        }).take(8).map(|diagnostic| view! {
                            <p class="rounded bg-muted px-2 py-1">
                                <strong>{diagnostic.code}</strong>
                                <span class="ml-1">{diagnostic.message}</span>
                            </p>
                        }).collect_view()}
                    </div>
                }
            }}
        </section>
    }
}

fn parse_kind(value: &str) -> ContextValueKind {
    match value {
        "null" => ContextValueKind::Null,
        "boolean" => ContextValueKind::Boolean,
        "number" => ContextValueKind::Number,
        "string" => ContextValueKind::String,
        "object" => ContextValueKind::Object,
        "array" => ContextValueKind::Array,
        _ => ContextValueKind::Any,
    }
}

fn parse_optional_kind(value: &str) -> Option<ContextValueKind> {
    (!value.trim().is_empty()).then(|| parse_kind(value))
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
