use crate::{
    ComponentObject, ProjectDocument, RuntimeLocaleSelection, ValidationDiagnostic,
    ValidationSeverity,
    component_visit::{visit_project_components, visit_project_components_mut},
    interaction_route::{
        InteractionRouteCatalog, build_interaction_href, interaction_locale_candidates,
    },
    safe_url::normalize_safe_url,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const FLY_PAGE_LINK_FIELD: &str = "flyPageLink";

const GENERATED_INTERNAL_LINK_ATTRIBUTES: &[&str] = &["href", "target", "rel"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InternalPageLink {
    pub page_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fragment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_href: Option<String>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

impl InternalPageLink {
    pub fn normalized(&self) -> Result<Self, String> {
        let page_id = self.page_id.trim().to_string();
        if page_id.is_empty() {
            return Err("internal page link page_id must not be empty".to_string());
        }
        let base_path = normalize_base_path(self.base_path.as_deref())?;
        let query = normalize_suffix(self.query.as_deref(), '?', "query")?;
        let fragment = normalize_suffix(self.fragment.as_deref(), '#', "fragment")?;
        let fallback_href = self
            .fallback_href
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| normalize_safe_url(value, "internal page link fallback_href"))
            .transpose()?;
        Ok(Self {
            page_id,
            base_path,
            query,
            fragment,
            fallback_href,
            extensions: self.extensions.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InternalLinkMaterialization {
    pub document: ProjectDocument,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub resolved_links: usize,
    pub fallback_links: usize,
    pub unresolved_links: usize,
}

#[derive(Default)]
struct InternalLinkCounters {
    resolved: usize,
    fallback: usize,
    unresolved: usize,
}

struct InternalLinkResolution<'a> {
    routes: &'a InteractionRouteCatalog,
    locale_candidates: &'a [String],
}

pub fn materialize_internal_page_links(
    document: &ProjectDocument,
    context: &Value,
) -> InternalLinkMaterialization {
    let routes = InteractionRouteCatalog::from_document(document);
    let locale_candidates =
        interaction_locale_candidates(&RuntimeLocaleSelection::from_context(context));
    let resolution = InternalLinkResolution {
        routes: &routes,
        locale_candidates: &locale_candidates,
    };
    let mut materialized = document.clone();
    let mut diagnostics = Vec::new();
    let mut counters = InternalLinkCounters::default();

    visit_project_components_mut(&mut materialized.project, |component, visit| {
        materialize_component(
            component,
            visit.path(),
            &resolution,
            &mut diagnostics,
            &mut counters,
        );
    });

    InternalLinkMaterialization {
        document: materialized,
        diagnostics,
        resolved_links: counters.resolved,
        fallback_links: counters.fallback,
        unresolved_links: counters.unresolved,
    }
}

pub fn validate_internal_page_links(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    let routes = InteractionRouteCatalog::from_document(document);
    let mut diagnostics = Vec::new();

    visit_project_components(&document.project, |component, visit| {
        validate_component(component, visit.path(), &routes, &mut diagnostics);
    });
    diagnostics
}

fn materialize_component(
    component: &mut ComponentObject,
    path: &str,
    resolution: &InternalLinkResolution<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
    counters: &mut InternalLinkCounters,
) {
    let Some(raw) = component.extensions.get(FLY_PAGE_LINK_FIELD).cloned() else {
        return;
    };
    clear_internal_link_materialization(component);
    let component_id = component.id.clone();
    let link = match serde_json::from_value::<InternalPageLink>(raw) {
        Ok(link) => match link.normalized() {
            Ok(link) => link,
            Err(error) => {
                record_unresolved(
                    diagnostics,
                    counters,
                    path,
                    component_id,
                    "internal_page_link_invalid",
                    error,
                );
                return;
            }
        },
        Err(error) => {
            record_unresolved(
                diagnostics,
                counters,
                path,
                component_id,
                "internal_page_link_invalid",
                format!("internal page link cannot be decoded: {error}"),
            );
            return;
        }
    };

    let Some(target_page_index) = resolution.routes.page_index(&link.page_id) else {
        record_unresolved(
            diagnostics,
            counters,
            path,
            component_id,
            "internal_page_link_target_missing",
            format!(
                "internal page link target `{}` does not exist",
                link.page_id
            ),
        );
        return;
    };

    if let Some(slug) = resolution
        .routes
        .slug_for(target_page_index, resolution.locale_candidates)
    {
        apply_href(
            component,
            build_interaction_href(
                link.base_path.as_deref(),
                slug,
                link.query.as_deref(),
                link.fragment.as_deref(),
            ),
        );
        counters.resolved = counters.resolved.saturating_add(1);
        return;
    }

    if let Some(fallback_href) = link.fallback_href {
        apply_href(component, fallback_href);
        counters.fallback = counters.fallback.saturating_add(1);
        diagnostics.push(link_diagnostic(
            ValidationSeverity::Info,
            "internal_page_link_fallback_used",
            path,
            component_id,
            format!(
                "internal page link target `{}` has no localized slug; fallback_href was used",
                link.page_id
            ),
        ));
        return;
    }

    record_unresolved(
        diagnostics,
        counters,
        path,
        component_id,
        "internal_page_link_slug_unresolved",
        format!(
            "internal page link target `{}` has no route for the active locale",
            link.page_id
        ),
    );
}

fn validate_component(
    component: &ComponentObject,
    path: &str,
    routes: &InteractionRouteCatalog,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let Some(raw) = component.extensions.get(FLY_PAGE_LINK_FIELD).cloned() else {
        return;
    };
    let component_id = component.id.clone();
    let link = match serde_json::from_value::<InternalPageLink>(raw) {
        Ok(link) => match link.normalized() {
            Ok(link) => link,
            Err(error) => {
                diagnostics.push(link_diagnostic(
                    ValidationSeverity::Error,
                    "internal_page_link_invalid",
                    path,
                    component_id,
                    error,
                ));
                return;
            }
        },
        Err(error) => {
            diagnostics.push(link_diagnostic(
                ValidationSeverity::Error,
                "internal_page_link_invalid",
                path,
                component_id,
                format!("internal page link cannot be decoded: {error}"),
            ));
            return;
        }
    };

    match routes.page_index(&link.page_id) {
        Some(page_index) if routes.has_route(page_index) => {}
        Some(_) if link.fallback_href.is_some() => diagnostics.push(link_diagnostic(
            ValidationSeverity::Info,
            "internal_page_link_route_missing_with_fallback",
            path,
            component_id,
            format!(
                "internal page link target `{}` has no explicit slug and relies on fallback_href",
                link.page_id
            ),
        )),
        Some(_) => diagnostics.push(link_diagnostic(
            ValidationSeverity::Warning,
            "internal_page_link_route_missing",
            path,
            component_id,
            format!(
                "internal page link target `{}` has no explicit page slug",
                link.page_id
            ),
        )),
        None => diagnostics.push(link_diagnostic(
            ValidationSeverity::Error,
            "internal_page_link_target_missing",
            path,
            component_id,
            format!(
                "internal page link target `{}` does not exist",
                link.page_id
            ),
        )),
    }
}

fn apply_href(component: &mut ComponentObject, href: String) {
    component.tag_name = Some("a".to_string());
    component
        .attributes
        .insert("href".to_string(), Value::String(href));
}

fn record_unresolved(
    diagnostics: &mut Vec<ValidationDiagnostic>,
    counters: &mut InternalLinkCounters,
    path: &str,
    component_id: Option<String>,
    code: &'static str,
    message: String,
) {
    counters.unresolved = counters.unresolved.saturating_add(1);
    diagnostics.push(link_diagnostic(
        ValidationSeverity::Warning,
        code,
        path,
        component_id,
        message,
    ));
}

fn clear_internal_link_materialization(component: &mut ComponentObject) {
    for attribute in GENERATED_INTERNAL_LINK_ATTRIBUTES {
        component.attributes.remove(*attribute);
    }
}

fn normalize_base_path(value: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
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
            "internal page link base_path `{value}` must be a safe absolute path prefix"
        ));
    }
    let normalized = value.trim_end_matches('/');
    Ok(Some(if normalized.is_empty() {
        "/".to_string()
    } else {
        normalized.to_string()
    }))
}

fn normalize_suffix(
    value: Option<&str>,
    prefix: char,
    label: &str,
) -> Result<Option<String>, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.contains('\\')
        || value.chars().any(|character| character.is_control())
        || value.chars().any(char::is_whitespace)
        || (prefix == '?' && value.contains('#'))
    {
        return Err(format!("internal page link {label} is not safely encoded"));
    }
    let value = value.trim_start_matches(prefix).trim();
    Ok((!value.is_empty()).then(|| value.to_string()))
}

fn link_diagnostic(
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
            .map(|id| format!("component:{id}.{FLY_PAGE_LINK_FIELD}"))
            .unwrap_or_else(|| format!("{path}.{FLY_PAGE_LINK_FIELD}")),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "flyLocales": {
                "default_locale": "en",
                "supported_locales": ["en", "ru"]
            },
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "slug": { "$localized": { "en": "home", "ru": "glavnaya" } }
                },
                "component": {
                    "id": "home-root",
                    "type": "wrapper",
                    "components": [{
                        "id": "about-link",
                        "type": "link",
                        "tagName": "a",
                        "attributes": {
                            "href": "/old-about",
                            "target": "_blank",
                            "rel": "opener"
                        },
                        "flyPageLink": {
                            "page_id": "about",
                            "base_path": "/site",
                            "query": "source=hero",
                            "fragment": "team"
                        }
                    }]
                }
            }, {
                "id": "about",
                "flyPageMeta": {
                    "slug": { "$localized": { "en": "about", "ru": "o-nas" } }
                },
                "component": { "id": "about-root", "type": "wrapper" }
            }]
        }))
        .expect("document")
    }

    #[test]
    fn internal_page_link_materializes_locale_specific_href() {
        let document = document();
        let result = materialize_internal_page_links(
            &document,
            &json!({ "$locale": "ru-RU", "$fallback_locales": ["en"] }),
        );
        let link = result.document.component("about-link").unwrap();
        assert_eq!(link.tag_name.as_deref(), Some("a"));
        assert_eq!(link.attributes["href"], "/site/o-nas?source=hero#team");
        assert!(!link.attributes.contains_key("target"));
        assert!(!link.attributes.contains_key("rel"));
        assert_eq!(
            document.component("about-link").unwrap().attributes["href"],
            "/old-about"
        );
    }

    #[test]
    fn missing_target_is_blocking_validation_and_clears_stale_href_at_runtime() {
        let mut document = document();
        document
            .component_mut("about-link")
            .unwrap()
            .extensions
            .get_mut(FLY_PAGE_LINK_FIELD)
            .unwrap()["page_id"] = json!("missing");
        let diagnostics = validate_internal_page_links(&document);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "internal_page_link_target_missing"
                && diagnostic.severity == ValidationSeverity::Error
        }));
        let result = materialize_internal_page_links(&document, &json!({ "$locale": "en" }));
        assert_eq!(result.unresolved_links, 1);
        let link = result.document.component("about-link").unwrap();
        assert!(!link.attributes.contains_key("href"));
        assert!(!link.attributes.contains_key("target"));
        assert!(!link.attributes.contains_key("rel"));
    }

    #[test]
    fn fallback_href_is_used_when_target_page_has_no_slug() {
        let mut document = document();
        document.project.pages[1]
            .extensions
            .remove(crate::FLY_PAGE_METADATA_FIELD);
        document
            .component_mut("about-link")
            .unwrap()
            .extensions
            .get_mut(FLY_PAGE_LINK_FIELD)
            .unwrap()["fallback_href"] = json!("/fallback-about");
        let result = materialize_internal_page_links(&document, &json!({ "$locale": "ru" }));
        assert_eq!(result.fallback_links, 1);
        assert_eq!(
            result.document.component("about-link").unwrap().attributes["href"],
            "/fallback-about"
        );
    }

    #[test]
    fn unsafe_fallback_and_network_base_path_are_rejected() {
        let mut unsafe_fallback = document();
        unsafe_fallback
            .component_mut("about-link")
            .unwrap()
            .extensions
            .get_mut(FLY_PAGE_LINK_FIELD)
            .unwrap()["fallback_href"] = json!("//attacker.example/path");
        assert!(
            validate_internal_page_links(&unsafe_fallback)
                .iter()
                .any(|diagnostic| diagnostic.code == "internal_page_link_invalid")
        );

        let mut network_base = document();
        network_base
            .component_mut("about-link")
            .unwrap()
            .extensions
            .get_mut(FLY_PAGE_LINK_FIELD)
            .unwrap()["base_path"] = json!("//cdn.example");
        assert!(
            validate_internal_page_links(&network_base)
                .iter()
                .any(|diagnostic| diagnostic.code == "internal_page_link_invalid")
        );
    }

    #[test]
    fn unencoded_query_and_backslash_fragment_are_rejected() {
        let mut query = document();
        query
            .component_mut("about-link")
            .unwrap()
            .extensions
            .get_mut(FLY_PAGE_LINK_FIELD)
            .unwrap()["query"] = json!("source=hero#override");
        assert!(
            validate_internal_page_links(&query)
                .iter()
                .any(|diagnostic| diagnostic.code == "internal_page_link_invalid")
        );

        let mut fragment = document();
        fragment
            .component_mut("about-link")
            .unwrap()
            .extensions
            .get_mut(FLY_PAGE_LINK_FIELD)
            .unwrap()["fragment"] = json!("team\\details");
        assert!(
            validate_internal_page_links(&fragment)
                .iter()
                .any(|diagnostic| diagnostic.code == "internal_page_link_invalid")
        );
    }

    #[test]
    fn anonymous_component_diagnostics_use_the_shared_canonical_path() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "type": "link",
                        "flyPageLink": { "page_id": "missing" }
                    }]
                }
            }]
        }))
        .expect("document");
        let diagnostics = validate_internal_page_links(&document);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.path == "project.pages[0].component.components[0].flyPageLink"
        }));
    }
}
