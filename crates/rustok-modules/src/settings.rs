//! Module settings schema validation and normalization.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

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
    for (key, spec) in schema {
        validate_setting_spec(module_slug, key, spec)?;
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
    if !is_valid_setting_key(key) {
        return Err(ModuleSettingsValidationError::InvalidKey {
            module_slug: module_slug.to_string(),
            key: key.to_string(),
        });
    }

    let value_type = spec.value_type.trim();
    if !is_supported_setting_type(value_type) {
        return Err(invalid_schema(
            module_slug,
            key,
            format!("unsupported type '{value_type}'"),
        ));
    }
    if let Some(default) = &spec.default {
        if !setting_value_matches_type(value_type, default) {
            return Err(invalid_schema(
                module_slug,
                key,
                "default does not match declared type",
            ));
        }
    }
    if let (Some(min), Some(max)) = (spec.min, spec.max) {
        if min > max {
            return Err(invalid_schema(
                module_slug,
                key,
                format!("min ({min}) must not exceed max ({max})"),
            ));
        }
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
        if let Some(default) = &spec.default {
            if !spec.options.iter().any(|option| option == default) {
                return Err(invalid_schema(
                    module_slug,
                    key,
                    "default must be one of the declared options",
                ));
            }
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
        for (property_key, property_spec) in &spec.properties {
            validate_setting_spec(module_slug, &format!("{key}.{property_key}"), property_spec)?;
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
        if let Some(item_type) = spec.item_type.as_deref() {
            if items.value_type.trim() != item_type.trim() {
                return Err(invalid_schema(
                    module_slug,
                    key,
                    "item_type must match items.type when both are provided",
                ));
            }
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
            if let Some(min) = spec.min {
                if numeric_value < min {
                    return Err(invalid_value(module_slug, key, format!("must be >= {min}")));
                }
            }
            if let Some(max) = spec.max {
                if numeric_value > max {
                    return Err(invalid_value(module_slug, key, format!("must be <= {max}")));
                }
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

fn validate_length(
    module_slug: &str,
    key: &str,
    length: f64,
    spec: &ModuleSettingSpec,
) -> Result<(), ModuleSettingsValidationError> {
    if let Some(min) = spec.min {
        if length < min {
            return Err(invalid_value(
                module_slug,
                key,
                format!("length must be >= {min}"),
            ));
        }
    }
    if let Some(max) = spec.max {
        if length > max {
            return Err(invalid_value(
                module_slug,
                key,
                format!("length must be <= {max}"),
            ));
        }
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
