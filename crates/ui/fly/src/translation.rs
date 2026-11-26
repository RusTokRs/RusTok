use crate::{
    FlyError, FlyResult, LOCALIZED_FALLBACK_FIELD, LOCALIZED_VALUES_FIELD, ProjectDocument,
    ProjectLocalePolicy, RUNTIME_FALLBACK_LOCALES_FIELD, RUNTIME_LOCALE_FIELD,
    ValidationDiagnostic, ValidationSeverity, clear_project_locale_policy,
    materialize_runtime_locale_context, normalize_locale_tag, set_project_locale_policy,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use crate::runtime_locale::{duplicate_locale_key, invalid_locale_key};

pub const FLY_TRANSLATIONS_FIELD: &str = "flyTranslations";
pub const RUNTIME_TRANSLATIONS_CONTEXT_FIELD: &str = "translations";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranslationEntry {
    pub id: String,
    #[serde(default)]
    pub values: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_locale: Option<String>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TranslationCommand {
    Upsert { entry: Box<TranslationEntry> },
    Remove { translation_id: String },
    SetLocalePolicy { policy: Box<ProjectLocalePolicy> },
    ClearLocalePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TranslationCatalog {
    pub entries: Vec<TranslationEntry>,
    pub unknown_entries: Vec<Value>,
}

impl TranslationCatalog {
    pub fn from_document(document: &ProjectDocument) -> Self {
        let mut catalog = Self::default();
        let Some(Value::Array(entries)) = document.project().extensions.get(FLY_TRANSLATIONS_FIELD)
        else {
            return catalog;
        };
        for entry in entries {
            match serde_json::from_value::<TranslationEntry>(entry.clone()) {
                Ok(entry) => catalog.entries.push(entry),
                Err(_) => catalog.unknown_entries.push(entry.clone()),
            }
        }
        catalog
    }

    pub fn get(&self, translation_id: &str) -> Option<&TranslationEntry> {
        self.entries.iter().find(|entry| entry.id == translation_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranslationMaterialization {
    pub context: Value,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub resolved_translations: usize,
    pub fallback_translations: usize,
    pub unresolved_translations: usize,
}

pub fn apply_translation_command(
    document: &mut ProjectDocument,
    command: &TranslationCommand,
) -> FlyResult<()> {
    if matches!(
        command,
        TranslationCommand::Upsert { .. } | TranslationCommand::Remove { .. }
    ) && invalid_catalog_shape(document)
    {
        return Err(FlyError::Decode(format!(
            "{FLY_TRANSLATIONS_FIELD} must be an array"
        )));
    }
    match command {
        TranslationCommand::Upsert { entry } => {
            validate_translation_identity(entry)?;
            let mut catalog = TranslationCatalog::from_document(document);
            if let Some(index) = catalog
                .entries
                .iter()
                .position(|candidate| candidate.id == entry.id)
            {
                catalog.entries[index] = entry.as_ref().clone();
            } else {
                catalog.entries.push(entry.as_ref().clone());
            }
            write_catalog(document, catalog)
        }
        TranslationCommand::Remove { translation_id } => {
            let mut catalog = TranslationCatalog::from_document(document);
            let before = catalog.entries.len();
            catalog
                .entries
                .retain(|entry| entry.id.as_str() != translation_id.as_str());
            if catalog.entries.len() == before {
                return Err(FlyError::Decode(format!(
                    "translation `{translation_id}` was not found"
                )));
            }
            write_catalog(document, catalog)
        }
        TranslationCommand::SetLocalePolicy { policy } => {
            set_project_locale_policy(document, policy)
        }
        TranslationCommand::ClearLocalePolicy => {
            clear_project_locale_policy(document);
            Ok(())
        }
    }
}

pub fn materialize_project_translations(
    document: &ProjectDocument,
    input_context: &Value,
) -> TranslationMaterialization {
    let catalog = TranslationCatalog::from_document(document);
    let Some(mut context) = input_context.as_object().cloned() else {
        return TranslationMaterialization {
            context: input_context.clone(),
            diagnostics: vec![translation_diagnostic(
                ValidationSeverity::Warning,
                "runtime_translation_context_not_object",
                None,
                "project translations require an object runtime context",
            )],
            resolved_translations: 0,
            fallback_translations: 0,
            unresolved_translations: catalog.entries.len(),
        };
    };
    let mut translated = context
        .get(RUNTIME_TRANSLATIONS_CONTEXT_FIELD)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut diagnostics = catalog_shape_diagnostics(document);
    let mut resolved_translations = 0usize;
    let mut fallback_translations = 0usize;
    let mut unresolved_translations = 0usize;

    for entry in &catalog.entries {
        if invalid_locale_key(&entry.values).is_some()
            || duplicate_locale_key(&entry.values).is_some()
        {
            // A caller-provided projection must not mask an invalid authored locale map.
            translated.remove(&entry.id);
        }
        let mut localized_wrapper = Map::new();
        localized_wrapper.insert(
            LOCALIZED_VALUES_FIELD.to_string(),
            Value::Object(entry.values.clone()),
        );
        if let Some(fallback_locale) = entry.fallback_locale.as_deref() {
            localized_wrapper.insert(
                LOCALIZED_FALLBACK_FIELD.to_string(),
                Value::String(fallback_locale.to_string()),
            );
        }
        let mut probe = Map::new();
        copy_locale_metadata(&context, &mut probe);
        probe.insert("value".to_string(), Value::Object(localized_wrapper));
        let materialized = materialize_runtime_locale_context(&Value::Object(probe));
        for mut diagnostic in materialized.diagnostics {
            diagnostic.path = format!("project.translations.{}", entry.id);
            diagnostic.message = format!("translation `{}`: {}", entry.id, diagnostic.message);
            diagnostics.push(diagnostic);
        }
        if materialized.resolved_values > 0
            && let Some(value) = materialized.context.get("value")
        {
            translated.insert(entry.id.clone(), value.clone());
            resolved_translations = resolved_translations.saturating_add(1);
            if materialized.fallback_values > 0 {
                fallback_translations = fallback_translations.saturating_add(1);
            }
            continue;
        }
        unresolved_translations = unresolved_translations.saturating_add(1);
    }

    if !catalog.unknown_entries.is_empty() {
        diagnostics.push(translation_diagnostic(
            ValidationSeverity::Info,
            "opaque_translation_entries",
            None,
            format!(
                "{} translation entries are opaque and preserved",
                catalog.unknown_entries.len()
            ),
        ));
    }
    context.insert(
        RUNTIME_TRANSLATIONS_CONTEXT_FIELD.to_string(),
        Value::Object(translated),
    );
    TranslationMaterialization {
        context: Value::Object(context),
        diagnostics,
        resolved_translations,
        fallback_translations,
        unresolved_translations,
    }
}

pub fn validate_translation_definitions(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    let catalog = TranslationCatalog::from_document(document);
    let mut diagnostics = catalog_shape_diagnostics(document);
    let mut ids = BTreeSet::new();
    for entry in &catalog.entries {
        let id = entry.id.as_str();
        if id.trim().is_empty() {
            diagnostics.push(translation_diagnostic(
                ValidationSeverity::Error,
                "translation_id_empty",
                None,
                "translation id must not be empty",
            ));
        } else if !valid_translation_id(id) {
            diagnostics.push(translation_diagnostic(
                ValidationSeverity::Error,
                "translation_id_invalid",
                Some(id.to_string()),
                format!("translation id `{id}` contains unsupported characters"),
            ));
        } else if !ids.insert(id.to_string()) {
            diagnostics.push(translation_diagnostic(
                ValidationSeverity::Error,
                "duplicate_translation_id",
                Some(id.to_string()),
                format!("translation id `{id}` is duplicated"),
            ));
        }
        if entry.values.is_empty() {
            diagnostics.push(translation_diagnostic(
                ValidationSeverity::Error,
                "translation_values_empty",
                Some(id.to_string()),
                format!("translation `{id}` has no locale values"),
            ));
        }
        for locale in entry.values.keys() {
            if normalize_locale_tag(locale).is_none() {
                diagnostics.push(translation_diagnostic(
                    ValidationSeverity::Error,
                    "translation_locale_invalid",
                    Some(id.to_string()),
                    format!("translation `{id}` locale `{locale}` is invalid"),
                ));
            }
        }
        if let Some(locale) = duplicate_locale_key(&entry.values) {
            diagnostics.push(translation_diagnostic(
                ValidationSeverity::Error,
                "translation_locale_ambiguous",
                Some(id.to_string()),
                format!("translation `{id}` contains multiple values for locale `{locale}`"),
            ));
        }
        if let Some(fallback_locale) = entry.fallback_locale.as_deref() {
            match normalize_locale_tag(fallback_locale) {
                None => diagnostics.push(translation_diagnostic(
                    ValidationSeverity::Error,
                    "translation_fallback_locale_invalid",
                    Some(id.to_string()),
                    format!("translation `{id}` fallback locale `{fallback_locale}` is invalid"),
                )),
                Some(fallback_locale)
                    if !entry.values.keys().any(|locale| {
                        normalize_locale_tag(locale).as_deref() == Some(fallback_locale.as_str())
                    }) =>
                {
                    diagnostics.push(translation_diagnostic(
                        ValidationSeverity::Warning,
                        "translation_fallback_locale_missing",
                        Some(id.to_string()),
                        format!(
                            "translation `{id}` fallback locale `{fallback_locale}` has no value"
                        ),
                    ));
                }
                Some(_) => {}
            }
        }
    }
    if !catalog.unknown_entries.is_empty() {
        diagnostics.push(translation_diagnostic(
            ValidationSeverity::Info,
            "opaque_translation_entries",
            None,
            format!(
                "{} translation entries are opaque and preserved",
                catalog.unknown_entries.len()
            ),
        ));
    }
    diagnostics
}

fn invalid_catalog_shape(document: &ProjectDocument) -> bool {
    document
        .project()
        .extensions
        .get(FLY_TRANSLATIONS_FIELD)
        .is_some_and(|value| !value.is_array())
}

fn catalog_shape_diagnostics(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    if invalid_catalog_shape(document) {
        return vec![translation_diagnostic(
            ValidationSeverity::Error,
            "translation_catalog_invalid",
            None,
            format!("{FLY_TRANSLATIONS_FIELD} must be an array"),
        )];
    }
    Vec::new()
}

fn validate_translation_identity(entry: &TranslationEntry) -> FlyResult<()> {
    let id = entry.id.as_str();
    if id.trim().is_empty() {
        return Err(FlyError::Decode(
            "translation id must not be empty".to_string(),
        ));
    }
    if !valid_translation_id(id) {
        return Err(FlyError::Decode(format!(
            "translation id `{id}` contains unsupported characters"
        )));
    }
    if entry.values.is_empty() {
        return Err(FlyError::Decode(format!(
            "translation `{id}` must contain at least one locale value"
        )));
    }
    for locale in entry.values.keys() {
        if normalize_locale_tag(locale).is_none() {
            return Err(FlyError::Decode(format!(
                "translation `{id}` locale `{locale}` is invalid"
            )));
        }
    }
    if let Some(locale) = duplicate_locale_key(&entry.values) {
        return Err(FlyError::Decode(format!(
            "translation `{id}` contains multiple values for locale `{locale}`"
        )));
    }
    if let Some(fallback_locale) = entry.fallback_locale.as_deref()
        && normalize_locale_tag(fallback_locale).is_none()
    {
        return Err(FlyError::Decode(format!(
            "translation `{id}` fallback locale `{fallback_locale}` is invalid"
        )));
    }
    Ok(())
}

fn valid_translation_id(id: &str) -> bool {
    id.chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn write_catalog(document: &mut ProjectDocument, catalog: TranslationCatalog) -> FlyResult<()> {
    let mut entries = catalog
        .entries
        .into_iter()
        .map(|entry| {
            serde_json::to_value(entry).map_err(|error| FlyError::Encode(error.to_string()))
        })
        .collect::<FlyResult<Vec<_>>>()?;
    entries.extend(catalog.unknown_entries);
    if entries.is_empty() {
        document.project_mut().extensions.remove(FLY_TRANSLATIONS_FIELD);
    } else {
        document
            .project_mut()
            .extensions
            .insert(FLY_TRANSLATIONS_FIELD.to_string(), Value::Array(entries));
    }
    document.invalidate_caches();
    Ok(())
}

fn copy_locale_metadata(source: &Map<String, Value>, target: &mut Map<String, Value>) {
    if let Some(locale) = source
        .get(RUNTIME_LOCALE_FIELD)
        .or_else(|| source.get("locale"))
    {
        target.insert(RUNTIME_LOCALE_FIELD.to_string(), locale.clone());
    }
    if let Some(fallback_locales) = source
        .get(RUNTIME_FALLBACK_LOCALES_FIELD)
        .or_else(|| source.get("fallback_locales"))
    {
        target.insert(
            RUNTIME_FALLBACK_LOCALES_FIELD.to_string(),
            fallback_locales.clone(),
        );
    }
}

fn translation_diagnostic(
    severity: ValidationSeverity,
    code: impl Into<String>,
    translation_id: Option<String>,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
        code: code.into(),
        path: translation_id
            .map(|id| format!("project.translations.{id}"))
            .unwrap_or_else(|| "project.translations".to_string()),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FLY_LOCALES_FIELD, GrapesJsCodec};
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": { "id": "root", "type": "wrapper" }
            }]
        }))
        .expect("document")
    }

    #[test]
    fn commands_preserve_opaque_entries_and_support_removal() {
        let mut document = document();
        document.project_mut().extensions.insert(
            FLY_TRANSLATIONS_FIELD.to_string(),
            json!([{ "future": true }]),
        );
        let before_hash = document.hash();
        apply_translation_command(
            &mut document,
            &TranslationCommand::Upsert {
                entry: Box::new(TranslationEntry {
                    id: "hero_title".to_string(),
                    values: serde_json::from_value(json!({
                        "en": "Hello",
                        "ru": "Привет"
                    }))
                    .unwrap(),
                    fallback_locale: Some("en".to_string()),
                    extensions: Map::new(),
                }),
            },
        )
        .expect("upsert");
        assert_ne!(document.hash(), before_hash);
        let catalog = TranslationCatalog::from_document(&document);
        assert_eq!(catalog.entries.len(), 1);
        assert_eq!(catalog.unknown_entries, vec![json!({ "future": true })]);
        apply_translation_command(
            &mut document,
            &TranslationCommand::Remove {
                translation_id: "hero_title".to_string(),
            },
        )
        .expect("remove");
        assert!(
            TranslationCatalog::from_document(&document)
                .entries
                .is_empty()
        );
    }

    #[test]
    fn translation_ids_are_validated_and_removed_without_lossy_trimming() {
        let mut document = document();
        let entry: TranslationEntry = serde_json::from_value(json!({
            "id": " hero ",
            "values": { "en": "Hello" }
        }))
        .unwrap();
        let before_hash = document.hash();
        assert!(apply_translation_command(
            &mut document,
            &TranslationCommand::Upsert {
                entry: Box::new(entry),
            },
        )
        .is_err());
        assert_eq!(document.hash(), before_hash);

        document.project_mut().extensions.insert(
            FLY_TRANSLATIONS_FIELD.to_string(),
            json!([
                { "id": "hero", "values": { "en": "Canonical" } },
                { "id": " hero ", "values": { "en": "Imported" } }
            ]),
        );
        assert!(validate_translation_definitions(&document).iter().any(|diagnostic| {
            diagnostic.code == "translation_id_invalid"
        }));
        apply_translation_command(
            &mut document,
            &TranslationCommand::Remove {
                translation_id: " hero ".to_string(),
            },
        )
        .expect("remove exact imported identity");
        let catalog = TranslationCatalog::from_document(&document);
        assert_eq!(catalog.entries.len(), 1);
        assert_eq!(catalog.entries[0].id, "hero");
        assert!(validate_translation_definitions(&document).is_empty());
    }

    #[test]
    fn malformed_catalog_is_diagnosed_and_never_overwritten_by_commands() {
        let mut document = document();
        let malformed = json!({ "future": { "payload": true } });
        document.project_mut().extensions.insert(
            FLY_TRANSLATIONS_FIELD.to_string(),
            malformed.clone(),
        );
        let before_hash = document.hash();
        let entry: TranslationEntry = serde_json::from_value(json!({
            "id": "hero",
            "values": { "en": "Hello" }
        }))
        .unwrap();
        for command in [
            TranslationCommand::Upsert {
                entry: Box::new(entry),
            },
            TranslationCommand::Remove {
                translation_id: "hero".to_string(),
            },
        ] {
            assert!(apply_translation_command(&mut document, &command).is_err());
            assert_eq!(document.hash(), before_hash);
            assert_eq!(
                document.project().extensions[FLY_TRANSLATIONS_FIELD],
                malformed
            );
        }
        let diagnostics = validate_translation_definitions(&document);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "translation_catalog_invalid"
                && diagnostic.severity == ValidationSeverity::Error
        }));
        let materialized = materialize_project_translations(&document, &json!({}));
        assert!(materialized.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "translation_catalog_invalid"
                && diagnostic.severity == ValidationSeverity::Error
        }));
    }

    #[test]
    fn locale_policy_commands_share_translation_transaction_surface() {
        let mut document = document();
        apply_translation_command(
            &mut document,
            &TranslationCommand::SetLocalePolicy {
                policy: Box::new(ProjectLocalePolicy {
                    default_locale: Some("RU_ru".to_string()),
                    supported_locales: vec!["ru-RU".to_string(), "en".to_string()],
                    required_locales: vec!["ru-RU".to_string()],
                    fallback_locales: vec!["en".to_string()],
                    enforce_required_locales: false,
                    extensions: Map::from_iter([("future".to_string(), json!(true))]),
                }),
            },
        )
        .expect("set locale policy");
        assert_eq!(
            document.project().extensions[FLY_LOCALES_FIELD]["default_locale"],
            "ru-RU"
        );
        assert_eq!(
            document.project().extensions[FLY_LOCALES_FIELD]["future"],
            true
        );
        apply_translation_command(&mut document, &TranslationCommand::ClearLocalePolicy)
            .expect("clear locale policy");
        assert!(!document.project().extensions.contains_key(FLY_LOCALES_FIELD));
    }

    #[test]
    fn catalog_materializes_into_binding_context() {
        let mut document = document();
        apply_translation_command(
            &mut document,
            &TranslationCommand::Upsert {
                entry: Box::new(TranslationEntry {
                    id: "hero_title".to_string(),
                    values: serde_json::from_value(json!({
                        "en": "Hello",
                        "ru": "Привет"
                    }))
                    .unwrap(),
                    fallback_locale: Some("en".to_string()),
                    extensions: Map::new(),
                }),
            },
        )
        .expect("upsert");
        let materialized = materialize_project_translations(
            &document,
            &json!({ "$locale": "ru-RU", "customer": { "name": "Ada" } }),
        );
        assert_eq!(materialized.context["translations"]["hero_title"], "Привет");
        assert_eq!(materialized.context["customer"]["name"], "Ada");
        assert_eq!(materialized.resolved_translations, 1);
        assert_eq!(materialized.fallback_translations, 1);
    }

    #[test]
    fn canonical_locale_collisions_block_authoring_and_runtime_projection() {
        let mut document = document();
        let raw = json!({
            "id": "hero",
            "values": { "ru-RU": "First", "ru_RU": "Second" }
        });
        let entry: TranslationEntry = serde_json::from_value(raw.clone()).unwrap();
        let before = document.hash();
        assert!(apply_translation_command(
            &mut document,
            &TranslationCommand::Upsert { entry: Box::new(entry) },
        ).is_err());
        assert_eq!(document.hash(), before);
        document.project_mut().extensions.insert(
            FLY_TRANSLATIONS_FIELD.to_string(),
            json!([raw]),
        );
        assert!(validate_translation_definitions(&document).iter().any(|diagnostic| {
            diagnostic.code == "translation_locale_ambiguous"
        }));
        let result = materialize_project_translations(
            &document,
            &json!({ "$locale": "ru-RU", "translations": { "hero": "Stale", "other": "Keep" } }),
        );
        assert_eq!(result.unresolved_translations, 1);
        assert_eq!(result.resolved_translations, 0);
        assert!(result.context["translations"].get("hero").is_none());
        assert_eq!(result.context["translations"]["other"], "Keep");
    }

    #[test]
    fn invalid_catalog_locale_cannot_resolve_or_retain_stale_projection() {
        let mut document = document();
        for values in [
            json!({ "invalid locale": "Invalid" }),
            json!({ "en": "Valid", "invalid locale": "Invalid" }),
        ] {
            document.project_mut().extensions.insert(
                FLY_TRANSLATIONS_FIELD.to_string(),
                json!([{ "id": "hero", "values": values }]),
            );
            let result = materialize_project_translations(
                &document,
                &json!({ "$locale": "en", "translations": { "hero": "Stale", "other": "Keep" } }),
            );
            assert_eq!(result.resolved_translations, 0);
            assert_eq!(result.unresolved_translations, 1);
            assert!(result.context["translations"].get("hero").is_none());
            assert_eq!(result.context["translations"]["other"], "Keep");
            assert!(result.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "runtime_localized_value_locale_invalid"
                    && diagnostic.severity == ValidationSeverity::Error
            }));
        }
    }

    #[test]
    fn validation_reports_duplicate_and_invalid_locale_definitions() {
        let mut document = document();
        document.project_mut().extensions.insert(
            FLY_TRANSLATIONS_FIELD.to_string(),
            json!([{
                "id": "hero",
                "values": { "invalid locale": "Hello" }
            }, {
                "id": "hero",
                "values": { "en": "Hello" },
                "fallback_locale": "ru"
            }]),
        );
        let diagnostics = validate_translation_definitions(&document);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "translation_locale_invalid")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "duplicate_translation_id")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "translation_fallback_locale_missing")
        );
    }
}
