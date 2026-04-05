pub const PLATFORM_FALLBACK_LOCALE: &str = "en";

pub fn normalize_locale_tag(raw: &str) -> Option<String> {
    let candidate = raw.trim().replace('_', "-");
    if candidate.is_empty() || candidate.len() > 32 {
        return None;
    }

    if !candidate
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return None;
    }

    let parts = candidate.split('-').collect::<Vec<_>>();
    let language = parts.first()?.trim();
    if language.len() < 2
        || language.len() > 8
        || !language.chars().all(|ch| ch.is_ascii_alphabetic())
    {
        return None;
    }

    let mut normalized = Vec::with_capacity(parts.len());
    normalized.push(language.to_ascii_lowercase());

    for part in parts.into_iter().skip(1) {
        if part.is_empty() || part.len() > 8 {
            return None;
        }

        let normalized_part = if part.len() == 2 && part.chars().all(|ch| ch.is_ascii_alphabetic())
        {
            part.to_ascii_uppercase()
        } else if part.len() == 4 && part.chars().all(|ch| ch.is_ascii_alphabetic()) {
            let mut chars = part.chars();
            let head = chars
                .next()
                .map(|ch| ch.to_ascii_uppercase().to_string())
                .unwrap_or_default();
            let tail = chars.as_str().to_ascii_lowercase();
            format!("{head}{tail}")
        } else if part.len() == 3 && part.chars().all(|ch| ch.is_ascii_digit()) {
            part.to_string()
        } else if (5..=8).contains(&part.len()) && part.chars().all(|ch| ch.is_ascii_alphanumeric())
        {
            part.to_ascii_lowercase()
        } else {
            return None;
        };

        normalized.push(normalized_part);
    }

    Some(normalized.join("-"))
}

pub fn is_valid_locale_tag(raw: &str) -> bool {
    normalize_locale_tag(raw).is_some()
}

pub fn locale_primary_language(raw: &str) -> Option<String> {
    normalize_locale_tag(raw).and_then(|value| {
        value
            .split_once('-')
            .map(|(language, _)| language.to_string())
            .or(Some(value))
    })
}

pub fn locale_tags_match(left: &str, right: &str) -> bool {
    normalize_locale_tag(left) == normalize_locale_tag(right)
}

pub fn push_locale_candidate(
    candidates: &mut Vec<String>,
    locale: Option<&str>,
    include_language_fallback: bool,
) {
    let Some(normalized) = locale.and_then(normalize_locale_tag) else {
        return;
    };

    if !candidates.iter().any(|candidate| candidate == &normalized) {
        candidates.push(normalized.clone());
    }

    if include_language_fallback {
        if let Some(language) = locale_primary_language(normalized.as_str()) {
            if language != normalized && !candidates.iter().any(|candidate| candidate == &language)
            {
                candidates.push(language);
            }
        }
    }
}

pub fn build_locale_candidates<'a>(
    locales: impl IntoIterator<Item = Option<&'a str>>,
    include_language_fallback: bool,
) -> Vec<String> {
    let mut candidates = Vec::new();
    for locale in locales {
        push_locale_candidate(&mut candidates, locale, include_language_fallback);
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::{
        build_locale_candidates, is_valid_locale_tag, locale_primary_language, locale_tags_match,
        normalize_locale_tag,
    };

    #[test]
    fn normalize_locale_tag_canonicalizes_common_bcp47_forms() {
        assert_eq!(normalize_locale_tag("ru"), Some("ru".to_string()));
        assert_eq!(normalize_locale_tag("ru_ru"), Some("ru-RU".to_string()));
        assert_eq!(normalize_locale_tag("pt_br"), Some("pt-BR".to_string()));
        assert_eq!(normalize_locale_tag("zh-hant"), Some("zh-Hant".to_string()));
        assert_eq!(normalize_locale_tag("es-419"), Some("es-419".to_string()));
    }

    #[test]
    fn normalize_locale_tag_rejects_invalid_values() {
        assert_eq!(normalize_locale_tag(""), None);
        assert_eq!(normalize_locale_tag("e"), None);
        assert_eq!(normalize_locale_tag("en-*"), None);
        assert_eq!(normalize_locale_tag("12"), None);
    }

    #[test]
    fn locale_candidates_preserve_priority_and_add_language_fallback() {
        let candidates = build_locale_candidates([Some("pt-BR"), Some("en"), Some("pt_br")], true);

        assert_eq!(candidates, vec!["pt-BR", "pt", "en"]);
    }

    #[test]
    fn locale_primary_language_uses_normalized_language() {
        assert_eq!(locale_primary_language("zh-hant").as_deref(), Some("zh"));
    }

    #[test]
    fn locale_tags_match_compares_canonical_forms() {
        assert!(locale_tags_match("pt_br", "pt-BR"));
        assert!(is_valid_locale_tag("zh-Hant"));
    }
}
