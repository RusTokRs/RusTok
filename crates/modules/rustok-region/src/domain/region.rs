use std::collections::HashSet;

use crate::error::{RegionError, RegionResult};

pub(crate) fn normalize_currency_code(value: &str) -> RegionResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() == 3 && normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Ok(normalized)
    } else {
        Err(RegionError::Validation(
            "currency_code must be a 3-letter alphabetic code".to_string(),
        ))
    }
}

pub(crate) fn normalize_tax_provider_id(value: Option<&str>) -> RegionResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = value.to_ascii_lowercase();
    if normalized.len() > 64 {
        return Err(RegionError::Validation(
            "tax_provider_id must be at most 64 characters".to_string(),
        ));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(RegionError::Validation(
            "tax_provider_id must use lowercase ASCII, digits, underscore, or hyphen".to_string(),
        ));
    }
    Ok(Some(normalized))
}

pub(crate) fn normalize_countries(values: Vec<String>) -> RegionResult<Vec<String>> {
    if values.is_empty() {
        return Err(RegionError::Validation(
            "countries must contain at least one country code".to_string(),
        ));
    }

    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(values.len());
    for value in values {
        let code = normalize_country_code(&value)?;
        if !seen.insert(code.clone()) {
            return Err(RegionError::Validation(format!(
                "duplicate country code `{code}` in region countries"
            )));
        }
        normalized.push(code);
    }
    Ok(normalized)
}

pub(crate) fn normalize_country_code(value: &str) -> RegionResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() == 2 && normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Ok(normalized)
    } else {
        Err(RegionError::InvalidCountryCode(value.to_string()))
    }
}
