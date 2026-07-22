use crate::{
    FLY_PAGE_METADATA_FIELD, FlyError, FlyResult, LOCALIZED_VALUES_FIELD, ProjectDocument,
    RUNTIME_FALLBACK_LOCALES_FIELD, RUNTIME_LOCALE_FIELD, TranslationCatalog, ValidationDiagnostic,
    ValidationSeverity, normalize_locale_tag,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub const FLY_LOCALES_FIELD: &str = "flyLocales";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ProjectLocalePolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_locale: Option<String>,
    #[serde(default)]
    pub supported_locales: Vec<String>,
    #[serde(default)]
    pub required_locales: Vec<String>,
    #[serde(default)]
    pub fallback_locales: Vec<String>,
    #[serde(default)]
    pub enforce_required_locales: bool,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

impl ProjectLocalePolicy {
    pub fn from_document(document: &ProjectDocument) -> Option<Self> {
        document
            .project
            .extensions
            .get(FLY_LOCALES_FIELD)
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    pub fn normalized(&self) -> Result<Self, String> {
        let default_locale = self
            .default_locale
            .as_deref()
            .map(normalize_required_locale)
            .transpose()?;
        let supported_locales = normalize_locale_list(&self.supported_locales, "supported locale")?;
        let required_locales = normalize_locale_list(&self.required_locales, "required locale")?;
        let fallback_locales = normalize_locale_list(&self.fallback_locales, "fallback locale")?;

        if !supported_locales.is_empty() {
            for (kind, locale) in default_locale
                .iter()
                .map(|locale| ("default", locale))
                .chain(required_locales.iter().map(|locale| ("required", locale)))
                .chain(fallback_locales.iter().map(|locale| ("fallback", locale)))
            {
                if !supported_locales.contains(locale) {
                    return Err(format!(
                        "{kind} locale `{locale}` is not present in supported_locales"
                    ));
                }
            }
        }

        Ok(Self {
            default_locale,
            supported_locales,
            required_locales,
            fallback_locales,
            enforce_required_locales: self.enforce_required_locales,
            extensions: self.extensions.clone(),
        })
    }

    pub fn supports(&self, locale: &str) -> bool {
        let Some(locale) = normalize_locale_tag(locale) else {
            return false;
        };
        self.supported_locales.is_empty() || self.supported_locales.contains(&locale)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalePolicyMaterialization {
    pub context: Value,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub default_locale_applied: bool,
    pub fallback_locales_applied: usize,
    pub unsupported_locale_replaced: bool,
}

pub fn set_project_locale_policy(
    document: &mut ProjectDocument,
    policy: &ProjectLocalePolicy,
) -> FlyResult<()> {
    let policy = policy.normalized().map_err(FlyError::Decode)?;
    let value =
        serde_json::to_value(policy).map_err(|error| FlyError::Encode(error.to_string()))?;
    document
        .project
        .extensions
        .insert(FLY_LOCALES_FIELD.to_string(), value);
    Ok(())
}

pub fn clear_project_locale_policy(document: &mut ProjectDocument) {
    document.project.extensions.remove(FLY_LOCALES_FIELD);
}

pub fn materialize_project_locale_context(
    document: &ProjectDocument,
    input_context: &Value,
) -> LocalePolicyMaterialization {
    let Some(raw_policy) = document.project.extensions.get(FLY_LOCALES_FIELD) else {
        return LocalePolicyMaterialization {
            context: input_context.clone(),
            diagnostics: Vec::new(),
            default_locale_applied: false,
            fallback_locales_applied: 0,
            unsupported_locale_replaced: false,
        };
    };
    let policy = match serde_json::from_value::<ProjectLocalePolicy>(raw_policy.clone())
        .ok()
        .and_then(|policy| policy.normalized().ok())
    {
        Some(policy) => policy,
        None => {
            return LocalePolicyMaterialization {
                context: input_context.clone(),
                diagnostics: vec![locale_policy_diagnostic(
                    ValidationSeverity::Warning,
                    "runtime_locale_policy_invalid",
                    "project locale policy is invalid and was ignored at runtime",
                )],
                default_locale_applied: false,
                fallback_locales_applied: 0,
                unsupported_locale_replaced: false,
            };
        }
    };
    let Some(mut context) = input_context.as_object().cloned() else {
        return LocalePolicyMaterialization {
            context: input_context.clone(),
            diagnostics: vec![locale_policy_diagnostic(
                ValidationSeverity::Warning,
                "runtime_locale_policy_context_not_object",
                "project locale policy requires an object runtime context",
            )],
            default_locale_applied: false,
            fallback_locales_applied: 0,
            unsupported_locale_replaced: false,
        };
    };

    let mut diagnostics = Vec::new();
    let requested_source = context
        .get(RUNTIME_LOCALE_FIELD)
        .or_else(|| context.get("locale"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let requested_locale = requested_source.as_deref().and_then(normalize_locale_tag);
    if let Some(source) = requested_source.as_deref() {
        if requested_locale.is_none() {
            diagnostics.push(locale_policy_diagnostic(
                ValidationSeverity::Warning,
                "runtime_locale_invalid",
                format!("runtime locale `{source}` is invalid and was replaced by project policy"),
            ));
        }
    }
    let mut active_locale = requested_locale;
    let mut default_locale_applied = false;
    let mut unsupported_locale_replaced = false;

    if let Some(locale) = active_locale.as_deref() {
        if !policy.supports(locale) {
            unsupported_locale_replaced = true;
            diagnostics.push(locale_policy_diagnostic(
                ValidationSeverity::Warning,
                "runtime_locale_unsupported",
                format!("runtime locale `{locale}` is not supported by the project locale policy"),
            ));
            active_locale = policy.default_locale.clone();
            default_locale_applied = active_locale.is_some();
        }
    } else if policy.default_locale.is_some() {
        active_locale = policy.default_locale.clone();
        default_locale_applied = true;
    }

    context.remove("locale");
    match active_locale.as_deref() {
        Some(locale) => {
            context.insert(
                RUNTIME_LOCALE_FIELD.to_string(),
                Value::String(locale.to_string()),
            );
        }
        None => {
            context.remove(RUNTIME_LOCALE_FIELD);
        }
    }

    let fallback_source = context
        .get(RUNTIME_FALLBACK_LOCALES_FIELD)
        .or_else(|| context.get("fallback_locales"))
        .cloned();
    let mut fallback_locales = fallback_source
        .as_ref()
        .map(runtime_locale_list)
        .unwrap_or_default();
    fallback_locales.extend(policy.fallback_locales.iter().cloned());
    if let Some(default_locale) = policy.default_locale.as_deref() {
        if active_locale.as_deref() != Some(default_locale) {
            fallback_locales.push(default_locale.to_string());
        }
    }
    let mut seen = BTreeSet::new();
    fallback_locales.retain(|locale| {
        policy.supports(locale)
            && active_locale.as_deref() != Some(locale.as_str())
            && seen.insert(locale.clone())
    });
    let fallback_locales_applied = fallback_locales.len();
    context.remove("fallback_locales");
    if fallback_locales.is_empty() {
        context.remove(RUNTIME_FALLBACK_LOCALES_FIELD);
    } else {
        context.insert(
            RUNTIME_FALLBACK_LOCALES_FIELD.to_string(),
            Value::Array(fallback_locales.into_iter().map(Value::String).collect()),
        );
    }

    LocalePolicyMaterialization {
        context: Value::Object(context),
        diagnostics,
        default_locale_applied,
        fallback_locales_applied,
        unsupported_locale_replaced,
    }
}

pub fn validate_project_locale_policy(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    let Some(raw_policy) = document.project.extensions.get(FLY_LOCALES_FIELD) else {
        return Vec::new();
    };
    let decoded = match serde_json::from_value::<ProjectLocalePolicy>(raw_policy.clone()) {
        Ok(policy) => policy,
        Err(error) => {
            return vec![locale_policy_diagnostic(
                ValidationSeverity::Error,
                "locale_policy_invalid",
                format!("project locale policy cannot be decoded: {error}"),
            )];
        }
    };
    let policy = match decoded.normalized() {
        Ok(policy) => policy,
        Err(error) => {
            return vec![locale_policy_diagnostic(
                ValidationSeverity::Error,
                "locale_policy_invalid",
                error,
            )];
        }
    };

    let mut diagnostics = Vec::new();
    if policy.supported_locales.is_empty() {
        diagnostics.push(locale_policy_diagnostic(
            ValidationSeverity::Info,
            "locale_policy_supported_locales_open",
            "supported_locales is empty, so the project accepts any valid runtime locale",
        ));
    }
    if policy.enforce_required_locales && policy.required_locales.is_empty() {
        diagnostics.push(locale_policy_diagnostic(
            ValidationSeverity::Warning,
            "locale_policy_required_locales_empty",
            "required locale enforcement is enabled, but required_locales is empty",
        ));
    }

    let missing_severity = if policy.enforce_required_locales {
        ValidationSeverity::Error
    } else {
        ValidationSeverity::Warning
    };
    let catalog = TranslationCatalog::from_document(document);
    for entry in &catalog.entries {
        for locale in &policy.required_locales {
            if !map_contains_locale(&entry.values, locale) {
                diagnostics.push(locale_coverage_diagnostic(
                    missing_severity,
                    "translation_required_locale_missing",
                    format!("project.translations.{}", entry.id),
                    format!(
                        "translation `{}` has no value for required locale `{locale}`",
                        entry.id
                    ),
                ));
            }
        }
    }

    for (page_index, page) in document.project.pages.iter().enumerate() {
        let Some(metadata) = page
            .extensions
            .get(FLY_PAGE_METADATA_FIELD)
            .and_then(Value::as_object)
        else {
            continue;
        };
        for field in LOCALIZED_METADATA_FIELDS {
            let Some(wrapper) = metadata.get(*field).and_then(Value::as_object) else {
                continue;
            };
            let Some(values) = wrapper
                .get(LOCALIZED_VALUES_FIELD)
                .and_then(Value::as_object)
            else {
                continue;
            };
            for locale in values.keys() {
                if normalize_locale_tag(locale).is_none() {
                    diagnostics.push(locale_coverage_diagnostic(
                        ValidationSeverity::Error,
                        "localized_metadata_locale_invalid",
                        format!("project.pages[{page_index}].{FLY_PAGE_METADATA_FIELD}.{field}"),
                        format!("localized metadata locale `{locale}` is invalid"),
                    ));
                }
            }
            for locale in &policy.required_locales {
                if !map_contains_locale(values, locale) {
                    diagnostics.push(locale_coverage_diagnostic(
                        missing_severity,
                        "localized_metadata_required_locale_missing",
                        format!("project.pages[{page_index}].{FLY_PAGE_METADATA_FIELD}.{field}"),
                        format!(
                            "localized page metadata field `{field}` has no value for required locale `{locale}`"
                        ),
                    ));
                }
            }
        }
    }

    diagnostics
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

fn normalize_required_locale(locale: &str) -> Result<String, String> {
    normalize_locale_tag(locale).ok_or_else(|| format!("locale `{locale}` is invalid"))
}

fn normalize_locale_list(locales: &[String], label: &str) -> Result<Vec<String>, String> {
    let mut normalized = Vec::new();
    for locale in locales {
        let locale =
            normalize_locale_tag(locale).ok_or_else(|| format!("{label} `{locale}` is invalid"))?;
        if !normalized.contains(&locale) {
            normalized.push(locale);
        }
    }
    Ok(normalized)
}

fn runtime_locale_list(value: &Value) -> Vec<String> {
    match value {
        Value::String(locale) => normalize_locale_tag(locale).into_iter().collect(),
        Value::Array(locales) => locales
            .iter()
            .filter_map(Value::as_str)
            .filter_map(normalize_locale_tag)
            .collect(),
        _ => Vec::new(),
    }
}

fn map_contains_locale(values: &Map<String, Value>, required_locale: &str) -> bool {
    values
        .keys()
        .any(|locale| normalize_locale_tag(locale).as_deref() == Some(required_locale))
}

fn locale_policy_diagnostic(
    severity: ValidationSeverity,
    code: impl Into<String>,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
        code: code.into(),
        path: "project.locales".to_string(),
        message: message.into(),
    }
}

fn locale_coverage_diagnostic(
    severity: ValidationSeverity,
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
        code: code.into(),
        path: path.into(),
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
    fn policy_commands_normalize_and_preserve_extensions() {
        let mut document = document(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }]
        }));
        set_project_locale_policy(
            &mut document,
            &ProjectLocalePolicy {
                default_locale: Some(" RU_ru ".to_string()),
                supported_locales: vec!["ru-RU".to_string(), "EN".to_string()],
                required_locales: vec!["ru_ru".to_string()],
                fallback_locales: vec!["en".to_string()],
                enforce_required_locales: false,
                extensions: Map::from_iter([("providerFuture".to_string(), json!(true))]),
            },
        )
        .expect("set locale policy");
        let policy = ProjectLocalePolicy::from_document(&document).expect("locale policy");
        assert_eq!(policy.default_locale.as_deref(), Some("ru-ru"));
        assert_eq!(policy.supported_locales, vec!["ru-ru", "en"]);
        assert_eq!(policy.extensions["providerFuture"], true);
        clear_project_locale_policy(&mut document);
        assert!(!document.project.extensions.contains_key(FLY_LOCALES_FIELD));
    }

    #[test]
    fn runtime_policy_defaults_locale_and_merges_fallback_chain() {
        let document = document(json!({
            "flyLocales": {
                "default_locale": "ru",
                "supported_locales": ["ru", "en", "de"],
                "fallback_locales": ["en"]
            },
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }]
        }));
        let defaulted = materialize_project_locale_context(&document, &json!({}));
        assert_eq!(defaulted.context[RUNTIME_LOCALE_FIELD], "ru");
        assert_eq!(
            defaulted.context[RUNTIME_FALLBACK_LOCALES_FIELD],
            json!(["en"])
        );
        assert!(defaulted.default_locale_applied);

        let explicit = materialize_project_locale_context(
            &document,
            &json!({ "$locale": "de", "$fallback_locales": ["ru"] }),
        );
        assert_eq!(explicit.context[RUNTIME_LOCALE_FIELD], "de");
        assert_eq!(
            explicit.context[RUNTIME_FALLBACK_LOCALES_FIELD],
            json!(["ru", "en"])
        );
        assert!(!explicit.default_locale_applied);
    }

    #[test]
    fn current_locale_aliases_are_canonicalized() {
        let document = document(json!({
            "flyLocales": {
                "supported_locales": ["en", "ru"]
            },
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }]
        }));
        let result = materialize_project_locale_context(
            &document,
            &json!({ "locale": "de", "fallback_locales": ["ru"] }),
        );
        assert!(result.context.get("locale").is_none());
        assert!(result.context.get(RUNTIME_LOCALE_FIELD).is_none());
        assert!(result.context.get("fallback_locales").is_none());
        assert_eq!(
            result.context[RUNTIME_FALLBACK_LOCALES_FIELD],
            json!(["ru"])
        );
        assert!(result.unsupported_locale_replaced);
    }

    #[test]
    fn invalid_runtime_locale_is_diagnosed_before_defaulting() {
        let document = document(json!({
            "flyLocales": {
                "default_locale": "en",
                "supported_locales": ["en", "ru"]
            },
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }]
        }));
        let result =
            materialize_project_locale_context(&document, &json!({ "$locale": "invalid locale" }));
        assert_eq!(result.context[RUNTIME_LOCALE_FIELD], "en");
        assert!(result.default_locale_applied);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_locale_invalid")
        );
    }

    #[test]
    fn required_locale_coverage_is_warning_until_enforcement_is_enabled() {
        let mut document = document(json!({
            "flyLocales": {
                "default_locale": "en",
                "supported_locales": ["en", "ru"],
                "required_locales": ["en", "ru"]
            },
            "flyTranslations": [{
                "id": "hero",
                "values": { "en": "Welcome" }
            }],
            "pages": [{
                "flyPageMeta": {
                    "title": { "$localized": { "en": "Home" } }
                },
                "component": { "id": "root", "type": "wrapper" }
            }]
        }));
        let diagnostics = validate_project_locale_policy(&document);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "translation_required_locale_missing"
                && diagnostic.severity == ValidationSeverity::Warning
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "localized_metadata_required_locale_missing"
                && diagnostic.severity == ValidationSeverity::Warning
        }));

        document.project.extensions[FLY_LOCALES_FIELD]["enforce_required_locales"] = json!(true);
        let diagnostics = validate_project_locale_policy(&document);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "translation_required_locale_missing"
                && diagnostic.severity == ValidationSeverity::Error
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "localized_metadata_required_locale_missing"
                && diagnostic.severity == ValidationSeverity::Error
        }));
    }

    #[test]
    fn unsupported_runtime_locale_falls_back_to_project_default() {
        let document = document(json!({
            "flyLocales": {
                "default_locale": "en",
                "supported_locales": ["en", "ru"]
            },
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }]
        }));
        let result = materialize_project_locale_context(&document, &json!({ "$locale": "de" }));
        assert_eq!(result.context[RUNTIME_LOCALE_FIELD], "en");
        assert!(result.unsupported_locale_replaced);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_locale_unsupported")
        );
    }
}
