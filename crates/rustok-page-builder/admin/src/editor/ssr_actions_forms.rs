use crate::AdminCanvasController;
use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    ComponentAction, ComponentForm, ComponentPatch, EditorCommand, FLY_ACTION_FIELD,
    FLY_FORM_FIELD, FLY_PAGE_LINK_FIELD, FormEncoding, FormMethod, ValidationDiagnostic,
    ValidationSeverity, validate_component_actions,
};
use fly_ui::UiIntent;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

const MAX_ACTION_PAYLOAD_BYTES: usize = 64 * 1024;
const MAX_FORM_INPUT_BYTES: usize = 64 * 1024;
const MAX_PATTERN_BYTES: usize = 2 * 1024;
const ACTION_CANONICAL_FIELDS: &[&str] = &[
    "kind",
    "page_id",
    "base_path",
    "query",
    "fragment",
    "fallback_href",
    "href",
    "new_window",
    "form_id",
    "event",
    "payload",
    "provider",
    "action",
    "input",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SsrComponentActionRequest {
    pub component_id: String,
    pub kind: String,
    #[serde(default)]
    pub page_id: String,
    #[serde(default)]
    pub base_path: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub fragment: String,
    #[serde(default)]
    pub fallback_href: String,
    #[serde(default)]
    pub href: String,
    #[serde(default)]
    pub new_window: bool,
    #[serde(default)]
    pub form_id: String,
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub payload_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrComponentActionRemoveRequest {
    pub component_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SsrComponentFormRequest {
    pub component_id: String,
    pub form_id: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub encoding: String,
    #[serde(default)]
    pub action_url: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub input_json: String,
    #[serde(default)]
    pub novalidate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrComponentFormRemoveRequest {
    pub component_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SsrNativeFormFieldRequest {
    pub component_id: String,
    #[serde(default)]
    pub tag_name: String,
    pub name: String,
    #[serde(default)]
    pub dom_id: String,
    #[serde(default)]
    pub field_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub min: String,
    #[serde(default)]
    pub max: String,
    #[serde(default)]
    pub min_length: Option<u64>,
    #[serde(default)]
    pub max_length: Option<u64>,
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub autocomplete: String,
    #[serde(default)]
    pub placeholder: String,
    #[serde(default)]
    pub aria_label: String,
}

impl AdminCanvasController {
    pub fn ssr_component_action_intent(
        &self,
        request: SsrComponentActionRequest,
    ) -> Result<UiIntent, String> {
        let component_id = required(&request.component_id, "component id")?;
        let component = self
            .editor()
            .document()
            .component(component_id)
            .ok_or_else(|| format!("component `{component_id}` does not exist"))?;
        if component.extensions.contains_key(FLY_PAGE_LINK_FIELD) {
            return Err(format!(
                "component `{component_id}` already defines `{FLY_PAGE_LINK_FIELD}`; remove the internal link before adding an action"
            ));
        }
        let preserved_extensions =
            preserved_action_extensions(component.extensions.get(FLY_ACTION_FIELD));
        let mut value = serde_json::to_value(action_from_request(request.clone())?)
            .map_err(|error| format!("component action cannot be encoded: {error}"))?;
        let Value::Object(action) = &mut value else {
            return Err("component action must encode as a JSON object".to_string());
        };
        action.extend(preserved_extensions);
        let patch = validated_contract_patch(self, component_id, FLY_ACTION_FIELD, value)?;
        Ok(UiIntent::execute(EditorCommand::Patch {
            component_id: component_id.to_string(),
            patch,
        }))
    }

    pub fn ssr_remove_component_action_intent(
        &self,
        request: SsrComponentActionRemoveRequest,
    ) -> Result<UiIntent, String> {
        remove_contract_intent(
            self,
            &request.component_id,
            FLY_ACTION_FIELD,
            "component action",
        )
    }

    pub fn ssr_component_form_intent(
        &self,
        request: SsrComponentFormRequest,
    ) -> Result<UiIntent, String> {
        let component_id = required(&request.component_id, "component id")?;
        let component = self
            .editor()
            .document()
            .component(component_id)
            .ok_or_else(|| format!("component `{component_id}` does not exist"))?;
        let extensions = component
            .extensions
            .get(FLY_FORM_FIELD)
            .cloned()
            .and_then(|value| serde_json::from_value::<ComponentForm>(value).ok())
            .map(|form| form.extensions)
            .unwrap_or_default();
        let value = serde_json::to_value(form_from_request(request.clone(), extensions)?)
            .map_err(|error| format!("component form cannot be encoded: {error}"))?;
        let patch = validated_contract_patch(self, component_id, FLY_FORM_FIELD, value)?;
        Ok(UiIntent::execute(EditorCommand::Patch {
            component_id: component_id.to_string(),
            patch,
        }))
    }

    pub fn ssr_remove_component_form_intent(
        &self,
        request: SsrComponentFormRemoveRequest,
    ) -> Result<UiIntent, String> {
        remove_contract_intent(
            self,
            &request.component_id,
            FLY_FORM_FIELD,
            "component form",
        )
    }

    pub fn ssr_native_form_field_intent(
        &self,
        request: SsrNativeFormFieldRequest,
    ) -> Result<UiIntent, String> {
        let component_id = required(&request.component_id, "component id")?;
        ensure_component(self, component_id)?;
        let tag_name = match request.tag_name.trim().to_ascii_lowercase().as_str() {
            "" | "input" => "input",
            "textarea" => "textarea",
            "select" => "select",
            other => {
                return Err(format!(
                    "native form field tag `{other}` is unsupported; use input, textarea, or select"
                ));
            }
        };
        let name = validate_token(&request.name, "field name")?;
        let field_type = if tag_name == "input" {
            validate_input_type(&request.field_type)?
        } else {
            None
        };
        validate_native_field_constraints(tag_name, field_type.as_deref(), &request)?;
        let dom_id = optional_token(request.dom_id, "DOM id")?;

        let mut patch = ComponentPatch::default();
        patch
            .fields
            .insert("tagName".to_string(), Value::String(tag_name.to_string()));
        set_attribute(&mut patch, "name", Some(name));
        set_attribute(&mut patch, "id", dom_id);
        set_attribute(&mut patch, "type", field_type);
        set_boolean_attribute(&mut patch, "required", request.required);
        set_attribute(&mut patch, "min", optional(request.min));
        set_attribute(&mut patch, "max", optional(request.max));
        set_attribute(
            &mut patch,
            "minlength",
            request.min_length.map(|value| value.to_string()),
        );
        set_attribute(
            &mut patch,
            "maxlength",
            request.max_length.map(|value| value.to_string()),
        );
        set_attribute(&mut patch, "pattern", optional(request.pattern));
        set_attribute(&mut patch, "autocomplete", optional(request.autocomplete));
        set_attribute(&mut patch, "placeholder", optional(request.placeholder));
        set_attribute(&mut patch, "aria-label", optional(request.aria_label));
        Ok(UiIntent::execute(EditorCommand::Patch {
            component_id: component_id.to_string(),
            patch,
        }))
    }
}

#[component]
pub fn SsrActionsFormsPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let route_context = use_context::<UiRouteContext>().unwrap_or_default();
        let locale = route_context.locale;
        let title = t(
            locale.as_deref(),
            "page_builder.actionsForms.title",
            "Actions, forms, and fields",
        );
        let empty = t(
            locale.as_deref(),
            "page_builder.actionsForms.empty",
            "Select a component to configure an action, form, or native form field.",
        );
        let action_title = t(
            locale.as_deref(),
            "page_builder.actionsForms.actionTitle",
            "Component action",
        );
        let form_title = t(
            locale.as_deref(),
            "page_builder.actionsForms.formTitle",
            "Component form",
        );
        let field_title = t(
            locale.as_deref(),
            "page_builder.actionsForms.fieldTitle",
            "Native form field",
        );
        let save = t(
            locale.as_deref(),
            "page_builder.actionsForms.save",
            "Save contract",
        );
        let remove = t(
            locale.as_deref(),
            "page_builder.actionsForms.remove",
            "Remove contract",
        );
        let selected_component_id = runtime
            .controller
            .with(|controller| controller.ui().state.selection.component_id.clone());
        let Some(component_id) = selected_component_id else {
            return view! {
                <section class="space-y-2 rounded-xl border border-border bg-card p-3" data-fly-ssr-actions-forms="true">
                    <h2 class="font-semibold">{title}</h2>
                    <p class="text-xs text-muted-foreground">{empty}</p>
                </section>
            }
            .into_any();
        };
        let (current_action, current_form, attributes, tag_name, page_options) =
            runtime.controller.with(|controller| {
                let component = controller.editor().document().component(&component_id);
                let action = component
                    .and_then(|component| component.extensions.get(FLY_ACTION_FIELD))
                    .cloned()
                    .and_then(|value| serde_json::from_value::<ComponentAction>(value).ok());
                let form = component
                    .and_then(|component| component.extensions.get(FLY_FORM_FIELD))
                    .cloned()
                    .and_then(|value| serde_json::from_value::<ComponentForm>(value).ok());
                let attributes = component
                    .map(|component| component.attributes.clone())
                    .unwrap_or_default();
                let tag_name = component
                    .and_then(|component| component.tag_name.clone())
                    .unwrap_or_else(|| "input".to_string());
                (
                    action,
                    form,
                    attributes,
                    tag_name,
                    controller.page_summaries(),
                )
            });
        let action_values = ActionValues::new(current_action.as_ref());
        let form_values = FormValues::new(current_form.as_ref());
        let has_action = current_action.is_some();
        let has_form = current_form.is_some();
        let action_component = component_id.clone();
        let action_remove_component = component_id.clone();
        let form_component = component_id.clone();
        let form_remove_component = component_id.clone();
        let field_component = component_id;
        let save_action = save.clone();
        let save_form = save.clone();
        let remove_action = remove.clone();
        let remove_form = remove;

        view! {
            <section class="space-y-3 rounded-xl border border-border bg-card p-3" data-fly-ssr-actions-forms="true">
                <h2 class="font-semibold">{title}</h2>
                <details class="rounded border border-border p-2">
                    <summary class="cursor-pointer text-xs font-semibold">{action_title}</summary>
                    <form class="mt-3 grid gap-2" data-fly-intent-form="set_component_action">
                        <input type="hidden" name="component_id" value=action_component data-fly-selected-component-input="true"/>
                        <select name="kind" class="rounded border border-input bg-background px-2 py-1 text-xs">
                            {option("navigate_page", "Navigate to page", &action_values.kind)}
                            {option("navigate_url", "Navigate to URL", &action_values.kind)}
                            {option("submit_form", "Submit form", &action_values.kind)}
                            {option("emit_event", "Emit event", &action_values.kind)}
                            {option("provider_action", "Provider action", &action_values.kind)}
                        </select>
                        <select name="page_id" class="rounded border border-input bg-background px-2 py-1 text-xs">
                            <option value="">"Target page"</option>
                            {page_options.into_iter().filter_map(|page| {
                                let page_id = page.id?;
                                let selected = page_id == action_values.page_id;
                                Some(view! { <option value=page_id.clone() selected=selected>{format!("{} ({page_id})", page.name)}</option> })
                            }).collect_view()}
                        </select>
                        <input name="base_path" value=action_values.base_path placeholder="Base path: /" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        <input name="query" value=action_values.query placeholder="Query: source=hero" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        <input name="fragment" value=action_values.fragment placeholder="Fragment: pricing" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        <input name="fallback_href" value=action_values.fallback_href placeholder="Fallback href" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        <input name="href" value=action_values.href placeholder="Navigation URL" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        <label class="flex items-center gap-2 text-xs"><input type="checkbox" name="new_window" value="true" checked=action_values.new_window/><span>"Open in new window"</span></label>
                        <input name="form_id" value=action_values.form_id placeholder="Form id" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        <input name="event" value=action_values.event placeholder="Event name" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        <div class="grid grid-cols-2 gap-2">
                            <input name="provider" value=action_values.provider placeholder="Provider" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                            <input name="action" value=action_values.action placeholder="Provider action" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        </div>
                        <textarea name="payload_json" class="min-h-20 rounded border border-input bg-background px-2 py-1 font-mono text-xs" placeholder="Payload/input JSON">{action_values.payload_json}</textarea>
                        <button type="submit" class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary">{save_action}</button>
                    </form>
                    {has_action.then(|| view! {
                        <form class="mt-2" data-fly-intent-form="remove_component_action">
                            <input type="hidden" name="component_id" value=action_remove_component data-fly-selected-component-input="true"/>
                            <button type="submit" class="rounded border border-destructive/40 px-2 py-1 text-xs text-destructive">{remove_action}</button>
                        </form>
                    })}
                </details>

                <details class="rounded border border-border p-2">
                    <summary class="cursor-pointer text-xs font-semibold">{form_title}</summary>
                    <form class="mt-3 grid gap-2" data-fly-intent-form="set_component_form">
                        <input type="hidden" name="component_id" value=form_component data-fly-selected-component-input="true"/>
                        <input required name="form_id" value=form_values.id placeholder="Stable form id" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        <div class="grid grid-cols-2 gap-2">
                            <select name="method" class="rounded border border-input bg-background px-2 py-1 text-xs">
                                {option("get", "GET", &form_values.method)}
                                {option("post", "POST", &form_values.method)}
                                {option("dialog", "Dialog", &form_values.method)}
                            </select>
                            <select name="encoding" class="rounded border border-input bg-background px-2 py-1 text-xs">
                                {option("url_encoded", "URL encoded", &form_values.encoding)}
                                {option("multipart", "Multipart", &form_values.encoding)}
                                {option("text_plain", "Text plain", &form_values.encoding)}
                            </select>
                        </div>
                        <input name="action_url" value=form_values.action_url placeholder="Native action URL" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        <div class="grid grid-cols-2 gap-2">
                            <input name="provider" value=form_values.provider placeholder="Provider" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                            <input name="action" value=form_values.action placeholder="Provider action" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        </div>
                        <textarea name="input_json" class="min-h-20 rounded border border-input bg-background px-2 py-1 font-mono text-xs" placeholder="Provider input JSON">{form_values.input_json}</textarea>
                        <label class="flex items-center gap-2 text-xs"><input type="checkbox" name="novalidate" value="true" checked=form_values.novalidate/><span>"Disable browser validation"</span></label>
                        <button type="submit" class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary">{save_form}</button>
                    </form>
                    {has_form.then(|| view! {
                        <form class="mt-2" data-fly-intent-form="remove_component_form">
                            <input type="hidden" name="component_id" value=form_remove_component data-fly-selected-component-input="true"/>
                            <button type="submit" class="rounded border border-destructive/40 px-2 py-1 text-xs text-destructive">{remove_form}</button>
                        </form>
                    })}
                </details>

                <details class="rounded border border-border p-2">
                    <summary class="cursor-pointer text-xs font-semibold">{field_title}</summary>
                    <form class="mt-3 grid gap-2" data-fly-intent-form="set_native_form_field">
                        <input type="hidden" name="component_id" value=field_component data-fly-selected-component-input="true"/>
                        <div class="grid grid-cols-2 gap-2">
                            <select name="tag_name" class="rounded border border-input bg-background px-2 py-1 text-xs">
                                {option("input", "input", &tag_name)}
                                {option("textarea", "textarea", &tag_name)}
                                {option("select", "select", &tag_name)}
                            </select>
                            <input name="field_type" value=attribute(&attributes, "type") placeholder="email, text, number..." class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        </div>
                        <div class="grid grid-cols-2 gap-2">
                            <input required name="name" value=attribute(&attributes, "name") placeholder="Field name" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                            <input name="dom_id" value=attribute(&attributes, "id") placeholder="DOM id" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        </div>
                        <label class="flex items-center gap-2 text-xs"><input type="checkbox" name="required" value="true" checked=boolean_attribute(&attributes, "required")/><span>"Required"</span></label>
                        <div class="grid grid-cols-2 gap-2">
                            <input name="min" value=attribute(&attributes, "min") placeholder="Min" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                            <input name="max" value=attribute(&attributes, "max") placeholder="Max" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                            <input type="number" min="0" name="min_length" value=attribute(&attributes, "minlength") placeholder="Min length" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                            <input type="number" min="0" name="max_length" value=attribute(&attributes, "maxlength") placeholder="Max length" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        </div>
                        <input name="pattern" value=attribute(&attributes, "pattern") placeholder="HTML pattern" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        <input name="autocomplete" value=attribute(&attributes, "autocomplete") placeholder="Autocomplete: email" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        <input name="placeholder" value=attribute(&attributes, "placeholder") placeholder="Placeholder" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        <input name="aria_label" value=attribute(&attributes, "aria-label") placeholder="Accessible label" class="rounded border border-input bg-background px-2 py-1 text-xs"/>
                        <button type="submit" class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary">{save}</button>
                    </form>
                </details>
            </section>
        }
        .into_any()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = runtime;
        view! { <span hidden data-fly-ssr-actions-forms="disabled"></span> }.into_any()
    }
}

#[derive(Default)]
struct ActionValues {
    kind: String,
    page_id: String,
    base_path: String,
    query: String,
    fragment: String,
    fallback_href: String,
    href: String,
    new_window: bool,
    form_id: String,
    event: String,
    provider: String,
    action: String,
    payload_json: String,
}

impl ActionValues {
    fn new(action: Option<&ComponentAction>) -> Self {
        let mut values = Self {
            kind: "navigate_page".to_string(),
            payload_json: "{}".to_string(),
            ..Self::default()
        };
        match action {
            Some(ComponentAction::NavigatePage {
                page_id,
                base_path,
                query,
                fragment,
                fallback_href,
            }) => {
                values.page_id = page_id.clone();
                values.base_path = base_path.clone().unwrap_or_default();
                values.query = query.clone().unwrap_or_default();
                values.fragment = fragment.clone().unwrap_or_default();
                values.fallback_href = fallback_href.clone().unwrap_or_default();
            }
            Some(ComponentAction::NavigateUrl { href, new_window }) => {
                values.kind = "navigate_url".to_string();
                values.href = href.clone();
                values.new_window = *new_window;
            }
            Some(ComponentAction::SubmitForm { form_id }) => {
                values.kind = "submit_form".to_string();
                values.form_id = form_id.clone();
            }
            Some(ComponentAction::EmitEvent { event, payload }) => {
                values.kind = "emit_event".to_string();
                values.event = event.clone();
                values.payload_json = pretty_json(payload);
            }
            Some(ComponentAction::ProviderAction {
                provider,
                action,
                input,
            }) => {
                values.kind = "provider_action".to_string();
                values.provider = provider.clone();
                values.action = action.clone();
                values.payload_json = pretty_json(input);
            }
            None => {}
        }
        values
    }
}

#[derive(Default)]
struct FormValues {
    id: String,
    method: String,
    encoding: String,
    action_url: String,
    provider: String,
    action: String,
    input_json: String,
    novalidate: bool,
}

impl FormValues {
    fn new(form: Option<&ComponentForm>) -> Self {
        match form {
            Some(form) => Self {
                id: form.id.clone(),
                method: form.method.as_str().to_string(),
                encoding: match form.encoding {
                    FormEncoding::UrlEncoded => "url_encoded",
                    FormEncoding::Multipart => "multipart",
                    FormEncoding::TextPlain => "text_plain",
                }
                .to_string(),
                action_url: form.action_url.clone().unwrap_or_default(),
                provider: form.provider.clone().unwrap_or_default(),
                action: form.action.clone().unwrap_or_default(),
                input_json: pretty_json(&form.input),
                novalidate: form.novalidate,
            },
            None => Self {
                method: "get".to_string(),
                encoding: "url_encoded".to_string(),
                input_json: "{}".to_string(),
                ..Self::default()
            },
        }
    }
}

fn action_from_request(request: SsrComponentActionRequest) -> Result<ComponentAction, String> {
    match request.kind.trim().to_ascii_lowercase().as_str() {
        "navigate_page" => Ok(ComponentAction::NavigatePage {
            page_id: validate_token(&request.page_id, "target page id")?,
            base_path: optional(request.base_path),
            query: optional(request.query),
            fragment: optional(request.fragment),
            fallback_href: optional(request.fallback_href),
        }),
        "navigate_url" => Ok(ComponentAction::NavigateUrl {
            href: required_owned(request.href, "navigation URL")?,
            new_window: request.new_window,
        }),
        "submit_form" => Ok(ComponentAction::SubmitForm {
            form_id: validate_token(&request.form_id, "form id")?,
        }),
        "emit_event" => Ok(ComponentAction::EmitEvent {
            event: validate_token(&request.event, "event name")?,
            payload: parse_json(
                &request.payload_json,
                "event payload",
                MAX_ACTION_PAYLOAD_BYTES,
            )?,
        }),
        "provider_action" => Ok(ComponentAction::ProviderAction {
            provider: validate_token(&request.provider, "provider")?,
            action: validate_token(&request.action, "provider action")?,
            input: parse_json(
                &request.payload_json,
                "provider input",
                MAX_ACTION_PAYLOAD_BYTES,
            )?,
        }),
        other => Err(format!("component action kind `{other}` is unsupported")),
    }
}

fn form_from_request(
    request: SsrComponentFormRequest,
    extensions: Map<String, Value>,
) -> Result<ComponentForm, String> {
    let method = match request.method.trim().to_ascii_lowercase().as_str() {
        "" | "get" => FormMethod::Get,
        "post" => FormMethod::Post,
        "dialog" => FormMethod::Dialog,
        other => return Err(format!("form method `{other}` is unsupported")),
    };
    let encoding = match request.encoding.trim().to_ascii_lowercase().as_str() {
        "" | "url_encoded" => FormEncoding::UrlEncoded,
        "multipart" => FormEncoding::Multipart,
        "text_plain" => FormEncoding::TextPlain,
        other => return Err(format!("form encoding `{other}` is unsupported")),
    };
    Ok(ComponentForm {
        id: validate_token(&request.form_id, "form id")?,
        method,
        encoding,
        action_url: optional(request.action_url),
        provider: optional_token(request.provider, "form provider")?,
        action: optional_token(request.action, "form provider action")?,
        input: parse_json(&request.input_json, "form input", MAX_FORM_INPUT_BYTES)?,
        novalidate: request.novalidate,
        extensions,
    })
}

fn preserved_action_extensions(value: Option<&Value>) -> Map<String, Value> {
    value
        .and_then(Value::as_object)
        .map(|action| {
            action
                .iter()
                .filter(|(key, _)| !ACTION_CANONICAL_FIELDS.contains(&key.as_str()))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn validated_contract_patch(
    controller: &AdminCanvasController,
    component_id: &str,
    field: &str,
    value: Value,
) -> Result<ComponentPatch, String> {
    let before = validate_component_actions(controller.editor().document())
        .iter()
        .map(diagnostic_identity)
        .collect::<BTreeSet<_>>();
    let mut candidate = controller.editor().document().clone();
    candidate
        .component_mut(component_id)
        .ok_or_else(|| format!("component `{component_id}` does not exist"))?
        .extensions
        .insert(field.to_string(), value.clone());
    let new_errors = validate_component_actions(&candidate)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
        .filter(|diagnostic| !before.contains(&diagnostic_identity(diagnostic)))
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();
    if !new_errors.is_empty() {
        return Err(new_errors.join("; "));
    }
    Ok(ComponentPatch {
        fields: Map::from_iter([(field.to_string(), value)]),
        ..ComponentPatch::default()
    })
}

fn remove_contract_intent(
    controller: &AdminCanvasController,
    component_id: &str,
    field: &str,
    label: &str,
) -> Result<UiIntent, String> {
    let component_id = required(component_id, "component id")?;
    let component = controller
        .editor()
        .document()
        .component(component_id)
        .ok_or_else(|| format!("component `{component_id}` does not exist"))?;
    if !component.extensions.contains_key(field) {
        return Err(format!(
            "component `{component_id}` does not define {label}"
        ));
    }
    Ok(UiIntent::execute(EditorCommand::Patch {
        component_id: component_id.to_string(),
        patch: ComponentPatch {
            remove_fields: vec![field.to_string()],
            ..ComponentPatch::default()
        },
    }))
}

fn diagnostic_identity(diagnostic: &ValidationDiagnostic) -> (u8, String, String, String) {
    (
        diagnostic.severity as u8,
        diagnostic.code.clone(),
        diagnostic.path.clone(),
        diagnostic.message.clone(),
    )
}

fn ensure_component(controller: &AdminCanvasController, component_id: &str) -> Result<(), String> {
    controller
        .editor()
        .document()
        .component(component_id)
        .is_some()
        .then_some(())
        .ok_or_else(|| format!("component `{component_id}` does not exist"))
}

fn set_attribute(patch: &mut ComponentPatch, name: &str, value: Option<String>) {
    match value {
        Some(value) => {
            patch
                .attributes
                .insert(name.to_string(), Value::String(value));
        }
        None => patch.remove_attributes.push(name.to_string()),
    }
}

fn set_boolean_attribute(patch: &mut ComponentPatch, name: &str, enabled: bool) {
    if enabled {
        patch
            .attributes
            .insert(name.to_string(), Value::String(String::new()));
    } else {
        patch.remove_attributes.push(name.to_string());
    }
}

fn validate_input_type(value: &str) -> Result<Option<String>, String> {
    let value = value.trim().to_ascii_lowercase();
    let value = if value.is_empty() {
        "text"
    } else {
        value.as_str()
    };
    const ALLOWED: &[&str] = &[
        "button",
        "checkbox",
        "color",
        "date",
        "datetime-local",
        "email",
        "file",
        "hidden",
        "image",
        "month",
        "number",
        "password",
        "radio",
        "range",
        "reset",
        "search",
        "submit",
        "tel",
        "text",
        "time",
        "url",
        "week",
    ];
    ALLOWED
        .contains(&value)
        .then(|| Some(value.to_string()))
        .ok_or_else(|| format!("input type `{value}` is unsupported"))
}

fn validate_native_field_constraints(
    tag_name: &str,
    input_type: Option<&str>,
    request: &SsrNativeFormFieldRequest,
) -> Result<(), String> {
    validate_text(&request.min, "minimum value", 256)?;
    validate_text(&request.max, "maximum value", 256)?;
    validate_text(&request.pattern, "pattern", MAX_PATTERN_BYTES)?;
    validate_autocomplete(&request.autocomplete)?;
    validate_text(&request.placeholder, "placeholder", 1024)?;
    validate_text(&request.aria_label, "aria label", 1024)?;
    if request
        .min_length
        .zip(request.max_length)
        .is_some_and(|(minimum, maximum)| minimum > maximum)
    {
        return Err("minimum length cannot exceed maximum length".to_string());
    }

    let has_range = !request.min.trim().is_empty() || !request.max.trim().is_empty();
    let has_length = request.min_length.is_some() || request.max_length.is_some();
    let has_pattern = !request.pattern.trim().is_empty();

    match tag_name {
        "select" => {
            if has_range || has_length || has_pattern {
                return Err(
                    "select fields do not support min, max, minlength, maxlength, or pattern"
                        .to_string(),
                );
            }
        }
        "textarea" => {
            if has_range {
                return Err("textarea fields do not support min or max".to_string());
            }
            if has_pattern {
                return Err("textarea fields do not support pattern".to_string());
            }
        }
        "input" => {
            let input_type = input_type.unwrap_or("text");
            const TEXTUAL_TYPES: &[&str] = &["email", "password", "search", "tel", "text", "url"];
            const RANGE_TYPES: &[&str] = &[
                "date",
                "datetime-local",
                "month",
                "number",
                "range",
                "time",
                "week",
            ];
            const REQUIRED_IGNORED_TYPES: &[&str] =
                &["button", "hidden", "image", "reset", "submit"];

            if has_length && !TEXTUAL_TYPES.contains(&input_type) {
                return Err(format!(
                    "input type `{input_type}` does not support minlength or maxlength"
                ));
            }
            if has_pattern && !TEXTUAL_TYPES.contains(&input_type) {
                return Err(format!(
                    "input type `{input_type}` does not support pattern"
                ));
            }
            if has_range && !RANGE_TYPES.contains(&input_type) {
                return Err(format!(
                    "input type `{input_type}` does not support min or max"
                ));
            }
            if request.required && REQUIRED_IGNORED_TYPES.contains(&input_type) {
                return Err(format!(
                    "input type `{input_type}` does not support required"
                ));
            }
            if matches!(input_type, "number" | "range") {
                let minimum = parse_numeric_bound(&request.min, "minimum value")?;
                let maximum = parse_numeric_bound(&request.max, "maximum value")?;
                if minimum
                    .zip(maximum)
                    .is_some_and(|(minimum, maximum)| minimum > maximum)
                {
                    return Err("minimum value cannot exceed maximum value".to_string());
                }
            }
        }
        _ => unreachable!("native field tag was normalized before validation"),
    }
    Ok(())
}

fn parse_numeric_bound(value: &str, label: &str) -> Result<Option<f64>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let parsed = value
        .parse::<f64>()
        .map_err(|_| format!("{label} `{value}` is not a valid number"))?;
    if !parsed.is_finite() {
        return Err(format!("{label} `{value}` must be finite"));
    }
    Ok(Some(parsed))
}

fn validate_token(value: &str, label: &str) -> Result<String, String> {
    let value = required(value, label)?;
    if !value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':' | '[' | ']')
    }) {
        return Err(format!("{label} `{value}` contains unsupported characters"));
    }
    Ok(value.to_string())
}

fn optional_token(value: String, label: &str) -> Result<Option<String>, String> {
    match optional(value) {
        Some(value) => validate_token(&value, label).map(Some),
        None => Ok(None),
    }
}

fn validate_text(value: &str, label: &str, maximum_bytes: usize) -> Result<(), String> {
    if value.len() > maximum_bytes {
        return Err(format!("{label} exceeds {maximum_bytes} bytes"));
    }
    if value
        .chars()
        .any(|character| matches!(character, '\0' | '\r' | '\n'))
    {
        return Err(format!("{label} contains a forbidden control character"));
    }
    Ok(())
}

fn validate_autocomplete(value: &str) -> Result<(), String> {
    validate_text(value, "autocomplete", 256)?;
    if !value.trim().chars().all(|character| {
        character.is_ascii_alphanumeric() || character.is_ascii_whitespace() || character == '-'
    }) {
        return Err("autocomplete contains unsupported characters".to_string());
    }
    Ok(())
}

fn parse_json(value: &str, label: &str, maximum_bytes: usize) -> Result<Value, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    if value.len() > maximum_bytes {
        return Err(format!("{label} exceeds {maximum_bytes} bytes"));
    }
    serde_json::from_str(value).map_err(|error| format!("{label} JSON is invalid: {error}"))
}

fn required<'a>(value: &'a str, label: &str) -> Result<&'a str, String> {
    let value = value.trim();
    if value.is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(value)
    }
}

fn required_owned(value: String, label: &str) -> Result<String, String> {
    required(&value, label).map(ToString::to_string)
}

fn optional(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
}

fn attribute(attributes: &Map<String, Value>, name: &str) -> String {
    attributes
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn boolean_attribute(attributes: &Map<String, Value>, name: &str) -> bool {
    attributes.get(name).is_some_and(|value| match value {
        Value::Bool(value) => *value,
        Value::String(value) => {
            !matches!(value.to_ascii_lowercase().as_str(), "false" | "0" | "off")
        }
        _ => false,
    })
}

fn option(value: &'static str, label: &'static str, selected_value: &str) -> impl IntoView {
    let selected = value == selected_value;
    view! { <option value=value selected=selected>{label}</option> }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "flyPageMeta": { "slug": "home" },
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{ "id": "cta", "type": "button" }, {
                            "id": "form",
                            "type": "wrapper",
                            "flyForm": { "id": "old", "providerFuture": { "enabled": true } }
                        }, { "id": "field", "type": "input" }]
                    }
                }, {
                    "id": "about",
                    "flyPageMeta": { "slug": "about" },
                    "component": { "id": "about-root", "type": "wrapper" }
                }]
            }),
        )
        .expect("controller")
    }

    #[test]
    fn action_editor_uses_patch_history() {
        let mut controller = controller();
        let intent = controller
            .ssr_component_action_intent(SsrComponentActionRequest {
                component_id: "cta".to_string(),
                kind: "navigate_page".to_string(),
                page_id: "about".to_string(),
                payload_json: "not relevant for navigate_page".to_string(),
                ..SsrComponentActionRequest::default()
            })
            .expect("action intent");
        controller.dispatch(intent).expect("action patch");
        assert_eq!(
            controller
                .editor()
                .document()
                .component("cta")
                .unwrap()
                .extensions[FLY_ACTION_FIELD]["page_id"],
            "about"
        );
        controller.dispatch(UiIntent::Undo).expect("undo action");
        assert!(
            !controller
                .editor()
                .document()
                .component("cta")
                .unwrap()
                .extensions
                .contains_key(FLY_ACTION_FIELD)
        );
    }

    #[test]
    fn action_editor_preserves_unknown_extensions() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::execute(EditorCommand::Patch {
                component_id: "cta".to_string(),
                patch: ComponentPatch {
                    fields: Map::from_iter([(
                        FLY_ACTION_FIELD.to_string(),
                        json!({
                            "kind": "navigate_url",
                            "href": "/current",
                            "providerFuture": { "enabled": true }
                        }),
                    )]),
                    ..ComponentPatch::default()
                },
            }))
            .expect("seed action extension through the controller");
        let intent = controller
            .ssr_component_action_intent(SsrComponentActionRequest {
                component_id: "cta".to_string(),
                kind: "navigate_page".to_string(),
                page_id: "about".to_string(),
                ..SsrComponentActionRequest::default()
            })
            .expect("action intent");
        controller.dispatch(intent).expect("action patch");
        let action = &controller
            .editor()
            .document()
            .component("cta")
            .unwrap()
            .extensions[FLY_ACTION_FIELD];
        assert_eq!(action["page_id"], "about");
        assert_eq!(action["providerFuture"]["enabled"], true);
        assert!(action.get("href").is_none());
    }

    #[test]
    fn form_editor_preserves_unknown_extensions() {
        let mut controller = controller();
        let intent = controller
            .ssr_component_form_intent(SsrComponentFormRequest {
                component_id: "form".to_string(),
                form_id: "contact".to_string(),
                method: "post".to_string(),
                provider: "crm".to_string(),
                action: "create_lead".to_string(),
                input_json: "{\"source\":\"landing\"}".to_string(),
                ..SsrComponentFormRequest::default()
            })
            .expect("form intent");
        controller.dispatch(intent).expect("form patch");
        let form = &controller
            .editor()
            .document()
            .component("form")
            .unwrap()
            .extensions[FLY_FORM_FIELD];
        assert_eq!(form["id"], "contact");
        assert_eq!(form["providerFuture"]["enabled"], true);
    }

    #[test]
    fn native_field_editor_sets_and_clears_html_constraints() {
        let mut controller = controller();
        let intent = controller
            .ssr_native_form_field_intent(SsrNativeFormFieldRequest {
                component_id: "field".to_string(),
                tag_name: "input".to_string(),
                name: "email".to_string(),
                dom_id: "contact-email".to_string(),
                field_type: "email".to_string(),
                required: true,
                min_length: Some(3),
                max_length: Some(120),
                pattern: ".+@.+".to_string(),
                autocomplete: "email".to_string(),
                aria_label: "Email".to_string(),
                ..SsrNativeFormFieldRequest::default()
            })
            .expect("field intent");
        controller.dispatch(intent).expect("field patch");
        let field = controller.editor().document().component("field").unwrap();
        assert_eq!(field.tag_name.as_deref(), Some("input"));
        assert_eq!(field.attributes["name"], "email");
        assert_eq!(field.attributes["minlength"], "3");
        assert_eq!(field.attributes["maxlength"], "120");
        assert_eq!(field.attributes["autocomplete"], "email");
        assert!(field.attributes.contains_key("required"));

        let clear = controller
            .ssr_native_form_field_intent(SsrNativeFormFieldRequest {
                component_id: "field".to_string(),
                tag_name: "textarea".to_string(),
                name: "message".to_string(),
                ..SsrNativeFormFieldRequest::default()
            })
            .expect("clear field intent");
        controller.dispatch(clear).expect("clear field patch");
        let field = controller.editor().document().component("field").unwrap();
        assert_eq!(field.tag_name.as_deref(), Some("textarea"));
        assert!(!field.attributes.contains_key("type"));
        assert!(!field.attributes.contains_key("required"));
        assert!(!field.attributes.contains_key("pattern"));
    }

    #[test]
    fn native_field_editor_rejects_inapplicable_constraints() {
        let controller = controller();
        let number_length = controller
            .ssr_native_form_field_intent(SsrNativeFormFieldRequest {
                component_id: "field".to_string(),
                tag_name: "input".to_string(),
                name: "quantity".to_string(),
                field_type: "number".to_string(),
                min_length: Some(2),
                ..SsrNativeFormFieldRequest::default()
            })
            .expect_err("number minlength must fail");
        assert!(number_length.contains("does not support minlength"));

        let textarea_pattern = controller
            .ssr_native_form_field_intent(SsrNativeFormFieldRequest {
                component_id: "field".to_string(),
                tag_name: "textarea".to_string(),
                name: "message".to_string(),
                pattern: ".+".to_string(),
                ..SsrNativeFormFieldRequest::default()
            })
            .expect_err("textarea pattern must fail");
        assert!(textarea_pattern.contains("do not support pattern"));

        let select_range = controller
            .ssr_native_form_field_intent(SsrNativeFormFieldRequest {
                component_id: "field".to_string(),
                tag_name: "select".to_string(),
                name: "country".to_string(),
                min: "1".to_string(),
                ..SsrNativeFormFieldRequest::default()
            })
            .expect_err("select min must fail");
        assert!(select_range.contains("select fields do not support"));

        let hidden_required = controller
            .ssr_native_form_field_intent(SsrNativeFormFieldRequest {
                component_id: "field".to_string(),
                tag_name: "input".to_string(),
                name: "token".to_string(),
                field_type: "hidden".to_string(),
                required: true,
                ..SsrNativeFormFieldRequest::default()
            })
            .expect_err("hidden required must fail");
        assert!(hidden_required.contains("does not support required"));
    }

    #[test]
    fn numeric_field_editor_rejects_invalid_or_inverted_bounds() {
        let controller = controller();
        let invalid = controller
            .ssr_native_form_field_intent(SsrNativeFormFieldRequest {
                component_id: "field".to_string(),
                tag_name: "input".to_string(),
                name: "amount".to_string(),
                field_type: "number".to_string(),
                min: "not-a-number".to_string(),
                ..SsrNativeFormFieldRequest::default()
            })
            .expect_err("invalid number bound must fail");
        assert!(invalid.contains("is not a valid number"));

        let inverted = controller
            .ssr_native_form_field_intent(SsrNativeFormFieldRequest {
                component_id: "field".to_string(),
                tag_name: "input".to_string(),
                name: "amount".to_string(),
                field_type: "number".to_string(),
                min: "10".to_string(),
                max: "2".to_string(),
                ..SsrNativeFormFieldRequest::default()
            })
            .expect_err("inverted numeric range must fail");
        assert!(inverted.contains("minimum value cannot exceed maximum value"));
    }
}
