use crate::{
    FLY_PAGE_METADATA_FIELD, ProjectDocument, RUNTIME_FALLBACK_LOCALES_FIELD, RUNTIME_LOCALE_FIELD,
    ValidationDiagnostic, materialize_runtime_locale_context,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalizedPageMetadataMaterialization {
    pub document: ProjectDocument,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub localized_pages: usize,
    pub resolved_values: usize,
    pub fallback_values: usize,
    pub unresolved_values: usize,
}

pub fn materialize_localized_page_metadata(
    document: &ProjectDocument,
    context: &Value,
) -> LocalizedPageMetadataMaterialization {
    let mut document = document.clone();
    let mut diagnostics = Vec::new();
    let mut localized_pages = 0usize;
    let mut resolved_values = 0usize;
    let mut fallback_values = 0usize;
    let mut unresolved_values = 0usize;

    for (page_index, page) in document.project.pages.iter_mut().enumerate() {
        let Some(metadata) = page.extensions.get(FLY_PAGE_METADATA_FIELD).cloned() else {
            continue;
        };
        let mut probe = Map::new();
        copy_locale_metadata(context, &mut probe);
        probe.insert("metadata".to_string(), metadata.clone());
        let materialized = materialize_runtime_locale_context(&Value::Object(probe));
        resolved_values = resolved_values.saturating_add(materialized.resolved_values);
        fallback_values = fallback_values.saturating_add(materialized.fallback_values);
        unresolved_values = unresolved_values.saturating_add(materialized.unresolved_values);
        for mut diagnostic in materialized.diagnostics {
            diagnostic.path = metadata_diagnostic_path(page_index, &diagnostic.path);
            diagnostics.push(diagnostic);
        }
        let Some(localized_metadata) = materialized.context.get("metadata").cloned() else {
            continue;
        };
        if localized_metadata != metadata {
            localized_pages = localized_pages.saturating_add(1);
            page.extensions
                .insert(FLY_PAGE_METADATA_FIELD.to_string(), localized_metadata);
        }
    }

    LocalizedPageMetadataMaterialization {
        document,
        diagnostics,
        localized_pages,
        resolved_values,
        fallback_values,
        unresolved_values,
    }
}

fn copy_locale_metadata(context: &Value, target: &mut Map<String, Value>) {
    let Some(context) = context.as_object() else {
        return;
    };
    if let Some(locale) = context
        .get(RUNTIME_LOCALE_FIELD)
        .or_else(|| context.get("locale"))
    {
        target.insert(RUNTIME_LOCALE_FIELD.to_string(), locale.clone());
    }
    if let Some(fallback_locales) = context
        .get(RUNTIME_FALLBACK_LOCALES_FIELD)
        .or_else(|| context.get("fallback_locales"))
    {
        target.insert(
            RUNTIME_FALLBACK_LOCALES_FIELD.to_string(),
            fallback_locales.clone(),
        );
    }
}

fn metadata_diagnostic_path(page_index: usize, path: &str) -> String {
    let suffix = path
        .strip_prefix("$.metadata")
        .or_else(|| path.strip_prefix("$"))
        .unwrap_or(path);
    format!("project.pages[{page_index}].{FLY_PAGE_METADATA_FIELD}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GrapesJsCodec, PageMetadata};
    use serde_json::json;

    #[test]
    fn localized_metadata_is_selected_without_mutating_source_document() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": {
                        "$localized": {
                            "en": "Home",
                            "ru": "Главная"
                        }
                    },
                    "description": {
                        "$localized": {
                            "en": "English description",
                            "ru": "Русское описание"
                        },
                        "$fallback": "en"
                    },
                    "providerFuture": { "enabled": true }
                },
                "component": { "id": "root", "type": "wrapper" }
            }]
        }))
        .expect("document");
        let materialized =
            materialize_localized_page_metadata(&document, &json!({ "$locale": "ru-RU" }));
        let metadata = PageMetadata::from_page(&materialized.document.project.pages[0]);
        assert_eq!(metadata.title.as_deref(), Some("Главная"));
        assert_eq!(metadata.description.as_deref(), Some("Русское описание"));
        assert_eq!(
            materialized.document.project.pages[0].extensions[FLY_PAGE_METADATA_FIELD]["providerFuture"]
                ["enabled"],
            true
        );
        assert!(document.project.pages[0].extensions[FLY_PAGE_METADATA_FIELD]["title"].is_object());
        assert_eq!(materialized.localized_pages, 1);
        assert_eq!(materialized.resolved_values, 2);
    }

    #[test]
    fn metadata_uses_context_fallback_chain_and_reports_it() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "flyPageMeta": {
                    "title": {
                        "$localized": {
                            "en": "Home",
                            "ru": "Главная"
                        }
                    }
                },
                "component": { "id": "root", "type": "wrapper" }
            }]
        }))
        .expect("document");
        let materialized = materialize_localized_page_metadata(
            &document,
            &json!({ "$locale": "de", "$fallback_locales": ["en"] }),
        );
        let metadata = PageMetadata::from_page(&materialized.document.project.pages[0]);
        assert_eq!(metadata.title.as_deref(), Some("Home"));
        assert_eq!(materialized.fallback_values, 1);
        assert!(materialized.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "runtime_localized_value_fallback"
                && diagnostic.path == "project.pages[0].flyPageMeta.title"
        }));
    }

    #[test]
    fn unresolved_metadata_wrapper_is_preserved_losslessly() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "flyPageMeta": {
                    "title": {
                        "$localized": {
                            "en": "Home",
                            "ru": "Главная"
                        }
                    }
                },
                "component": { "id": "root", "type": "wrapper" }
            }]
        }))
        .expect("document");
        let materialized =
            materialize_localized_page_metadata(&document, &json!({ "$locale": "de" }));
        assert!(
            materialized.document.project.pages[0].extensions[FLY_PAGE_METADATA_FIELD]["title"]
                .is_object()
        );
        assert_eq!(materialized.unresolved_values, 1);
        assert_eq!(materialized.localized_pages, 0);
    }
}
