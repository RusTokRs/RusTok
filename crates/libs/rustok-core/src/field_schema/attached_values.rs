//! Validation and locale resolution for attached custom fields using caller-supplied values.

use std::collections::{BTreeMap, HashSet};

use rustok_api::{
    PLATFORM_FALLBACK_LOCALE, build_locale_candidates, locale_tags_match, normalize_locale_tag,
};
use serde_json::{Map, Value};

use super::{CustomFieldsSchema, FieldDefinition, FlexError};

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedAttachedValuesWrite {
    pub metadata: Option<Value>,
    pub localized_values: Option<Value>,
    pub locale: Option<String>,
}

pub fn prepare_attached_values_create(
    schema: CustomFieldsSchema,
    payload: Option<Value>,
    locale: &str,
) -> Result<PreparedAttachedValuesWrite, FlexError> {
    prepare_write(
        schema,
        payload,
        Value::Object(Map::new()),
        Value::Object(Map::new()),
        locale,
    )
}

/// Prepare an attached update from the exact-locale values loaded by the donor owner.
pub fn prepare_attached_values_update_from_values(
    schema: CustomFieldsSchema,
    locale: &str,
    existing_metadata: &Value,
    existing_localized: Value,
    payload: Option<Value>,
) -> Result<PreparedAttachedValuesWrite, FlexError> {
    prepare_write(
        schema,
        payload,
        existing_metadata.clone(),
        existing_localized,
        &canonical_locale(locale)?,
    )
}

/// Split donor metadata into unmanaged/reserved operational keys and Flex-managed custom fields.
pub fn split_donor_metadata(
    schema: &CustomFieldsSchema,
    metadata: &Value,
) -> (Map<String, Value>, Map<String, Value>) {
    let mut reserved = Map::new();
    let mut custom_fields = Map::new();
    let known_keys: HashSet<&str> = schema
        .active_definitions()
        .iter()
        .map(|definition| definition.field_key.as_str())
        .collect();

    if let Some(map) = metadata.as_object() {
        for (key, value) in map {
            if known_keys.contains(key.as_str()) {
                custom_fields.insert(key.clone(), value.clone());
            } else {
                reserved.insert(key.clone(), value.clone());
            }
        }
    }

    (reserved, custom_fields)
}

/// Merge unmanaged/reserved donor operational metadata with validated Flex custom fields.
pub fn merge_reserved_donor_metadata(
    mut reserved: Map<String, Value>,
    custom_fields: Option<Value>,
) -> Value {
    if let Some(custom_fields) = custom_fields.and_then(|value| value.as_object().cloned()) {
        for (key, value) in custom_fields {
            reserved.insert(key, value);
        }
    }
    Value::Object(reserved)
}

/// Merge an existing reserved metadata map with a patch map.
pub fn merge_reserved_donor_patch(
    mut existing: Map<String, Value>,
    patch: Map<String, Value>,
) -> Map<String, Value> {
    for (key, value) in patch {
        existing.insert(key, value);
    }
    existing
}

/// Merge unmanaged/reserved donor metadata with a Flex custom fields payload.
pub fn merge_donor_flex_metadata(
    schema: &CustomFieldsSchema,
    existing_metadata: &Value,
    flex_patch: &Value,
) -> Value {
    let (reserved, _) = split_donor_metadata(schema, existing_metadata);
    merge_reserved_donor_metadata(reserved, Some(flex_patch.clone()))
}

/// Prepare attached values write for a donor entity being created.
///
/// Separates unmanaged/reserved donor metadata from active Flex custom fields,
/// validates and normalizes the Flex fields, and merges the reserved donor metadata
/// back into `PreparedAttachedValuesWrite.metadata`.
pub fn prepare_donor_attached_values_create(
    schema: CustomFieldsSchema,
    donor_payload: &Value,
    locale: &str,
) -> Result<PreparedAttachedValuesWrite, FlexError> {
    let (reserved, flex_payload) = split_donor_metadata(&schema, donor_payload);
    let mut prepared =
        prepare_attached_values_create(schema, Some(Value::Object(flex_payload)), locale)?;
    prepared.metadata = Some(merge_reserved_donor_metadata(reserved, prepared.metadata));
    Ok(prepared)
}

/// Prepare a donor metadata patch and its exact-locale values without querying storage.
pub fn prepare_donor_attached_values_update_from_values(
    schema: CustomFieldsSchema,
    locale: &str,
    existing_donor_metadata: &Value,
    existing_localized: Value,
    donor_patch: Option<&Value>,
) -> Result<PreparedAttachedValuesWrite, FlexError> {
    let (existing_reserved, existing_flex) = split_donor_metadata(&schema, existing_donor_metadata);
    let (reserved_patch, flex_patch) = match donor_patch {
        Some(patch) => {
            let (r, f) = split_donor_metadata(&schema, patch);
            (r, Some(Value::Object(f)))
        }
        None => (Map::new(), None),
    };
    let merged_reserved = merge_reserved_donor_patch(existing_reserved, reserved_patch);
    let mut prepared = prepare_attached_values_update_from_values(
        schema,
        locale,
        &Value::Object(existing_flex),
        existing_localized,
        flex_patch,
    )?;
    prepared.metadata = Some(merge_reserved_donor_metadata(
        merged_reserved,
        prepared.metadata,
    ));
    Ok(prepared)
}

/// Resolves donor metadata using one schema and locale preference for a batch.
pub struct AttachedPayloadResolver {
    localized: Option<(HashSet<String>, Vec<String>)>,
}

impl AttachedPayloadResolver {
    pub fn new(
        schema: &CustomFieldsSchema,
        preferred_locale: &str,
        tenant_default_locale: &str,
    ) -> Self {
        let definitions = schema.active_definitions();
        let localized = (!definitions.is_empty()).then(|| {
            let keys = definitions
                .into_iter()
                .filter(|definition| definition.is_localized)
                .map(|definition| definition.field_key.clone())
                .collect();
            let candidates = build_locale_candidates(
                [
                    Some(preferred_locale),
                    Some(tenant_default_locale),
                    Some(PLATFORM_FALLBACK_LOCALE),
                ],
                true,
            );
            (keys, candidates)
        });
        Self { localized }
    }

    pub fn resolve(
        &self,
        shared_metadata: &Value,
        localized_by_locale: Option<&BTreeMap<String, Map<String, Value>>>,
    ) -> Option<Value> {
        match &self.localized {
            Some((keys, candidates)) => resolve_with_locale_candidates(
                shared_metadata,
                keys,
                localized_by_locale,
                candidates,
            ),
            None => {
                let payload = shared_metadata
                    .as_object()
                    .filter(|payload| !payload.is_empty())?;
                Some(Value::Object(payload.clone()))
            }
        }
    }
}

/// Resolve an attached payload using normalized locale rows supplied by the donor owner.
pub fn resolve_attached_payload_from_values(
    schema: &CustomFieldsSchema,
    shared_metadata: &Value,
    localized_by_locale: &BTreeMap<String, Map<String, Value>>,
    preferred_locale: &str,
    tenant_default_locale: &str,
) -> Result<Option<Value>, FlexError> {
    Ok(
        AttachedPayloadResolver::new(schema, preferred_locale, tenant_default_locale)
            .resolve(shared_metadata, Some(localized_by_locale)),
    )
}

fn resolve_with_locale_candidates(
    shared_metadata: &Value,
    localized_keys: &HashSet<String>,
    localized_by_locale: Option<&BTreeMap<String, Map<String, Value>>>,
    candidates: &[String],
) -> Option<Value> {
    let (mut shared_values, legacy_localized) =
        split_existing_metadata(shared_metadata, localized_keys);
    if let Some(localized) =
        localized_by_locale.and_then(|rows| resolve_localized_values(rows, candidates))
    {
        shared_values.extend(localized);
    }
    for (key, value) in legacy_localized {
        shared_values.entry(key).or_insert(value);
    }
    (!shared_values.is_empty()).then_some(Value::Object(shared_values))
}

fn resolve_localized_values(
    localized_by_locale: &BTreeMap<String, Map<String, Value>>,
    candidates: &[String],
) -> Option<Map<String, Value>> {
    candidates
        .iter()
        .find_map(|candidate| {
            localized_by_locale
                .iter()
                .find(|(locale, _)| locale_tags_match(locale, candidate))
                .map(|(_, values)| values.clone())
        })
        .or_else(|| localized_by_locale.values().next().cloned())
}

fn prepare_write(
    schema: CustomFieldsSchema,
    payload: Option<Value>,
    existing_metadata: Value,
    existing_localized: Value,
    locale: &str,
) -> Result<PreparedAttachedValuesWrite, FlexError> {
    if schema.active_definitions().is_empty() {
        return Ok(PreparedAttachedValuesWrite {
            metadata: payload,
            localized_values: None,
            locale: None,
        });
    }

    let (definitions, localized_keys) = split_definitions(&schema);
    let (shared_definitions, localized_definitions): (Vec<_>, Vec<_>) = definitions
        .into_iter()
        .partition(|definition| !definition.is_localized);

    let payload_map = object_map(payload.as_ref());
    let (shared_patch, localized_patch) = split_patch(&payload_map, &localized_keys);
    let (mut shared_values, legacy_localized) =
        split_existing_metadata(&existing_metadata, &localized_keys);
    let mut localized_values = object_map(Some(&existing_localized));

    for (key, value) in legacy_localized {
        localized_values.entry(key).or_insert(value);
    }

    for (key, value) in object_map(Some(&existing_localized)) {
        if !localized_keys.contains(&key) {
            shared_values.entry(key).or_insert(value);
        }
    }

    merge_patch(&mut shared_values, shared_patch);
    merge_patch(&mut localized_values, localized_patch);

    let metadata = if shared_definitions.is_empty() {
        None
    } else {
        let shared_schema = CustomFieldsSchema::new(shared_definitions);
        let mut shared_value = Value::Object(shared_values);
        shared_schema.apply_defaults(&mut shared_value);
        shared_schema.strip_unknown(&mut shared_value);
        validate_schema(&shared_schema, &shared_value)?;
        Some(shared_value)
    };

    let localized_values = if localized_definitions.is_empty() {
        None
    } else {
        let localized_schema = CustomFieldsSchema::new(localized_definitions);
        let mut localized_value = Value::Object(localized_values);
        localized_schema.apply_defaults(&mut localized_value);
        localized_schema.strip_unknown(&mut localized_value);
        validate_schema(&localized_schema, &localized_value)?;
        Some(localized_value)
    };

    Ok(PreparedAttachedValuesWrite {
        metadata,
        localized_values,
        locale: Some(canonical_locale(locale)?),
    })
}

fn split_definitions(schema: &CustomFieldsSchema) -> (Vec<FieldDefinition>, HashSet<String>) {
    let definitions: Vec<FieldDefinition> =
        schema.active_definitions().into_iter().cloned().collect();
    let localized_keys = definitions
        .iter()
        .filter(|definition| definition.is_localized)
        .map(|definition| definition.field_key.clone())
        .collect();
    (definitions, localized_keys)
}

fn split_existing_metadata(
    metadata: &Value,
    localized_keys: &HashSet<String>,
) -> (Map<String, Value>, Map<String, Value>) {
    let mut shared = Map::new();
    let mut localized = Map::new();

    for (key, value) in object_map(Some(metadata)) {
        if localized_keys.contains(&key) {
            localized.insert(key, value);
        } else {
            shared.insert(key, value);
        }
    }

    (shared, localized)
}

fn split_patch(
    payload: &Map<String, Value>,
    localized_keys: &HashSet<String>,
) -> (Map<String, Value>, Map<String, Value>) {
    let mut shared = Map::new();
    let mut localized = Map::new();

    for (key, value) in payload {
        if localized_keys.contains(key) {
            localized.insert(key.clone(), value.clone());
        } else {
            shared.insert(key.clone(), value.clone());
        }
    }

    (shared, localized)
}

fn merge_patch(target: &mut Map<String, Value>, patch: Map<String, Value>) {
    for (key, value) in patch {
        target.insert(key, value);
    }
}

fn object_map(value: Option<&Value>) -> Map<String, Value> {
    value
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

fn validate_schema(schema: &CustomFieldsSchema, payload: &Value) -> Result<(), FlexError> {
    let errors = schema.validate(payload);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(FlexError::ValidationFailed(errors))
    }
}

pub fn canonical_locale(locale: &str) -> Result<String, FlexError> {
    normalize_locale_tag(locale).ok_or_else(|| FlexError::InvalidLocale(locale.to_string()))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::*;
    use crate::field_schema::FieldType;

    fn definition(field_key: &str, is_localized: bool) -> FieldDefinition {
        FieldDefinition {
            field_key: field_key.to_string(),
            field_type: FieldType::Text,
            label: HashMap::from([("en".to_string(), field_key.to_string())]),
            description: None,
            is_localized,
            is_required: false,
            default_value: None,
            validation: None,
            position: 0,
            is_active: true,
        }
    }

    #[test]
    fn split_existing_metadata_separates_legacy_localized_keys() {
        let schema =
            CustomFieldsSchema::new(vec![definition("nickname", false), definition("bio", true)]);
        let (_, localized_keys) = super::split_definitions(&schema);
        let (shared, localized) = split_existing_metadata(
            &json!({"nickname": "neo", "bio": "ru copy"}),
            &localized_keys,
        );

        assert_eq!(shared.get("nickname"), Some(&json!("neo")));
        assert_eq!(localized.get("bio"), Some(&json!("ru copy")));
    }
}
