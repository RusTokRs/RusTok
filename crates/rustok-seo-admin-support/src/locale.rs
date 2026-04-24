pub(crate) fn normalize_locale_tag(raw: &str) -> Option<String> {
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

pub(crate) fn locale_primary_language(raw: &str) -> Option<String> {
    normalize_locale_tag(raw).and_then(|value| {
        value
            .split_once('-')
            .map(|(language, _)| language.to_string())
            .or(Some(value))
    })
}
