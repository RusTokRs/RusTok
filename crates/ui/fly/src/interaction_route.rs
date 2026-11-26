use crate::{
    FLY_PAGE_METADATA_FIELD, FlyError, FlyResult, LOCALIZED_VALUES_FIELD, LocalizedPageRouteEntry,
    ProjectDocument, RuntimeLocaleSelection, localized_page_route_index, normalize_slug,
    localized_route::match_route_entry,
    runtime_locale::{duplicate_locale_key, select_translation},
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Shared route catalog used by every authoring interaction that targets a page.
///
/// Internal links and component actions must resolve the same page id, locale fallback chain,
/// slug, and href. Keeping this contract in one place prevents the editor preview, readiness
/// validation, and storefront runtime from producing different navigation URLs.
pub(crate) struct InteractionRouteCatalog {
    entries: Vec<LocalizedPageRouteEntry>,
    slugs: Vec<Option<Value>>,
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
        let slugs = document
            .project()
            .pages
            .iter()
            .map(|page| {
                page.extensions
                    .get(FLY_PAGE_METADATA_FIELD)
                    .and_then(|metadata| metadata.get("slug"))
                    .cloned()
            })
            .collect();
        Self {
            entries,
            slugs,
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

    pub(crate) fn slug_for(
        &self,
        page_index: usize,
        selection: &RuntimeLocaleSelection,
    ) -> FlyResult<Option<String>> {
        let Some(raw) = self.slugs.get(page_index).and_then(Option::as_ref) else {
            return Ok(None);
        };
        let selected = match raw {
            Value::String(value) => Some(value.as_str()),
            Value::Object(wrapper) => {
                let Some(values) = wrapper.get(LOCALIZED_VALUES_FIELD).and_then(Value::as_object)
                else {
                    return Ok(None);
                };
                if let Some(locale) = duplicate_locale_key(values) {
                    return Err(FlyError::Decode(format!(
                        "target page index {page_index} has ambiguous slug locale `{locale}`"
                    )));
                }
                select_translation(wrapper, values, selection)
                    .and_then(|(value, _, _)| value.as_str())
            }
            _ => None,
        };
        let Some(slug) = selected.map(|value| normalize_slug(value.to_string()))
            .filter(|slug| !slug.is_empty())
        else {
            return Ok(None);
        };
        // A slug belonging to the target page is not necessarily reachable under
        // the active locale: another page may shadow it at a higher precedence.
        match match_route_entry(&self.entries, &slug, &selection.candidates())? {
            Some(entry) if entry.page_index == page_index => Ok(Some(slug)),
            _ => Err(FlyError::Decode(format!(
                "localized slug `{slug}` does not resolve to target page index {page_index}"
            ))),
        }
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
    use crate::GrapesJsCodec;
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
    fn outgoing_navigation_respects_inbound_precedence_and_explicit_slug_fallback() {
        let mut document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "link", "type": "link",
                        "flyPageLink": { "page_id": "target", "fallback_href": "/fallback" }
                    }, {
                        "id": "action", "type": "button",
                        "flyAction": { "kind": "navigate_page", "page_id": "target", "fallback_href": "/fallback" }
                    }]
                }
            }, {
                "id": "target",
                "flyPageMeta": { "slug": {
                    "$localized": { "en": "shared", "fr": "unique" }, "$fallback": "en"
                } },
                "component": { "id": "target-root", "type": "wrapper" }
            }, {
                "id": "other",
                "flyPageMeta": { "slug": { "$localized": { "de": "shared" } } },
                "component": { "id": "other-root", "type": "wrapper" }
            }]
        })).expect("document");
        let context = json!({ "$locale": "de" });
        let incoming = crate::resolve_localized_page_route(&document, "shared", &context).unwrap();
        assert_eq!(incoming.page_id.as_deref(), Some("other"));

        let links = crate::materialize_internal_page_links(&document, &context);
        let actions = crate::materialize_component_actions(&document, &context);
        let full = crate::materialize_project_with_runtime_context(&document, &context);
        assert_eq!(links.unresolved_links, 1);
        assert_eq!(actions.unresolved_actions, 1);
        assert_eq!(full.unresolved_internal_links, 1);
        assert_eq!(full.unresolved_actions, 1);
        assert_eq!(full.fallback_internal_links, 0);
        assert_eq!(full.fallback_actions, 0);
        for (result, id) in [
            (&links.document, "link"),
            (&actions.document, "action"),
            (&full.document, "link"),
            (&full.document, "action"),
        ] {
            assert!(!result.component(id).unwrap().attributes.contains_key("href"));
        }

        document.project_mut().pages[1].extensions
            .get_mut(FLY_PAGE_METADATA_FIELD).unwrap()["slug"]["$fallback"] = json!("fr");
        let full = crate::materialize_project_with_runtime_context(&document, &context);
        let links = crate::materialize_internal_page_links(&document, &context);
        let actions = crate::materialize_component_actions(&document, &context);
        for (result, id) in [
            (&links.document, "link"),
            (&actions.document, "action"),
            (&full.document, "link"),
            (&full.document, "action"),
        ] {
            assert_eq!(result.component(id).unwrap().attributes["href"], "/unique");
        }
        let incoming = crate::resolve_localized_page_route(&document, "unique", &context).unwrap();
        assert_eq!(incoming.page_id.as_deref(), Some("target"));

        // A multi-language slug with no matching locale or configured fallback
        // must not silently select the first translation.
        document.project_mut().pages[1].extensions
            .get_mut(FLY_PAGE_METADATA_FIELD).unwrap()["slug"]
            .as_object_mut().unwrap().remove("$fallback");
        let full = crate::materialize_project_with_runtime_context(&document, &context);
        assert_eq!(full.fallback_internal_links, 1);
        assert_eq!(full.fallback_actions, 1);
    }

    #[test]
    fn ambiguous_slug_locale_blocks_inbound_and_outbound_navigation() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root", "type": "wrapper",
                    "components": [{
                        "id": "link", "type": "link",
                        "flyPageLink": { "page_id": "target", "fallback_href": "/fallback" }
                    }, {
                        "id": "action", "type": "button",
                        "flyAction": { "kind": "navigate_page", "page_id": "target", "fallback_href": "/fallback" }
                    }]
                }
            }, {
                "id": "target",
                "flyPageMeta": {
                    "slug": { "$localized": { "ru-RU": "first", "ru_RU": "second", "en": "third" } }
                },
                "component": { "id": "target-root", "type": "wrapper" }
            }]
        })).expect("document");
        let context = json!({ "$locale": "ru-RU" });
        for slug in ["first", "second", "third"] {
            assert!(crate::resolve_localized_page_route(&document, slug, &context).is_err());
        }
        assert!(crate::validate_runtime_extensions(&document).iter().any(|diagnostic| {
            diagnostic.code == "localized_metadata_locale_ambiguous"
        }));
        let full = crate::materialize_project_with_runtime_context(&document, &context);
        assert_eq!(full.unresolved_internal_links, 1);
        assert_eq!(full.unresolved_actions, 1);
        assert_eq!(full.fallback_internal_links, 0);
        assert_eq!(full.fallback_actions, 0);
        for id in ["link", "action"] {
            assert!(!full.document.component(id).unwrap().attributes.contains_key("href"));
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
        let selection = RuntimeLocaleSelection {
            locale: Some("ru-RU".to_string()),
            fallback_locales: vec!["en".to_string()],
        };
        assert!(catalog.has_route(page_index));
        assert_eq!(
            catalog.slug_for(page_index, &selection).unwrap().as_deref(),
            Some("o-nas")
        );
        assert_eq!(
            build_interaction_href(Some("/site/"), "o-nas", Some("from=hero"), Some("team")),
            "/site/o-nas?from=hero#team"
        );
    }
}
