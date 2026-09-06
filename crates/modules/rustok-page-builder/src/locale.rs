use fly::{RUNTIME_FALLBACK_LOCALES_FIELD, RUNTIME_LOCALE_FIELD, normalize_locale_tag};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::cmp::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PageBuilderLocaleContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(default)]
    pub fallback_locales: Vec<String>,
}

impl PageBuilderLocaleContext {
    pub fn new<I, S>(locale: Option<&str>, fallback_locales: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let locale = locale.and_then(normalize_locale_tag);
        let mut normalized_fallbacks = Vec::new();
        for fallback in fallback_locales {
            let Some(fallback) = normalize_locale_tag(fallback.as_ref()) else {
                continue;
            };
            if locale.as_deref() == Some(fallback.as_str())
                || normalized_fallbacks.contains(&fallback)
            {
                continue;
            }
            normalized_fallbacks.push(fallback);
        }
        Self {
            locale,
            fallback_locales: normalized_fallbacks,
        }
    }

    pub fn from_request(
        route_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
        accept_language: Option<&str>,
        configured_fallbacks: &[String],
    ) -> Self {
        let accepted = accept_language
            .map(parse_accept_language)
            .unwrap_or_default();
        let locale = route_locale
            .and_then(normalize_locale_tag)
            .or_else(|| accepted.first().cloned())
            .or_else(|| tenant_default_locale.and_then(normalize_locale_tag));
        let fallback_locales = accepted
            .into_iter()
            .skip_while(|candidate| locale.as_deref() == Some(candidate.as_str()))
            .chain(tenant_default_locale.and_then(normalize_locale_tag))
            .chain(
                configured_fallbacks
                    .iter()
                    .filter_map(|value| normalize_locale_tag(value)),
            )
            .collect::<Vec<_>>();
        Self::new(locale.as_deref(), fallback_locales)
    }

    pub fn apply_to_context(&self, context: &Value) -> Value {
        let mut context = context.as_object().cloned().unwrap_or_default();
        match self.locale.as_deref() {
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
        if self.fallback_locales.is_empty() {
            context.remove(RUNTIME_FALLBACK_LOCALES_FIELD);
        } else {
            context.insert(
                RUNTIME_FALLBACK_LOCALES_FIELD.to_string(),
                Value::Array(
                    self.fallback_locales
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            );
        }
        Value::Object(context)
    }

    pub fn as_context(&self) -> Value {
        self.apply_to_context(&Value::Object(Map::new()))
    }
}

pub fn parse_accept_language(header: &str) -> Vec<String> {
    let mut candidates = header
        .split(',')
        .enumerate()
        .filter_map(|(index, part)| parse_language_range(part, index))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .quality
            .partial_cmp(&left.quality)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.index.cmp(&right.index))
    });
    let mut locales = Vec::new();
    for candidate in candidates {
        if !locales.contains(&candidate.locale) {
            locales.push(candidate.locale);
        }
    }
    locales
}

#[derive(Debug)]
struct LanguageCandidate {
    locale: String,
    quality: f32,
    index: usize,
}

fn parse_language_range(part: &str, index: usize) -> Option<LanguageCandidate> {
    let mut segments = part.trim().split(';');
    let locale = segments.next()?.trim();
    if locale == "*" {
        return None;
    }
    let locale = normalize_locale_tag(locale)?;
    let mut quality = 1.0f32;
    for parameter in segments {
        let Some((name, value)) = parameter.trim().split_once('=') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("q") {
            quality = value.trim().parse::<f32>().ok()?.clamp(0.0, 1.0);
        }
    }
    (quality > 0.0).then_some(LanguageCandidate {
        locale,
        quality,
        index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accept_language_is_sorted_by_quality_and_stable_order() {
        assert_eq!(
            parse_accept_language("en-US;q=0.7, ru-RU, de;q=0.7, *;q=0.9, fr;q=0"),
            vec!["ru-ru", "en-us", "de"]
        );
    }

    #[test]
    fn request_locale_prefers_route_then_header_then_tenant_default() {
        let configured = vec!["en".to_string(), "de-DE".to_string()];
        let route = PageBuilderLocaleContext::from_request(
            Some("ru_RU"),
            Some("en"),
            Some("fr, de;q=0.8"),
            &configured,
        );
        assert_eq!(route.locale.as_deref(), Some("ru-ru"));
        assert_eq!(route.fallback_locales, vec!["fr", "de", "en", "de-de"]);

        let header = PageBuilderLocaleContext::from_request(
            None,
            Some("en"),
            Some("fr, de;q=0.8"),
            &configured,
        );
        assert_eq!(header.locale.as_deref(), Some("fr"));
        assert_eq!(header.fallback_locales, vec!["de", "en", "de-de"]);
    }

    #[test]
    fn applying_locale_preserves_business_context() {
        let locale = PageBuilderLocaleContext::new(Some("ru-RU"), ["ru", "en"]);
        let context = locale.apply_to_context(&json!({
            "customer": { "name": "Ada" },
            "$locale": "de"
        }));
        assert_eq!(context["$locale"], "ru-ru");
        assert_eq!(context["$fallback_locales"], json!(["ru", "en"]));
        assert_eq!(context["customer"]["name"], "Ada");
    }

    #[test]
    fn invalid_and_duplicate_fallbacks_are_removed() {
        let locale =
            PageBuilderLocaleContext::new(Some("ru"), ["ru", "invalid locale", "en", "EN"]);
        assert_eq!(locale.fallback_locales, vec!["en"]);
    }
}
