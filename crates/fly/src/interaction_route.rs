use crate::{
    LocalizedPageRouteEntry, ProjectDocument, RuntimeLocaleSelection, localized_page_route_index,
    normalize_locale_tag,
};
use std::collections::{BTreeMap, BTreeSet};

/// Shared route catalog used by every authoring interaction that targets a page.
///
/// Internal links and component actions must resolve the same page id, locale fallback chain,
/// slug, and href. Keeping this contract in one place prevents the editor preview, readiness
/// validation, and storefront runtime from producing different navigation URLs.
pub(crate) struct InteractionRouteCatalog {
    entries: Vec<LocalizedPageRouteEntry>,
    page_ids: BTreeMap<String, usize>,
    routed_pages: BTreeSet<usize>,
}

impl InteractionRouteCatalog {
    pub(crate) fn from_document(document: &ProjectDocument) -> Self {
        let entries = localized_page_route_index(document);
        let routed_pages = entries.iter().map(|entry| entry.page_index).collect();
        let page_ids = document
            .project
            .pages
            .iter()
            .enumerate()
            .filter_map(|(index, page)| page.id.as_deref().map(|id| (id.to_string(), index)))
            .collect();
        Self {
            entries,
            page_ids,
            routed_pages,
        }
    }

    pub(crate) fn page_index(&self, page_id: &str) -> Option<usize> {
        self.page_ids.get(page_id).copied()
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

pub(crate) fn interaction_locale_candidates(selection: &RuntimeLocaleSelection) -> Vec<String> {
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
        let candidates = interaction_locale_candidates(&RuntimeLocaleSelection {
            locale: Some("ru-RU".to_string()),
            fallback_locales: vec!["en".to_string()],
        });
        assert!(catalog.has_route(page_index));
        assert_eq!(catalog.slug_for(page_index, &candidates), Some("o-nas"));
        assert_eq!(
            build_interaction_href(Some("/site/"), "o-nas", Some("from=hero"), Some("team")),
            "/site/o-nas?from=hero#team"
        );
    }
}
