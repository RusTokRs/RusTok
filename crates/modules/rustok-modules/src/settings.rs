//! Module settings schema validation, normalization, and localization metadata.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use thiserror::Error;

const MAX_LOCALIZED_FIELD_ID_BYTES: usize = 128;

/// Declarative schema for one module setting.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModuleSettingSpec {
    #[serde(rename = "type", default)]
    pub value_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub options: Vec<serde_json::Value>,
    #[serde(default)]
    pub object_keys: Vec<String>,
    #[serde(default)]
    pub item_type: Option<String>,
    #[serde(default)]
    pub properties: HashMap<String, ModuleSettingSpec>,
    #[serde(default)]
    pub items: Option<Box<ModuleSettingSpec>>,
}

impl ModuleSettingSpec {
    /// Validates owner-declared Translation metadata without changing the
    /// settings schema's public Rust shape.
    ///
    /// `localized_fields` maps a stable owner field ID to a schema path.
    /// `sensitive_paths` fences a schema node and all of its descendants from
    /// localization. Localization is therefore explicit opt-in and can be
    /// onboarded independently of legacy `ModuleSettingSpec` struct literals.
    pub fn validate_localization_registry(
        module_slug: &str,
        schema: &HashMap<String, ModuleSettingSpec>,
        localized_fields: &BTreeMap<String, String>,
        sensitive_paths: &BTreeSet<String>,
    ) -> Result<(), ModuleSettingsValidationError> {
        validate_module_settings_schema(module_slug, schema)?;

        for sensitive_path in sensitive_paths {
            resolve_setting_spec(module_slug, schema, sensitive_path)?;
        }

        let mut claimed_paths = HashMap::<String, String>::new();
        for (field_id, path) in localized_fields {
            if field_id != field_id.trim() || !is_valid_setting_field_id(field_id) {
                return Err(invalid_schema(
                    module_slug,
                    path,
                    "localized field IDs must be canonical 1..128 byte stable tokens using ASCII letters, digits, '.', '_' or '-'",
                ));
            }

            if let Some(existing_field_id) = claimed_paths.insert(path.clone(), field_id.clone()) {
                return Err(invalid_schema(
                    module_slug,
                    path,
                    format!(
                        "localized path is already claimed by field ID '{existing_field_id}'"
                    ),
                ));
            }

            let spec = resolve_setting_spec(module_slug, schema, path)?;
            if spec.value_type.trim() != "string" {
                return Err(invalid_schema(
                    module_slug,
                    path,
                    "localized settings must be string leaves",
                ));
            }
            if !spec.options.is_empty() {
                return Err(invalid_schema(
                    module_slug,
                    path,
                    "localized settings cannot use options because enum/config values are not translatable copy",
                ));
            }
            if let Some(sensitive_path) = sensitive_paths
                .iter()
                .find(|sensitive_path| path_is_at_or_below(path, sensitive_path))
            {
                return Err(invalid_schema(
                    module_slug,
                    path,
                    format!(
                        "localized setting is fenced by sensitive path '{sensitive_path}'"
                    ),
                ));
            }
        }

        Ok(())
    }

    /// Returns a deterministic `(field_id, schema_path)` inventory for the
    /// owner-declared localized settings registry.
    pub fn localized_field_paths(
        module_slug: &str,
        schema: &HashMap<String, ModuleSettingSpec>,
        localized_fields: &BTreeMap<String, String>,
        sensitive_paths: &BTreeSet<String>,
    ) -> Result<Vec<(String, String)>, ModuleSettingsValidationError> {
        Self::validate_localization_registry(
            module_slug,
            schema,
            localized_fields,
            sensitive_paths,
        )?;
        Ok(localized_fields
            .iter()
            .map(|(field_id, path)| (field_id.clone(), path.clone()))
            .collect())
    }

    /// Extracts only owner-declared localized string values from normalized
    /// settings, keyed by stable field ID.
    ///
    /// Missing optional leaves are omitted. Non-string values, enum/config
    /// options, array-item paths, and sensitivity-fenced paths are rejected by
    /// registry validation before any value can be exposed to Translation.
    pub fn localized_value_snapshot(
        module_slug: &str,
        schema: &HashMap<String, ModuleSettingSpec>,
        localized_fields: &BTreeMap<String, String>,
        sensitive_paths: &BTreeSet<String>,
        settings: serde_json::Value,
    ) -> Result<BTreeMap<String, String>, ModuleSettingsValidationError> {
        Self::validate_localization_registry(
            module_slug,
            schema,
            localized_fields,
            sensitive_paths,
        )?;
        let normalized = normalize_module_settings(module_slug, schema, settings)?;
        let mut snapshot = BTreeMap::new();

        for (field_id, path) in localized_fields {
            if let Some(value) = setting_value_at_path(&normalized, path)
                .and_then(serde_json::Value::as_str)
            {
                snapshot.insert(field_id.clone(), value.to_string());
            }
        }

        Ok(snapshot)
    }
}

/// Module-owned settings validation failures.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModuleSettingsValidationError {
    #[error("Module '{module_slug}' has invalid setting key '{key}'")]
    InvalidKey { module_slug: String, key: String },
    #[error("Module '{module_slug}' setting '{key}' has invalid schema: {reason}")]
    InvalidSchema {
        module_slug: String,
        key: String,
        reason: String,
    },
    #[error("Module '{module_slug}' setting '{key}' is invalid: {reason}")]
    InvalidValue {
        module_slug: String,
        key: String,
        reason: String,
    },
}

pub fn validate_module_settings_schema(
    module_slug: &str,
    schema: &HashMap<String, ModuleSettingSpec>,
) -> Result<(), ModuleSettingsValidationError> {
    let mut keys = schema.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        if !is_valid_setting_key(&key) {
            return Err(ModuleSettingsValidationError::InvalidKey {
                module_slug: module_slug.to_string(),
                key,
            });
        }
        let spec = schema
            .get(&key)
            .expect("sorted settings key must exist in schema");
        validate_setting_spec(module_slug, &key, spec)?;
    }
    Ok(())
}

pub fn normalize_module_settings(
    module_slug: &str,
    schema: &HashMap<String, ModuleSettingSpec>,
    settings: serde_json::Value,
) -> Result<serde_json::Value, ModuleSettingsValidationError> {
    validate_module_settings_schema(module_slug, schema)?;

    let mut settings_object = settings.as_object().cloned().ok_or_else(|| {
        invalid_value(
            module_slug,
            "$root",
            "module settings must be a JSON object",
        )
    })?;

    if schema.is_empty() {
        return Ok(serde_json::Value::Object(settings_object));
    }

    let mut allowed_keys = schema.keys().cloned().collect::<Vec<_>>();
    allowed_keys.sort();
    let mut unknown_keys = settings_object
        .keys()
        .filter(|key| !schema.contains_key(*key))
        .cloned()
        .collect::<Vec<_>>();
    unknown_keys.sort();
    if let Some(key) = unknown_keys.first() {
        return Err(invalid_value(
            module_slug,
            key,
            format!("unknown setting; allowed keys: {}", allowed_keys.join(", ")),
        ));
    }

    let mut normalized = serde_json::Map::new();
    for key in allowed_keys {
        let spec = schema
            .get(&key)
            .expect("allowed settings key must exist in schema");
        match settings_object.remove(&key) {
            Some(value) => {
                validate_setting_value(module_slug, &key, spec, &value)?;
                normalized.insert(key, value);
            }
            None if spec.required && spec.default.is_none() => {
                return Err(invalid_value(
                    module_slug,
                    key,
                    "required setting is missing",
                ));
            }
            None => {
                if let Some(default) = spec.default.clone() {
                    normalized.insert(key, default);
                }
            }
        }
    }

    Ok(serde_json::Value::Object(normalized))
}

fn validate_setting_spec(
    module_slug: &str,
    key: &str,
    spec: &ModuleSettingSpec,
) -> Result<(), ModuleSettingsValidationError> {
    let value_type = spec.value_type.trim();
    if !is_supported_setting_type(value_type) {
        return Err(invalid_schema(
            module_slug,
            key,
            format!("unsupported type '{value_type}'"),
        ));
    }
    if let Some(default) = &spec.default
        && !setting_value_matches_type(value_type, default)
    {
        return Err(invalid_schema(
            module_slug,
            key,
            "default does not match declared type",
        ));
    }
    if let (Some(min), Some(max)) = (spec.min, spec.max)
        && min > max
    {
        return Err(invalid_schema(
            module_slug,
            key,
            format!("min ({min}) must not exceed max ({max})"),
        ));
    }
    if (spec.min.is_some() || spec.max.is_some())
        && !matches!(value_type, "integer" | "number" | "string" | "array")
    {
        return Err(invalid_schema(
            module_slug,
            key,
            "min/max are only supported for string, array, integer, and number",
        ));
    }
    if !spec.options.is_empty() {
        if !matches!(value_type, "string" | "integer" | "number" | "boolean") {
            return Err(invalid_schema(
                module_slug,
                key,
                "options are only supported for scalar string/integer/number/boolean settings",
            ));
        }
        if spec
            .options
            .iter()
            .any(|option| !setting_value_matches_type(value_type, option))
        {
            return Err(invalid_schema(
                module_slug,
                key,
                "all options must match the declared type",
            ));
        }
        if let Some(default) = &spec.default
            && !spec.options.iter().any(|option| option == default)
        {
            return Err(invalid_schema(
                module_slug,
                key,
                "default must be one of the declared options",
            ));
        }
    }
    if !spec.object_keys.is_empty() {
        if value_type != "object" {
            return Err(invalid_schema(
                module_slug,
                key,
                "object_keys are only supported for object settings",
            ));
        }
        let mut seen_keys = HashSet::new();
        for object_key in &spec.object_keys {
            if !is_valid_setting_key(object_key) {
                return Err(invalid_schema(
                    module_slug,
                    key,
                    format!("invalid object key '{object_key}'"),
                ));
            }
            if !seen_keys.insert(object_key) {
                return Err(invalid_schema(
                    module_slug,
                    key,
                    format!("duplicate object key '{object_key}'"),
                ));
            }
        }
        if let Some(unknown_key) = spec
            .default
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .and_then(|object| {
                object
                    .keys()
                    .find(|candidate| !spec.object_keys.iter().any(|allowed| allowed == *candidate))
            })
        {
            return Err(invalid_schema(
                module_slug,
                key,
                format!("default contains undeclared object key '{unknown_key}'"),
            ));
        }
    }
    if !spec.properties.is_empty() {
        if value_type != "object" {
            return Err(invalid_schema(
                module_slug,
                key,
                "properties are only supported for object settings",
            ));
        }
        let mut property_keys = spec.properties.keys().cloned().collect::<Vec<_>>();
        property_keys.sort();
        let mut explicit_object_keys = spec.object_keys.clone();
        explicit_object_keys.sort();
        if !spec.object_keys.is_empty() && property_keys != explicit_object_keys {
            return Err(invalid_schema(
                module_slug,
                key,
                "object_keys must match declared properties when both are provided",
            ));
        }
        for property_key in property_keys {
            if !is_valid_setting_key(&property_key) {
                return Err(ModuleSettingsValidationError::InvalidKey {
                    module_slug: module_slug.to_string(),
                    key: format!("{key}.{property_key}"),
                });
            }
            let property_spec = spec
                .properties
                .get(&property_key)
                .expect("sorted property key must exist in schema");
            validate_setting_spec(
                module_slug,
                &format!("{key}.{property_key}"),
                property_spec,
            )?;
        }
        if let Some(default) = spec.default.as_ref().and_then(serde_json::Value::as_object) {
            for (property_key, property_value) in default {
                if let Some(property_spec) = spec.properties.get(property_key) {
                    validate_setting_value(
                        module_slug,
                        &format!("{key}.{property_key}"),
                        property_spec,
                        property_value,
                    )?;
                }
            }
        }
    }
    if let Some(item_type) = spec.item_type.as_deref() {
        let item_type = item_type.trim();
        if value_type != "array" {
            return Err(invalid_schema(
                module_slug,
                key,
                "item_type is only supported for array settings",
            ));
        }
        if !is_supported_setting_type(item_type) {
            return Err(invalid_schema(
                module_slug,
                key,
                format!("unsupported array item type '{item_type}'"),
            ));
        }
        if spec
            .default
            .as_ref()
            .and_then(serde_json::Value::as_array)
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|item| !setting_value_matches_type(item_type, item))
            })
        {
            return Err(invalid_schema(
                module_slug,
                key,
                "default array items must match declared item_type",
            ));
        }
    }
    if let Some(items) = &spec.items {
        if value_type != "array" {
            return Err(invalid_schema(
                module_slug,
                key,
                "items are only supported for array settings",
            ));
        }
        validate_setting_spec(module_slug, &format!("{key}[]"), items)?;
        if let Some(item_type) = spec.item_type.as_deref()
            && items.value_type.trim() != item_type.trim()
        {
            return Err(invalid_schema(
                module_slug,
                key,
                "item_type must match items.type when both are provided",
            ));
        }
        if let Some(default) = spec.default.as_ref().and_then(serde_json::Value::as_array) {
            for (index, item) in default.iter().enumerate() {
                validate_setting_value(module_slug, &format!("{key}[{index}]"), items, item)?;
            }
        }
    }
    Ok(())
}

fn validate_setting_value(
    module_slug: &str,
    key: &str,
    spec: &ModuleSettingSpec,
    value: &serde_json::Value,
) -> Result<(), ModuleSettingsValidationError> {
    let value_type = spec.value_type.trim();
    if !setting_value_matches_type(value_type, value) {
        return Err(invalid_value(
            module_slug,
            key,
            format!("expected {value_type}"),
        ));
    }
    if !spec.options.is_empty() && !spec.options.iter().any(|option| option == value) {
        let allowed = spec
            .options
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(invalid_value(
            module_slug,
            key,
            format!("must be one of: {allowed}"),
        ));
    }
    let allowed_keys = declared_object_keys(spec);
    if !allowed_keys.is_empty() {
        let object = value
            .as_object()
            .expect("object keys require an object value");
        let mut unknown_keys = object
            .keys()
            .filter(|candidate| !allowed_keys.iter().any(|allowed| allowed == *candidate))
            .cloned()
            .collect::<Vec<_>>();
        unknown_keys.sort();
        if let Some(unknown_key) = unknown_keys.first() {
            return Err(invalid_value(
                module_slug,
                key,
                format!(
                    "unknown object key '{unknown_key}'; allowed keys: {}",
                    allowed_keys.join(", ")
                ),
            ));
        }
    }
    if let Some(item_type) = declared_item_type(spec) {
        let array = value
            .as_array()
            .expect("array item type requires an array value");
        if let Some((index, _)) = array
            .iter()
            .enumerate()
            .find(|(_, item)| !setting_value_matches_type(item_type, item))
        {
            return Err(invalid_value(
                module_slug,
                key,
                format!("array item at index {index} must be {item_type}"),
            ));
        }
    }
    if !spec.properties.is_empty() {
        let object = value
            .as_object()
            .expect("properties require an object value");
        for (property_key, property_value) in object {
            if let Some(property_spec) = spec.properties.get(property_key) {
                validate_setting_value(
                    module_slug,
                    &format!("{key}.{property_key}"),
                    property_spec,
                    property_value,
                )?;
            }
        }
    }
    if let Some(items) = &spec.items {
        let array = value.as_array().expect("items require an array value");
        for (index, item) in array.iter().enumerate() {
            validate_setting_value(module_slug, &format!("{key}[{index}]"), items, item)?;
        }
    }
    match value_type {
        "integer" | "number" => {
            let numeric_value = value
                .as_f64()
                .ok_or_else(|| invalid_value(module_slug, key, format!("expected {value_type}")))?;
            if let Some(min) = spec.min
                && numeric_value < min
            {
                return Err(invalid_value(module_slug, key, format!("must be >= {min}")));
            }
            if let Some(max) = spec.max
                && numeric_value > max
            {
                return Err(invalid_value(module_slug, key, format!("must be <= {max}")));
            }
        }
        "string" => validate_length(
            module_slug,
            key,
            value
                .as_str()
                .map(|item| item.chars().count())
                .unwrap_or_default() as f64,
            spec,
        )?,
        "array" => validate_length(
            module_slug,
            key,
            value.as_array().map(Vec::len).unwrap_or_default() as f64,
            spec,
        )?,
        _ => {}
    }
    Ok(())
}

fn resolve_setting_spec<'a>(
    module_slug: &str,
    schema: &'a HashMap<String, ModuleSettingSpec>,
    path: &str,
) -> Result<&'a ModuleSettingSpec, ModuleSettingsValidationError> {
    if path != path.trim() || path.is_empty() {
        return Err(invalid_schema(
            module_slug,
            path,
            "settings metadata paths must be canonical non-empty dot-separated setting keys",
        ));
    }

    let mut segments = path.split('.');
    let root_key = segments
        .next()
        .expect("non-empty settings metadata path must have a root segment");
    if !is_valid_setting_key(root_key) {
        return Err(invalid_schema(
            module_slug,
            path,
            "settings metadata paths must use valid setting-key segments",
        ));
    }

    let mut spec = schema.get(root_key).ok_or_else(|| {
        invalid_schema(
            module_slug,
            path,
            format!("settings metadata path references unknown setting '{root_key}'"),
        )
    })?;

    for segment in segments {
        if !is_valid_setting_key(segment) {
            return Err(invalid_schema(
                module_slug,
                path,
                "settings metadata paths must use valid setting-key segments",
            ));
        }
        if spec.value_type.trim() == "array" {
            return Err(invalid_schema(
                module_slug,
                path,
                "localized settings cannot be declared inside arrays without stable item identity",
            ));
        }
        if spec.value_type.trim() != "object" {
            return Err(invalid_schema(
                module_slug,
                path,
                "settings metadata path cannot descend through a non-object setting",
            ));
        }
        spec = spec.properties.get(segment).ok_or_else(|| {
            invalid_schema(
                module_slug,
                path,
                format!("settings metadata path references undeclared property '{segment}'"),
            )
        })?;
    }

    Ok(spec)
}

fn setting_value_at_path<'a>(
    root: &'a serde_json::Value,
    path: &str,
) -> Option<&'a serde_json::Value> {
    path.split('.').try_fold(root, |value, segment| {
        value.as_object().and_then(|object| object.get(segment))
    })
}

fn path_is_at_or_below(path: &str, ancestor: &str) -> bool {
    path == ancestor
        || path
            .strip_prefix(ancestor)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn validate_length(
    module_slug: &str,
    key: &str,
    length: f64,
    spec: &ModuleSettingSpec,
) -> Result<(), ModuleSettingsValidationError> {
    if let Some(min) = spec.min
        && length < min
    {
        return Err(invalid_value(
            module_slug,
            key,
            format!("length must be >= {min}"),
        ));
    }
    if let Some(max) = spec.max
        && length > max
    {
        return Err(invalid_value(
            module_slug,
            key,
            format!("length must be <= {max}"),
        ));
    }
    Ok(())
}

fn declared_object_keys(spec: &ModuleSettingSpec) -> Vec<String> {
    if !spec.properties.is_empty() {
        let mut keys = spec.properties.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        keys
    } else {
        spec.object_keys.clone()
    }
}

fn declared_item_type(spec: &ModuleSettingSpec) -> Option<&str> {
    spec.items
        .as_deref()
        .map(|item| item.value_type.trim())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            spec.item_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
}

fn is_valid_setting_key(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
}

fn is_valid_setting_field_id(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_LOCALIZED_FIELD_ID_BYTES {
        return false;
    }

    value
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .as_bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && !value.contains("..")
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character == '.'
                || character == '_'
                || character == '-'
        })
}

fn is_supported_setting_type(value_type: &str) -> bool {
    matches!(
        value_type,
        "string" | "integer" | "number" | "boolean" | "object" | "array" | "json" | "any"
    )
}

fn setting_value_matches_type(value_type: &str, value: &serde_json::Value) -> bool {
    match value_type {
        "string" => value.is_string(),
        "integer" => {
            value.as_i64().is_some()
                || value.as_u64().is_some()
                || value
                    .as_f64()
                    .is_some_and(|number| number.fract().abs() < f64::EPSILON)
        }
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "json" | "any" => true,
        _ => false,
    }
}

fn invalid_schema(
    module_slug: &str,
    key: &str,
    reason: impl Into<String>,
) -> ModuleSettingsValidationError {
    ModuleSettingsValidationError::InvalidSchema {
        module_slug: module_slug.to_string(),
        key: key.to_string(),
        reason: reason.into(),
    }
}

fn invalid_value(
    module_slug: &str,
    key: impl Into<String>,
    reason: impl Into<String>,
) -> ModuleSettingsValidationError {
    ModuleSettingsValidationError::InvalidValue {
        module_slug: module_slug.to_string(),
        key: key.into(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string_spec() -> ModuleSettingSpec {
        ModuleSettingSpec {
            value_type: "string".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn legacy_struct_literal_shape_is_unchanged() {
        let _spec = ModuleSettingSpec {
            value_type: "string".to_string(),
            required: false,
            default: None,
            description: None,
            min: None,
            max: None,
            options: Vec::new(),
            object_keys: Vec::new(),
            item_type: None,
            properties: HashMap::new(),
            items: None,
        };
    }

    #[test]
    fn nested_schema_validates_real_property_keys_not_diagnostic_paths() {
        let schema = HashMap::from([(
            "hero".to_string(),
            ModuleSettingSpec {
                value_type: "object".to_string(),
                properties: HashMap::from([("title".to_string(), string_spec())]),
                ..Default::default()
            },
        )]);

        assert!(validate_module_settings_schema("storefront", &schema).is_ok());
    }

    #[test]
    fn localized_registry_inventory_and_snapshot_are_stable() {
        let schema = HashMap::from([(
            "hero".to_string(),
            ModuleSettingSpec {
                value_type: "object".to_string(),
                properties: HashMap::from([
                    ("title".to_string(), string_spec()),
                    ("url".to_string(), string_spec()),
                ]),
                ..Default::default()
            },
        )]);
        let localized_fields = BTreeMap::from([(
            "storefront.hero.title".to_string(),
            "hero.title".to_string(),
        )]);
        let sensitive_paths = BTreeSet::new();

        let fields = ModuleSettingSpec::localized_field_paths(
            "storefront",
            &schema,
            &localized_fields,
            &sensitive_paths,
        )
        .unwrap();
        assert_eq!(
            fields,
            vec![(
                "storefront.hero.title".to_string(),
                "hero.title".to_string()
            )]
        );

        let snapshot = ModuleSettingSpec::localized_value_snapshot(
            "storefront",
            &schema,
            &localized_fields,
            &sensitive_paths,
            serde_json::json!({"hero": {"title": "Hello", "url": "/sale"}}),
        )
        .unwrap();
        assert_eq!(
            snapshot.get("storefront.hero.title").map(String::as_str),
            Some("Hello")
        );
        assert_eq!(snapshot.len(), 1);
    }

    #[test]
    fn sensitive_parent_blocks_localized_descendant() {
        let schema = HashMap::from([(
            "credentials".to_string(),
            ModuleSettingSpec {
                value_type: "object".to_string(),
                properties: HashMap::from([("label".to_string(), string_spec())]),
                ..Default::default()
            },
        )]);
        let localized_fields = BTreeMap::from([(
            "payments.credentials.label".to_string(),
            "credentials.label".to_string(),
        )]);
        let sensitive_paths = BTreeSet::from(["credentials".to_string()]);

        let error = ModuleSettingSpec::validate_localization_registry(
            "payments",
            &schema,
            &localized_fields,
            &sensitive_paths,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ModuleSettingsValidationError::InvalidSchema { ref reason, .. }
                if reason.contains("sensitive path")
        ));
    }

    #[test]
    fn localized_enum_is_rejected() {
        let schema = HashMap::from([(
            "mode".to_string(),
            ModuleSettingSpec {
                value_type: "string".to_string(),
                options: vec![serde_json::json!("compact"), serde_json::json!("full")],
                ..Default::default()
            },
        )]);
        let localized_fields =
            BTreeMap::from([("checkout.mode".to_string(), "mode".to_string())]);

        let error = ModuleSettingSpec::validate_localization_registry(
            "checkout",
            &schema,
            &localized_fields,
            &BTreeSet::new(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ModuleSettingsValidationError::InvalidSchema { ref reason, .. }
                if reason.contains("enum/config")
        ));
    }

    #[test]
    fn localized_non_string_is_rejected() {
        let schema = HashMap::from([(
            "count".to_string(),
            ModuleSettingSpec {
                value_type: "integer".to_string(),
                ..Default::default()
            },
        )]);
        let localized_fields =
            BTreeMap::from([("storefront.count".to_string(), "count".to_string())]);

        let error = ModuleSettingSpec::validate_localization_registry(
            "storefront",
            &schema,
            &localized_fields,
            &BTreeSet::new(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ModuleSettingsValidationError::InvalidSchema { ref reason, .. }
                if reason.contains("string leaves")
        ));
    }

    #[test]
    fn localized_array_items_are_rejected_without_stable_item_identity() {
        let schema = HashMap::from([(
            "slides".to_string(),
            ModuleSettingSpec {
                value_type: "array".to_string(),
                items: Some(Box::new(ModuleSettingSpec {
                    value_type: "object".to_string(),
                    properties: HashMap::from([("title".to_string(), string_spec())]),
                    ..Default::default()
                })),
                ..Default::default()
            },
        )]);
        let localized_fields = BTreeMap::from([(
            "storefront.slide.title".to_string(),
            "slides.title".to_string(),
        )]);

        let error = ModuleSettingSpec::validate_localization_registry(
            "storefront",
            &schema,
            &localized_fields,
            &BTreeSet::new(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ModuleSettingsValidationError::InvalidSchema { ref reason, .. }
                if reason.contains("inside arrays")
        ));
    }

    #[test]
    fn duplicate_localized_paths_are_rejected() {
        let schema = HashMap::from([("title".to_string(), string_spec())]);
        let localized_fields = BTreeMap::from([
            ("storefront.title.primary".to_string(), "title".to_string()),
            ("storefront.title.secondary".to_string(), "title".to_string()),
        ]);

        let error = ModuleSettingSpec::validate_localization_registry(
            "storefront",
            &schema,
            &localized_fields,
            &BTreeSet::new(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ModuleSettingsValidationError::InvalidSchema { ref reason, .. }
                if reason.contains("already claimed")
        ));
    }

    #[test]
    fn localized_field_id_must_be_canonical_and_bounded() {
        let schema = HashMap::from([("title".to_string(), string_spec())]);

        for field_id in [" bad", "bad..path", ".bad", "bad.", "bad/path"] {
            let localized_fields =
                BTreeMap::from([(field_id.to_string(), "title".to_string())]);
            assert!(
                ModuleSettingSpec::validate_localization_registry(
                    "storefront",
                    &schema,
                    &localized_fields,
                    &BTreeSet::new(),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn unknown_localized_path_is_rejected() {
        let schema = HashMap::from([("title".to_string(), string_spec())]);
        let localized_fields = BTreeMap::from([(
            "storefront.subtitle".to_string(),
            "subtitle".to_string(),
        )]);

        let error = ModuleSettingSpec::validate_localization_registry(
            "storefront",
            &schema,
            &localized_fields,
            &BTreeSet::new(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ModuleSettingsValidationError::InvalidSchema { ref reason, .. }
                if reason.contains("unknown setting")
        ));
    }
}
