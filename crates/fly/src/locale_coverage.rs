use crate::{
    FLY_LOCALES_FIELD, FLY_PAGE_METADATA_FIELD, LOCALIZED_VALUES_FIELD, ProjectDocument,
    ProjectLocalePolicy, TranslationCatalog, normalize_locale_tag,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
            .or_else(|| {
                locale.split_once('-').and_then(|(language, _)| {
                    self.summaries
                        .iter()
                        .find(|summary| summary.locale == language)
                })
            })
    }

    pub fn required_gaps(&self) -> impl Iterator<Item = &LocaleCoverageGap> {
        self.gaps.iter().filter(|gap| gap.required)
    }
}

pub fn analyze_project_locale_coverage(document: &ProjectDocument) -> LocaleCoverageReport {
    let policy_present = document.project.extensions.contains_key(FLY_LOCALES_FIELD);
    let policy =
        ProjectLocalePolicy::from_document(document).and_then(|policy| policy.normalized().ok());
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
    let metadata = collect_metadata(document);
    let tracked_locales = collect_locales(policy.as_ref(), &catalog, &metadata);
    let mut summaries = Vec::new();
    let mut gaps = Vec::new();

    for locale in &tracked_locales {
        let required = required_locales.contains(locale);
        let translation_present = catalog
            .entries
            .iter()
            .filter(|entry| has_locale(&entry.values, locale))
            .count();
        for entry in catalog
            .entries
            .iter()
            .filter(|entry| !has_locale(&entry.values, locale))
        {
            gaps.push(LocaleCoverageGap {
                locale: locale.clone(),
                kind: LocaleCoverageKind::Translation,
                path: format!("project.translations.{}", entry.id),
                label: entry.id.clone(),
                required,
            });
        }

        let metadata_present = metadata
            .iter()
            .filter(|field| has_locale(&field.values, locale))
            .count();
        for field in metadata
            .iter()
            .filter(|field| !has_locale(&field.values, locale))
        {
            gaps.push(LocaleCoverageGap {
                locale: locale.clone(),
                kind: LocaleCoverageKind::PageMetadata,
                path: field.path.clone(),
                label: field.label.clone(),
                required,
            });
        }

        let translation_total = catalog.entries.len();
        let metadata_total = metadata.len();
        summaries.push(LocaleCoverageSummary {
            locale: locale.clone(),
            required,
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
        metadata_total: metadata.len(),
        summaries,
        gaps,
    }
}

#[derive(Debug, Clone)]
struct MetadataField {
    path: String,
    label: String,
    values: Map<String, Value>,
}

fn collect_metadata(document: &ProjectDocument) -> Vec<MetadataField> {
    let mut result = Vec::new();
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
            result.push(MetadataField {
                path: format!("project.pages[{page_index}].{FLY_PAGE_METADATA_FIELD}.{field}"),
                label: format!("{page_label}.{field}"),
                values,
            });
        }
    }
    result
}

fn collect_locales(
    policy: Option<&ProjectLocalePolicy>,
    catalog: &TranslationCatalog,
    metadata: &[MetadataField],
) -> Vec<String> {
    let mut locales = Vec::new();
    if let Some(policy) = policy {
        push_locale(&mut locales, policy.default_locale.as_deref());
        for locale in policy
            .supported_locales
            .iter()
            .chain(&policy.required_locales)
            .chain(&policy.fallback_locales)
        {
            push_locale(&mut locales, Some(locale));
        }
    }
    for locale in catalog
        .entries
        .iter()
        .flat_map(|entry| entry.values.keys())
        .chain(metadata.iter().flat_map(|field| field.values.keys()))
    {
        push_locale(&mut locales, Some(locale));
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

fn has_locale(values: &Map<String, Value>, required: &str) -> bool {
    values
        .keys()
        .any(|locale| normalize_locale_tag(locale).as_deref() == Some(required))
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
    use crate::GrapesJsCodec;
    use serde_json::json;

    fn report(project: Value) -> LocaleCoverageReport {
        let document = GrapesJsCodec::decode_value(project).expect("project document");
        analyze_project_locale_coverage(&document)
    }

    #[test]
    fn coverage_reports_exact_translation_and_metadata_gaps() {
        let report = report(json!({
            "flyLocales": {
                "default_locale": "en",
                "supported_locales": ["en", "ru"],
                "required_locales": ["en", "ru"],
                "enforce_required_locales": true
            },
            "flyTranslations": [
                { "id": "hero", "values": { "en": "Welcome" } },
                { "id": "cta", "values": { "en": "Buy", "ru": "Купить" } }
            ],
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": { "$localized": { "en": "Home", "ru": "Главная" } },
                    "description": { "$localized": { "en": "Welcome" } }
                },
                "component": { "id": "root", "type": "wrapper" }
            }]
        }));
        let ru = report.summary_for("ru-RU").expect("base locale fallback");
        assert_eq!(ru.missing, 2);
        assert!(!report.required_complete());
        assert!(!report.strict_ready());
        assert!(report.gaps.iter().any(|gap| gap.label == "hero"));
        assert!(
            report
                .gaps
                .iter()
                .any(|gap| gap.label == "home.description")
        );
    }

    #[test]
    fn coverage_discovers_optional_locales_without_policy() {
        let report = report(json!({
            "flyTranslations": [
                { "id": "hero", "values": { "en": "Welcome", "de-DE": "Willkommen" } },
                { "id": "cta", "values": { "en": "Buy" } }
            ],
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }]
        }));
        assert_eq!(report.tracked_locales, vec!["en", "de-de"]);
        assert!(report.required_complete());
        assert!(!report.complete());
        assert!(report.gaps.iter().all(|gap| !gap.required));
    }

    #[test]
    fn invalid_policy_prevents_strict_readiness() {
        let report = report(json!({
            "flyLocales": {
                "default_locale": "invalid locale",
                "supported_locales": ["en"]
            },
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }]
        }));
        assert!(report.policy_present);
        assert!(!report.policy_valid);
        assert!(!report.strict_ready());
    }
}
