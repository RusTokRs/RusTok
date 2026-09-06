use crate::editor::AdminEditorRuntime;
use crate::{
    PageBuilderContributionHostContext, PageBuilderContributionPropertyIssue,
    PageBuilderContributionPropertySchemaRequest, PageBuilderContributionPropertyValidationRequest,
};
use fly::{ComponentPatch, EditorCommand};
use fly_ui::{ContributionAssemblyResult, UiIntent};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde_json::{Map, Number, Value};
use std::collections::BTreeSet;
use std::sync::Arc;

const MAX_OWNER_PROPERTY_FIELDS: usize = 32;

#[derive(Debug, Clone, PartialEq)]
struct SelectedContributionPropertyContract {
    provider: String,
    component_type: String,
    component_id: String,
    property_schema: Value,
    props: Value,
}

#[derive(Debug, Clone)]
struct LoadedContributionProperties {
    component_id: String,
    provider: String,
    component_type: String,
    property_schema: Value,
    schema_id: String,
    fields: Vec<ContributionPropertyField>,
    values: Map<String, Value>,
}

#[derive(Debug, Clone)]
struct ContributionPropertyField {
    id: String,
    label: String,
    required: bool,
    min_length: Option<usize>,
    max_length: Option<usize>,
    kind: ContributionPropertyFieldKind,
}

#[derive(Debug, Clone)]
enum ContributionPropertyFieldKind {
    Text {
        format: Option<String>,
    },
    Select {
        options: Vec<String>,
    },
    Integer {
        minimum: Option<u64>,
        maximum: Option<u64>,
    },
    Boolean,
}

#[component]
pub fn ContributionPropertiesPanel(
    runtime: AdminEditorRuntime,
    #[prop(optional_no_strip)] contribution_assembly: Option<Arc<ContributionAssemblyResult>>,
) -> impl IntoView {
    let Some(host) = use_context::<PageBuilderContributionHostContext>() else {
        return ().into_any();
    };
    if host.is_empty() {
        return ().into_any();
    }
    let Some(assembly) = contribution_assembly else {
        return ().into_any();
    };

    let request_runtime = runtime.clone();
    let request_host = host.clone();
    let request_assembly = assembly.clone();
    let selected_request = Signal::derive(move || {
        selected_property_request(&request_runtime, &request_assembly, &request_host)
    });
    let loaded = RwSignal::new(None::<LoadedContributionProperties>);
    let busy = RwSignal::new(false);
    let saved = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let issues = RwSignal::new(Vec::<PageBuilderContributionPropertyIssue>::new());

    let load_host = host.clone();
    let on_load = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some(request) = selected_request.get_untracked() else {
            error.set(Some(
                "Selected component has no admitted owner property contract".to_string(),
            ));
            return;
        };
        let Some(port) = load_host.property_port(&request.provider) else {
            error.set(Some(format!(
                "Property provider `{}` is not mounted",
                request.provider
            )));
            return;
        };

        loaded.set(None);
        busy.set(true);
        saved.set(false);
        error.set(None);
        issues.set(Vec::new());
        spawn_local(async move {
            let response = port
                .schema(PageBuilderContributionPropertySchemaRequest {
                    provider: request.provider.clone(),
                    component_type: request.component_type.clone(),
                    component_id: request.component_id.clone(),
                    property_schema: request.property_schema.clone(),
                })
                .await;
            match response {
                Ok(schema) => match parse_owner_property_schema(&schema.schema, &request.props) {
                    Ok((fields, values)) => {
                        if selected_request.get_untracked().as_ref() != Some(&request) {
                            error.set(Some(
                                "The selected component changed while its owner schema was loading; reload properties"
                                    .to_string(),
                            ));
                            loaded.set(None);
                        } else if let Some(expected_schema_id) = request
                            .property_schema
                            .get("schema_id")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            && expected_schema_id != schema.schema_id.trim()
                        {
                            error.set(Some(format!(
                                "Owner schema `{}` does not match contribution schema `{expected_schema_id}`",
                                schema.schema_id
                            )));
                            loaded.set(None);
                        } else {
                            loaded.set(Some(LoadedContributionProperties {
                                component_id: request.component_id,
                                provider: request.provider,
                                component_type: request.component_type,
                                property_schema: request.property_schema,
                                schema_id: schema.schema_id,
                                fields,
                                values,
                            }));
                        }
                    }
                    Err(schema_error) => {
                        loaded.set(None);
                        error.set(Some(schema_error));
                    }
                },
                Err(load_error) => {
                    loaded.set(None);
                    error.set(Some(load_error.to_string()));
                }
            }
            busy.set(false);
        });
    };

    let form_runtime = runtime.clone();
    let save_host = host;
    let on_save = Callback::new(move |_: ()| {
        if busy.get_untracked() {
            return;
        }
        let Some(current) = loaded.get_untracked() else {
            error.set(Some(
                "Load the owner property schema before applying values".to_string(),
            ));
            return;
        };
        let Some(selected) = selected_request.get_untracked() else {
            error.set(Some(
                "The selected contribution property contract is unavailable".to_string(),
            ));
            return;
        };
        if selected.component_id != current.component_id
            || selected.provider != current.provider
            || selected.component_type != current.component_type
            || selected.property_schema != current.property_schema
        {
            error.set(Some(
                "The selected component changed; reload its owner property schema".to_string(),
            ));
            return;
        }
        let Some(port) = save_host.property_port(&current.provider) else {
            error.set(Some(format!(
                "Property provider `{}` is not mounted",
                current.provider
            )));
            return;
        };
        let baseline_project_hash = form_runtime
            .controller
            .with(|controller| controller.editor().revision().project_hash.hex());

        busy.set(true);
        saved.set(false);
        error.set(None);
        issues.set(Vec::new());
        let runtime = form_runtime.clone();
        spawn_local(async move {
            let response = port
                .validate(PageBuilderContributionPropertyValidationRequest {
                    provider: current.provider.clone(),
                    component_type: current.component_type.clone(),
                    component_id: current.component_id.clone(),
                    property_schema: current.property_schema.clone(),
                    props: Value::Object(current.values.clone()),
                })
                .await;
            match response {
                Ok(validation) => {
                    issues.set(validation.issues.clone());
                    if !validation.valid {
                        error.set(Some(
                            "Owner validation rejected the current widget properties".to_string(),
                        ));
                    } else if !validation.normalized_props.is_object() {
                        error.set(Some(
                            "Owner validation returned non-object normalized properties"
                                .to_string(),
                        ));
                    } else {
                        let selection_still_matches = runtime.controller.with(|controller| {
                            controller.ui().state.selection.component_id.as_deref()
                                == Some(current.component_id.as_str())
                                && controller.editor().revision().project_hash.hex()
                                    == baseline_project_hash
                        });
                        if !selection_still_matches {
                            error.set(Some(
                                "The selected component or Fly document changed while owner validation was running; properties were not applied"
                                    .to_string(),
                            ));
                        } else {
                            let normalized_props = validation.normalized_props.clone();
                            runtime.dispatch(UiIntent::execute(EditorCommand::Patch {
                                component_id: current.component_id.clone(),
                                patch: ComponentPatch::default()
                                    .set_field("props", normalized_props.clone()),
                            }));
                            if runtime.last_error.get_untracked().is_none() {
                                if let Value::Object(values) = normalized_props {
                                    loaded.update(|loaded| {
                                        if let Some(loaded) = loaded.as_mut() {
                                            loaded.values = values;
                                        }
                                    });
                                }
                                saved.set(true);
                                runtime.announce(
                                    "Owner-normalized component properties applied to Fly draft",
                                );
                            }
                        }
                    }
                }
                Err(validate_error) => error.set(Some(validate_error.to_string())),
            }
            busy.set(false);
        });
    });

    view! {
        <section
            class="space-y-3 rounded-xl border border-border bg-card p-3"
            data-page-builder-contribution-properties="true"
        >
            <div class="flex items-start justify-between gap-3">
                <div>
                    <h2 class="text-sm font-semibold text-card-foreground">"Component properties"</h2>
                    <p class="mt-1 text-xs text-muted-foreground">
                        "Loads the provider-owned schema and validates configuration before updating the Fly draft."
                    </p>
                </div>
                <button
                    type="button"
                    class="rounded border border-border px-3 py-1.5 text-sm disabled:opacity-50"
                    disabled=move || busy.get() || selected_request.get().is_none()
                    on:click=on_load
                >
                    {move || if busy.get() && loaded.get().is_none() { "Loading..." } else { "Load schema" }}
                </button>
            </div>

            {move || match selected_request.get() {
                Some(request) => view! {
                    <p class="text-xs text-muted-foreground" data-page-builder-contribution-property-provider=request.provider.clone()>
                        {format!("{} · {}", request.provider, request.component_type)}
                    </p>
                }.into_any(),
                None => view! {
                    <p class="text-xs text-muted-foreground">
                        "Select a component with an admitted owner-backed property editor."
                    </p>
                }.into_any(),
            }}

            {move || loaded.get().map(|current| {
                let values = loaded;
                let fields = current.fields.clone();
                let schema_id = current.schema_id.clone();
                let save_callback = on_save;
                view! {
                    <div class="space-y-3" data-page-builder-contribution-property-schema=schema_id.clone()>
                        <p class="text-[11px] text-muted-foreground">{format!("Schema: {schema_id}")}</p>
                        {fields.into_iter().map(|field| property_field_view(field, values, saved)).collect_view()}
                        <button
                            type="button"
                            class="rounded bg-primary px-3 py-2 text-sm font-medium text-primary-foreground disabled:opacity-50"
                            disabled=move || busy.get()
                            on:click=move |_| save_callback.run(())
                        >
                            {move || if busy.get() { "Validating..." } else { "Apply normalized properties" }}
                        </button>
                    </div>
                }.into_any()
            })}

            {move || (!issues.get().is_empty()).then(|| {
                let current = issues.get();
                view! {
                    <div class="space-y-1" data-page-builder-contribution-property-issues="true">
                        {current.into_iter().map(|issue| view! {
                            <p class=if issue.class == "validation" {
                                "text-xs text-destructive"
                            } else {
                                "text-xs text-amber-700"
                            }>
                                {format!("{}: {}", issue.path.unwrap_or_else(|| "props".to_string()), issue.message)}
                            </p>
                        }).collect_view()}
                    </div>
                }
            })}
            {move || error.get().map(|message| view! {
                <div
                    class="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive"
                    role="alert"
                >{message}</div>
            })}
            {move || saved.get().then(|| view! {
                <div
                    class="rounded-lg border border-emerald-300/40 bg-emerald-50 px-3 py-2 text-xs text-emerald-800"
                    role="status"
                >"Owner-normalized properties applied to the Fly draft"</div>
            })}
        </section>
    }
    .into_any()
}

fn property_field_view(
    field: ContributionPropertyField,
    loaded: RwSignal<Option<LoadedContributionProperties>>,
    saved: RwSignal<bool>,
) -> AnyView {
    let field_id = field.id.clone();
    let label = if field.required {
        format!("{} *", field.label)
    } else {
        field.label.clone()
    };
    let input_id = format!("fly-owner-property-{}", field.id);

    match field.kind {
        ContributionPropertyFieldKind::Boolean => {
            let read_id = field_id.clone();
            let write_id = field_id;
            view! {
                <label class="flex items-center gap-2 text-sm" for=input_id.clone()>
                    <input
                        id=input_id.clone()
                        type="checkbox"
                        prop:checked=move || loaded.with(|loaded| loaded.as_ref()
                            .and_then(|loaded| loaded.values.get(&read_id))
                            .and_then(Value::as_bool)
                            .unwrap_or(false))
                        on:change=move |event| {
                            saved.set(false);
                            let checked = event_target_checked(&event);
                            loaded.update(|loaded| {
                                if let Some(loaded) = loaded.as_mut() {
                                    loaded.values.insert(write_id.clone(), Value::Bool(checked));
                                }
                            });
                        }
                    />
                    <span>{label}</span>
                </label>
            }
            .into_any()
        }
        ContributionPropertyFieldKind::Select { options } => {
            let read_id = field_id.clone();
            let write_id = field_id;
            view! {
                <label class="block text-sm font-medium" for=input_id.clone()>
                    {label}
                    <select
                        id=input_id.clone()
                        class="mt-1 w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        prop:value=move || loaded.with(|loaded| loaded.as_ref()
                            .and_then(|loaded| loaded.values.get(&read_id))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string())
                        on:change=move |event| {
                            saved.set(false);
                            let value = event_target_value(&event);
                            loaded.update(|loaded| {
                                if let Some(loaded) = loaded.as_mut() {
                                    loaded.values.insert(write_id.clone(), Value::String(value));
                                }
                            });
                        }
                    >
                        {options.into_iter().map(|option| {
                            let opt_val = option.clone();
                            view! {
                                <option value=opt_val>{option}</option>
                            }
                        }).collect_view()}
                    </select>
                </label>
            }
            .into_any()
        }
        ContributionPropertyFieldKind::Integer { minimum, maximum } => {
            let read_id = field_id.clone();
            let write_id = field_id;
            view! {
                <label class="block text-sm font-medium" for=input_id.clone()>
                    {label}
                    <input
                        id=input_id.clone()
                        type="number"
                        min=minimum.map(|value| value.to_string()).unwrap_or_default()
                        max=maximum.map(|value| value.to_string()).unwrap_or_default()
                        class="mt-1 w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        prop:value=move || loaded.with(|loaded| loaded.as_ref()
                            .and_then(|loaded| loaded.values.get(&read_id))
                            .and_then(Value::as_u64)
                            .map(|value| value.to_string())
                            .unwrap_or_default())
                        on:input=move |event| {
                            saved.set(false);
                            let raw = event_target_value(&event);
                            loaded.update(|loaded| {
                                if let Some(loaded) = loaded.as_mut() {
                                    if raw.trim().is_empty() {
                                        loaded.values.remove(&write_id);
                                    } else if let Ok(value) = raw.parse::<u64>() {
                                        loaded.values.insert(
                                            write_id.clone(),
                                            Value::Number(Number::from(value)),
                                        );
                                    } else {
                                        loaded.values.insert(write_id.clone(), Value::String(raw.clone()));
                                    }
                                }
                            });
                        }
                    />
                </label>
            }
            .into_any()
        }
        ContributionPropertyFieldKind::Text { format } => {
            let read_id = field_id.clone();
            let write_id = field_id;
            let placeholder = match format.as_deref() {
                Some("uuid") => "UUID",
                _ => "",
            };
            view! {
                <label class="block text-sm font-medium" for=input_id.clone()>
                    {label}
                    <input
                        id=input_id.clone()
                        type="text"
                        placeholder=placeholder
                        minlength=field.min_length.map(|value| value.to_string()).unwrap_or_default()
                        maxlength=field.max_length.map(|value| value.to_string()).unwrap_or_default()
                        class="mt-1 w-full rounded border border-input bg-background px-2 py-1 text-sm"
                        prop:value=move || loaded.with(|loaded| loaded.as_ref()
                            .and_then(|loaded| loaded.values.get(&read_id))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string())
                        on:input=move |event| {
                            saved.set(false);
                            let value = event_target_value(&event);
                            loaded.update(|loaded| {
                                if let Some(loaded) = loaded.as_mut() {
                                    if value.is_empty() {
                                        loaded.values.remove(&write_id);
                                    } else {
                                        loaded.values.insert(write_id.clone(), Value::String(value));
                                    }
                                }
                            });
                        }
                    />
                </label>
            }
            .into_any()
        }
    }
}

fn selected_property_request(
    runtime: &AdminEditorRuntime,
    assembly: &ContributionAssemblyResult,
    host: &PageBuilderContributionHostContext,
) -> Option<SelectedContributionPropertyContract> {
    runtime.controller.with(|controller| {
        let component_id = controller.ui().state.selection.component_id.as_deref()?;
        let component = controller.editor().document().component(component_id)?;
        let provider = component.provider.as_deref()?.trim();
        let component_type = component.component_type().trim();
        if provider.is_empty()
            || component_type.is_empty()
            || host.property_port(provider).is_none()
        {
            return None;
        }
        let property_editor = assembly.registry.iter().find_map(|(_, contribution)| {
            contribution.property_editors.iter().find(|editor| {
                editor.provider == provider && editor.component_type == component_type
            })
        })?;
        let props = component
            .extensions
            .get("props")
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
        if !props.is_object() {
            return None;
        }
        Some(SelectedContributionPropertyContract {
            provider: provider.to_string(),
            component_type: component_type.to_string(),
            component_id: component_id.to_string(),
            property_schema: property_editor.property_schema.clone(),
            props,
        })
    })
}

fn parse_owner_property_schema(
    schema: &Value,
    current_props: &Value,
) -> Result<(Vec<ContributionPropertyField>, Map<String, Value>), String> {
    let object = schema
        .as_object()
        .ok_or_else(|| "Owner property schema must be a JSON object".to_string())?;
    if object.get("type").and_then(Value::as_str) != Some("object") {
        return Err("Owner property schema must describe an object".to_string());
    }
    if object.get("additionalProperties").and_then(Value::as_bool) != Some(false) {
        return Err(
            "Owner property schema must explicitly forbid additional properties".to_string(),
        );
    }
    let properties = object
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| "Owner property schema requires a properties object".to_string())?;
    if properties.is_empty() || properties.len() > MAX_OWNER_PROPERTY_FIELDS {
        return Err(format!(
            "Owner property schema must contain between 1 and {MAX_OWNER_PROPERTY_FIELDS} fields"
        ));
    }
    let required = match object.get("required") {
        None => BTreeSet::new(),
        Some(required) => required
            .as_array()
            .ok_or_else(|| "Owner property schema `required` must be an array".to_string())?
            .iter()
            .map(|value| {
                value.as_str().map(ToString::to_string).ok_or_else(|| {
                    "Owner property schema required entries must be strings".to_string()
                })
            })
            .collect::<Result<BTreeSet<_>, _>>()?,
    };
    if !required.iter().all(|field| properties.contains_key(field)) {
        return Err("Owner property schema requires an unknown field".to_string());
    }
    let mut values = current_props
        .as_object()
        .cloned()
        .ok_or_else(|| "Current contribution props must be an object".to_string())?;
    let mut fields = Vec::with_capacity(properties.len());

    for (id, definition) in properties {
        if id.trim().is_empty() || id.len() > 128 {
            return Err("Owner property schema contains an invalid field id".to_string());
        }
        let definition = definition
            .as_object()
            .ok_or_else(|| format!("Owner property `{id}` definition must be an object"))?;
        let property_type = definition
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Owner property `{id}` requires a type"))?;
        let kind = match property_type {
            "string" => {
                if let Some(raw_options) = definition.get("enum") {
                    let options = raw_options
                        .as_array()
                        .ok_or_else(|| format!("Owner property `{id}` enum must be an array"))?;
                    let options = options
                        .iter()
                        .map(|value| {
                            value.as_str().map(ToString::to_string).ok_or_else(|| {
                                format!("Owner property `{id}` enum values must be strings")
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if options.is_empty() {
                        return Err(format!("Owner property `{id}` enum must not be empty"));
                    }
                    ContributionPropertyFieldKind::Select { options }
                } else {
                    let format = match definition.get("format") {
                        None => None,
                        Some(value) => {
                            let format = value.as_str().ok_or_else(|| {
                                format!("Owner property `{id}` format must be a string")
                            })?;
                            match format {
                                "uuid" => Some(format.to_string()),
                                other => {
                                    return Err(format!(
                                        "Owner property `{id}` uses unsupported string format `{other}`"
                                    ));
                                }
                            }
                        }
                    };
                    ContributionPropertyFieldKind::Text { format }
                }
            }
            "integer" => {
                let minimum = optional_u64_constraint(definition, id, "minimum")?;
                let maximum = optional_u64_constraint(definition, id, "maximum")?;
                if minimum
                    .zip(maximum)
                    .is_some_and(|(minimum, maximum)| maximum < minimum)
                {
                    return Err(format!(
                        "Owner property `{id}` maximum must be greater than or equal to minimum"
                    ));
                }
                ContributionPropertyFieldKind::Integer { minimum, maximum }
            }
            "boolean" => ContributionPropertyFieldKind::Boolean,
            other => {
                return Err(format!(
                    "Owner property `{id}` uses unsupported field type `{other}`"
                ));
            }
        };

        if !values.contains_key(id)
            && let Some(default) = definition.get("default").cloned()
        {
            values.insert(id.clone(), default);
        }

        let (min_length, max_length) = if property_type == "string" {
            let min_length = optional_usize_constraint(definition, id, "minLength")?;
            let max_length = optional_usize_constraint(definition, id, "maxLength")?;
            if min_length
                .zip(max_length)
                .is_some_and(|(minimum, maximum)| maximum < minimum)
            {
                return Err(format!(
                    "Owner property `{id}` maxLength must be greater than or equal to minLength"
                ));
            }
            (min_length, max_length)
        } else {
            if definition.contains_key("minLength") || definition.contains_key("maxLength") {
                return Err(format!(
                    "Owner property `{id}` uses string length constraints on a non-string field"
                ));
            }
            (None, None)
        };
        fields.push(ContributionPropertyField {
            id: id.clone(),
            label: humanize_property_id(id),
            required: required.contains(id),
            min_length,
            max_length,
            kind,
        });
    }

    Ok((fields, values))
}

fn optional_u64_constraint(
    definition: &Map<String, Value>,
    field_id: &str,
    constraint: &str,
) -> Result<Option<u64>, String> {
    match definition.get(constraint) {
        None => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            format!("Owner property `{field_id}` {constraint} must be an unsigned integer")
        }),
    }
}

fn optional_usize_constraint(
    definition: &Map<String, Value>,
    field_id: &str,
    constraint: &str,
) -> Result<Option<usize>, String> {
    optional_u64_constraint(definition, field_id, constraint)?
        .map(|value| {
            usize::try_from(value)
                .map_err(|_| format!("Owner property `{field_id}` {constraint} is too large"))
        })
        .transpose()
}

fn humanize_property_id(id: &str) -> String {
    let mut result = String::with_capacity(id.len());
    let mut uppercase_next = true;
    for character in id.chars() {
        if matches!(character, '_' | '-' | '.') {
            result.push(' ');
            uppercase_next = true;
        } else if uppercase_next {
            result.extend(character.to_uppercase());
            uppercase_next = false;
        } else {
            result.push(character);
        }
    }
    result
}
