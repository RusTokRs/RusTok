use crate::{
    FLY_PAGE_METADATA_FIELD, FlyError, FlyResult, LOCALIZED_VALUES_FIELD, PageSelection,
    ProjectDocument, RUNTIME_LOCALE_FIELD, RuntimeLocaleSelection, ValidationDiagnostic,
    ValidationSeverity, materialize_project_locale_context, normalize_locale_tag, normalize_slug,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalizedPageRouteEntry {
    pub page_index: usize,
    pub page_id: Option<String>,
    pub locale: Option<String>,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalizedPageRouteResolution {
    pub page_index: usize,
    pub page_id: Option<String>,
    pub requested_slug: String,
    pub resolved_slug: String,
    pub canonical_slug: String,
    pub requested_locale: Option<String>,
    pub matched_locale: Option<String>,
    pub fallback_used: bool,
    pub inferred_locale: bool,
    pub context: Value,
}

impl LocalizedPageRouteResolution {
    pub fn selection(&self) -> PageSelection {
        PageSelection::Index(self.page_index)
    }

    pub fn canonical_redirect_needed(&self) -> bool {
        self.requested_slug != self.canonical_slug
    }
}

pub fn localized_page_route_index(document: &ProjectDocument) -> Vec<LocalizedPageRouteEntry> {
    let mut entries = Vec::new();
    for (page_index, page) in document.project.pages.iter().enumerate() {
        let Some(metadata) = page
            .extensions
            .get(FLY_PAGE_METADATA_FIELD)
            .and_then(Value::as_object)
        else {
            continue;
        };
        let Some(slug) = metadata.get("slug") else {
            continue;
        };
        match slug {
            Value::String(slug) => {
                push_route_entry(&mut entries, page_index, page.id.clone(), None, slug)
            }
            Value::Object(wrapper) => {
                if let Some(localized) = wrapper
                    .get(LOCALIZED_VALUES_FIELD)
                    .and_then(Value::as_object)
                {
                    for (locale, slug) in localized {
                        let Some(locale) = normalize_locale_tag(locale) else {
                            continue;
                        };
                        if let Some(slug) = slug.as_str() {
                            push_route_entry(
                                &mut entries,
                                page_index,
                                page.id.clone(),
                                Some(locale),
                                slug,
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
    entries
}

pub fn resolve_localized_page_route(
    document: &ProjectDocument,
    requested_slug: &str,
    input_context: &Value,
) -> FlyResult<LocalizedPageRouteResolution> {
    let requested_slug = normalize_slug(requested_slug.to_string());
    if requested_slug.is_empty() {
        return Err(FlyError::PageNotFound(requested_slug));
    }
    let policy_context = materialize_project_locale_context(document, input_context).context;
    let selection = RuntimeLocaleSelection::from_context(&policy_context);
    let requested_locale = selection.locale.clone();
    let candidates = locale_candidates(&selection);
    let entries = localized_page_route_index(document);

    let mut matched = None;
    for locale in &candidates {
        let matches = matching_entries(&entries, &requested_slug, Some(locale));
        matched = unique_match(matches, &requested_slug, Some(locale))?;
        if matched.is_some() {
            break;
        }
    }
    if matched.is_none() {
        matched = unique_match(
            matching_entries(&entries, &requested_slug, None),
            &requested_slug,
            None,
        )?;
    }
    if matched.is_none() {
        let matches = entries
            .iter()
            .filter(|entry| entry.slug == requested_slug)
            .collect::<Vec<_>>();
        matched = unique_match(matches, &requested_slug, None)?;
    }
    let matched = matched.ok_or_else(|| FlyError::PageNotFound(requested_slug.clone()))?;

    let mut context = policy_context.as_object().cloned().unwrap_or_default();
    let inferred_locale = requested_locale.is_none() && matched.locale.is_some();
    if let Some(locale) = matched.locale.as_deref() {
        context.insert(
            RUNTIME_LOCALE_FIELD.to_string(),
            Value::String(locale.to_string()),
        );
    }
    let canonical_slug = canonical_slug_for_page(
        &entries,
        matched.page_index,
        matched.locale.as_deref().or(requested_locale.as_deref()),
    )
    .unwrap_or_else(|| matched.slug.clone());
    let fallback_used = match (requested_locale.as_deref(), matched.locale.as_deref()) {
        (Some(requested), Some(matched)) => requested != matched,
        (Some(_), None) => true,
        _ => false,
    };

    Ok(LocalizedPageRouteResolution {
        page_index: matched.page_index,
        page_id: matched.page_id.clone(),
        requested_slug,
        resolved_slug: matched.slug.clone(),
        canonical_slug,
        requested_locale,
        matched_locale: matched.locale.clone(),
        fallback_used,
        inferred_locale,
        context: Value::Object(context),
    })
}

pub fn validate_localized_page_routes(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    let mut diagnostics = Vec::new();
    let entries = localized_page_route_index(document);
    let mut exact = BTreeMap::<(Option<String>, String), BTreeSet<usize>>::new();
    let mut global = BTreeMap::<String, BTreeSet<usize>>::new();
    for entry in &entries {
        exact
            .entry((entry.locale.clone(), entry.slug.clone()))
            .or_default()
            .insert(entry.page_index);
        global
            .entry(entry.slug.clone())
            .or_default()
            .insert(entry.page_index);
    }
    for ((locale, slug), pages) in exact {
        if pages.len() > 1 {
            diagnostics.push(route_diagnostic(
                ValidationSeverity::Error,
                "duplicate_localized_page_slug",
                format!(
                    "slug `{slug}` maps to multiple pages for locale `{}`",
                    locale.as_deref().unwrap_or("<plain>")
                ),
            ));
        }
    }
    for (slug, pages) in global {
        if pages.len() > 1 {
            diagnostics.push(route_diagnostic(
                ValidationSeverity::Warning,
                "localized_page_slug_requires_locale",
                format!(
                    "slug `{slug}` maps to different pages across locales and requires an explicit locale"
                ),
            ));
        }
    }
    diagnostics.extend(validate_raw_slug_values(document));
    diagnostics
}

fn validate_raw_slug_values(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    let mut diagnostics = Vec::new();
    for (page_index, page) in document.project.pages.iter().enumerate() {
        let Some(slug) = page
            .extensions
            .get(FLY_PAGE_METADATA_FIELD)
            .and_then(Value::as_object)
            .and_then(|metadata| metadata.get("slug"))
        else {
            continue;
        };
        match slug {
            Value::String(slug) if normalize_slug(slug.clone()).is_empty() => {
                diagnostics.push(raw_slug_diagnostic(
                    page_index,
                    ValidationSeverity::Error,
                    "page_slug_empty",
                    "page slug is empty after normalization",
                ));
            }
            Value::Object(wrapper) => match wrapper.get(LOCALIZED_VALUES_FIELD) {
                Some(Value::Object(values)) => {
                    for (locale, slug) in values {
                        if normalize_locale_tag(locale).is_none() {
                            diagnostics.push(raw_slug_diagnostic(
                                page_index,
                                ValidationSeverity::Error,
                                "localized_page_slug_locale_invalid",
                                format!("localized page slug locale `{locale}` is invalid"),
                            ));
                        }
                        match slug.as_str() {
                            Some(slug) if !normalize_slug(slug.to_string()).is_empty() => {}
                            Some(_) => diagnostics.push(raw_slug_diagnostic(
                                page_index,
                                ValidationSeverity::Error,
                                "localized_page_slug_empty",
                                format!(
                                    "localized page slug for locale `{locale}` is empty after normalization"
                                ),
                            )),
                            None => diagnostics.push(raw_slug_diagnostic(
                                page_index,
                                ValidationSeverity::Error,
                                "localized_page_slug_value_invalid",
                                format!(
                                    "localized page slug for locale `{locale}` must be a string"
                                ),
                            )),
                        }
                    }
                }
                _ => diagnostics.push(raw_slug_diagnostic(
                    page_index,
                    ValidationSeverity::Error,
                    "localized_page_slug_wrapper_invalid",
                    format!("localized page slug must contain `{LOCALIZED_VALUES_FIELD}` object"),
                )),
            },
            Value::String(_) => {}
            _ => diagnostics.push(raw_slug_diagnostic(
                page_index,
                ValidationSeverity::Error,
                "page_slug_value_invalid",
                "page slug must be a string or localized value wrapper",
            )),
        }
    }
    diagnostics
}

fn push_route_entry(
    entries: &mut Vec<LocalizedPageRouteEntry>,
    page_index: usize,
    page_id: Option<String>,
    locale: Option<String>,
    slug: &str,
) {
    let slug = normalize_slug(slug.to_string());
    if !slug.is_empty() {
        entries.push(LocalizedPageRouteEntry {
            page_index,
            page_id,
            locale,
            slug,
        });
    }
}

fn matching_entries<'a>(
    entries: &'a [LocalizedPageRouteEntry],
    slug: &str,
    locale: Option<&str>,
) -> Vec<&'a LocalizedPageRouteEntry> {
    entries
        .iter()
        .filter(|entry| entry.slug == slug && entry.locale.as_deref() == locale)
        .collect()
}

fn unique_match<'a>(
    matches: Vec<&'a LocalizedPageRouteEntry>,
    slug: &str,
    locale: Option<&str>,
) -> FlyResult<Option<&'a LocalizedPageRouteEntry>> {
    let pages = matches
        .iter()
        .map(|entry| entry.page_index)
        .collect::<BTreeSet<_>>();
    if pages.len() > 1 {
        return Err(FlyError::Decode(format!(
            "localized slug `{slug}` is ambiguous for locale `{}`",
            locale.unwrap_or("<unspecified>")
        )));
    }
    Ok(matches.into_iter().next())
}

fn canonical_slug_for_page(
    entries: &[LocalizedPageRouteEntry],
    page_index: usize,
    locale: Option<&str>,
) -> Option<String> {
    let candidates = locale
        .map(|locale| {
            let selection = RuntimeLocaleSelection {
                locale: Some(locale.to_string()),
                fallback_locales: Vec::new(),
            };
            locale_candidates(&selection)
        })
        .unwrap_or_default();
    for candidate in candidates {
        if let Some(entry) = entries.iter().find(|entry| {
            entry.page_index == page_index && entry.locale.as_deref() == Some(candidate.as_str())
        }) {
            return Some(entry.slug.clone());
        }
    }
    entries
        .iter()
        .find(|entry| entry.page_index == page_index && entry.locale.is_none())
        .or_else(|| entries.iter().find(|entry| entry.page_index == page_index))
        .map(|entry| entry.slug.clone())
}

fn locale_candidates(selection: &RuntimeLocaleSelection) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(locale) = selection.locale.as_deref() {
        push_candidate(&mut candidates, locale);
    }
    for locale in &selection.fallback_locales {
        push_candidate(&mut candidates, locale);
    }
    candidates
}

fn push_candidate(candidates: &mut Vec<String>, locale: &str) {
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

fn route_diagnostic(
    severity: ValidationSeverity,
    code: impl Into<String>,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
        code: code.into(),
        path: "project.pages.routes".to_string(),
        message: message.into(),
    }
}

fn raw_slug_diagnostic(
    page_index: usize,
    severity: ValidationSeverity,
    code: impl Into<String>,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
        code: code.into(),
        path: format!("project.pages[{page_index}].{FLY_PAGE_METADATA_FIELD}.slug"),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::json;

    fn document(project: Value) -> ProjectDocument {
        GrapesJsCodec::decode_value(project).expect("project document")
    }

    #[test]
    fn localized_slug_resolution_selects_page_and_render_locale() {
        let document = document(json!({
            "flyLocales": {
                "default_locale": "en",
                "supported_locales": ["en", "ru"]
            },
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "slug": { "$localized": { "en": "home", "ru": "glavnaya" } }
                },
                "component": { "id": "root", "type": "wrapper" }
            }]
        }));
        let resolution =
            resolve_localized_page_route(&document, "glavnaya", &json!({ "$locale": "ru-RU" }))
                .expect("localized route");
        assert_eq!(resolution.page_id.as_deref(), Some("home"));
        assert_eq!(resolution.matched_locale.as_deref(), Some("ru"));
        assert_eq!(resolution.context[RUNTIME_LOCALE_FIELD], "ru");
        assert!(resolution.fallback_used);
        assert_eq!(resolution.selection(), PageSelection::Index(0));
    }

    #[test]
    fn unique_localized_slug_can_infer_locale() {
        let document = document(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "slug": { "$localized": { "en": "home", "ru": "glavnaya" } }
                },
                "component": { "id": "root", "type": "wrapper" }
            }]
        }));
        let resolution = resolve_localized_page_route(&document, "glavnaya", &json!({}))
            .expect("inferred route");
        assert!(resolution.inferred_locale);
        assert_eq!(resolution.context[RUNTIME_LOCALE_FIELD], "ru");
    }

    #[test]
    fn duplicate_slug_for_same_locale_is_rejected_and_validated() {
        let document = document(json!({
            "pages": [{
                "id": "one",
                "flyPageMeta": { "slug": { "$localized": { "en": "shared" } } },
                "component": { "id": "root-one", "type": "wrapper" }
            }, {
                "id": "two",
                "flyPageMeta": { "slug": { "$localized": { "en": "shared" } } },
                "component": { "id": "root-two", "type": "wrapper" }
            }]
        }));
        assert!(matches!(
            resolve_localized_page_route(&document, "shared", &json!({ "$locale": "en" })),
            Err(FlyError::Decode(_))
        ));
        assert!(
            validate_localized_page_routes(&document)
                .iter()
                .any(|diagnostic| diagnostic.code == "duplicate_localized_page_slug")
        );
    }
}
