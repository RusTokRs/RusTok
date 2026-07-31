fn normalize_date_window_request(
    request: ForumStorefrontSearchDateWindowRequest,
) -> Result<NormalizedForumStorefrontDateWindowRequest, ForumStorefrontSearchExecutionError> {
    let ForumStorefrontSearchDateWindowRequest {
        request,
        published_from,
        published_to,
    } = request;
    if request.tenant_id.is_nil() {
        return validation("Forum storefront Search requires a tenant");
    }
    let query = normalize_date_window_query(&request.query)?;
    let locale = normalize_date_window_locale(request.locale.as_deref())?;
    let fallback_locale = normalize_date_window_required_locale(&request.fallback_locale)?;
    let exact_locale = locale.clone().unwrap_or_else(|| fallback_locale.clone());
    let published_from =
        normalize_optional_date_window_rfc3339("published_from", published_from.as_deref())?;
    let published_to =
        normalize_optional_date_window_rfc3339("published_to", published_to.as_deref())?;
    if published_from
        .as_ref()
        .zip(published_to.as_ref())
        .is_some_and(|(from, to)| from > to)
    {
        return validation("published_from must not be after published_to");
    }
    let requested_channel_id =
        parse_date_window_optional_uuid("channel_id", request.channel_id.as_deref())?;
    let request_context = request.request_context.ok_or_else(|| {
        ForumStorefrontSearchExecutionError::Validation(
            "Forum storefront Search requires trusted request context".to_string(),
        )
    })?;
    let trusted_channel = resolve_trusted_storefront_channel(
        &request_context,
        request.tenant_id,
        requested_channel_id,
    )
    .map_err(|error| ForumStorefrontSearchExecutionError::Validation(error.to_string()))?;
    let source_modules =
        normalize_date_window_filter_values("source_modules", request.source_modules)?;
    if source_modules.as_slice() != [FORUM_SEARCH_SOURCE_MODULE] {
        return validation("Forum storefront Search requires source_modules: [forum]");
    }
    let category_ids = normalize_date_window_uuid_values("category_ids", request.category_ids)?;
    if category_ids.is_empty() {
        return validation("Forum storefront Search requires at least one category_id");
    }
    let ranking_profile = request
        .ranking_profile
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if let Some(value) = ranking_profile.as_deref() {
        if SearchRankingProfile::try_from_str(value).is_none() {
            return validation("Unsupported ranking profile");
        }
    }

    Ok(NormalizedForumStorefrontDateWindowRequest {
        tenant_id: request.tenant_id,
        query,
        locale,
        fallback_locale,
        channel_id: trusted_channel.channel_id,
        limit: request.limit.unwrap_or(12).clamp(1, 50) as usize,
        offset: request.offset.unwrap_or(0).max(0) as usize,
        ranking_profile,
        preset_key: normalize_date_window_preset_key(request.preset_key)?,
        entity_types: normalize_date_window_filter_values("entity_types", request.entity_types)?,
        source_modules,
        statuses: normalize_date_window_filter_values("statuses", request.statuses)?,
        category_ids,
        document_filters: ForumStorefrontDocumentFilters {
            author_ids: normalize_date_window_uuid_values("author_ids", request.author_ids)?,
            tags: normalize_date_window_tag_values("tags", request.tags)?,
            solved: request.solved,
        },
        locale_date_filters: ForumStorefrontLocaleDateFilters {
            exact_locale,
            published_from,
            published_to,
        },
        attribute_filters: normalize_date_window_attribute_filters(request.attribute_filters)?,
        sort_attribute_code: normalize_date_window_attribute_code(request.sort_attribute_code)?,
        sort_desc: request.sort_desc,
        auth: request.auth,
        request_context: Some(request_context),
        trusted_channel,
        transport: request.transport,
    })
}

fn normalize_date_window_query(value: &str) -> Result<String, ForumStorefrontSearchExecutionError> {
    let value = value.trim();
    if value.len() > MAX_SEARCH_QUERY_LEN {
        return validation("Search query exceeds the maximum length of 256 characters");
    }
    if value.chars().any(char::is_control) {
        return validation("Search query contains unsupported control characters");
    }
    Ok(value.to_string())
}

fn normalize_date_window_locale(
    value: Option<&str>,
) -> Result<Option<String>, ForumStorefrontSearchExecutionError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_date_window_required_locale)
        .transpose()
}

fn normalize_date_window_required_locale(
    value: &str,
) -> Result<String, ForumStorefrontSearchExecutionError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_LOCALE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return validation("Invalid locale format");
    }
    Ok(value.to_ascii_lowercase())
}

fn normalize_optional_date_window_rfc3339(
    field: &str,
    value: Option<&str>,
) -> Result<Option<DateTime<Utc>>, ForumStorefrontSearchExecutionError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            DateTime::parse_from_rfc3339(value)
                .map(|timestamp| timestamp.with_timezone(&Utc))
                .map_err(|_| {
                    ForumStorefrontSearchExecutionError::Validation(format!(
                        "{field} must be RFC3339"
                    ))
                })
        })
        .transpose()
}

fn normalize_date_window_filter_values(
    field: &str,
    values: Vec<String>,
) -> Result<Vec<String>, ForumStorefrontSearchExecutionError> {
    if values.len() > MAX_FILTER_VALUES {
        return validation(format!(
            "{field} exceeds the maximum size of {MAX_FILTER_VALUES} values"
        ));
    }
    values
        .into_iter()
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            if value.is_empty()
                || value.len() > MAX_FILTER_VALUE_LEN
                || !value
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
            {
                return validation(format!("{field} contains an invalid value"));
            }
            Ok(value)
        })
        .collect()
}

fn normalize_date_window_tag_values(
    field: &str,
    values: Vec<String>,
) -> Result<Vec<String>, ForumStorefrontSearchExecutionError> {
    if values.len() > MAX_FILTER_VALUES {
        return validation(format!(
            "{field} exceeds the maximum size of {MAX_FILTER_VALUES} values"
        ));
    }
    let mut normalized = values
        .into_iter()
        .map(|value| {
            let value = value.trim();
            if value.is_empty()
                || value.chars().count() > MAX_FILTER_VALUE_LEN
                || value.chars().any(char::is_control)
            {
                return validation(format!("{field} contains an invalid value"));
            }
            Ok(value.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

fn normalize_date_window_uuid_values(
    field: &str,
    values: Vec<String>,
) -> Result<Vec<Uuid>, ForumStorefrontSearchExecutionError> {
    if values.len() > MAX_FILTER_VALUES {
        return validation(format!(
            "{field} exceeds the maximum size of {MAX_FILTER_VALUES} values"
        ));
    }
    values
        .into_iter()
        .map(|value| {
            Uuid::parse_str(value.trim()).map_err(|_| {
                ForumStorefrontSearchExecutionError::Validation(format!(
                    "{field} contains an invalid UUID"
                ))
            })
        })
        .collect()
}

fn parse_date_window_optional_uuid(
    field: &str,
    value: Option<&str>,
) -> Result<Option<Uuid>, ForumStorefrontSearchExecutionError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            Uuid::parse_str(value).map_err(|_| {
                ForumStorefrontSearchExecutionError::Validation(format!(
                    "{field} contains an invalid UUID"
                ))
            })
        })
        .transpose()
}

fn normalize_date_window_preset_key(
    value: Option<String>,
) -> Result<Option<String>, ForumStorefrontSearchExecutionError> {
    let value = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if let Some(value) = value.as_deref() {
        if value.len() > MAX_FILTER_VALUE_LEN
            || !value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
        {
            return validation("Invalid preset key");
        }
    }
    Ok(value)
}

fn normalize_date_window_attribute_code(
    value: Option<String>,
) -> Result<Option<String>, ForumStorefrontSearchExecutionError> {
    let value = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if let Some(value) = value.as_deref() {
        validate_date_window_attribute_code(value)?;
    }
    Ok(value)
}

fn normalize_date_window_attribute_filters(
    filters: Vec<crate::ForumStorefrontSearchAttributeFilter>,
) -> Result<Vec<SearchAttributeFilter>, ForumStorefrontSearchExecutionError> {
    if filters.len() > MAX_ATTRIBUTE_FILTERS {
        return validation(format!(
            "attribute_filters exceeds the maximum size of {MAX_ATTRIBUTE_FILTERS} filters"
        ));
    }
    filters
        .into_iter()
        .map(|filter| {
            let attribute_code = filter.attribute_code.trim().to_ascii_lowercase();
            validate_date_window_attribute_code(&attribute_code)?;
            Ok(SearchAttributeFilter {
                attribute_code,
                values: normalize_date_window_filter_values(
                    "attribute_filter.values",
                    filter.values,
                )?,
                min: normalize_date_window_attribute_bound(filter.min)?,
                max: normalize_date_window_attribute_bound(filter.max)?,
            })
        })
        .collect()
}

fn validate_date_window_attribute_code(
    value: &str,
) -> Result<(), ForumStorefrontSearchExecutionError> {
    if value.is_empty()
        || value.len() > MAX_FILTER_VALUE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return validation("attribute_code contains an invalid value");
    }
    Ok(())
}

fn normalize_date_window_attribute_bound(
    value: Option<String>,
) -> Result<Option<String>, ForumStorefrontSearchExecutionError> {
    let value = value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(value) = value.as_deref() {
        if value.len() > MAX_FILTER_VALUE_LEN || value.chars().any(char::is_control) {
            return validation("attribute filter bound contains an invalid value");
        }
    }
    Ok(value)
}

fn validation<T>(message: impl Into<String>) -> Result<T, ForumStorefrontSearchExecutionError> {
    Err(ForumStorefrontSearchExecutionError::Validation(
        message.into(),
    ))
}
