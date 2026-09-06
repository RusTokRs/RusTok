use super::model::{
    ActionMaterialization, ComponentAction, ComponentForm, FLY_ACTION_DATA_ATTRIBUTE,
    FLY_ACTION_FIELD, FLY_ACTION_KIND_ATTRIBUTE, FLY_FORM_FIELD, FormMethod,
    GENERATED_INTERACTION_ATTRIBUTES,
};
use super::validation::{FormIndex, action_diagnostic, collect_form_ids, decode_form};
use crate::{
    ComponentObject, ProjectDocument, RuntimeLocaleSelection, ValidationDiagnostic,
    ValidationSeverity,
    component_visit::visit_project_components_mut,
    interaction_route::{
        InteractionRouteCatalog, build_interaction_href, interaction_locale_candidates,
    },
};
use serde_json::Value;

#[derive(Default)]
struct ActionCounters {
    forms: usize,
    native: usize,
    custom: usize,
    fallback: usize,
    unresolved: usize,
}

struct ActionResolution<'a> {
    routes: &'a InteractionRouteCatalog,
    locale_candidates: &'a [String],
    form_ids: &'a FormIndex,
}

pub fn materialize_component_actions(
    document: &ProjectDocument,
    context: &Value,
) -> ActionMaterialization {
    let routes = InteractionRouteCatalog::from_document(document);
    let locale_candidates =
        interaction_locale_candidates(&RuntimeLocaleSelection::from_context(context));
    let form_ids = collect_form_ids(document);
    let resolution = ActionResolution {
        routes: &routes,
        locale_candidates: &locale_candidates,
        form_ids: &form_ids,
    };
    let mut materialized = document.clone();
    let mut diagnostics = Vec::new();
    let mut counters = ActionCounters::default();

    visit_project_components_mut(&mut materialized.project, |component, visit| {
        materialize_component(
            component,
            visit.path(),
            &resolution,
            &mut diagnostics,
            &mut counters,
        );
    });

    ActionMaterialization {
        document: materialized,
        diagnostics,
        materialized_forms: counters.forms,
        native_actions: counters.native,
        custom_actions: counters.custom,
        fallback_actions: counters.fallback,
        unresolved_actions: counters.unresolved,
    }
}

fn materialize_component(
    component: &mut ComponentObject,
    path: &str,
    resolution: &ActionResolution<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
    counters: &mut ActionCounters,
) {
    let component_id = component.id.clone();

    if let Some(raw) = component.extensions.get(FLY_FORM_FIELD).cloned() {
        clear_interaction_materialization(component);
        match decode_form(raw) {
            Ok(form) => {
                apply_form(component, &form);
                counters.forms = counters.forms.saturating_add(1);
            }
            Err(error) => diagnostics.push(action_diagnostic(
                ValidationSeverity::Warning,
                "runtime_form_invalid",
                path,
                component_id.clone(),
                error,
            )),
        }
    }

    if let Some(raw) = component.extensions.get(FLY_ACTION_FIELD).cloned() {
        clear_interaction_materialization(component);
        match serde_json::from_value::<ComponentAction>(raw) {
            Ok(action) => match apply_action(component, &action, resolution) {
                AppliedAction::Native => counters.native = counters.native.saturating_add(1),
                AppliedAction::Custom => counters.custom = counters.custom.saturating_add(1),
                AppliedAction::Fallback(message) => {
                    counters.fallback = counters.fallback.saturating_add(1);
                    diagnostics.push(action_diagnostic(
                        ValidationSeverity::Info,
                        "runtime_action_fallback_used",
                        path,
                        component_id.clone(),
                        message,
                    ));
                }
                AppliedAction::Unresolved(message) => {
                    counters.unresolved = counters.unresolved.saturating_add(1);
                    diagnostics.push(action_diagnostic(
                        ValidationSeverity::Warning,
                        "runtime_action_unresolved",
                        path,
                        component_id.clone(),
                        message,
                    ));
                }
            },
            Err(error) => {
                counters.unresolved = counters.unresolved.saturating_add(1);
                diagnostics.push(action_diagnostic(
                    ValidationSeverity::Warning,
                    "runtime_action_invalid",
                    path,
                    component_id,
                    format!("component action cannot be decoded: {error}"),
                ));
            }
        }
    }
}

fn clear_interaction_materialization(component: &mut ComponentObject) {
    for attribute in GENERATED_INTERACTION_ATTRIBUTES {
        component.attributes.remove(*attribute);
    }
}

fn apply_form(component: &mut ComponentObject, form: &ComponentForm) {
    component.tag_name = Some("form".to_string());
    component
        .attributes
        .insert("id".to_string(), Value::String(form.id.clone()));
    component.attributes.insert(
        "method".to_string(),
        Value::String(form.method.as_str().to_string()),
    );
    if form.method == FormMethod::Post {
        component.attributes.insert(
            "enctype".to_string(),
            Value::String(form.encoding.as_str().to_string()),
        );
    }
    if let Some(action_url) = form.action_url.as_deref() {
        component.attributes.insert(
            "action".to_string(),
            Value::String(action_url.trim().to_string()),
        );
    }
    if form.novalidate {
        component
            .attributes
            .insert("novalidate".to_string(), Value::String(String::new()));
    }
    if let (Some(provider), Some(action)) = (form.provider.as_deref(), form.action.as_deref()) {
        component.attributes.insert(
            "data-fly-form-provider".to_string(),
            Value::String(provider.trim().to_string()),
        );
        component.attributes.insert(
            "data-fly-form-action".to_string(),
            Value::String(action.trim().to_string()),
        );
        if let Ok(input) = serde_json::to_string(&form.input) {
            component
                .attributes
                .insert("data-fly-form-input".to_string(), Value::String(input));
        }
    }
}

enum AppliedAction {
    Native,
    Custom,
    Fallback(String),
    Unresolved(String),
}

fn apply_action(
    component: &mut ComponentObject,
    action: &ComponentAction,
    resolution: &ActionResolution<'_>,
) -> AppliedAction {
    component.attributes.insert(
        FLY_ACTION_KIND_ATTRIBUTE.to_string(),
        Value::String(action.kind().to_string()),
    );
    match action {
        ComponentAction::NavigatePage {
            page_id,
            base_path,
            query,
            fragment,
            fallback_href,
        } => {
            let Some(page_index) = resolution.routes.page_index(page_id) else {
                return AppliedAction::Unresolved(format!(
                    "target page `{page_id}` does not exist"
                ));
            };
            let href = match resolution
                .routes
                .slug_for(page_index, resolution.locale_candidates)
            {
                Some(slug) => build_interaction_href(
                    base_path.as_deref(),
                    slug,
                    query.as_deref(),
                    fragment.as_deref(),
                ),
                None => match fallback_href.as_deref() {
                    Some(href) => {
                        component.tag_name = Some("a".to_string());
                        component
                            .attributes
                            .insert("href".to_string(), Value::String(href.to_string()));
                        return AppliedAction::Fallback(format!(
                            "target page `{page_id}` has no localized slug; fallback_href was used"
                        ));
                    }
                    None => {
                        return AppliedAction::Unresolved(format!(
                            "target page `{page_id}` has no route for the active locale"
                        ));
                    }
                },
            };
            component.tag_name = Some("a".to_string());
            component
                .attributes
                .insert("href".to_string(), Value::String(href));
            AppliedAction::Native
        }
        ComponentAction::NavigateUrl { href, new_window } => {
            component.tag_name = Some("a".to_string());
            component
                .attributes
                .insert("href".to_string(), Value::String(href.trim().to_string()));
            if *new_window {
                component
                    .attributes
                    .insert("target".to_string(), Value::String("_blank".to_string()));
                component.attributes.insert(
                    "rel".to_string(),
                    Value::String("noopener noreferrer".to_string()),
                );
            }
            AppliedAction::Native
        }
        ComponentAction::SubmitForm { form_id } => {
            if !resolution.form_ids.contains_key(form_id) {
                return AppliedAction::Unresolved(format!("form `{form_id}` does not exist"));
            }
            component.tag_name = Some("button".to_string());
            component
                .attributes
                .insert("type".to_string(), Value::String("submit".to_string()));
            component
                .attributes
                .insert("form".to_string(), Value::String(form_id.clone()));
            AppliedAction::Native
        }
        ComponentAction::EmitEvent { .. } | ComponentAction::ProviderAction { .. } => {
            component.tag_name = Some("button".to_string());
            component
                .attributes
                .insert("type".to_string(), Value::String("button".to_string()));
            if let Ok(payload) = serde_json::to_string(action) {
                component.attributes.insert(
                    FLY_ACTION_DATA_ATTRIBUTE.to_string(),
                    Value::String(payload),
                );
            }
            AppliedAction::Custom
        }
    }
}
