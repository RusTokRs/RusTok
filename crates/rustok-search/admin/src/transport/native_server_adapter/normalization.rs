#[cfg(feature = "ssr")]
struct ResolvedSearchInput {
    preset_key: Option<String>,
    entity_types: Vec<String>,
    source_modules: Vec<String>,
    statuses: Vec<String>,
    ranking_profile: rustok_search::SearchRankingProfile,
}

#[cfg(feature = "ssr")]
fn normalize_search_preview_input(
    input: SearchPreviewInput,
) -> Result<SearchPreviewInput, ServerFnError> {
    Ok(SearchPreviewInput {
        query: normalize_query(&input.query)?,
        locale: normalize_locale(input.locale.as_deref())?,
        channel_id: input.channel_id,
        tenant_id: input.tenant_id,
        limit: input.limit,
        offset: input.offset,
        ranking_profile: normalize_ranking_profile(input.ranking_profile)?,
        preset_key: normalize_preset_key(input.preset_key)?,
        entity_types: Some(normalize_filter_values("entity_types", input.entity_types)?),
        source_modules: Some(normalize_filter_values(
            "source_modules",
            input.source_modules,
        )?),
        statuses: Some(normalize_filter_values("statuses", input.statuses)?),
        category_ids: input.category_ids,
        attribute_filters: input.attribute_filters,
        sort_attribute_code: input.sort_attribute_code,
        sort_desc: input.sort_desc,
    })
}

#[cfg(feature = "ssr")]
fn normalize_query(value: &str) -> Result<String, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.len() > MAX_SEARCH_QUERY_LEN {
        return Err(ServerFnError::new(format!(
            "Search query exceeds the maximum length of {MAX_SEARCH_QUERY_LEN} characters"
        )));
    }

    if trimmed.chars().any(|ch| ch.is_control()) {
        return Err(ServerFnError::new(
            "Search query contains unsupported control characters",
        ));
    }

    Ok(trimmed.to_string())
}

#[cfg(feature = "ssr")]
fn normalize_locale(value: Option<&str>) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    if value.len() > MAX_LOCALE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(ServerFnError::new("Invalid locale format"));
    }

    Ok(Some(value.to_ascii_lowercase()))
}

#[cfg(feature = "ssr")]
fn normalize_filter_values(
    field_name: &str,
    values: Option<Vec<String>>,
) -> Result<Vec<String>, ServerFnError> {
    let values = values.unwrap_or_default();
    if values.len() > MAX_FILTER_VALUES {
        return Err(ServerFnError::new(format!(
            "{field_name} exceeds the maximum size of {MAX_FILTER_VALUES} values"
        )));
    }

    values
        .into_iter()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                return Err(ServerFnError::new(format!(
                    "{field_name} contains an empty value"
                )));
            }
            if normalized.len() > MAX_FILTER_VALUE_LEN
                || !normalized
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
            {
                return Err(ServerFnError::new(format!(
                    "{field_name} contains an invalid value"
                )));
            }
            Ok(normalized)
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn normalize_uuid_values(
    field_name: &str,
    values: Option<Vec<String>>,
) -> Result<Vec<uuid::Uuid>, ServerFnError> {
    let values = values.unwrap_or_default();
    if values.len() > MAX_FILTER_VALUES {
        return Err(ServerFnError::new(format!(
            "{field_name} exceeds the maximum size of {MAX_FILTER_VALUES} values"
        )));
    }

    values
        .into_iter()
        .map(|value| {
            uuid::Uuid::parse_str(value.trim())
                .map_err(|_| ServerFnError::new(format!("{field_name} contains an invalid UUID")))
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn normalize_attribute_code(value: Option<String>) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    validate_attribute_code("sort_attribute_code", &value)?;
    Ok(Some(value))
}

#[cfg(feature = "ssr")]
fn normalize_attribute_filters(
    filters: Option<Vec<SearchAttributeFilterInput>>,
) -> Result<Vec<rustok_search::SearchAttributeFilter>, ServerFnError> {
    let filters = filters.unwrap_or_default();
    if filters.len() > MAX_ATTRIBUTE_FILTERS {
        return Err(ServerFnError::new(format!(
            "attribute_filters exceeds the maximum size of {MAX_ATTRIBUTE_FILTERS} filters"
        )));
    }

    filters
        .into_iter()
        .map(|filter| {
            let attribute_code = filter.attribute_code.trim().to_ascii_lowercase();
            validate_attribute_code("attribute_code", &attribute_code)?;
            let values = normalize_filter_values("attribute_filter.values", filter.values)?;
            Ok(rustok_search::SearchAttributeFilter {
                attribute_code,
                values,
                min: normalize_attribute_bound("attribute_filter.min", filter.min)?,
                max: normalize_attribute_bound("attribute_filter.max", filter.max)?,
            })
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn validate_attribute_code(field_name: &str, value: &str) -> Result<(), ServerFnError> {
    if value.is_empty()
        || value.len() > MAX_FILTER_VALUE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(ServerFnError::new(format!(
            "{field_name} contains an invalid value"
        )));
    }

    Ok(())
}

#[cfg(feature = "ssr")]
fn normalize_attribute_bound(
    field_name: &str,
    value: Option<String>,
) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    if value.len() > MAX_FILTER_VALUE_LEN || value.chars().any(|ch| ch.is_control()) {
        return Err(ServerFnError::new(format!(
            "{field_name} contains an invalid value"
        )));
    }

    Ok(Some(value))
}

#[cfg(feature = "ssr")]
fn normalize_ranking_profile(value: Option<String>) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    rustok_search::SearchRankingProfile::try_from_str(&value)
        .map(|_| Some(value))
        .ok_or_else(|| ServerFnError::new("Unsupported ranking profile"))
}

#[cfg(feature = "ssr")]
fn normalize_preset_key(value: Option<String>) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    if value.len() > MAX_FILTER_VALUE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
    {
        return Err(ServerFnError::new("Invalid preset key"));
    }

    Ok(Some(value))
}

#[cfg(feature = "ssr")]
fn normalize_surface(value: &str) -> Result<String, ServerFnError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > 64 {
        return Err(ServerFnError::new("Invalid search surface"));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(ServerFnError::new("Invalid search surface"));
    }
    Ok(normalized)
}

#[cfg(feature = "ssr")]
fn resolve_preset_and_ranking(
    config: &serde_json::Value,
    surface: &str,
    preset_key: Option<&str>,
    requested_ranking_profile: Option<&str>,
    entity_types: Vec<String>,
    source_modules: Vec<String>,
    statuses: Vec<String>,
) -> Result<ResolvedSearchInput, ServerFnError> {
    let resolved_preset = rustok_search::SearchFilterPresetService::resolve(
        config,
        surface,
        preset_key,
        entity_types,
        source_modules,
        statuses,
    )
    .map_err(map_core_error)?;
    let ranking_profile = rustok_search::SearchRankingProfile::resolve(
        config,
        surface,
        requested_ranking_profile,
        resolved_preset.ranking_profile,
    )
    .map_err(map_core_error)?;

    Ok(ResolvedSearchInput {
        preset_key: resolved_preset.preset.map(|preset| preset.key),
        entity_types: resolved_preset.entity_types,
        source_modules: resolved_preset.source_modules,
        statuses: resolved_preset.statuses,
        ranking_profile,
    })
}
