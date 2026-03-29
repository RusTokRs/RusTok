//! # Flex — Custom Fields Contract
//!
//! Core types, validators, and helpers for runtime-defined custom fields.
//! This is Phase 0 of the Flex system: the portable contract shared by every
//! module that needs runtime-defined custom fields (users, products, nodes, …).
//!
//! ## Design
//!
//! Flex is a library inside `rustok-core` — like `serde`, it provides types and
//! a trait, while each consuming module owns its own tables and data.
//!
//! ## Guardrails
//!
//! | Guardrail | Value | Where enforced |
//! |-----------|-------|----------------|
//! | Max fields per entity type per tenant | 50 | `CustomFieldDefinitionService::create()` |
//! | Max JSON nesting depth (`FieldType::Json`) | [`MAX_JSON_NESTING_DEPTH`] = 2 | `validate_field_value()` |
//! | `field_key` format | `^[a-z][a-z0-9_]{0,127}$` | [`is_valid_field_key`] |
//! | Locale key format | BCP 47 short form | [`is_valid_locale_key`] |
//!
//! ### JSON depth counting — Variant A (arrays transparent)
//!
//! For `FieldType::Json`, only JSON *objects* (`{…}`) contribute to depth.
//! Arrays (`[…]`) are transparent. This means the common pattern
//! `{"items": [{"id": 1}]}` has object-depth **2** (not 3), staying within the
//! limit. Use [`json_object_depth`] to compute the depth programmatically.
//!
//! ## Usage
//!
//! ```rust
//! use rustok_core::field_schema::{
//!     CustomFieldsSchema, FieldDefinition, FieldType, ValidationRule, SelectOption,
//! };
//! use serde_json::json;
//! use std::collections::HashMap;
//!
//! let definitions = vec![
//!     FieldDefinition {
//!         field_key: "phone".to_string(),
//!         field_type: FieldType::Phone,
//!         label: [("en".to_string(), "Phone".to_string())].into_iter().collect(),
//!         description: None,
//!         is_required: false,
//!         default_value: None,
//!         validation: Some(ValidationRule {
//!             pattern: Some(r"^\+?[0-9\s\-]{7,20}$".to_string()),
//!             ..Default::default()
//!         }),
//!         position: 0,
//!         is_active: true,
//!     },
//! ];
//! let schema = CustomFieldsSchema::new(definitions);
//! let errors = schema.validate(&json!({"phone": "+1-555-0100"}));
//! assert!(errors.is_empty());
//! ```

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Guardrail regexes (compiled once)
// ---------------------------------------------------------------------------

static FIELD_KEY_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z][a-z0-9_]{0,127}$").expect("valid regex"));

static LOCALE_KEY_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z]{2}(-[A-Z]{2})?$").expect("valid regex"));

static COLOR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^#[0-9A-Fa-f]{6}$").expect("valid regex"));

// ---------------------------------------------------------------------------
// JSON nesting depth guardrail
// ---------------------------------------------------------------------------

/// Maximum allowed object-nesting depth for `FieldType::Json` values.
///
/// **Counting method: Variant A — arrays are transparent.**
///
/// Only JSON *objects* (`{…}`) create a depth level. JSON arrays (`[…]`) are
/// treated as transparent containers and do not add to the depth count.
/// This means a list of objects is counted the same as a single object at the
/// same position, which covers the common CMS pattern `{"items": [{"id": 1}]}`
/// without requiring a higher limit.
///
/// | Value | Object depth |
/// |-------|-------------|
/// | `42` / `"hello"` / `true` | 0 |
/// | `[1, 2, 3]` | 0 |
/// | `{"key": "value"}` | 1 |
/// | `{"items": [1, 2, 3]}` | 1 |
/// | `{"address": {"city": "NY"}}` | 2 ← limit |
/// | `{"items": [{"id": 1, "name": "x"}]}` | 2 ← limit (array transparent) |
/// | `{"a": {"b": {"c": 1}}}` | 3 ← **rejected** |
///
/// Rationale: keeping nesting shallow prevents `FieldType::Json` from being
/// used as a full document store, which would bypass the schema system and
/// hurt JSONB query performance. For deeper structures, a proper entity/table
/// is the right tool.
pub const MAX_JSON_NESTING_DEPTH: usize = 2;

/// Compute the maximum object-nesting depth of a JSON value.
///
/// Arrays do **not** contribute to depth (Variant A). See [`MAX_JSON_NESTING_DEPTH`]
/// for the rationale and a full example table.
///
/// ```rust
/// use rustok_core::field_schema::json_object_depth;
/// use serde_json::json;
///
/// assert_eq!(json_object_depth(&json!(42)), 0);
/// assert_eq!(json_object_depth(&json!([1, 2, 3])), 0);
/// assert_eq!(json_object_depth(&json!({"key": "value"})), 1);
/// assert_eq!(json_object_depth(&json!({"items": [1, 2, 3]})), 1);
/// assert_eq!(json_object_depth(&json!({"address": {"city": "NY"}})), 2);
/// assert_eq!(json_object_depth(&json!({"items": [{"id": 1}]})), 2);
/// assert_eq!(json_object_depth(&json!({"a": {"b": {"c": 1}}})), 3);
/// ```
pub fn json_object_depth(value: &serde_json::Value) -> usize {
    match value {
        // Each object adds exactly one level; recurse into values.
        serde_json::Value::Object(map) => {
            1 + map.values().map(json_object_depth).max().unwrap_or(0)
        }
        // Arrays are transparent: their depth equals the deepest element's depth.
        serde_json::Value::Array(arr) => arr.iter().map(json_object_depth).max().unwrap_or(0),
        // Primitives (string, number, bool, null) have depth 0.
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// FieldType
// ---------------------------------------------------------------------------

/// Supported field types for custom fields.
/// Shared platform contract — used by any module that needs
/// runtime-defined field types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    /// Single-line text
    Text,
    /// Multi-line text
    Textarea,
    /// Integer number (i64)
    Integer,
    /// Decimal number (f64)
    Decimal,
    /// true/false
    Boolean,
    /// ISO 8601 date (YYYY-MM-DD)
    Date,
    /// ISO 8601 date-time
    DateTime,
    /// URL (format validated)
    Url,
    /// Email (format validated)
    Email,
    /// Phone (free-form with optional regex pattern)
    Phone,
    /// Single select from options list
    Select,
    /// Multi select from options list
    MultiSelect,
    /// Color hex (#RRGGBB)
    Color,
    /// Arbitrary JSON (any value accepted)
    Json,
}

impl FieldType {
    /// Returns `true` if this type requires `options` in [`ValidationRule`].
    pub fn requires_options(&self) -> bool {
        matches!(self, Self::Select | Self::MultiSelect)
    }

    /// Returns `true` if `min`/`max` in [`ValidationRule`] applies to string
    /// length rather than numeric value. For [`FieldType::MultiSelect`] min/max
    /// instead applies to the number of selected items.
    pub fn min_max_is_length(&self) -> bool {
        matches!(
            self,
            Self::Text | Self::Textarea | Self::Url | Self::Email | Self::Phone
        )
    }

    /// Returns `true` if this type supports regex `pattern` validation.
    pub fn supports_pattern(&self) -> bool {
        matches!(self, Self::Text | Self::Textarea | Self::Phone)
    }
}

// ---------------------------------------------------------------------------
// ValidationRule & SelectOption
// ---------------------------------------------------------------------------

/// Validation constraints attached to a [`FieldDefinition`].
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValidationRule {
    /// Minimum value (numeric) or minimum length (string/array).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Maximum value (numeric) or maximum length (string/array).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Regex pattern (Text, Textarea, Phone only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Allowed options (Select, MultiSelect only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<SelectOption>>,
    /// Localized custom error message override: `{"en": "…", "ru": "…"}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<HashMap<String, String>>,
}

/// A single selectable option for [`FieldType::Select`] / [`FieldType::MultiSelect`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    /// Machine-readable value stored in metadata.
    pub value: String,
    /// Localized display labels: `{"en": "Male", "ru": "Мужской"}`.
    pub label: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// FieldDefinition
// ---------------------------------------------------------------------------

fn default_true() -> bool {
    true
}

/// Runtime field definition — portable DTO.
///
/// DB rows, config files, and JSONB field configs can all be converted into
/// this struct. It is the single source of truth for how a custom field
/// behaves at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    /// Snake-case key used in metadata JSON. Must match `^[a-z][a-z0-9_]{0,127}$`.
    pub field_key: String,
    /// Type of the field value.
    pub field_type: FieldType,
    /// Localized display label: `{"en": "Phone", "ru": "Телефон"}`.
    pub label: HashMap<String, String>,
    /// Optional localized description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<HashMap<String, String>>,
    /// Whether a non-null value must be present in metadata.
    #[serde(default)]
    pub is_required: bool,
    /// Value inserted by [`CustomFieldsSchema::apply_defaults`] when the key is absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
    /// Optional validation constraints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationRule>,
    /// Display order (ascending).
    #[serde(default)]
    pub position: i32,
    /// Inactive fields are excluded from validation and defaults.
    #[serde(default = "default_true")]
    pub is_active: bool,
}

// ---------------------------------------------------------------------------
// HasCustomFields trait
// ---------------------------------------------------------------------------

/// Trait for entities that support custom fields via a `metadata` JSONB column.
///
/// Each consuming module implements this for its own entity models, e.g.:
///
/// ```rust,ignore
/// impl HasCustomFields for User {
///     fn entity_type() -> &'static str { "user" }
///     fn metadata(&self) -> &serde_json::Value { &self.metadata }
///     fn set_metadata(&mut self, value: serde_json::Value) { self.metadata = value; }
/// }
/// ```
pub trait HasCustomFields {
    /// Entity type key used for routing and table naming, e.g. `"user"`, `"product"`.
    fn entity_type() -> &'static str;

    /// Current metadata value.
    fn metadata(&self) -> &serde_json::Value;

    /// Replace the metadata value.
    fn set_metadata(&mut self, value: serde_json::Value);
}

// ---------------------------------------------------------------------------
// FieldErrorCode & FieldValidationError
// ---------------------------------------------------------------------------

/// Machine-readable error codes returned by [`CustomFieldsSchema::validate`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldErrorCode {
    /// Required field is absent or null.
    Required,
    /// Value has wrong JSON type.
    InvalidType,
    /// String or array is shorter than `min`.
    TooShort,
    /// String or array is longer than `max`.
    TooLong,
    /// Numeric value is below `min`.
    BelowMinimum,
    /// Numeric value is above `max`.
    AboveMaximum,
    /// Value does not match the `pattern` regex.
    PatternMismatch,
    /// Value is not in the allowed `options` list.
    InvalidOption,
    /// Value does not satisfy format constraints (URL, email, date, color, …).
    InvalidFormat,
    /// `FieldType::Json` value exceeds the maximum allowed object-nesting depth.
    ///
    /// See [`MAX_JSON_NESTING_DEPTH`] and [`json_object_depth`] for the exact
    /// counting rules (Variant A — arrays are transparent).
    NestingTooDeep,
}

/// A single field-level validation error produced by [`CustomFieldsSchema::validate`].
#[derive(Debug, Clone, Serialize)]
pub struct FieldValidationError {
    /// The field key that failed validation.
    pub field_key: String,
    /// Human-readable error description (English).
    pub message: String,
    /// Machine-readable error code.
    pub error_code: FieldErrorCode,
}

// ---------------------------------------------------------------------------
// CustomFieldsSchema
// ---------------------------------------------------------------------------

/// Schema-based validator constructed from a list of [`FieldDefinition`]s.
///
/// The definitions can come from any source: a database table, a config file,
/// or a JSONB column. The schema is stateless and cheap to clone.
pub struct CustomFieldsSchema {
    definitions: Vec<FieldDefinition>,
}

impl CustomFieldsSchema {
    /// Construct a schema from the given field definitions.
    pub fn new(definitions: Vec<FieldDefinition>) -> Self {
        Self { definitions }
    }

    /// Validate `metadata` against the schema.
    ///
    /// Returns an empty `Vec` when valid. Each entry in the returned `Vec`
    /// represents one field-level failure.
    ///
    /// Only active definitions are checked. A `null` JSON value is treated the
    /// same as an absent key.
    pub fn validate(&self, metadata: &serde_json::Value) -> Vec<FieldValidationError> {
        let obj = match metadata.as_object() {
            Some(o) => o,
            None => return vec![],
        };

        let mut errors = Vec::new();

        for def in self.active_definitions() {
            let value = obj.get(&def.field_key);

            match value {
                None | Some(serde_json::Value::Null) => {
                    if def.is_required {
                        errors.push(FieldValidationError {
                            field_key: def.field_key.clone(),
                            message: format!("Field '{}' is required", def.field_key),
                            error_code: FieldErrorCode::Required,
                        });
                    }
                }
                Some(v) => {
                    errors.extend(validate_field_value(def, v));
                }
            }
        }

        errors
    }

    /// Fill in [`FieldDefinition::default_value`] for any key that is absent or
    /// `null` in `metadata`.
    ///
    /// Only active definitions with a `default_value` are considered.
    pub fn apply_defaults(&self, metadata: &mut serde_json::Value) {
        let obj = match metadata.as_object_mut() {
            Some(o) => o,
            None => return,
        };

        for def in &self.definitions {
            if !def.is_active {
                continue;
            }
            if let Some(default) = &def.default_value {
                obj.entry(def.field_key.clone())
                    .or_insert_with(|| default.clone());
            }
        }
    }

    /// Remove all keys from `metadata` that are not defined in the schema.
    pub fn strip_unknown(&self, metadata: &mut serde_json::Value) {
        let obj = match metadata.as_object_mut() {
            Some(o) => o,
            None => return,
        };

        let known: std::collections::HashSet<&str> = self
            .definitions
            .iter()
            .map(|d| d.field_key.as_str())
            .collect();

        obj.retain(|k, _| known.contains(k.as_str()));
    }

    /// Returns references to all active definitions in position order.
    pub fn active_definitions(&self) -> Vec<&FieldDefinition> {
        self.definitions.iter().filter(|d| d.is_active).collect()
    }
}

// ---------------------------------------------------------------------------
// validate_field_value — internal, all 14 types
// ---------------------------------------------------------------------------

/// Validate a single non-null field value against its definition.
/// Returns zero or more errors.
fn validate_field_value(
    def: &FieldDefinition,
    value: &serde_json::Value,
) -> Vec<FieldValidationError> {
    let mut errors = Vec::new();
    let key = &def.field_key;
    let rule = def.validation.as_ref();

    match def.field_type {
        // ── Text / Textarea / Phone ──────────────────────────────────────────
        FieldType::Text | FieldType::Textarea | FieldType::Phone => {
            let s = match value.as_str() {
                Some(s) => s,
                None => {
                    errors.push(make_invalid_type(key, "string"));
                    return errors;
                }
            };

            if let Some(r) = rule {
                let char_count = s.chars().count() as f64;

                if let Some(min) = r.min {
                    if char_count < min {
                        errors.push(FieldValidationError {
                            field_key: key.clone(),
                            message: format!(
                                "Field '{}' is too short (minimum {} characters)",
                                key, min as usize
                            ),
                            error_code: FieldErrorCode::TooShort,
                        });
                    }
                }

                if let Some(max) = r.max {
                    if char_count > max {
                        errors.push(FieldValidationError {
                            field_key: key.clone(),
                            message: format!(
                                "Field '{}' is too long (maximum {} characters)",
                                key, max as usize
                            ),
                            error_code: FieldErrorCode::TooLong,
                        });
                    }
                }

                if let Some(pattern) = &r.pattern {
                    if let Ok(re) = Regex::new(pattern) {
                        if !re.is_match(s) {
                            errors.push(FieldValidationError {
                                field_key: key.clone(),
                                message: format!(
                                    "Field '{}' does not match the required pattern",
                                    key
                                ),
                                error_code: FieldErrorCode::PatternMismatch,
                            });
                        }
                    } // Invalid pattern in definition — skip silently
                }
            }
        }

        // ── Url ──────────────────────────────────────────────────────────────
        FieldType::Url => {
            let s = match value.as_str() {
                Some(s) => s,
                None => {
                    errors.push(make_invalid_type(key, "string"));
                    return errors;
                }
            };

            if url::Url::parse(s).is_err() {
                errors.push(FieldValidationError {
                    field_key: key.clone(),
                    message: format!("Field '{}' is not a valid URL", key),
                    error_code: FieldErrorCode::InvalidFormat,
                });
            }

            if let Some(r) = rule {
                let len = s.len() as f64;
                if let Some(min) = r.min {
                    if len < min {
                        errors.push(FieldValidationError {
                            field_key: key.clone(),
                            message: format!("Field '{}' is too short", key),
                            error_code: FieldErrorCode::TooShort,
                        });
                    }
                }
                if let Some(max) = r.max {
                    if len > max {
                        errors.push(FieldValidationError {
                            field_key: key.clone(),
                            message: format!("Field '{}' is too long", key),
                            error_code: FieldErrorCode::TooLong,
                        });
                    }
                }
            }
        }

        // ── Email ─────────────────────────────────────────────────────────────
        FieldType::Email => {
            let s = match value.as_str() {
                Some(s) => s,
                None => {
                    errors.push(make_invalid_type(key, "string"));
                    return errors;
                }
            };

            if !email_address::EmailAddress::is_valid(s) {
                errors.push(FieldValidationError {
                    field_key: key.clone(),
                    message: format!("Field '{}' is not a valid email address", key),
                    error_code: FieldErrorCode::InvalidFormat,
                });
            }

            if let Some(r) = rule {
                let len = s.len() as f64;
                if let Some(min) = r.min {
                    if len < min {
                        errors.push(FieldValidationError {
                            field_key: key.clone(),
                            message: format!("Field '{}' is too short", key),
                            error_code: FieldErrorCode::TooShort,
                        });
                    }
                }
                if let Some(max) = r.max {
                    if len > max {
                        errors.push(FieldValidationError {
                            field_key: key.clone(),
                            message: format!("Field '{}' is too long", key),
                            error_code: FieldErrorCode::TooLong,
                        });
                    }
                }
            }
        }

        // ── Integer ───────────────────────────────────────────────────────────
        FieldType::Integer => {
            // Accept i64 directly, or f64 with no fractional part
            let n: f64 = match value.as_i64() {
                Some(n) => n as f64,
                None => match value.as_f64() {
                    Some(f) if f.fract() == 0.0 => f,
                    _ => {
                        errors.push(make_invalid_type(key, "integer"));
                        return errors;
                    }
                },
            };

            if let Some(r) = rule {
                if let Some(min) = r.min {
                    if n < min {
                        errors.push(FieldValidationError {
                            field_key: key.clone(),
                            message: format!("Field '{}' must be at least {}", key, min as i64),
                            error_code: FieldErrorCode::BelowMinimum,
                        });
                    }
                }
                if let Some(max) = r.max {
                    if n > max {
                        errors.push(FieldValidationError {
                            field_key: key.clone(),
                            message: format!("Field '{}' must be at most {}", key, max as i64),
                            error_code: FieldErrorCode::AboveMaximum,
                        });
                    }
                }
            }
        }

        // ── Decimal ───────────────────────────────────────────────────────────
        FieldType::Decimal => {
            let n = match value.as_f64() {
                Some(n) => n,
                None => {
                    errors.push(make_invalid_type(key, "number"));
                    return errors;
                }
            };

            if let Some(r) = rule {
                if let Some(min) = r.min {
                    if n < min {
                        errors.push(FieldValidationError {
                            field_key: key.clone(),
                            message: format!("Field '{}' must be at least {}", key, min),
                            error_code: FieldErrorCode::BelowMinimum,
                        });
                    }
                }
                if let Some(max) = r.max {
                    if n > max {
                        errors.push(FieldValidationError {
                            field_key: key.clone(),
                            message: format!("Field '{}' must be at most {}", key, max),
                            error_code: FieldErrorCode::AboveMaximum,
                        });
                    }
                }
            }
        }

        // ── Boolean ───────────────────────────────────────────────────────────
        FieldType::Boolean => {
            if !value.is_boolean() {
                errors.push(make_invalid_type(key, "boolean"));
            }
        }

        // ── Date ──────────────────────────────────────────────────────────────
        FieldType::Date => {
            let s = match value.as_str() {
                Some(s) => s,
                None => {
                    errors.push(make_invalid_type(key, "string (ISO date YYYY-MM-DD)"));
                    return errors;
                }
            };

            if chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_err() {
                errors.push(FieldValidationError {
                    field_key: key.clone(),
                    message: format!(
                        "Field '{}' is not a valid ISO 8601 date (expected YYYY-MM-DD)",
                        key
                    ),
                    error_code: FieldErrorCode::InvalidFormat,
                });
            }
        }

        // ── DateTime ──────────────────────────────────────────────────────────
        FieldType::DateTime => {
            let s = match value.as_str() {
                Some(s) => s,
                None => {
                    errors.push(make_invalid_type(key, "string (ISO datetime)"));
                    return errors;
                }
            };

            let valid = chrono::DateTime::parse_from_rfc3339(s).is_ok()
                || chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").is_ok()
                || chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f").is_ok();

            if !valid {
                errors.push(FieldValidationError {
                    field_key: key.clone(),
                    message: format!("Field '{}' is not a valid ISO 8601 datetime", key),
                    error_code: FieldErrorCode::InvalidFormat,
                });
            }
        }

        // ── Select ────────────────────────────────────────────────────────────
        FieldType::Select => {
            let s = match value.as_str() {
                Some(s) => s,
                None => {
                    errors.push(make_invalid_type(key, "string"));
                    return errors;
                }
            };

            if let Some(r) = rule {
                if let Some(options) = &r.options {
                    if !options.iter().any(|o| o.value == s) {
                        errors.push(FieldValidationError {
                            field_key: key.clone(),
                            message: format!("Field '{}' has an invalid option value '{}'", key, s),
                            error_code: FieldErrorCode::InvalidOption,
                        });
                    }
                }
            }
        }

        // ── MultiSelect ───────────────────────────────────────────────────────
        FieldType::MultiSelect => {
            let arr = match value.as_array() {
                Some(a) => a,
                None => {
                    errors.push(make_invalid_type(key, "array"));
                    return errors;
                }
            };

            // Each element must be a string
            for (i, item) in arr.iter().enumerate() {
                if !item.is_string() {
                    errors.push(FieldValidationError {
                        field_key: key.clone(),
                        message: format!("Field '{}' item at index {} must be a string", key, i),
                        error_code: FieldErrorCode::InvalidType,
                    });
                }
            }

            if let Some(r) = rule {
                let count = arr.len() as f64;

                if let Some(min) = r.min {
                    if count < min {
                        errors.push(FieldValidationError {
                            field_key: key.clone(),
                            message: format!(
                                "Field '{}' requires at least {} selection(s)",
                                key, min as usize
                            ),
                            error_code: FieldErrorCode::TooShort,
                        });
                    }
                }

                if let Some(max) = r.max {
                    if count > max {
                        errors.push(FieldValidationError {
                            field_key: key.clone(),
                            message: format!(
                                "Field '{}' allows at most {} selection(s)",
                                key, max as usize
                            ),
                            error_code: FieldErrorCode::TooLong,
                        });
                    }
                }

                if let Some(options) = &r.options {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            if !options.iter().any(|o| o.value == s) {
                                errors.push(FieldValidationError {
                                    field_key: key.clone(),
                                    message: format!(
                                        "Field '{}' has an invalid option value '{}'",
                                        key, s
                                    ),
                                    error_code: FieldErrorCode::InvalidOption,
                                });
                            }
                        }
                    }
                }
            }
        }

        // ── Color ─────────────────────────────────────────────────────────────
        FieldType::Color => {
            let s = match value.as_str() {
                Some(s) => s,
                None => {
                    errors.push(make_invalid_type(key, "string (#RRGGBB)"));
                    return errors;
                }
            };

            if !COLOR_REGEX.is_match(s) {
                errors.push(FieldValidationError {
                    field_key: key.clone(),
                    message: format!(
                        "Field '{}' must be a valid hex color in #RRGGBB format",
                        key
                    ),
                    error_code: FieldErrorCode::InvalidFormat,
                });
            }
        }

        // ── Json ──────────────────────────────────────────────────────────────
        FieldType::Json => {
            // Accept any JSON type, but enforce the object-nesting depth limit.
            // Arrays are transparent (Variant A) — see `json_object_depth` and
            // `MAX_JSON_NESTING_DEPTH` for full rationale and examples.
            let depth = json_object_depth(value);
            if depth > MAX_JSON_NESTING_DEPTH {
                errors.push(FieldValidationError {
                    field_key: key.clone(),
                    message: format!(
                        "Field '{}' exceeds maximum JSON nesting depth \
                         ({} object levels, limit is {}). \
                         Arrays are transparent — only {{…}} objects count. \
                         Flatten the structure or use a dedicated entity.",
                        key, depth, MAX_JSON_NESTING_DEPTH
                    ),
                    error_code: FieldErrorCode::NestingTooDeep,
                });
            }
        }
    }

    errors
}

/// Convenience constructor for an `InvalidType` error.
fn make_invalid_type(key: &str, expected: &str) -> FieldValidationError {
    FieldValidationError {
        field_key: key.to_string(),
        message: format!("Field '{}' must be a {}", key, expected),
        error_code: FieldErrorCode::InvalidType,
    }
}

// ---------------------------------------------------------------------------
// Guardrail validation helpers (public)
// ---------------------------------------------------------------------------

/// Returns `true` if `key` matches the required field key format:
/// `^[a-z][a-z0-9_]{0,127}$`
pub fn is_valid_field_key(key: &str) -> bool {
    FIELD_KEY_REGEX.is_match(key)
}

/// Returns `true` if `key` is a valid BCP 47 short locale key:
/// `^[a-z]{2}(-[A-Z]{2})?$`
pub fn is_valid_locale_key(key: &str) -> bool {
    LOCALE_KEY_REGEX.is_match(key)
}

// ---------------------------------------------------------------------------
// FlexError — service-layer error type
// ---------------------------------------------------------------------------

/// Top-level error type for Flex operations (field definition CRUD, validation).
#[derive(Debug)]
pub enum FlexError {
    /// Requested entity type is not registered.
    UnknownEntityType(String),
    /// Too many field definitions for the entity type in this tenant.
    TooManyFields { entity_type: String, max: usize },
    /// The provided field key does not match `^[a-z][a-z0-9_]{0,127}$`.
    InvalidFieldKey(String),
    /// A field with this key already exists for the entity type + tenant.
    DuplicateFieldKey(String),
    /// Field definition not found.
    NotFound(uuid::Uuid),
    /// Metadata validation failed.
    ValidationFailed(Vec<FieldValidationError>),
    /// Underlying database error.
    Database(String),
}

impl std::fmt::Display for FlexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownEntityType(t) => write!(f, "Unknown entity type: {t}"),
            Self::TooManyFields { entity_type, max } => {
                write!(
                    f,
                    "Too many field definitions for {entity_type} (max {max})"
                )
            }
            Self::InvalidFieldKey(k) => write!(f, "Invalid field key: {k}"),
            Self::DuplicateFieldKey(k) => write!(f, "Field key already exists: {k}"),
            Self::NotFound(id) => write!(f, "Field definition not found: {id}"),
            Self::ValidationFailed(_) => write!(f, "Custom field validation failed"),
            Self::Database(e) => write!(f, "Database error: {e}"),
        }
    }
}

impl std::error::Error for FlexError {}

// ---------------------------------------------------------------------------
// JSONB query helpers — Phase 1
// ---------------------------------------------------------------------------

/// Builds a SQL condition equivalent to `metadata->>'key' = 'value'` for PostgreSQL JSONB.
///
/// `column` should point to a JSON/JSONB column, typically `Column::Metadata`.
pub fn json_field_eq(
    column: impl Into<sea_orm::sea_query::SimpleExpr>,
    key: &str,
    value: &str,
) -> sea_orm::Condition {
    use sea_orm::sea_query::{Expr, Value};

    let column = Expr::expr(column.into());
    let expr = Expr::cust_with_exprs(
        "$1->>$2 = $3",
        [
            column.into(),
            Expr::value(Value::String(Some(Box::new(key.to_string())))),
            Expr::value(Value::String(Some(Box::new(value.to_string())))),
        ],
    );

    sea_orm::Condition::all().add(expr)
}

/// Builds a SQL condition equivalent to `metadata ? 'key'` (key existence in JSONB object).
pub fn json_field_exists(
    column: impl Into<sea_orm::sea_query::SimpleExpr>,
    key: &str,
) -> sea_orm::Condition {
    use sea_orm::sea_query::{Expr, Value};

    let column = Expr::expr(column.into());
    let expr = Expr::cust_with_exprs(
        "$1 ? $2",
        [
            column.into(),
            Expr::value(Value::String(Some(Box::new(key.to_string())))),
        ],
    );

    sea_orm::Condition::all().add(expr)
}

/// Builds a SQL expression equivalent to `metadata->>'key'` for custom ORDER BY / projection.
pub fn json_field_extract(
    column: impl Into<sea_orm::sea_query::SimpleExpr>,
    key: &str,
) -> sea_orm::sea_query::SimpleExpr {
    use sea_orm::sea_query::{Expr, Value};

    let column = Expr::expr(column.into());
    Expr::cust_with_exprs(
        "$1->>$2",
        [
            column.into(),
            Expr::value(Value::String(Some(Box::new(key.to_string())))),
        ],
    )
}

/// Builds a SQL condition equivalent to `metadata @> '{"key": value}'::jsonb`.
pub fn json_field_contains(
    column: impl Into<sea_orm::sea_query::SimpleExpr>,
    key: &str,
    value: serde_json::Value,
) -> sea_orm::Condition {
    use sea_orm::sea_query::{Expr, Value};

    let payload = serde_json::json!({ key: value });
    let payload = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    let column = Expr::expr(column.into());

    let expr = Expr::cust_with_exprs(
        "$1 @> $2::jsonb",
        [
            column.into(),
            Expr::value(Value::String(Some(Box::new(payload)))),
        ],
    );

    sea_orm::Condition::all().add(expr)
}

// ---------------------------------------------------------------------------
// Migration helpers — Phase 1
// ---------------------------------------------------------------------------

/// Creates a `{prefix}_field_definitions` table in any module's migration.
///
/// # Column layout
///
/// | Column | Type | Notes |
/// |---|---|---|
/// | id | UUID PK | |
/// | tenant_id | UUID NOT NULL | tenant isolation |
/// | field_key | VARCHAR(128) NOT NULL | snake_case |
/// | field_type | VARCHAR(32) NOT NULL | FieldType serialised |
/// | label | JSONB NOT NULL | `{"en": "…"}` |
/// | description | JSONB | nullable |
/// | is_required | BOOLEAN NOT NULL DEFAULT false | |
/// | default_value | JSONB | nullable |
/// | validation | JSONB | nullable |
/// | position | INTEGER NOT NULL DEFAULT 0 | |
/// | is_active | BOOLEAN NOT NULL DEFAULT true | |
/// | created_at | TIMESTAMPTZ NOT NULL DEFAULT now() | |
/// | updated_at | TIMESTAMPTZ NOT NULL DEFAULT now() | |
///
/// # Indexes created
///
/// * `UNIQUE (tenant_id, field_key)` — prevents duplicates per tenant
/// * `idx_{prefix}_fd_tenant_active (tenant_id, is_active)` — fast active-field queries
///
/// # Example
///
/// ```rust,ignore
/// use rustok_core::field_schema::create_field_definitions_table;
///
/// async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
///     create_field_definitions_table(manager, "user", "users").await
/// }
/// ```
pub async fn create_field_definitions_table(
    manager: &sea_orm_migration::prelude::SchemaManager<'_>,
    prefix: &str,
    _parent_table: &str,
) -> Result<(), sea_orm_migration::prelude::DbErr> {
    use sea_orm_migration::prelude::*;

    let table_name = format!("{prefix}_field_definitions");

    manager
        .create_table(
            Table::create()
                .table(Alias::new(&table_name))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                .col(
                    ColumnDef::new(Alias::new("field_key"))
                        .string_len(128)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Alias::new("field_type"))
                        .string_len(32)
                        .not_null(),
                )
                .col(ColumnDef::new(Alias::new("label")).json_binary().not_null())
                .col(ColumnDef::new(Alias::new("description")).json_binary())
                .col(
                    ColumnDef::new(Alias::new("is_required"))
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(ColumnDef::new(Alias::new("default_value")).json_binary())
                .col(ColumnDef::new(Alias::new("validation")).json_binary())
                .col(
                    ColumnDef::new(Alias::new("position"))
                        .integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(Alias::new("is_active"))
                        .boolean()
                        .not_null()
                        .default(true),
                )
                .col(
                    ColumnDef::new(Alias::new("created_at"))
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(Alias::new("updated_at"))
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .to_owned(),
        )
        .await?;

    // UNIQUE (tenant_id, field_key) — prevents duplicates per tenant
    manager
        .create_index(
            Index::create()
                .name(format!("uq_{prefix}_fd_tenant_key"))
                .table(Alias::new(&table_name))
                .col(Alias::new("tenant_id"))
                .col(Alias::new("field_key"))
                .unique()
                .to_owned(),
        )
        .await?;

    // idx_{prefix}_fd_tenant_active — fast active-field queries
    manager
        .create_index(
            Index::create()
                .name(format!("idx_{prefix}_fd_tenant_active"))
                .table(Alias::new(&table_name))
                .col(Alias::new("tenant_id"))
                .col(Alias::new("is_active"))
                .to_owned(),
        )
        .await
}

/// Drops the `{prefix}_field_definitions` table created by
/// [`create_field_definitions_table`].
pub async fn drop_field_definitions_table(
    manager: &sea_orm_migration::prelude::SchemaManager<'_>,
    prefix: &str,
) -> Result<(), sea_orm_migration::prelude::DbErr> {
    use sea_orm_migration::prelude::*;

    let table_name = format!("{prefix}_field_definitions");

    manager
        .drop_table(Table::drop().table(Alias::new(&table_name)).to_owned())
        .await
}

/// Generates a SeaORM entity model for a `*_field_definitions` table.
///
/// # Example
///
/// ```rust,ignore
/// rustok_core::define_field_definitions_entity!("user_field_definitions");
/// ```
#[macro_export]
macro_rules! define_field_definitions_entity {
    ($table_name:literal) => {
        use sea_orm::entity::prelude::*;

        #[derive(
            Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize,
        )]
        #[sea_orm(table_name = $table_name)]
        pub struct Model {
            #[sea_orm(primary_key, auto_increment = false)]
            pub id: sea_orm::entity::prelude::Uuid,
            pub tenant_id: sea_orm::entity::prelude::Uuid,
            pub field_key: String,
            pub field_type: String,
            pub label: sea_orm::entity::prelude::Json,
            pub description: Option<sea_orm::entity::prelude::Json>,
            pub is_required: bool,
            pub default_value: Option<sea_orm::entity::prelude::Json>,
            pub validation: Option<sea_orm::entity::prelude::Json>,
            pub position: i32,
            pub is_active: bool,
            pub created_at: sea_orm::entity::prelude::DateTimeWithTimeZone,
            pub updated_at: sea_orm::entity::prelude::DateTimeWithTimeZone,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    };
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn text_def(key: &str, required: bool, rule: Option<ValidationRule>) -> FieldDefinition {
        FieldDefinition {
            field_key: key.to_string(),
            field_type: FieldType::Text,
            label: [("en".to_string(), key.to_string())].into_iter().collect(),
            description: None,
            is_required: required,
            default_value: None,
            validation: rule,
            position: 0,
            is_active: true,
        }
    }

    fn typed_def(
        key: &str,
        field_type: FieldType,
        required: bool,
        rule: Option<ValidationRule>,
    ) -> FieldDefinition {
        FieldDefinition {
            field_key: key.to_string(),
            field_type,
            label: [("en".to_string(), key.to_string())].into_iter().collect(),
            description: None,
            is_required: required,
            default_value: None,
            validation: rule,
            position: 0,
            is_active: true,
        }
    }

    fn options(vals: &[&str]) -> Vec<SelectOption> {
        vals.iter()
            .map(|v| SelectOption {
                value: v.to_string(),
                label: [("en".to_string(), v.to_string())].into_iter().collect(),
            })
            .collect()
    }

    // ── Required ─────────────────────────────────────────────────────────────

    #[test]
    fn validate_required_field_missing() {
        let schema = CustomFieldsSchema::new(vec![text_def("name", true, None)]);
        let errors = schema.validate(&json!({}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::Required);
    }

    #[test]
    fn validate_required_field_present() {
        let schema = CustomFieldsSchema::new(vec![text_def("name", true, None)]);
        let errors = schema.validate(&json!({"name": "Alice"}));
        assert!(errors.is_empty());
    }

    // ── Text ─────────────────────────────────────────────────────────────────

    #[test]
    fn validate_text_min_length() {
        let rule = ValidationRule {
            min: Some(5.0),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![text_def("bio", false, Some(rule))]);
        let errors = schema.validate(&json!({"bio": "hi"}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::TooShort);
    }

    #[test]
    fn validate_text_max_length() {
        let rule = ValidationRule {
            max: Some(3.0),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![text_def("tag", false, Some(rule))]);
        let errors = schema.validate(&json!({"tag": "toolong"}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::TooLong);
    }

    #[test]
    fn validate_text_pattern_match() {
        let rule = ValidationRule {
            pattern: Some(r"^\d{4}$".to_string()),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![text_def("code", false, Some(rule))]);
        let errors = schema.validate(&json!({"code": "1234"}));
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_text_pattern_mismatch() {
        let rule = ValidationRule {
            pattern: Some(r"^\d{4}$".to_string()),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![text_def("code", false, Some(rule))]);
        let errors = schema.validate(&json!({"code": "abcd"}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::PatternMismatch);
    }

    // ── Integer ───────────────────────────────────────────────────────────────

    #[test]
    fn validate_integer_in_range() {
        let rule = ValidationRule {
            min: Some(1.0),
            max: Some(100.0),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![typed_def(
            "age",
            FieldType::Integer,
            false,
            Some(rule),
        )]);
        let errors = schema.validate(&json!({"age": 42}));
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_integer_below_minimum() {
        let rule = ValidationRule {
            min: Some(18.0),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![typed_def(
            "age",
            FieldType::Integer,
            false,
            Some(rule),
        )]);
        let errors = schema.validate(&json!({"age": 10}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::BelowMinimum);
    }

    #[test]
    fn validate_integer_above_maximum() {
        let rule = ValidationRule {
            max: Some(100.0),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![typed_def(
            "score",
            FieldType::Integer,
            false,
            Some(rule),
        )]);
        let errors = schema.validate(&json!({"score": 150}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::AboveMaximum);
    }

    // ── Decimal ───────────────────────────────────────────────────────────────

    #[test]
    fn validate_decimal_precision() {
        let rule = ValidationRule {
            min: Some(0.0),
            max: Some(1.0),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![typed_def(
            "ratio",
            FieldType::Decimal,
            false,
            Some(rule),
        )]);
        // In range
        assert!(schema.validate(&json!({"ratio": 0.75})).is_empty());
        // Above max
        let errors = schema.validate(&json!({"ratio": 1.5}));
        assert_eq!(errors[0].error_code, FieldErrorCode::AboveMaximum);
    }

    // ── Select ────────────────────────────────────────────────────────────────

    #[test]
    fn validate_select_valid_option() {
        let rule = ValidationRule {
            options: Some(options(&["red", "green", "blue"])),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![typed_def(
            "color",
            FieldType::Select,
            false,
            Some(rule),
        )]);
        assert!(schema.validate(&json!({"color": "red"})).is_empty());
    }

    #[test]
    fn validate_select_invalid_option() {
        let rule = ValidationRule {
            options: Some(options(&["red", "green", "blue"])),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![typed_def(
            "color",
            FieldType::Select,
            false,
            Some(rule),
        )]);
        let errors = schema.validate(&json!({"color": "yellow"}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::InvalidOption);
    }

    // ── MultiSelect ───────────────────────────────────────────────────────────

    #[test]
    fn validate_multiselect_valid() {
        let rule = ValidationRule {
            options: Some(options(&["a", "b", "c"])),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![typed_def(
            "tags",
            FieldType::MultiSelect,
            false,
            Some(rule),
        )]);
        assert!(schema.validate(&json!({"tags": ["a", "b"]})).is_empty());
    }

    #[test]
    fn validate_multiselect_invalid_option() {
        let rule = ValidationRule {
            options: Some(options(&["a", "b", "c"])),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![typed_def(
            "tags",
            FieldType::MultiSelect,
            false,
            Some(rule),
        )]);
        let errors = schema.validate(&json!({"tags": ["a", "z"]}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::InvalidOption);
    }

    #[test]
    fn validate_multiselect_too_many() {
        let rule = ValidationRule {
            max: Some(2.0),
            options: Some(options(&["a", "b", "c", "d"])),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![typed_def(
            "tags",
            FieldType::MultiSelect,
            false,
            Some(rule),
        )]);
        let errors = schema.validate(&json!({"tags": ["a", "b", "c"]}));
        assert!(errors
            .iter()
            .any(|e| e.error_code == FieldErrorCode::TooLong));
    }

    // ── Email ─────────────────────────────────────────────────────────────────

    #[test]
    fn validate_email_valid() {
        let schema =
            CustomFieldsSchema::new(vec![typed_def("email", FieldType::Email, false, None)]);
        assert!(schema
            .validate(&json!({"email": "user@example.com"}))
            .is_empty());
    }

    #[test]
    fn validate_email_invalid() {
        let schema =
            CustomFieldsSchema::new(vec![typed_def("email", FieldType::Email, false, None)]);
        let errors = schema.validate(&json!({"email": "not-an-email"}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::InvalidFormat);
    }

    // ── URL ───────────────────────────────────────────────────────────────────

    #[test]
    fn validate_url_valid() {
        let schema = CustomFieldsSchema::new(vec![typed_def("site", FieldType::Url, false, None)]);
        assert!(schema
            .validate(&json!({"site": "https://example.com"}))
            .is_empty());
    }

    #[test]
    fn validate_url_invalid() {
        let schema = CustomFieldsSchema::new(vec![typed_def("site", FieldType::Url, false, None)]);
        let errors = schema.validate(&json!({"site": "not a url"}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::InvalidFormat);
    }

    // ── Date ──────────────────────────────────────────────────────────────────

    #[test]
    fn validate_date_iso8601_valid() {
        let schema = CustomFieldsSchema::new(vec![typed_def("dob", FieldType::Date, false, None)]);
        assert!(schema.validate(&json!({"dob": "2000-01-15"})).is_empty());
    }

    #[test]
    fn validate_date_iso8601_invalid() {
        let schema = CustomFieldsSchema::new(vec![typed_def("dob", FieldType::Date, false, None)]);
        let errors = schema.validate(&json!({"dob": "15/01/2000"}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::InvalidFormat);
    }

    // ── DateTime ─────────────────────────────────────────────────────────────

    #[test]
    fn validate_datetime_valid() {
        let schema =
            CustomFieldsSchema::new(vec![typed_def("ts", FieldType::DateTime, false, None)]);
        assert!(schema
            .validate(&json!({"ts": "2024-06-01T12:00:00Z"}))
            .is_empty());
        assert!(schema
            .validate(&json!({"ts": "2024-06-01T12:00:00"}))
            .is_empty());
    }

    // ── Color ─────────────────────────────────────────────────────────────────

    #[test]
    fn validate_color_hex_valid() {
        let schema = CustomFieldsSchema::new(vec![typed_def("bg", FieldType::Color, false, None)]);
        assert!(schema.validate(&json!({"bg": "#FF00AA"})).is_empty());
        assert!(schema.validate(&json!({"bg": "#000000"})).is_empty());
    }

    #[test]
    fn validate_color_hex_invalid() {
        let schema = CustomFieldsSchema::new(vec![typed_def("bg", FieldType::Color, false, None)]);
        let errors = schema.validate(&json!({"bg": "red"}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::InvalidFormat);
    }

    // ── Boolean ───────────────────────────────────────────────────────────────

    #[test]
    fn validate_boolean_type_mismatch() {
        let schema =
            CustomFieldsSchema::new(vec![typed_def("active", FieldType::Boolean, false, None)]);
        let errors = schema.validate(&json!({"active": "yes"}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::InvalidType);
    }

    // ── Phone ─────────────────────────────────────────────────────────────────

    #[test]
    fn validate_phone_with_pattern() {
        let rule = ValidationRule {
            pattern: Some(r"^\+?[0-9\s\-]{7,20}$".to_string()),
            ..Default::default()
        };
        let schema = CustomFieldsSchema::new(vec![typed_def(
            "phone",
            FieldType::Phone,
            false,
            Some(rule),
        )]);
        assert!(schema.validate(&json!({"phone": "+1-555-0100"})).is_empty());
        let errors = schema.validate(&json!({"phone": "abc"}));
        assert_eq!(errors[0].error_code, FieldErrorCode::PatternMismatch);
    }

    // ── Json — depth validation (Variant A: arrays transparent) ──────────────

    // Helper: schema with a single Json field.
    fn json_schema() -> CustomFieldsSchema {
        CustomFieldsSchema::new(vec![typed_def("meta", FieldType::Json, false, None)])
    }

    #[test]
    fn json_depth_0_primitives_ok() {
        let s = json_schema();
        assert!(s.validate(&json!({"meta": true})).is_empty());
        assert!(s.validate(&json!({"meta": 42})).is_empty());
        assert!(s.validate(&json!({"meta": "anything"})).is_empty());
        assert!(s.validate(&json!({"meta": null})).is_empty());
    }

    #[test]
    fn json_depth_0_flat_array_ok() {
        // Array of primitives has object-depth 0 (array is transparent).
        let s = json_schema();
        assert!(s.validate(&json!({"meta": [1, 2, 3]})).is_empty());
        assert!(s.validate(&json!({"meta": ["a", "b"]})).is_empty());
    }

    #[test]
    fn json_depth_1_flat_object_ok() {
        let s = json_schema();
        assert!(s.validate(&json!({"meta": {"key": "value"}})).is_empty());
    }

    #[test]
    fn json_depth_1_object_with_array_of_primitives_ok() {
        // {"items": [1, 2, 3]} → depth 1 (array transparent).
        let s = json_schema();
        assert!(s
            .validate(&json!({"meta": {"items": [1, 2, 3]}}))
            .is_empty());
    }

    #[test]
    fn json_depth_2_nested_object_ok() {
        // {"address": {"city": "NY"}} → depth 2, at the limit.
        let s = json_schema();
        assert!(s
            .validate(&json!({"meta": {"address": {"city": "NY"}}}))
            .is_empty());
    }

    #[test]
    fn json_depth_2_array_of_objects_ok() {
        // {"items": [{"id": 1, "name": "x"}]} → depth 2 (array transparent).
        // This is the key Variant-A case: a list of objects stays within limit.
        let s = json_schema();
        assert!(s
            .validate(&json!({"meta": {"items": [{"id": 1, "name": "x"}]}}))
            .is_empty());
    }

    #[test]
    fn json_depth_3_rejected() {
        // {"a": {"b": {"c": 1}}} → depth 3, exceeds limit.
        let s = json_schema();
        let errors = s.validate(&json!({"meta": {"a": {"b": {"c": 1}}}}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::NestingTooDeep);
        assert_eq!(errors[0].field_key, "meta");
        assert!(errors[0].message.contains("limit is 2"));
    }

    #[test]
    fn json_depth_3_via_array_rejected() {
        // {"a": {"b": [{"c": 1}]}} → depth 3 (a=1, b=2, c-object=3, array transparent).
        let s = json_schema();
        let errors = s.validate(&json!({"meta": {"a": {"b": [{"c": 1}]}}}));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_code, FieldErrorCode::NestingTooDeep);
    }

    // ── json_object_depth unit tests ──────────────────────────────────────────

    #[test]
    fn depth_primitives() {
        assert_eq!(json_object_depth(&json!(42)), 0);
        assert_eq!(json_object_depth(&json!("hello")), 0);
        assert_eq!(json_object_depth(&json!(true)), 0);
        assert_eq!(json_object_depth(&json!(null)), 0);
    }

    #[test]
    fn depth_flat_array() {
        assert_eq!(json_object_depth(&json!([1, 2, 3])), 0);
    }

    #[test]
    fn depth_flat_object() {
        assert_eq!(json_object_depth(&json!({"key": "value"})), 1);
    }

    #[test]
    fn depth_object_with_flat_array() {
        assert_eq!(json_object_depth(&json!({"items": [1, 2, 3]})), 1);
    }

    #[test]
    fn depth_nested_object() {
        assert_eq!(json_object_depth(&json!({"address": {"city": "NY"}})), 2);
    }

    #[test]
    fn depth_array_of_objects() {
        // Core Variant-A case: array is transparent.
        assert_eq!(json_object_depth(&json!({"items": [{"id": 1}]})), 2);
    }

    #[test]
    fn depth_triple_nesting() {
        assert_eq!(json_object_depth(&json!({"a": {"b": {"c": 1}}})), 3);
    }

    #[test]
    fn depth_empty_object_and_array() {
        assert_eq!(json_object_depth(&json!({})), 1);
        assert_eq!(json_object_depth(&json!([])), 0);
    }

    // ── apply_defaults ────────────────────────────────────────────────────────

    #[test]
    fn apply_defaults_fills_missing() {
        let def = FieldDefinition {
            field_key: "tier".to_string(),
            field_type: FieldType::Text,
            label: [("en".to_string(), "Tier".to_string())]
                .into_iter()
                .collect(),
            description: None,
            is_required: false,
            default_value: Some(json!("free")),
            validation: None,
            position: 0,
            is_active: true,
        };
        let schema = CustomFieldsSchema::new(vec![def]);
        let mut meta = json!({});
        schema.apply_defaults(&mut meta);
        assert_eq!(meta["tier"], "free");
    }

    #[test]
    fn apply_defaults_preserves_existing() {
        let def = FieldDefinition {
            field_key: "tier".to_string(),
            field_type: FieldType::Text,
            label: [("en".to_string(), "Tier".to_string())]
                .into_iter()
                .collect(),
            description: None,
            is_required: false,
            default_value: Some(json!("free")),
            validation: None,
            position: 0,
            is_active: true,
        };
        let schema = CustomFieldsSchema::new(vec![def]);
        let mut meta = json!({"tier": "pro"});
        schema.apply_defaults(&mut meta);
        assert_eq!(meta["tier"], "pro");
    }

    // ── strip_unknown ─────────────────────────────────────────────────────────

    #[test]
    fn strip_unknown_removes_extra_keys() {
        let schema = CustomFieldsSchema::new(vec![text_def("name", false, None)]);
        let mut meta = json!({"name": "Alice", "unknown_field": "extra"});
        schema.strip_unknown(&mut meta);
        assert!(meta.get("unknown_field").is_none());
        assert_eq!(meta["name"], "Alice");
    }

    #[test]
    fn strip_unknown_keeps_defined() {
        let schema = CustomFieldsSchema::new(vec![
            text_def("first_name", false, None),
            text_def("last_name", false, None),
        ]);
        let mut meta = json!({"first_name": "Alice", "last_name": "Smith", "junk": 99});
        schema.strip_unknown(&mut meta);
        assert_eq!(meta["first_name"], "Alice");
        assert_eq!(meta["last_name"], "Smith");
        assert!(meta.get("junk").is_none());
    }

    // ── Empty schema ──────────────────────────────────────────────────────────

    #[test]
    fn empty_schema_accepts_anything() {
        let schema = CustomFieldsSchema::new(vec![]);
        let errors = schema.validate(&json!({"whatever": 42, "foo": "bar"}));
        assert!(errors.is_empty());
    }

    // ── FieldType helpers ─────────────────────────────────────────────────────

    #[test]
    fn field_type_requires_options() {
        assert!(FieldType::Select.requires_options());
        assert!(FieldType::MultiSelect.requires_options());
        assert!(!FieldType::Text.requires_options());
        assert!(!FieldType::Integer.requires_options());
        assert!(!FieldType::Json.requires_options());
    }

    #[test]
    fn field_type_min_max_is_length() {
        assert!(FieldType::Text.min_max_is_length());
        assert!(FieldType::Textarea.min_max_is_length());
        assert!(FieldType::Url.min_max_is_length());
        assert!(FieldType::Email.min_max_is_length());
        assert!(FieldType::Phone.min_max_is_length());
        assert!(!FieldType::Integer.min_max_is_length());
        assert!(!FieldType::Decimal.min_max_is_length());
        assert!(!FieldType::MultiSelect.min_max_is_length());
    }

    #[test]
    fn field_type_supports_pattern() {
        assert!(FieldType::Text.supports_pattern());
        assert!(FieldType::Textarea.supports_pattern());
        assert!(FieldType::Phone.supports_pattern());
        assert!(!FieldType::Url.supports_pattern());
        assert!(!FieldType::Email.supports_pattern());
        assert!(!FieldType::Integer.supports_pattern());
    }

    // ── Guardrail helpers ─────────────────────────────────────────────────────

    #[test]
    fn field_key_guardrail_valid() {
        assert!(is_valid_field_key("name"));
        assert!(is_valid_field_key("first_name"));
        assert!(is_valid_field_key("field123"));
        assert!(is_valid_field_key("a"));
        let long_valid = format!("a{}", "b".repeat(127));
        assert!(is_valid_field_key(&long_valid));
    }

    #[test]
    fn field_key_guardrail_invalid() {
        assert!(!is_valid_field_key(""));
        assert!(!is_valid_field_key("1name")); // starts with digit
        assert!(!is_valid_field_key("_name")); // starts with underscore
        assert!(!is_valid_field_key("CamelCase")); // uppercase
        assert!(!is_valid_field_key("has-hyphen")); // hyphen not allowed
        let too_long = format!("a{}", "b".repeat(128)); // 129 chars total
        assert!(!is_valid_field_key(&too_long));
    }

    #[test]
    fn locale_key_guardrail_valid() {
        assert!(is_valid_locale_key("en"));
        assert!(is_valid_locale_key("ru"));
        assert!(is_valid_locale_key("zh"));
        assert!(is_valid_locale_key("en-US"));
        assert!(is_valid_locale_key("pt-BR"));
    }

    #[test]
    fn locale_key_guardrail_invalid() {
        assert!(!is_valid_locale_key("EN")); // uppercase lang
        assert!(!is_valid_locale_key("en-us")); // lowercase country
        assert!(!is_valid_locale_key("english")); // too long
        assert!(!is_valid_locale_key("e")); // too short
        assert!(!is_valid_locale_key("en_US")); // underscore separator
    }

    #[test]
    fn json_field_eq_builds_postgres_operator_expression() {
        use sea_orm::sea_query::{Alias, Expr, PostgresQueryBuilder, Query};

        let condition = json_field_eq(Expr::col(Alias::new("metadata")), "phone", "123");
        let sql = Query::select()
            .column((Alias::new("users"), Alias::new("id")))
            .from(Alias::new("users"))
            .cond_where(condition)
            .to_string(PostgresQueryBuilder);

        assert!(sql.contains("->>"));
        assert!(sql.contains("metadata"));
    }

    #[test]
    fn json_field_exists_builds_key_existence_expression() {
        use sea_orm::sea_query::{Alias, Expr, PostgresQueryBuilder, Query};

        let condition = json_field_exists(Expr::col(Alias::new("metadata")), "phone");
        let sql = Query::select()
            .column((Alias::new("users"), Alias::new("id")))
            .from(Alias::new("users"))
            .cond_where(condition)
            .to_string(PostgresQueryBuilder);

        assert!(sql.contains(" ? "));
        assert!(sql.contains("metadata"));
    }

    #[test]
    fn json_field_extract_builds_projection_expression() {
        use sea_orm::sea_query::{Alias, Expr, PostgresQueryBuilder, Query};

        let extracted = json_field_extract(Expr::col(Alias::new("metadata")), "phone");
        let sql = Query::select()
            .expr(extracted)
            .from(Alias::new("users"))
            .to_string(PostgresQueryBuilder);

        assert!(sql.contains("->>"));
        assert!(sql.contains("metadata"));
    }

    #[test]
    fn json_field_contains_builds_jsonb_contains_expression() {
        use sea_orm::sea_query::{Alias, Expr, PostgresQueryBuilder, Query};

        let condition = json_field_contains(Expr::col(Alias::new("metadata")), "age", json!(42));
        let sql = Query::select()
            .column((Alias::new("users"), Alias::new("id")))
            .from(Alias::new("users"))
            .cond_where(condition)
            .to_string(PostgresQueryBuilder);

        assert!(sql.contains("@>"));
        assert!(sql.contains("jsonb"));
    }

    mod macro_smoke {
        crate::define_field_definitions_entity!("macro_smoke_field_definitions");
    }

    #[test]
    fn define_field_definitions_entity_macro_sets_table_name() {
        use sea_orm::EntityName;

        assert_eq!(
            macro_smoke::Entity.table_name(),
            "macro_smoke_field_definitions"
        );
    }

    #[tokio::test]
    async fn create_and_drop_field_definitions_table_sqlite() {
        use sea_orm::{ConnectionTrait, Database, Statement};
        use sea_orm_migration::prelude::SchemaManager;

        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite in-memory connection should work");
        let manager = SchemaManager::new(&db);

        create_field_definitions_table(&manager, "user", "users")
            .await
            .expect("table creation should succeed");

        let exists_after_create = db
            .query_one(Statement::from_string(
                db.get_database_backend(),
                "SELECT name FROM sqlite_master WHERE type='table' AND name='user_field_definitions'"
                    .to_string(),
            ))
            .await
            .expect("table lookup should succeed");
        assert!(exists_after_create.is_some());

        drop_field_definitions_table(&manager, "user")
            .await
            .expect("table drop should succeed");

        let exists_after_drop = db
            .query_one(Statement::from_string(
                db.get_database_backend(),
                "SELECT name FROM sqlite_master WHERE type='table' AND name='user_field_definitions'"
                    .to_string(),
            ))
            .await
            .expect("table lookup should succeed");
        assert!(exists_after_drop.is_none());
    }
}
