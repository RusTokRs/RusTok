use crate::{ValidationDiagnostic, ValidationSeverity};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const RUNTIME_LOCALE_FIELD: &str = "$locale";
pub const RUNTIME_FALLBACK_LOCALES_FIELD: &str = "$fallback_locales";
pub const LOCALIZED_VALUES_FIELD: &str = "$localized";
pub const LOCALIZED_FALLBACK_FIELD: &str = "$fallback";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeLocaleSelection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(default)]
    pub fallback_locales: Vec<String>,
}

impl RuntimeLocaleSelection {
    pub fn from_context(context: &Value) -> Self {
        let locale = context
            .get(RUNTIME_LOCALE_FIELD)
            .or_else(|| context.get("locale"))
            .and_then(Value::as_str)
            .and_then(normalize_locale_tag);
        let fallback_locales = context
            .get(RUNTIME_FALLBACK_LOCALES_FIELD)
            .or_else(|| context.get("fallback_locales"))
            .map(locale_list)
            .unwrap_or_default();
        Self {
            locale,
            fallback_locales,
        }
    }

    fn candidates(&self) -> Vec<String> {
        let mut candidates = Vec::new();
        if let Some(locale) = self.locale.as_deref() {
            push_locale_candidate(&mut candidates, locale);
        }
        for locale in &self.fallback_locales {
            push_locale_candidate(&mut candidates, locale);
        }
        candidates
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeLocaleMaterialization {
    pub context: Value,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub resolved_values: usize,
    pub fallback_values: usize,
    pub unresolved_values: usize,
}

pub fn materialize_runtime_locale_context(context: &Value) -> RuntimeLocaleMaterialization {
    let selection = RuntimeLocaleSelection::from_context(context);
    let mut state = LocaleMaterializationState::default();
    let context = materialize_value(context, &selection, "$", &mut state);
    RuntimeLocaleMaterialization {
        context,
        diagnostics: state.diagnostics,
        resolved_values: state.resolved_values,
        fallback_values: state.fallback_values,
        unresolved_values: state.unresolved_values,
    }
}

#[derive(Default)]
struct LocaleMaterializationState {
    diagnostics: Vec<ValidationDiagnostic>,
    resolved_values: usize,
    fallback_values: usize,
    unresolved_values: usize,
}

fn materialize_value(
    value: &Value,
    selection: &RuntimeLocaleSelection,
    path: &str,
    state: &mut LocaleMaterializationState,
) -> Value {
    if let Value::Object(object) = value {
        if let Some(localized) = object.get(LOCALIZED_VALUES_FIELD) {
            let Some(translations) = localized.as_object() else {
                state.unresolved_values = state.unresolved_values.saturating_add(1);
                state.diagnostics.push(locale_diagnostic(
                    ValidationSeverity::Warning,
                    "runtime_localized_value_invalid",
                    path,
                    format!("`{LOCALIZED_VALUES_FIELD}` must be an object keyed by locale"),
                ));
                return materialize_object(object, selection, path, state);
            };
            if let Some((selected, used_fallback, selected_locale)) =
                select_translation(object, translations, selection)
            {
                state.resolved_values = state.resolved_values.saturating_add(1);
                if used_fallback {
                    state.fallback_values = state.fallback_values.saturating_add(1);
                    state.diagnostics.push(locale_diagnostic(
                        ValidationSeverity::Info,
                        "runtime_localized_value_fallback",
                        path,
                        format!(
                            "localized value resolved through fallback locale `{selected_locale}`"
                        ),
                    ));
                }
                return materialize_value(selected, selection, path, state);
            }
            state.unresolved_values = state.unresolved_values.saturating_add(1);
            let requested = selection.locale.as_deref().unwrap_or("<unset>");
            state.diagnostics.push(locale_diagnostic(
                ValidationSeverity::Info,
                "runtime_localized_value_unresolved",
                path,
                format!(
                    "localized value has no translation for locale `{requested}` or its fallbacks"
                ),
            ));
            return value.clone();
        }
        return materialize_object(object, selection, path, state);
    }
    if let Value::Array(values) = value {
        return Value::Array(
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    materialize_value(value, selection, &format!("{path}[{index}]"), state)
                })
                .collect(),
        );
    }
    value.clone()
}

fn materialize_object(
    object: &Map<String, Value>,
    selection: &RuntimeLocaleSelection,
    path: &str,
    state: &mut LocaleMaterializationState,
) -> Value {
    Value::Object(
        object
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    materialize_value(value, selection, &format!("{path}.{key}"), state),
                )
            })
            .collect(),
    )
}

fn select_translation<'a>(
    wrapper: &'a Map<String, Value>,
    translations: &'a Map<String, Value>,
    selection: &RuntimeLocaleSelection,
) -> Option<(&'a Value, bool, String)> {
    let requested_candidates = selection
        .locale
        .as_deref()
        .map(|locale| {
            let mut candidates = Vec::new();
            push_locale_candidate(&mut candidates, locale);
            candidates
        })
        .unwrap_or_default();
    for candidate in &requested_candidates {
        if let Some((key, value)) = translation_for(translations, candidate) {
            let used_fallback = match selection.locale.as_deref() {
                Some(requested) => candidate.as_str() != requested,
                None => true,
            };
            return Some((value, used_fallback, key));
        }
    }

    for candidate in selection
        .candidates()
        .into_iter()
        .filter(|candidate| !requested_candidates.contains(candidate))
    {
        if let Some((key, value)) = translation_for(translations, &candidate) {
            return Some((value, true, key));
        }
    }

    for candidate in wrapper
        .get(LOCALIZED_FALLBACK_FIELD)
        .map(locale_list)
        .unwrap_or_default()
    {
        let mut candidates = Vec::new();
        push_locale_candidate(&mut candidates, &candidate);
        for candidate in candidates {
            if let Some((key, value)) = translation_for(translations, &candidate) {
                return Some((value, true, key));
            }
        }
    }

    if translations.len() == 1 {
        return translations
            .iter()
            .next()
            .map(|(locale, value)| (value, true, locale.clone()));
    }
    None
}

fn translation_for<'a>(
    translations: &'a Map<String, Value>,
    candidate: &str,
) -> Option<(String, &'a Value)> {
    let normalized_candidate = normalize_locale_tag(candidate)?;
    for (locale, value) in translations {
        if normalize_locale_tag(locale).as_deref() == Some(normalized_candidate.as_str()) {
            return Some((locale.clone(), value));
        }
    }
    None
}

fn locale_list(value: &Value) -> Vec<String> {
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

pub fn normalize_locale_tag(locale: &str) -> Option<String> {
    let locale = locale.trim().replace('_', "-").to_ascii_lowercase();
    if locale.is_empty()
        || locale.starts_with('-')
        || locale.ends_with('-')
        || locale.split('-').any(str::is_empty)
        || !locale
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return None;
    }
    Some(locale)
}

fn locale_diagnostic(
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
    use serde_json::json;

    #[test]
    fn exact_locale_and_nested_values_are_materialized() {
        let result = materialize_runtime_locale_context(&json!({
            "$locale": "ru-RU",
            "page": {
                "title": {
                    "$localized": {
                        "en": "Hello",
                        "ru-RU": "Привет"
                    }
                }
            }
        }));
        assert_eq!(result.context["page"]["title"], "Привет");
        assert_eq!(result.resolved_values, 1);
        assert_eq!(result.fallback_values, 0);
    }

    #[test]
    fn regional_locale_falls_back_to_language() {
        let result = materialize_runtime_locale_context(&json!({
            "$locale": "ru-RU",
            "page": {
                "title": {
                    "$localized": {
                        "en": "Hello",
                        "ru": "Привет"
                    }
                }
            }
        }));
        assert_eq!(result.context["page"]["title"], "Привет");
        assert_eq!(result.fallback_values, 1);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_localized_value_fallback")
        );
    }

    #[test]
    fn context_and_value_fallback_chains_are_supported() {
        let context_fallback = materialize_runtime_locale_context(&json!({
            "$locale": "fr",
            "$fallback_locales": ["en"],
            "label": {
                "$localized": {
                    "en": "Fallback",
                    "ru": "Резерв"
                }
            }
        }));
        assert_eq!(context_fallback.context["label"], "Fallback");

        let value_fallback = materialize_runtime_locale_context(&json!({
            "$locale": "fr",
            "label": {
                "$localized": {
                    "en": "Fallback",
                    "ru": "Резерв"
                },
                "$fallback": "ru"
            }
        }));
        assert_eq!(value_fallback.context["label"], "Резерв");
    }

    #[test]
    fn unresolved_localized_value_is_preserved_losslessly() {
        let input = json!({
            "$locale": "de",
            "label": {
                "$localized": {
                    "en": "Hello",
                    "ru": "Привет"
                }
            }
        });
        let result = materialize_runtime_locale_context(&input);
        assert_eq!(result.context["label"], input["label"]);
        assert_eq!(result.unresolved_values, 1);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_localized_value_unresolved")
        );
    }

    #[test]
    fn locale_tags_are_case_separator_and_subtag_sensitive() {
        assert_eq!(normalize_locale_tag(" RU_ru ").as_deref(), Some("ru-ru"));
        assert_eq!(normalize_locale_tag("invalid locale"), None);
        assert_eq!(normalize_locale_tag("ru--RU"), None);
    }
}
