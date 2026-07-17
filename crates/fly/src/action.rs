use crate::{
    localized_page_route_index, normalize_locale_tag,
    safe_url::validate_safe_url as validate_shared_safe_url, ComponentNode,
    LocalizedPageRouteEntry, ProjectDocument, RuntimeLocaleSelection, ValidationDiagnostic,
    ValidationSeverity, FLY_PAGE_LINK_FIELD,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const FLY_ACTION_FIELD: &str = "flyAction";
pub const FLY_FORM_FIELD: &str = "flyForm";
pub const FLY_ACTION_DATA_ATTRIBUTE: &str = "data-fly-action";
pub const FLY_ACTION_KIND_ATTRIBUTE: &str = "data-fly-action-kind";

const GENERATED_INTERACTION_ATTRIBUTES: &[&str] = &[
    "href",
    "target",
    "rel",
    "type",
    "form",
    "action",
    "method",
    "enctype",
    "novalidate",
    FLY_ACTION_DATA_ATTRIBUTE,
    FLY_ACTION_KIND_ATTRIBUTE,
    "data-fly-form-provider",
    "data-fly-form-action",
    "data-fly-form-input",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ComponentAction {
    NavigatePage {
        page_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        query: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fragment: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fallback_href: Option<String>,
    },
    NavigateUrl {
        href: String,
        #[serde(default)]
        new_window: bool,
    },
    SubmitForm {
        form_id: String,
    },
    EmitEvent {
        event: String,
        #[serde(default)]
        payload: Value,
    },
    ProviderAction {
        provider: String,
        action: String,
        #[serde(default)]
        input: Value,
    },
}

impl ComponentAction {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::NavigatePage { .. } => "navigate_page",
            Self::NavigateUrl { .. } => "navigate_url",
            Self::SubmitForm { .. } => "submit_form",
            Self::EmitEvent { .. } => "emit_event",
            Self::ProviderAction { .. } => "provider_action",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum FormMethod {
    #[default]
    Get,
    Post,
    Dialog,
}

impl FormMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post => "post",
            Self::Dialog => "dialog",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FormEncoding {
    #[default]
    UrlEncoded,
    Multipart,
    TextPlain,
}

impl FormEncoding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UrlEncoded => "application/x-www-form-urlencoded",
            Self::Multipart => "multipart/form-data",
            Self::TextPlain => "text/plain",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComponentForm {
    pub id: String,
    #[serde(default)]
    pub method: FormMethod,
    #[serde(default)]
    pub encoding: FormEncoding,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub novalidate: bool,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionMaterialization {
    pub document: ProjectDocument,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub materialized_forms: usize,
    pub native_actions: usize,
    pub custom_actions: usize,
    pub fallback_actions: usize,
    pub unresolved_actions: usize,
}

pub fn materialize_component_actions(
    document: &ProjectDocument,
    context: &Value,
) -> ActionMaterialization {
    let route_index = localized_page_route_index(document);
    let locale_candidates = action_locale_candidates(&RuntimeLocaleSelection::from_context(context));
    let page_ids = page_index(document);
    let form_ids = collect_form_ids(document);
    let mut materialized = document.clone();
    let mut diagnostics = Vec::new();
    let mut counters = ActionCounters::default();

    for (page_index, page) in materialized.project.pages.iter_mut().enumerate() {
        let Some(root) = page.component.as_mut() else {
            continue;
        };
        materialize_node(
            root,
            &format!("project.pages[{page_index}].component"),
            &route_index,
            &locale_candidates,
            &page_ids,
            &form_ids,
            &mut diagnostics,
            &mut counters,
        );
    }

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

pub fn validate_component_actions(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    let page_ids = page_index(document);
    let route_pages = localized_page_route_index(document)
        .into_iter()
        .map(|entry| entry.page_index)
        .collect::<BTreeSet<_>>();
    let form_ids = collect_form_ids(document);
    let mut diagnostics = Vec::new();
    let mut seen_form_ids = BTreeSet::new();

    for (page_index, page) in document.project.pages.iter().enumerate() {
        let Some(root) = page.component.as_ref() else {
            continue;
        };
        validate_node(
            root,
            &format!("project.pages[{page_index}].component"),
            &page_ids,
            &route_pages,
            &form_ids,
            &mut seen_form_ids,
            &mut diagnostics,
        );
    }
    diagnostics
}

type PageIndex = BTreeMap<String, usize>;
type FormIndex = BTreeMap<String, String>;

#[derive(Default)]
struct ActionCounters {
    forms: usize,
    native: usize,
    custom: usize,
    fallback: usize,
    unresolved: usize,
}

#[allow(clippy::too_many_arguments)]
fn materialize_node(
    node: &mut ComponentNode,
    path: &str,
    route_index: &[LocalizedPageRouteEntry],
    locale_candidates: &[String],
    page_ids: &PageIndex,
    form_ids: &FormIndex,
    diagnostics: &mut Vec<ValidationDiagnostic>,
    counters: &mut ActionCounters,
) {
    let Some(component) = node.as_object_mut() else {
        return;
    };
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
            Ok(action) => match apply_action(
                component,
                &action,
                route_index,
                locale_candidates,
                page_ids,
                form_ids,
            ) {
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
                    component_id.clone(),
                    format!("component action cannot be decoded: {error}"),
                ));
            }
        }
    }

    if let Some(children) = component.children_mut() {
        for (index, child) in children.iter_mut().enumerate() {
            materialize_node(
                child,
                &format!("{path}.components[{index}]"),
                route_index,
                locale_candidates,
                page_ids,
                form_ids,
                diagnostics,
                counters,
            );
        }
    }
}

fn validate_node(
    node: &ComponentNode,
    path: &str,
    page_ids: &PageIndex,
    route_pages: &BTreeSet<usize>,
    form_ids: &FormIndex,
    seen_form_ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let Some(component) = node.as_object() else {
        return;
    };
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
            }
            Err(error) => diagnostics.push(action_diagnostic(
                ValidationSeverity::Error,
                "form_definition_invalid",
                path,
                component_id.clone(),
                error,
            )),
        }
    }

    if let Some(raw) = component.extensions.get(FLY_ACTION_FIELD).cloned() {
        match serde_json::from_value::<ComponentAction>(raw) {
            Ok(action) => validate_action(
                &action,
                path,
                component_id.clone(),
                page_ids,
                route_pages,
                form_ids,
                diagnostics,
            ),
            Err(error) => diagnostics.push(action_diagnostic(
                ValidationSeverity::Error,
                "action_definition_invalid",
                path,
                component_id.clone(),
                format!("component action cannot be decoded: {error}"),
            )),
        }
    }

    for (index, child) in component.children().iter().enumerate() {
        validate_node(
            child,
            &format!("{path}.components[{index}]"),
            page_ids,
            route_pages,
            form_ids,
            seen_form_ids,
            diagnostics,
        );
    }
}

fn decode_form(raw: Value) -> Result<ComponentForm, String> {
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
        validate_safe_url(url, "form action_url")?;
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
    page_ids: &PageIndex,
    route_pages: &BTreeSet<usize>,
    form_ids: &FormIndex,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let result = match action {
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
            .and_then(|_| match page_ids.get(page_id).copied() {
                Some(page_index) if route_pages.contains(&page_index) => Ok(()),
                Some(_) if fallback_href.is_some() => Ok(()),
                Some(_) => Err(format!("target page `{page_id}` has no explicit slug")),
                None => Err(format!("target page `{page_id}` does not exist")),
            }),
        ComponentAction::NavigateUrl { href, .. } => validate_safe_url(href, "navigation href"),
        ComponentAction::SubmitForm { form_id } => {
            validate_identifier(form_id, "form id").and_then(|_| {
                form_ids
                    .contains_key(form_id)
                    .then_some(())
                    .ok_or_else(|| format!("form `{form_id}` does not exist"))
            })
        }
        ComponentAction::EmitEvent { event, .. } => validate_identifier(event, "event name"),
        ComponentAction::ProviderAction {
            provider, action, ..
        } => validate_identifier(provider, "provider")
            .and_then(|_| validate_identifier(action, "provider action")),
    };
    if let Err(message) = result {
        diagnostics.push(action_diagnostic(
            ValidationSeverity::Error,
            "action_definition_invalid",
            path,
            component_id,
            message,
        ));
    }
}

fn clear_interaction_materialization(component: &mut crate::ComponentObject) {
    for attribute in GENERATED_INTERACTION_ATTRIBUTES {
        component.attributes.remove(*attribute);
    }
}

fn apply_form(component: &mut crate::ComponentObject, form: &ComponentForm) {
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
    component: &mut crate::ComponentObject,
    action: &ComponentAction,
    route_index: &[LocalizedPageRouteEntry],
    locale_candidates: &[String],
    page_ids: &PageIndex,
    form_ids: &FormIndex,
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
            let Some(page_index) = page_ids.get(page_id).copied() else {
                return AppliedAction::Unresolved(format!("target page `{page_id}` does not exist"));
            };
            let slug = action_route_slug(route_index, page_index, locale_candidates);
            let href = match slug {
                Some(slug) => build_page_href(
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
                component.attributes.insert(
                    "target".to_string(),
                    Value::String("_blank".to_string()),
                );
                component.attributes.insert(
                    "rel".to_string(),
                    Value::String("noopener noreferrer".to_string()),
                );
            }
            AppliedAction::Native
        }
        ComponentAction::SubmitForm { form_id } => {
            if !form_ids.contains_key(form_id) {
                return AppliedAction::Unresolved(format!("form `{form_id}` does not exist"));
            }
            component.tag_name = Some("button".to_string());
            component.attributes.insert(
                "type".to_string(),
                Value::String("submit".to_string()),
            );
            component
                .attributes
                .insert("form".to_string(), Value::String(form_id.clone()));
            AppliedAction::Native
        }
        ComponentAction::EmitEvent { .. } | ComponentAction::ProviderAction { .. } => {
            component.tag_name = Some("button".to_string());
            component.attributes.insert(
                "type".to_string(),
                Value::String("button".to_string()),
            );
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

fn page_index(document: &ProjectDocument) -> PageIndex {
    document
        .project
        .pages
        .iter()
        .enumerate()
        .filter_map(|(index, page)| page.id.as_deref().map(|id| (id.to_string(), index)))
        .collect()
}

fn collect_form_ids(document: &ProjectDocument) -> FormIndex {
    let mut forms = FormIndex::new();
    for page in &document.project.pages {
        let Some(root) = page.component.as_ref() else {
            continue;
        };
        root.visit(0, "page.component", &mut |component, _, _| {
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
    }
    forms
}

fn action_locale_candidates(selection: &RuntimeLocaleSelection) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(locale) = selection.locale.as_deref() {
        push_locale(&mut candidates, locale);
    }
    for locale in &selection.fallback_locales {
        push_locale(&mut candidates, locale);
    }
    candidates
}

fn push_locale(candidates: &mut Vec<String>, locale: &str) {
    let Some(locale) = normalize_locale_tag(locale) else {
        return;
    };
    if !candidates.contains(&locale) {
        candidates.push(locale.clone());
    }
    if let Some((language, _)) = locale.split_once('-') {
        let language = language.to_string();
        if !candidates.contains(&language) {
            candidates.push(language);
        }
    }
}

fn action_route_slug<'a>(
    routes: &'a [LocalizedPageRouteEntry],
    page_index: usize,
    candidates: &[String],
) -> Option<&'a str> {
    for locale in candidates {
        if let Some(route) = routes.iter().find(|route| {
            route.page_index == page_index && route.locale.as_deref() == Some(locale.as_str())
        }) {
            return Some(route.slug.as_str());
        }
    }
    routes
        .iter()
        .find(|route| route.page_index == page_index && route.locale.is_none())
        .or_else(|| routes.iter().find(|route| route.page_index == page_index))
        .map(|route| route.slug.as_str())
}

fn build_page_href(
    base_path: Option<&str>,
    slug: &str,
    query: Option<&str>,
    fragment: Option<&str>,
) -> String {
    let base_path = base_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("/");
    let mut href = if base_path == "/" {
        format!("/{slug}")
    } else {
        format!("{}/{slug}", base_path.trim_end_matches('/'))
    };
    if let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) {
        href.push('?');
        href.push_str(query.trim_start_matches('?'));
    }
    if let Some(fragment) = fragment.map(str::trim).filter(|value| !value.is_empty()) {
        href.push('#');
        href.push_str(fragment.trim_start_matches('#'));
    }
    href
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

fn validate_safe_url(value: &str, label: &str) -> Result<(), String> {
    validate_shared_safe_url(value, label)
}

fn action_diagnostic(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsV1Codec;
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsV1Codec::decode_value(json!({
            "flyLocales": {
                "default_locale": "ru",
                "supported_locales": ["ru", "en"]
            },
            "pages": [{
                "id": "home",
                "flyPageMeta": { "slug": { "$localized": { "en": "home", "ru": "glavnaya" } } },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "contact-form",
                        "type": "wrapper",
                        "flyForm": {
                            "id": "contact",
                            "method": "post",
                            "provider": "crm",
                            "action": "create_lead",
                            "input": { "source": "landing" }
                        }
                    }, {
                        "id": "submit",
                        "type": "button",
                        "flyAction": { "kind": "submit_form", "form_id": "contact" }
                    }, {
                        "id": "about",
                        "type": "button",
                        "flyAction": { "kind": "navigate_page", "page_id": "about-page" }
                    }, {
                        "id": "track",
                        "type": "button",
                        "flyAction": {
                            "kind": "emit_event",
                            "event": "marketing.cta",
                            "payload": { "campaign": "summer" }
                        }
                    }]
                }
            }, {
                "id": "about-page",
                "flyPageMeta": { "slug": { "$localized": { "en": "about", "ru": "o-nas" } } },
                "component": { "id": "about-root", "type": "wrapper" }
            }]
        }))
        .expect("document")
    }

    #[test]
    fn actions_and_forms_materialize_to_native_and_custom_contracts() {
        let document = document();
        let result = materialize_component_actions(&document, &json!({ "$locale": "ru" }));
        assert_eq!(result.materialized_forms, 1);
        assert_eq!(result.native_actions, 2);
        assert_eq!(result.custom_actions, 1);
        let form = result.document.component("contact-form").unwrap();
        assert_eq!(form.tag_name.as_deref(), Some("form"));
        assert_eq!(form.attributes["data-fly-form-provider"], "crm");
        assert_eq!(
            result.document.component("submit").unwrap().attributes["form"],
            "contact"
        );
        assert_eq!(
            result.document.component("about").unwrap().attributes["href"],
            "/o-nas"
        );
        let track = result.document.component("track").unwrap();
        assert_eq!(track.tag_name.as_deref(), Some("button"));
        assert_eq!(track.attributes[FLY_ACTION_KIND_ATTRIBUTE], "emit_event");
    }

    #[test]
    fn materialization_clears_stale_interaction_attributes() {
        let document = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "search-form",
                        "type": "wrapper",
                        "attributes": {
                            "action": "/legacy",
                            "enctype": "multipart/form-data",
                            "novalidate": "",
                            "data-fly-form-provider": "legacy",
                            "data-fly-form-action": "send",
                            "data-fly-form-input": "{}",
                            "href": "/stale",
                            "target": "_blank",
                            "type": "button"
                        },
                        "flyForm": { "id": "search", "method": "get" }
                    }, {
                        "id": "track",
                        "type": "button",
                        "tagName": "a",
                        "attributes": {
                            "href": "/legacy",
                            "target": "_blank",
                            "rel": "opener",
                            "form": "legacy-form",
                            "action": "/legacy-submit",
                            "method": "post",
                            "enctype": "multipart/form-data",
                            "novalidate": "",
                            "data-fly-form-provider": "legacy",
                            "data-fly-action": "legacy"
                        },
                        "flyAction": {
                            "kind": "emit_event",
                            "event": "analytics.track"
                        }
                    }]
                }
            }]
        }))
        .expect("document");

        let result = materialize_component_actions(&document, &json!({}));
        let form = result.document.component("search-form").unwrap();
        assert_eq!(form.tag_name.as_deref(), Some("form"));
        assert_eq!(form.attributes["method"], "get");
        for attribute in [
            "action",
            "enctype",
            "novalidate",
            "data-fly-form-provider",
            "data-fly-form-action",
            "data-fly-form-input",
            "href",
            "target",
            "type",
        ] {
            assert!(!form.attributes.contains_key(attribute), "{attribute}");
        }

        let action = result.document.component("track").unwrap();
        assert_eq!(action.tag_name.as_deref(), Some("button"));
        assert_eq!(action.attributes["type"], "button");
        assert_eq!(action.attributes[FLY_ACTION_KIND_ATTRIBUTE], "emit_event");
        assert!(action.attributes.contains_key(FLY_ACTION_DATA_ATTRIBUTE));
        for attribute in [
            "href",
            "target",
            "rel",
            "form",
            "action",
            "method",
            "enctype",
            "novalidate",
            "data-fly-form-provider",
        ] {
            assert!(!action.attributes.contains_key(attribute), "{attribute}");
        }
    }

    #[test]
    fn missing_form_and_unsafe_url_are_blocking_validation() {
        let document = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "submit",
                        "type": "button",
                        "flyAction": { "kind": "submit_form", "form_id": "missing" }
                    }, {
                        "id": "bad-link",
                        "type": "link",
                        "flyAction": { "kind": "navigate_url", "href": "javascript:alert(1)" }
                    }]
                }
            }]
        }))
        .expect("document");
        let diagnostics = validate_component_actions(&document);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
                .count(),
            2
        );
    }

    #[test]
    fn network_paths_and_backslash_urls_are_blocking_validation() {
        let document = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "network-link",
                        "type": "link",
                        "flyAction": {
                            "kind": "navigate_url",
                            "href": "//attacker.example/path"
                        }
                    }, {
                        "id": "unsafe-form",
                        "type": "wrapper",
                        "flyForm": {
                            "id": "unsafe",
                            "method": "post",
                            "action_url": "/\\attacker.example/submit"
                        }
                    }]
                }
            }]
        }))
        .expect("document");
        let diagnostics = validate_component_actions(&document);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
                .count(),
            2
        );
    }

    #[test]
    fn duplicate_forms_and_interaction_conflicts_are_rejected() {
        let document = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": { "slug": "home" },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "one",
                        "type": "wrapper",
                        "flyForm": { "id": "same" }
                    }, {
                        "id": "two",
                        "type": "wrapper",
                        "flyForm": { "id": "same" }
                    }, {
                        "id": "navigation-conflict",
                        "type": "link",
                        "flyPageLink": { "page_id": "home" },
                        "flyAction": { "kind": "navigate_page", "page_id": "home" }
                    }, {
                        "id": "form-action-conflict",
                        "type": "wrapper",
                        "flyForm": { "id": "combined" },
                        "flyAction": { "kind": "emit_event", "event": "submit" }
                    }]
                }
            }]
        }))
        .expect("document");
        let diagnostics = validate_component_actions(&document);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "duplicate_form_id"));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "component_navigation_contract_conflict"
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "component_form_interaction_contract_conflict"
        }));
    }

    #[test]
    fn non_post_encoding_is_rejected() {
        let document = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "invalid-form",
                        "type": "wrapper",
                        "flyForm": {
                            "id": "search",
                            "method": "get",
                            "encoding": "multipart"
                        }
                    }]
                }
            }]
        }))
        .expect("document");
        let diagnostics = validate_component_actions(&document);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "form_definition_invalid"
                && diagnostic.message.contains("encoding requires post")
        }));
    }
}
