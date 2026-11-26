use crate::{LocalizedPageRouteEntry, ProjectDocument, localized_page_route_index};
use std::collections::{BTreeMap, BTreeSet};

/// Shared route catalog used by every authoring interaction that targets a page.
///
/// Internal links and component actions must resolve the same page id, locale fallback chain,
/// slug, and href. Keeping this contract in one place prevents the editor preview, readiness
/// validation, and storefront runtime from producing different navigation URLs.
pub(crate) struct InteractionRouteCatalog {
    entries: Vec<LocalizedPageRouteEntry>,
    page_ids: BTreeMap<String, Option<usize>>,
    routed_pages: BTreeSet<usize>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum PageTargetError {
    #[error("target page `{0}` does not exist")]
    Missing(String),
    #[error("target page `{0}` is ambiguous: duplicate page id")]
    Ambiguous(String),
}

impl InteractionRouteCatalog {
    pub(crate) fn from_document(document: &ProjectDocument) -> Self {
        let entries = localized_page_route_index(document);
        let routed_pages = entries.iter().map(|entry| entry.page_index).collect();
        let mut page_ids = BTreeMap::new();
        for (index, page) in document.project().pages.iter().enumerate() {
            if let Some(id) = &page.id {
                page_ids
                    .entry(id.clone())
                    .and_modify(|entry| *entry = None)
                    .or_insert(Some(index));
            }
        }
        Self {
            entries,
            page_ids,
            routed_pages,
        }
    }

    pub(crate) fn page_index(&self, page_id: &str) -> Result<usize, PageTargetError> {
        match self.page_ids.get(page_id) {
            Some(Some(index)) => Ok(*index),
            Some(None) => Err(PageTargetError::Ambiguous(page_id.to_string())),
            None => Err(PageTargetError::Missing(page_id.to_string())),
        }
    }

    pub(crate) fn has_route(&self, page_index: usize) -> bool {
        self.routed_pages.contains(&page_index)
    }

    pub(crate) fn slug_for(&self, page_index: usize, locale_candidates: &[String]) -> Option<&str> {
        for locale in locale_candidates {
            if let Some(route) = self.entries.iter().find(|route| {
                route.page_index == page_index && route.locale.as_deref() == Some(locale.as_str())
            }) {
                return Some(route.slug.as_str());
            }
        }
        self.entries
            .iter()
            .find(|route| route.page_index == page_index && route.locale.is_none())
            .or_else(|| {
                self.entries
                    .iter()
                    .find(|route| route.page_index == page_index)
            })
            .map(|route| route.slug.as_str())
    }
}

pub(crate) fn build_interaction_href(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GrapesJsCodec, RuntimeLocaleSelection};
    use serde_json::json;

    #[test]
    fn links_and_actions_use_the_same_script_fallback_as_runtime_values() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "link",
                        "type": "link",
                        "flyPageLink": { "page_id": "about" }
                    }, {
                        "id": "action",
                        "type": "button",
                        "flyAction": { "kind": "navigate_page", "page_id": "about" }
                    }]
                }
            }, {
                "id": "about",
                "flyPageMeta": {
                    "slug": { "$localized": { "zh": "generic", "zh-Hant": "traditional" } }
                },
                "component": { "id": "about-root", "type": "wrapper" }
            }]
        }))
        .expect("document");
        #[cfg(not(feature = "platform-i18n"))]
        let locales = ["zh-Hant-TW"];
        #[cfg(feature = "platform-i18n")]
        let locales = ["zh-Hant-TW", "zh-TW"];
        for locale in locales {
            let context = json!({ "$locale": locale });
            let linked = crate::materialize_internal_page_links(&document, &context);
            let actions = crate::materialize_component_actions(&document, &context);
            assert_eq!(
                linked.document.component("link").unwrap().attributes["href"],
                "/traditional",
                "{locale}"
            );
            assert_eq!(
                actions.document.component("action").unwrap().attributes["href"],
                "/traditional",
                "{locale}"
            );
        }
    }

    #[test]
    fn duplicate_page_identity_cannot_materialize_either_navigation_contract() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "link",
                        "type": "link",
                        "attributes": { "href": "/stale", "HREF": "/stale-uppercase" },
                        "flyPageLink": { "page_id": "about", "fallback_href": "/fallback" }
                    }, {
                        "id": "action",
                        "type": "button",
                        "attributes": { "href": "/stale", "HREF": "/stale-uppercase" },
                        "flyAction": {
                            "kind": "navigate_page", "page_id": "about",
                            "fallback_href": "/fallback"
                        }
                    }]
                }
            }, {
                "id": "about",
                "flyPageMeta": { "slug": "first" },
                "component": { "id": "first-root", "type": "wrapper" }
            }, {
                "id": "about",
                "flyPageMeta": { "slug": "second" },
                "component": { "id": "second-root", "type": "wrapper" }
            }, {
                "id": "about",
                "flyPageMeta": { "slug": "third" },
                "component": { "id": "third-root", "type": "wrapper" }
            }]
        }))
        .expect("imported project");
        let catalog = InteractionRouteCatalog::from_document(&document);
        assert!(matches!(
            catalog.page_index("about"),
            Err(PageTargetError::Ambiguous(_))
        ));
        assert!(matches!(
            catalog.page_index("missing"),
            Err(PageTargetError::Missing(_))
        ));
        assert!(crate::validate_internal_page_links(&document).iter().any(|diagnostic| {
            diagnostic.code == "internal_page_link_target_ambiguous"
                && diagnostic.severity == crate::ValidationSeverity::Error
        }));
        assert!(crate::validate_component_actions(&document).iter().any(|diagnostic| {
            diagnostic.code == "action_definition_invalid"
                && diagnostic.severity == crate::ValidationSeverity::Error
                && diagnostic.message.contains("ambiguous")
        }));
        let materialized = crate::materialize_project_with_runtime_context(&document, &json!({}));
        assert_eq!(materialized.unresolved_internal_links, 1);
        assert_eq!(materialized.unresolved_actions, 1);
        assert_eq!(materialized.fallback_internal_links, 0);
        assert_eq!(materialized.fallback_actions, 0);
        for id in ["link", "action"] {
            let attributes = &materialized.document.component(id).unwrap().attributes;
            assert!(!attributes.keys().any(|name| name.eq_ignore_ascii_case("href")));
            assert_eq!(document.component(id).unwrap().attributes["href"], "/stale");
            assert_eq!(document.component(id).unwrap().attributes["HREF"], "/stale-uppercase");
        }
    }

    #[test]
    fn catalog_resolves_identical_locale_fallback_for_all_interactions() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "about",
                "flyPageMeta": {
                    "slug": { "$localized": { "en": "about", "ru": "o-nas" } }
                },
                "component": { "id": "root", "type": "wrapper" }
            }]
        }))
        .expect("document");
        let catalog = InteractionRouteCatalog::from_document(&document);
        let page_index = catalog.page_index("about").expect("page");
        let candidates = RuntimeLocaleSelection {
            locale: Some("ru-RU".to_string()),
            fallback_locales: vec!["en".to_string()],
        }
        .candidates();
        assert!(catalog.has_route(page_index));
        assert_eq!(catalog.slug_for(page_index, &candidates), Some("o-nas"));
        assert_eq!(
            build_interaction_href(Some("/site/"), "o-nas", Some("from=hero"), Some("team")),
            "/site/o-nas?from=hero#team"
        );
    }
}
