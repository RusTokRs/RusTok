use crate::{
    normalize_locale_tag, ProjectDocument, ProjectLocalePolicy, TranslationCatalog,
    FLY_LOCALES_FIELD, FLY_PAGE_METADATA_FIELD, LOCALIZED_VALUES_FIELD,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LocaleCoverageKind {
    Translation,
    PageMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocaleCoverageGap {
    pub locale: String,
    pub kind: LocaleCoverageKind,
    pub path: String,
    pub label: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocaleCoverageSummary {
    pub locale: String,
    pub required: bool,
    pub translation_total: usize,
    pub translation_present: usize,
    pub metadata_total: usize,
    pub metadata_present: usize,
    pub missing: usize,
}

impl LocaleCoverageSummary {
    pub fn complete(&self) -> bool {
        self.missing == 0
    }

    pub fn present(&self) -> usize {
        self.translation_present
            .saturating_add(self.metadata_present)
    }

    pub fn total(&self) -> usize {
        self.translation_total.saturating_add(self.metadata_total)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocaleCoverageReport {
    pub policy_present: bool,
    pub policy_valid: bool,
    pub strict_enforcement: bool,
    pub default_locale: Option<String>,
    pub supported_locales: Vec<String>,
    pub required_locales: Vec<String>,
    pub tracked_locales: Vec<String>,
    pub translation_total: usize,
    pub metadata_total: usize,
    pub summaries: Vec<LocaleCoverageSummary>,
    pub gaps: Vec<LocaleCoverageGap>,
}

impl LocaleCoverageReport {
    pub fn complete(&self) -> bool {
        self.gaps.is_empty()
    }

    pub fn required_complete(&self) -> bool {
        self.gaps.iter().all(|gap| !gap.required)
    }

    pub fn strict_ready(&self) -> bool {
        self.policy_valid && self.required_complete()
    }

    pub fn summary_for(&self, locale: &str) -> Option<&LocaleCoverageSummary> {
        let locale = normalize_locale_tag(locale)?;
        self.summaries
            .iter()
            .find(|summary| summary.locale == locale)
    }

    pub fn required_gaps(&self) -> impl Iterator<Item = &LocaleCoverageGap> {
        self.gaps.iter().filter(|gap| gap.required)
    }
}

pub fn analyze_project_locale_coverage(document: &ProjectDocument) -> LocaleCoverageReport {
    let policy_present = document.project.extensions.contains_key(FLY_LOCALES_FIELD);
    let decoded_policy = ProjectLocalePolicy::from_document(document);
    let policy = decoded_policy
        .as_ref()
        .and_then(|policy| policy.normalized().ok());
    let policy_valid = !policy_present || policy.is_some();
    let strict_enforcement = policy
        .as_ref()
        .is_some_and(|policy| policy.enforce_required_locales);
    let default_locale = policy
        .as_ref()
        .and_then(|policy| policy.default_locale.clone());
    let supported_locales = policy
        .as_ref()
        .map(|policy| policy.supported_locales.clone())
        .unwrap_or_default();
    let required_locales = policy
        .as_ref()
        .map(|policy| policy.required_locales.clone())
        .unwrap_or_default();

    let catalog = TranslationCatalog::from_document(document);
    let metadata_fields = collect_localized_metadata_fields(document);
    let tracked_locales = tracked_locales(
        policy.as_ref(),
        &catalog,
        &metadata_fields,
    );
    let required = required_locales.iter().cloned().collect::<BTreeSet<_>>();
    let mut summaries = Vec::new();
    let mut gaps = Vec::new();

    for locale in &tracked_locales {
        let required_locale = required.contains(locale);
        let mut translation_present = 0usize;
        for entry in &catalog.entries {
            if map_contains_locale(&entry.values, locale) {
                translation_present = translation_present.saturating_add(1);
            } else {
                gaps.push(LocaleCoverageGap {
                    locale: locale.clone(),
                    kind: LocaleCoverageKind::Translation,
                    path: format!("project.translations.{}", entry.id),
                    label: entry.id.clone(),
                    required: required_locale,
                });
            }
        }

        let mut metadata_present = 0usize;
        for field in &metadata_fields {
            if map_contains_locale(&field.values, locale) {
                metadata_present = metadata_present.saturating_add(1);
            } else {
                gaps.push(LocaleCoverageGap {
                    locale: locale.clone(),
                    kind: LocaleCoverageKind::PageMetadata,
                    path: field.path.clone(),
                    label: field.label.clone(),
                    required: required_locale,
                });
            }
        }

        let translation_total = catalog.entries.len();
        let metadata_total = metadata_fields.len();
        summaries.push(LocaleCoverageSummary {
            locale: locale.clone(),
            required: required_locale,
            translation_total,
            translation_present,
            metadata_total,
            metadata_present,
            missing: translation_total
                .saturating_sub(translation_present)
                .saturating_add(metadata_total.saturating_sub(metadata_present)),
        });
    }

    LocaleCoverageReport {
        policy_present,
        policy_valid,
        strict_enforcement,
        default_locale,
        supported_locales,
        required_locales,
        tracked_locales,
        translation_total: catalog.entries.len(),
        metadata_total: metadata_fields.len(),
        summaries,
        gaps,
    }
}

#[derive(Debug, Clone)]
struct MetadataCoverageField {
    path: String,
    label: String,
    values: Map<String, Value>,
}

fn collect_localized_metadata_fields(
    document: &ProjectDocument,
) -> Vec<MetadataCoverageField> {
    let mut fields = Vec::new();
    for (page_index, page) in document.project.pages.iter().enumerate() {
        let page_label = page
            .id
            .clone()
            .unwrap_or_else(|| format!("page-{}", page_index + 1));
        let Some(metadata) = page
            .extensions
            .get(FLY_PAGE_METADATA_FIELD)
            .and_then(Value::as_object)
        else {
            continue;
        };
        for field in LOCALIZED_METADATA_FIELDS {
            let Some(values) = metadata
                .get(*field)
                .and_then(Value::as_object)
                .and_then(|wrapper| wrapper.get(LOCALIZED_VALUES_FIELD))
                .and_then(Value::as_object)
                .cloned()
            else {
                continue;
            };
            fields.push(MetadataCoverageField {
                path: format!(
                    "project.pages[{page_index}].{FLY_PAGE_METADATA_FIELD}.{field}"
                ),
                label: format!("{page_label}.{field}"),
                values,
            });
        }
    }
    fields
}

fn tracked_locales(
    policy: Option<&ProjectLocalePolicy>,
    catalog: &TranslationCatalog,
    metadata_fields: &[MetadataCoverageField],
) -> Vec<String> {
    let mut locales = Vec::new();
    if let Some(policy) = policy {
        push_locale(&mut locales, policy.default_locale.as_deref());
        for locale in &policy.supported_locales {
            push_locale(&mut locales, Some(locale));
        }
        for locale in &policy.required_locales {
            push_locale(&mut locales, Some(locale));
        }
        for locale in &policy.fallback_locales {
            push_locale(&mut locales, Some(locale));
        }
    }
    for entry in &catalog.entries {
        for locale in entry.values.keys() {
            push_locale(&mut locales, Some(locale));
        }
    }
    for field in metadata_fields {
        for locale in field.values.keys() {
            push_locale(&mut locales, Some(locale));
        }
    }
    locales
}

fn push_locale(locales: &mut Vec<String>, locale: Option<&str>) {
    let Some(locale) = locale.and_then(normalize_locale_tag) else {
        return;
    };
    if !locales.contains(&locale) {
        locales.push(locale);
    }
}

fn map_contains_locale(values: &Map<String, Value>, required_locale: &str) -> bool {
    values
        .keys()
        .any(|locale| normalize_locale_tag(locale).as_deref() == Some(required_locale))
}

const LOCALIZED_METADATA_FIELDS: &[&str] = &[
    "title",
    "description",
    "slug",
    "canonical_url",
    "open_graph_title",
    "open_graph_description",
    "open_graph_image",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsV1Codec;
    use serde_json::json;

    fn document(project: Value) -> ProjectDocument {
        GrapesJsV1Codec::decode_value(project).expect("project document")
    }

    #[test]
    fn coverage_reports_exact_translation_and_metadata_gaps() {
        let document = document(json!({
            "flyLocales": {
                "default_locale": "en",
                "supported_locales": ["en", "ru"],
                "required_locales": ["en", "ru"],
                "enforce_required_locales": true
            },
            "flyTranslations": [{
                "id": "hero",
                "values": { "en": "Welcome" }
            }, {
                "id": "cta",
                "values": { "en": "Buy", "ru": "Купить" }
            }],
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": { "$localized": { "en": "Home", "ru": "Главная" } },
                    "description": { "$localized": { "en": "Welcome" } }
                },
                "component": { "id": "root", "type": "wrapper" }
            }]
        }));
        let report = analyze_project_locale_coverage(&document);
        assert!(report.policy_valid);
        assert!(report.strict_enforcement);
        assert!(!report.required_complete());
        assert!(!report.strict_ready());
        let ru = report.summary_for("ru-RU").expect("ru summary");
        assert_eq!(ru.translation_present, 1);
        assert_eq!(ru.metadata_present, 1);
        assert_eq!(ru.missing, 2);
        assert!(report.gaps.iter().any(|gap| {
            gap.locale == "ru"
                && gap.kind == LocaleCoverageKind::Translation
                && gap.label == "hero"
        }));
        assert!(report.gaps.iter().any(|gap| {
            gap.locale == "ru"
                && gap.kind == LocaleCoverageKind::PageMetadata
                && gap.label == "home.description"
        }));
    }

    #[test]
    fn coverage_discovers_optional_locales_without_policy() {
        let document = document(json!({
            "flyTranslations": [{
                "id": "hero",
                "values": { "en": "Welcome", "de-DE": "Willkommen" }
            }, {
                "id": "cta",
                "values": { "en": "Buy" }
            }],
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }]
        }));
        let report = analyze_project_locale_coverage(&document);
        assert!(!report.policy_present);
        assert!(report.policy_valid);
        assert_eq!(report.tracked_locales, vec!["en", "de-de"]);
        assert!(report.required_complete());
        assert!(!report.complete());
        assert!(report.gaps.iter().all(|gap| !gap.required));
    }

    #[test]
    fn invalid_policy_prevents_strict_readiness() {
        let document = document(json!({
            "flyLocales": {
                "default_locale": "invalid locale",
                "supported_locales": ["en"]
            },
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }]
        }));
        let report = analyze_project_locale_coverage(&document);
        assert!(report.policy_present);
        assert!(!report.policy_valid);
        assert!(!report.strict_ready());
    }
}
