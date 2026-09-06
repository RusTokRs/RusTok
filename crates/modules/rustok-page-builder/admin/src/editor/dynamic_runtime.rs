use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    ConditionOperator, DynamicCatalog, DynamicCommand, EditorCommand, EmptyRepeaterBehavior,
    RuntimeCondition, RuntimeRepeater, materialize_runtime,
};
use fly_ui::UiIntent;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::{Map, Value};

#[component]
pub fn DynamicRuntimePanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.dynamicRuntime",
        "Runtime data",
    );
    let apply_context_label = t(
        locale.as_deref(),
        "page_builder.action.applyPreviewContext",
        "Apply preview context",
    );
    let condition_label = t(
        locale.as_deref(),
        "page_builder.dynamic.condition",
        "Visibility condition",
    );
    let repeater_label = t(
        locale.as_deref(),
        "page_builder.dynamic.repeater",
        "Collection repeater",
    );
    let upsert_label = t(locale.as_deref(), "page_builder.action.apply", "Apply");
    let remove_label = t(locale.as_deref(), "page_builder.action.remove", "Remove");
    let no_selection = t(
        locale.as_deref(),
        "page_builder.dynamic.noSelection",
        "Select a component to configure runtime behavior.",
    );

    let context_json = RwSignal::new(
        serde_json::to_string_pretty(&runtime.runtime_context.get_untracked())
            .unwrap_or_else(|_| "{}".to_string()),
    );
    let condition_path = RwSignal::new(String::new());
    let condition_operator = RwSignal::new("truthy".to_string());
    let condition_expected = RwSignal::new(String::new());
    let condition_invert = RwSignal::new(false);
    let repeater_path = RwSignal::new(String::new());
    let item_alias = RwSignal::new("item".to_string());
    let index_alias = RwSignal::new("index".to_string());
    let repeater_limit = RwSignal::new("100".to_string());
    let keep_template = RwSignal::new(false);

    let context_runtime = runtime.clone();
    let condition_runtime = runtime.clone();
    let repeater_runtime = runtime.clone();
    let list_runtime = runtime.clone();
    let report_runtime = runtime;

    view! {
        <section class="space-y-3 rounded-xl border border-border bg-card p-3">
            <h2 class="font-semibold">{title}</h2>

            <details open>
                <summary class="cursor-pointer text-sm font-medium">"Preview JSON context"</summary>
                <div class="mt-2 space-y-2">
                    <textarea
                        class="min-h-36 w-full rounded border border-input bg-background px-2 py-1 font-mono text-xs"
                        prop:value=move || context_json.get()
                        on:input=move |event| context_json.set(event_target_value(&event))
                    ></textarea>
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        on:click=move |_| {
                            match serde_json::from_str::<Value>(&context_json.get_untracked()) {
                                Ok(context) => {
                                    context_runtime.set_runtime_context(context);
                                    context_runtime.last_error.set(None);
                                    context_runtime.announce("Preview runtime context applied");
                                }
                                Err(error) => context_runtime.fail(format!(
                                    "Invalid preview context JSON: {error}"
                                )),
                            }
                        }
                    >{apply_context_label}</button>
                </div>
            </details>

            {move || {
                let selected_id = list_runtime.controller.with(|controller| {
                    controller.ui().state.selection.component_id.clone()
                });
                let Some(component_id) = selected_id else {
                    return view! {
                        <p class="text-sm text-muted-foreground">{no_selection.clone()}</p>
                    }
                    .into_any();
                };
                let catalog = list_runtime.controller.with(|controller| {
                    DynamicCatalog::from_document(controller.editor().document())
                });
                let conditions = catalog
                    .conditions
                    .into_iter()
                    .filter(|condition| condition.component_id == component_id)
                    .collect::<Vec<_>>();
                let repeaters = catalog
                    .repeaters
                    .into_iter()
                    .filter(|repeater| repeater.component_id == component_id)
                    .collect::<Vec<_>>();
                let condition_component_id = component_id.clone();
                let repeater_component_id = component_id.clone();
                let condition_apply_runtime = condition_runtime.clone();
                let repeater_apply_runtime = repeater_runtime.clone();

                view! {
                    <div class="space-y-3 border-t border-border pt-3">
                        <p class="break-all text-xs text-muted-foreground">
                            {format!("Selected: {component_id}")}
                        </p>

                        <div class="space-y-2">
                            <h3 class="text-sm font-medium">{condition_label.clone()}</h3>
                            <input
                                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                                placeholder="flags.visible"
                                prop:value=move || condition_path.get()
                                on:input=move |event| condition_path.set(event_target_value(&event))
                            />
                            <select
                                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                                prop:value=move || condition_operator.get()
                                on:change=move |event| condition_operator.set(event_target_value(&event))
                            >
                                <option value="truthy">"Truthy"</option>
                                <option value="falsy">"Falsy"</option>
                                <option value="exists">"Exists"</option>
                                <option value="equals">"Equals"</option>
                                <option value="not_equals">"Not equals"</option>
                                <option value="contains">"Contains"</option>
                            </select>
                            <input
                                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                                placeholder="Expected JSON or text"
                                prop:value=move || condition_expected.get()
                                on:input=move |event| condition_expected.set(event_target_value(&event))
                            />
                            <label class="flex items-center gap-2 text-xs">
                                <input
                                    type="checkbox"
                                    prop:checked=move || condition_invert.get()
                                    on:change=move |event| condition_invert.set(event_target_checked(&event))
                                />
                                <span>"Invert result"</span>
                            </label>
                            <button
                                type="button"
                                class="rounded border border-border px-2 py-1 text-xs"
                                on:click=move |_| {
                                    let path = condition_path.get_untracked().trim().to_string();
                                    if path.is_empty() {
                                        condition_apply_runtime.fail("Condition path must not be empty");
                                        return;
                                    }
                                    let operator = parse_condition_operator(
                                        &condition_operator.get_untracked()
                                    );
                                    let expected = parse_optional_value(
                                        &condition_expected.get_untracked()
                                    );
                                    let id = condition_apply_runtime.controller.with(|controller| {
                                        DynamicCatalog::from_document(controller.editor().document())
                                            .conditions
                                            .into_iter()
                                            .find(|condition| {
                                                condition.component_id == condition_component_id
                                            })
                                            .map(|condition| condition.id)
                                            .unwrap_or_else(|| format!(
                                                "fly.condition.{}",
                                                stable_suffix(&condition_component_id)
                                            ))
                                    });
                                    condition_apply_runtime.dispatch(UiIntent::execute(
                                        EditorCommand::Dynamic {
                                            command: DynamicCommand::UpsertCondition {
                                                condition: RuntimeCondition {
                                                    id,
                                                    component_id: condition_component_id.clone(),
                                                    path,
                                                    operator,
                                                    expected,
                                                    invert: condition_invert.get_untracked(),
                                                    extensions: Map::new(),
                                                },
                                            },
                                        },
                                    ));
                                }
                            >{upsert_label.clone()}</button>
                            <DefinitionList
                                runtime=list_runtime.clone()
                                conditions
                                repeaters=Vec::new()
                                remove_label=remove_label.clone()
                            />
                        </div>

                        <div class="space-y-2 border-t border-border pt-3">
                            <h3 class="text-sm font-medium">{repeater_label.clone()}</h3>
                            <input
                                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                                placeholder="items"
                                prop:value=move || repeater_path.get()
                                on:input=move |event| repeater_path.set(event_target_value(&event))
                            />
                            <div class="grid grid-cols-2 gap-2">
                                <input
                                    class="rounded border border-input bg-background px-2 py-1 text-sm"
                                    placeholder="item alias"
                                    prop:value=move || item_alias.get()
                                    on:input=move |event| item_alias.set(event_target_value(&event))
                                />
                                <input
                                    class="rounded border border-input bg-background px-2 py-1 text-sm"
                                    placeholder="index alias"
                                    prop:value=move || index_alias.get()
                                    on:input=move |event| index_alias.set(event_target_value(&event))
                                />
                            </div>
                            <input
                                type="number"
                                min="0"
                                max="1000"
                                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                                prop:value=move || repeater_limit.get()
                                on:input=move |event| repeater_limit.set(event_target_value(&event))
                            />
                            <label class="flex items-center gap-2 text-xs">
                                <input
                                    type="checkbox"
                                    prop:checked=move || keep_template.get()
                                    on:change=move |event| keep_template.set(event_target_checked(&event))
                                />
                                <span>"Keep template when collection is empty"</span>
                            </label>
                            <button
                                type="button"
                                class="rounded border border-border px-2 py-1 text-xs"
                                on:click=move |_| {
                                    let path = repeater_path.get_untracked().trim().to_string();
                                    let item = item_alias.get_untracked().trim().to_string();
                                    let index = index_alias.get_untracked().trim().to_string();
                                    if path.is_empty() || item.is_empty() || index.is_empty() {
                                        repeater_apply_runtime.fail(
                                            "Repeater path and aliases must not be empty"
                                        );
                                        return;
                                    }
                                    let limit = repeater_limit
                                        .get_untracked()
                                        .trim()
                                        .parse::<usize>()
                                        .ok();
                                    let id = repeater_apply_runtime.controller.with(|controller| {
                                        DynamicCatalog::from_document(controller.editor().document())
                                            .repeaters
                                            .into_iter()
                                            .find(|repeater| {
                                                repeater.component_id == repeater_component_id
                                            })
                                            .map(|repeater| repeater.id)
                                            .unwrap_or_else(|| format!(
                                                "fly.repeater.{}",
                                                stable_suffix(&repeater_component_id)
                                            ))
                                    });
                                    repeater_apply_runtime.dispatch(UiIntent::execute(
                                        EditorCommand::Dynamic {
                                            command: DynamicCommand::UpsertRepeater {
                                                repeater: RuntimeRepeater {
                                                    id,
                                                    component_id: repeater_component_id.clone(),
                                                    path,
                                                    item_alias: item,
                                                    index_alias: index,
                                                    limit,
                                                    empty_behavior: if keep_template.get_untracked() {
                                                        EmptyRepeaterBehavior::KeepTemplate
                                                    } else {
                                                        EmptyRepeaterBehavior::Hide
                                                    },
                                                    extensions: Map::new(),
                                                },
                                            },
                                        },
                                    ));
                                }
                            >{upsert_label.clone()}</button>
                            <DefinitionList
                                runtime=list_runtime.clone()
                                conditions=Vec::new()
                                repeaters
                                remove_label=remove_label.clone()
                            />
                        </div>
                    </div>
                }
                .into_any()
            }}

            {move || {
                let report = report_runtime.controller.with(|controller| {
                    materialize_runtime(
                        controller.editor().document(),
                        &report_runtime.runtime_context.get(),
                    )
                });
                view! {
                    <div class="space-y-1 border-t border-border pt-3 text-xs text-muted-foreground">
                        <p>{format!(
                            "{} conditions · {} hidden · {} repeated",
                            report.evaluated_conditions,
                            report.hidden_components,
                            report.repeated_nodes,
                        )}</p>
                        {report.diagnostics.into_iter().take(8).map(|diagnostic| view! {
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

#[component]
fn DefinitionList(
    runtime: AdminEditorRuntime,
    conditions: Vec<RuntimeCondition>,
    repeaters: Vec<RuntimeRepeater>,
    remove_label: String,
) -> impl IntoView {
    view! {
        <div class="space-y-1">
            {conditions.into_iter().map(|condition| {
                let remove_runtime = runtime.clone();
                let id = condition.id.clone();
                let remove_label = remove_label.clone();
                view! {
                    <div class="flex items-center justify-between gap-2 rounded bg-muted/50 px-2 py-1 text-xs">
                        <span class="min-w-0 truncate">{format!("{} · {}", condition.path, condition.id)}</span>
                        <button
                            type="button"
                            class="shrink-0 text-destructive"
                            on:click=move |_| remove_runtime.dispatch(UiIntent::execute(
                                EditorCommand::Dynamic {
                                    command: DynamicCommand::RemoveCondition {
                                        condition_id: id.clone(),
                                    },
                                },
                            ))
                        >{remove_label}</button>
                    </div>
                }
            }).collect_view()}
            {repeaters.into_iter().map(|repeater| {
                let remove_runtime = runtime.clone();
                let id = repeater.id.clone();
                let remove_label = remove_label.clone();
                view! {
                    <div class="flex items-center justify-between gap-2 rounded bg-muted/50 px-2 py-1 text-xs">
                        <span class="min-w-0 truncate">{format!("{} · {}", repeater.path, repeater.id)}</span>
                        <button
                            type="button"
                            class="shrink-0 text-destructive"
                            on:click=move |_| remove_runtime.dispatch(UiIntent::execute(
                                EditorCommand::Dynamic {
                                    command: DynamicCommand::RemoveRepeater {
                                        repeater_id: id.clone(),
                                    },
                                },
                            ))
                        >{remove_label}</button>
                    </div>
                }
            }).collect_view()}
        </div>
    }
}

fn parse_condition_operator(value: &str) -> ConditionOperator {
    match value {
        "exists" => ConditionOperator::Exists,
        "equals" => ConditionOperator::Equals,
        "not_equals" => ConditionOperator::NotEquals,
        "falsy" => ConditionOperator::Falsy,
        "contains" => ConditionOperator::Contains,
        _ => ConditionOperator::Truthy,
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
        "component".to_string()
    } else {
        suffix.to_string()
    }
}
