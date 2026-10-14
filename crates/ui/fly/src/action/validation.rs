use super::model::{
    ComponentAction, ComponentForm, FLY_ACTION_FIELD, FLY_FORM_FIELD, FormEncoding, FormMethod,
};
use crate::{
    ComponentObject, FLY_PAGE_LINK_FIELD, ProjectDocument, ValidationDiagnostic,
    ValidationSeverity, component_visit::visit_project_components,
    interaction_route::InteractionRouteCatalog,
    safe_url::validate_safe_url as validate_shared_safe_url,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(super) type FormIndex = BTreeMap<String, String>;

struct ActionValidation<'a> {
    routes: &'a InteractionRouteCatalog,
    form_ids: &'a FormIndex,
    /// Forms an author already wired to a host through `ComponentAction::SubmitForm`.
    ///
    /// Such a form is submitted by the host, so the absence of a native target is a decision
    /// rather than an oversight, and it must not be reported as one.
    host_submitted_form_ids: &'a BTreeSet<String>,
}

pub fn validate_component_actions(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    let routes = InteractionRouteCatalog::from_document(document);
    let form_ids = collect_form_ids(document);
    let host_submitted_form_ids = collect_host_submitted_form_ids(document);
    let validation = ActionValidation {
        routes: &routes,
        form_ids: &form_ids,
        host_submitted_form_ids: &host_submitted_form_ids,
    };
    let mut diagnostics = Vec::new();
    let mut seen_form_ids = BTreeSet::new();

    visit_project_components(&document.project, |component, visit| {
        validate_component(
            component,
            visit.path(),
            &validation,
            &mut seen_form_ids,
            &mut diagnostics,
        );
    });
    diagnostics
}

pub(super) fn collect_form_ids(document: &ProjectDocument) -> FormIndex {
    let mut forms = FormIndex::new();
    visit_project_components(&document.project, |component, _| {
        let Some(raw) = component.extensions.get(FLY_FORM_FIELD).cloned() else {
            return;
        };
        let Ok(form) = decode_form(raw) else {
            return;
        };
        forms
            .entry(form.id)
            .or_insert_with(|| component.id.clone().unwrap_or_default());
    });
    forms
}

fn validate_component(
    component: &ComponentObject,
    path: &str,
    validation: &ActionValidation<'_>,
    seen_form_ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let component_id = component.id.clone();
    let has_page_link = component.extensions.contains_key(FLY_PAGE_LINK_FIELD);
    let has_action = component.extensions.contains_key(FLY_ACTION_FIELD);
    let has_form = component.extensions.contains_key(FLY_FORM_FIELD);

    if has_page_link && has_action {
        diagnostics.push(action_diagnostic(
            ValidationSeverity::Error,
            "component_navigation_contract_conflict",
            path,
            component_id.clone(),
            format!(
                "component cannot define both `{FLY_PAGE_LINK_FIELD}` and `{FLY_ACTION_FIELD}`"
            ),
        ));
    }
    if has_form && (has_page_link || has_action) {
        diagnostics.push(action_diagnostic(
            ValidationSeverity::Error,
            "component_form_interaction_contract_conflict",
            path,
            component_id.clone(),
            format!(
                "component cannot combine `{FLY_FORM_FIELD}` with `{FLY_PAGE_LINK_FIELD}` or `{FLY_ACTION_FIELD}`"
            ),
        ));
    }

    if let Some(raw) = component.extensions.get(FLY_FORM_FIELD).cloned() {
        match decode_form(raw) {
            Ok(form) => {
                if !seen_form_ids.insert(form.id.clone()) {
                    diagnostics.push(action_diagnostic(
                        ValidationSeverity::Error,
                        "duplicate_form_id",
                        path,
                        component_id.clone(),
                        format!("form id `{}` is duplicated", form.id),
                    ));
                }
                if let Some(diagnostic) =
                    form_without_submission_target(&form, validation, path, component_id.as_deref())
                {
                    diagnostics.push(diagnostic);
                }
            }
            Err(error) => diagnostics.push(action_diagnostic(
                ValidationSeverity::Error,
                "form_definition_invalid",
                path,
                component_id.clone(),
                error,
            )),
        }
    } else if component
        .component_type()
        .trim()
        .eq_ignore_ascii_case("form")
        && !has_native_submission_target(component)
    {
        // A form block dropped from the palette carries no `flyForm` contract and no `action`
        // attribute, which makes the browser submit it back to the page URL. That is the default
        // shape of the block, so this is the case most authors meet first (functional audit G-7:
        // the form goes nowhere), and nothing in the editor used to say so.
        diagnostics.push(action_diagnostic(
            ValidationSeverity::Warning,
            "form_without_submission_target",
            path,
            component_id.clone(),
            "form has no submission target: the browser submits it to the current page URL; set a \
             form action_url or a provider action unless that is intended",
        ));
    }

    if let Some(raw) = component.extensions.get(FLY_ACTION_FIELD).cloned() {
        match serde_json::from_value::<ComponentAction>(raw) {
            Ok(action) => validate_action(&action, path, component_id, validation, diagnostics),
            Err(error) => diagnostics.push(action_diagnostic(
                ValidationSeverity::Error,
                "action_definition_invalid",
                path,
                component_id,
                format!("component action cannot be decoded: {error}"),
            )),
        }
    }
}

pub(super) fn decode_form(raw: Value) -> Result<ComponentForm, String> {
    let form = serde_json::from_value::<ComponentForm>(raw)
        .map_err(|error| format!("form definition cannot be decoded: {error}"))?;
    validate_form(&form)?;
    Ok(form)
}

fn validate_form(form: &ComponentForm) -> Result<(), String> {
    validate_identifier(&form.id, "form id")?;
    let action_url = form
        .action_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let provider = form
        .provider
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let action = form
        .action
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if action_url.is_some() && (provider.is_some() || action.is_some()) {
        return Err("form cannot combine action_url with provider action fields".to_string());
    }
    if provider.is_some() != action.is_some() {
        return Err("form provider and action must be supplied together".to_string());
    }
    if let Some(url) = action_url {
        validate_form_action_url(url)?;
    }
    if let Some(provider) = provider {
        validate_identifier(provider, "form provider")?;
    }
    if let Some(action) = action {
        validate_identifier(action, "form provider action")?;
    }
    if form.method == FormMethod::Dialog && (action_url.is_some() || provider.is_some()) {
        return Err("dialog form cannot define native or provider submission targets".to_string());
    }
    if form.method != FormMethod::Post && form.encoding != FormEncoding::UrlEncoded {
        return Err("non-default form encoding requires post method".to_string());
    }
    Ok(())
}

fn validate_action(
    action: &ComponentAction,
    path: &str,
    component_id: Option<String>,
    validation: &ActionValidation<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    if let Err(message) = validate_action_contract(action, validation.routes, validation.form_ids) {
        diagnostics.push(action_diagnostic(
            ValidationSeverity::Error,
            "action_definition_invalid",
            path,
            component_id,
            message,
        ));
    }
}

pub(super) fn validate_action_contract(
    action: &ComponentAction,
    routes: &InteractionRouteCatalog,
    form_ids: &FormIndex,
) -> Result<(), String> {
    match action {
        ComponentAction::NavigatePage {
            page_id,
            base_path,
            query,
            fragment,
            fallback_href,
        } => validate_identifier(page_id, "target page id")
            .and_then(|_| {
                if let Some(base_path) = base_path.as_deref() {
                    validate_base_path(base_path)?;
                }
                validate_suffix(query.as_deref(), "query")?;
                validate_suffix(fragment.as_deref(), "fragment")?;
                if let Some(href) = fallback_href.as_deref() {
                    validate_safe_url(href, "fallback href")?;
                }
                Ok(())
            })
            .and_then(|_| match routes.page_index(page_id) {
                Some(page_index) if routes.has_route(page_index) => Ok(()),
                Some(_) if fallback_href.is_some() => Ok(()),
                Some(_) => Err(format!("target page `{page_id}` has no explicit slug")),
                None => Err(format!("target page `{page_id}` does not exist")),
            }),
        ComponentAction::NavigateUrl { href, .. } => validate_safe_url(href, "navigation href"),
        ComponentAction::SubmitForm { form_id } => validate_identifier(form_id, "form id")
            .and_then(|_| {
                form_ids
                    .contains_key(form_id)
                    .then_some(())
                    .ok_or_else(|| format!("form `{form_id}` does not exist"))
            }),
        ComponentAction::EmitEvent { event, .. } => validate_identifier(event, "event name"),
        ComponentAction::ToggleVisibility { target_id, .. }
        | ComponentAction::ScrollTo { target_id } => {
            validate_identifier(target_id, "target component id")
        }
        ComponentAction::Animate {
            target_id,
            duration_ms,
            ..
        } => {
            validate_identifier(target_id, "target component id").and_then(|_| match duration_ms {
                Some(duration) if !(50..=10_000).contains(duration) => Err(format!(
                    "animation duration {duration}ms is outside the allowed 50..=10000 range"
                )),
                _ => Ok(()),
            })
        }
        ComponentAction::ProviderAction {
            provider, action, ..
        } => validate_identifier(provider, "provider")
            .and_then(|_| validate_identifier(action, "provider action")),
    }
}

fn collect_host_submitted_form_ids(document: &ProjectDocument) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    visit_project_components(&document.project, |component, _| {
        let Some(raw) = component.extensions.get(FLY_ACTION_FIELD).cloned() else {
            return;
        };
        if let Ok(ComponentAction::SubmitForm { form_id }) =
            serde_json::from_value::<ComponentAction>(raw)
        {
            ids.insert(form_id);
        }
    });
    ids
}

/// Report a form the browser will submit back to the page it is rendered on.
///
/// `ComponentForm` deliberately offers three mutually exclusive shapes: a native `action_url`, a
/// provider action a host resolves, or `method: dialog` for a host-driven dialog. When none is
/// present the browser falls back to the document URL, so the field values become query parameters
/// on a page reload (GET) or the page answers a POST it never expected. The form appears complete
/// and simply does nothing, which is exactly the failure the audit recorded as "the form goes
/// nowhere" — with no diagnostic to explain it.
///
/// A warning, not an error: GET-to-self is a legitimate pattern for a search or filter form, and
/// documents that already rely on it must keep publishing.
fn form_without_submission_target(
    form: &ComponentForm,
    validation: &ActionValidation<'_>,
    path: &str,
    component_id: Option<&str>,
) -> Option<ValidationDiagnostic> {
    if form.method == FormMethod::Dialog
        || non_empty(form.action_url.as_deref()).is_some()
        || non_empty(form.provider.as_deref()).is_some()
        || validation.host_submitted_form_ids.contains(&form.id)
    {
        return None;
    }
    Some(action_diagnostic(
        ValidationSeverity::Warning,
        "form_without_submission_target",
        path,
        component_id.map(ToString::to_string),
        format!(
            "form `{}` has no submission target: the browser submits it to the current page URL \
             (method `{}`); set a form action_url or a provider action unless that is intended",
            form.id,
            form.method.as_str()
        ),
    ))
}

/// Whether a component already points somewhere without a `flyForm` contract.
///
/// Two shapes count. A materialized document carries the generated `action` attribute, and a
/// materialized provider form carries `data-fly-form-provider` with no `action` at all — in both
/// cases the submission has an owner, so the block must not be reported as target-less. Shared by
/// both branches of the check so a form that was materialized earlier is not reported twice.
fn has_native_submission_target(component: &ComponentObject) -> bool {
    ["action", "data-fly-form-provider"]
        .iter()
        .any(|name| non_empty(component.attributes.get(*name).and_then(Value::as_str)).is_some())
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn validate_identifier(value: &str, label: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if !value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
    }) {
        return Err(format!("{label} `{value}` contains unsupported characters"));
    }
    Ok(())
}

fn validate_base_path(value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(());
    }
    if !value.starts_with('/')
        || value.starts_with("//")
        || value.contains("://")
        || value.contains('?')
        || value.contains('#')
        || value.contains('\\')
        || value.chars().any(|character| character.is_control())
        || value.chars().any(char::is_whitespace)
    {
        return Err(format!(
            "base path `{value}` is not a safe absolute path prefix"
        ));
    }
    Ok(())
}

fn validate_suffix(value: Option<&str>, label: &str) -> Result<(), String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    if value.contains('\\')
        || value.chars().any(|character| character.is_control())
        || value.chars().any(char::is_whitespace)
        || (label == "query" && value.contains('#'))
    {
        Err(format!("{label} is not safely encoded"))
    } else {
        Ok(())
    }
}

fn validate_form_action_url(value: &str) -> Result<(), String> {
    validate_safe_url(value, "form action_url")?;
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("mailto:") || lower.starts_with("tel:") || lower.starts_with('#') {
        return Err(
            "form action_url must be a relative, http, or https submission URL".to_string(),
        );
    }
    Ok(())
}

fn validate_safe_url(value: &str, label: &str) -> Result<(), String> {
    validate_shared_safe_url(value, label)
}

pub(super) fn action_diagnostic(
    severity: ValidationSeverity,
    code: impl Into<String>,
    path: &str,
    component_id: Option<String>,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
        code: code.into(),
        path: component_id
            .as_deref()
            .map(|id| format!("component:{id}"))
            .unwrap_or_else(|| path.to_string()),
        message: message.into(),
    }
}
