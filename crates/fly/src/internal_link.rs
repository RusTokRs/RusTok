use crate::{
    localized_page_route_index, normalize_locale_tag, safe_url::normalize_safe_url, ComponentNode,
    LocalizedPageRouteEntry, ProjectDocument, RuntimeLocaleSelection, ValidationDiagnostic,
    ValidationSeverity,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

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

pub fn materialize_internal_page_links(
    document: &ProjectDocument,
    context: &Value,
) -> InternalLinkMaterialization {
    let route_index = localized_page_route_index(document);
    let selection = RuntimeLocaleSelection::from_context(context);
    let candidates = locale_candidates(&selection);
    let page_ids = document
        .project
        .pages
        .iter()
        .enumerate()
        .filter_map(|(index, page)| page.id.as_deref().map(|id| (id.to_string(), index)))
        .collect::<MapIndex>();
    let mut materialized = document.clone();
    let mut diagnostics = Vec::new();
    let mut resolved_links = 0usize;
    let mut fallback_links = 0usize;
    let mut unresolved_links = 0usize;

    for (page_index, page) in materialized.project.pages.iter_mut().enumerate() {
        let Some(root) = page.component.as_mut() else {
            continue;
        };
        materialize_node(
            root,
            &format!("project.pages[{page_index}].component"),
            &route_index,
            &page_ids,
            &candidates,
            &mut diagnostics,
            &mut resolved_links,
            &mut fallback_links,
            &mut unresolved_links,
        );
    }

    InternalLinkMaterialization {
        document: materialized,
        diagnostics,
        resolved_links,
        fallback_links,
        unresolved_links,
    }
}

pub fn validate_internal_page_links(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    let route_index = localized_page_route_index(document);
    let page_ids = document
        .project
        .pages
        .iter()
        .enumerate()
        .filter_map(|(index, page)| page.id.as_deref().map(|id| (id.to_string(), index)))
        .collect::<MapIndex>();
    let routed_pages = route_index
        .iter()
        .map(|entry| entry.page_index)
        .collect::<BTreeSet<_>>();
    let mut diagnostics = Vec::new();
    for (page_index, page) in document.project.pages.iter().enumerate() {
        let Some(root) = page.component.as_ref() else {
            continue;
        };
        validate_node(
            root,
            &format!("project.pages[{page_index}].component"),
            &page_ids,
            &routed_pages,
            &mut diagnostics,
        );
    }
    diagnostics
}

type MapIndex = std::collections::BTreeMap<String, usize>;

#[allow(clippy::too_many_arguments)]
fn materialize_node(
    node: &mut ComponentNode,
    path: &str,
    route_index: &[LocalizedPageRouteEntry],
    page_ids: &MapIndex,
    candidates: &[String],
    diagnostics: &mut Vec<ValidationDiagnostic>,
    resolved_links: &mut usize,
    fallback_links: &mut usize,
    unresolved_links: &mut usize,
) {
    let Some(component) = node.as_object_mut() else {
        return;
    };
    if let Some(raw) = component.extensions.get(FLY_PAGE_LINK_FIELD).cloned() {
        clear_internal_link_materialization(component);
        let component_id = component.id.clone();
        match serde_json::from_value::<InternalPageLink>(raw) {
            Ok(link) => match link.normalized() {
                Ok(link) => match page_ids.get(&link.page_id).copied() {
                    Some(target_page_index) => {
                        if let Some(slug) = route_slug(route_index, target_page_index, candidates) {
                            component.tag_name = Some("a".to_string());
                            component.attributes.insert(
                                "href".to_string(),
                                Value::String(build_href(&link, slug)),
                            );
                            *resolved_links = resolved_links.saturating_add(1);
                        } else if let Some(fallback_href) = link.fallback_href {
                            component.tag_name = Some("a".to_string());
                            component
                                .attributes
                                .insert("href".to_string(), Value::String(fallback_href));
                            *fallback_links = fallback_links.saturating_add(1);
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
                        } else {
                            *unresolved_links = unresolved_links.saturating_add(1);
                            diagnostics.push(link_diagnostic(
                                ValidationSeverity::Warning,
                                "internal_page_link_slug_unresolved",
                                path,
                                component_id,
                                format!(
                                    "internal page link target `{}` has no route for the active locale",
                                    link.page_id
                                ),
                            ));
                        }
                    }
                    None => {
                        *unresolved_links = unresolved_links.saturating_add(1);
                        diagnostics.push(link_diagnostic(
                            ValidationSeverity::Warning,
                            "internal_page_link_target_missing",
                            path,
                            component_id,
                            format!(
                                "internal page link target `{}` does not exist",
                                link.page_id
                            ),
                        ));
                    }
                },
                Err(error) => {
                    *unresolved_links = unresolved_links.saturating_add(1);
                    diagnostics.push(link_diagnostic(
                        ValidationSeverity::Warning,
                        "internal_page_link_invalid",
                        path,
                        component_id,
                        error,
                    ));
                }
            },
            Err(error) => {
                *unresolved_links = unresolved_links.saturating_add(1);
                diagnostics.push(link_diagnostic(
                    ValidationSeverity::Warning,
                    "internal_page_link_invalid",
                    path,
                    component_id,
                    format!("internal page link cannot be decoded: {error}"),
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
                page_ids,
                candidates,
                diagnostics,
                resolved_links,
                fallback_links,
                unresolved_links,
            );
        }
    }
}

fn validate_node(
    node: &ComponentNode,
    path: &str,
    page_ids: &MapIndex,
    routed_pages: &BTreeSet<usize>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let Some(component) = node.as_object() else {
        return;
    };
    if let Some(raw) = component.extensions.get(FLY_PAGE_LINK_FIELD).cloned() {
        let component_id = component.id.clone();
        match serde_json::from_value::<InternalPageLink>(raw) {
            Ok(link) => match link.normalized() {
                Ok(link) => match page_ids.get(&link.page_id).copied() {
                    Some(page_index) if routed_pages.contains(&page_index) => {}
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
                },
                Err(error) => diagnostics.push(link_diagnostic(
                    ValidationSeverity::Error,
                    "internal_page_link_invalid",
                    path,
                    component_id,
                    error,
                )),
            },
            Err(error) => diagnostics.push(link_diagnostic(
                ValidationSeverity::Error,
                "internal_page_link_invalid",
                path,
                component_id,
                format!("internal page link cannot be decoded: {error}"),
            )),
        }
    }
    for (index, child) in component.children().iter().enumerate() {
        validate_node(
            child,
            &format!("{path}.components[{index}]"),
            page_ids,
            routed_pages,
            diagnostics,
        );
    }
}

fn clear_internal_link_materialization(component: &mut crate::ComponentObject) {
    for attribute in GENERATED_INTERNAL_LINK_ATTRIBUTES {
        component.attributes.remove(*attribute);
    }
}

fn route_slug<'a>(
    route_index: &'a [LocalizedPageRouteEntry],
    page_index: usize,
    candidates: &[String],
) -> Option<&'a str> {
    for locale in candidates {
        if let Some(entry) = route_index.iter().find(|entry| {
            entry.page_index == page_index && entry.locale.as_deref() == Some(locale.as_str())
        }) {
            return Some(entry.slug.as_str());
        }
    }
    route_index
        .iter()
        .find(|entry| entry.page_index == page_index && entry.locale.is_none())
        .or_else(|| route_index.iter().find(|entry| entry.page_index == page_index))
        .map(|entry| entry.slug.as_str())
}

fn locale_candidates(selection: &RuntimeLocaleSelection) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(locale) = selection.locale.as_deref() {
        push_locale_candidate(&mut candidates, locale);
    }
    for locale in &selection.fallback_locales {
        push_locale_candidate(&mut candidates, locale);
    }
    candidates
}

fn push_locale_candidate(candidates: &mut Vec<String>, locale: &str) {
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

fn build_href(link: &InternalPageLink, slug: &str) -> String {
    let base_path = link.base_path.as_deref().unwrap_or("/");
    let mut href = if base_path == "/" {
        format!("/{slug}")
    } else {
        format!("{base_path}/{slug}")
    };
    if let Some(query) = link.query.as_deref() {
        href.push('?');
        href.push_str(query);
    }
    if let Some(fragment) = link.fragment.as_deref() {
        href.push('#');
        href.push_str(fragment);
    }
    href
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
    use crate::GrapesJsV1Codec;
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsV1Codec::decode_value(json!({
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
        assert!(validate_internal_page_links(&unsafe_fallback)
            .iter()
            .any(|diagnostic| diagnostic.code == "internal_page_link_invalid"));

        let mut network_base = document();
        network_base
            .component_mut("about-link")
            .unwrap()
            .extensions
            .get_mut(FLY_PAGE_LINK_FIELD)
            .unwrap()["base_path"] = json!("//cdn.example");
        assert!(validate_internal_page_links(&network_base)
            .iter()
            .any(|diagnostic| diagnostic.code == "internal_page_link_invalid"));
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
        assert!(validate_internal_page_links(&query)
            .iter()
            .any(|diagnostic| diagnostic.code == "internal_page_link_invalid"));

        let mut fragment = document();
        fragment
            .component_mut("about-link")
            .unwrap()
            .extensions
            .get_mut(FLY_PAGE_LINK_FIELD)
            .unwrap()["fragment"] = json!("team\\details");
        assert!(validate_internal_page_links(&fragment)
            .iter()
            .any(|diagnostic| diagnostic.code == "internal_page_link_invalid"));
    }
}
