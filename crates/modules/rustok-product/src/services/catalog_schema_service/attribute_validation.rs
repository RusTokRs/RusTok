use super::*;
use regex::Regex;
use std::str::FromStr;

/// Canonical prefix of every rule-enforcement failure message.
///
/// `crate::public_error::map_product_public_error` recognises this prefix to return a
/// structured, bounded public failure instead of the generic `PRODUCT_VALIDATION` copy.
pub(crate) const ATTRIBUTE_VALIDATION_RULE_PREFIX: &str = "attribute validation rule";

/// Maximum accepted value of a bounded integer rule (`minLength`, `maxSelections`, ...).
const MAX_RULE_BOUND: u64 = 1_048_576;
/// Maximum accepted pattern length; longer patterns are rejected at authoring time.
const MAX_RULE_PATTERN_BYTES: usize = 512;
/// Maximum accepted number of declared locales and allowed options per rule.
const MAX_RULE_LIST_ITEMS: usize = 128;

/// The closed set of rules the engine enforces. Unknown keys are rejected at authoring time
/// so a typo cannot silently disable an intended constraint.
const DECLARED_RULE_KEYS: [&str; 15] = [
    "minLength",
    "maxLength",
    "pattern",
    "min",
    "max",
    "minDate",
    "maxDate",
    "minDatetime",
    "maxDatetime",
    "minSelections",
    "maxSelections",
    "options",
    "required",
    "requiredLocales",
    "maxJsonBytes",
];

/// Parsed, validated rule object of one effective attribute binding.
#[derive(Clone, Debug, Default)]
pub(crate) struct ProductAttributeValidationRules {
    pub(crate) min_length: Option<u32>,
    pub(crate) max_length: Option<u32>,
    pub(crate) pattern: Option<Regex>,
    pub(crate) min_number: Option<Decimal>,
    pub(crate) max_number: Option<Decimal>,
    pub(crate) min_date: Option<NaiveDate>,
    pub(crate) max_date: Option<NaiveDate>,
    pub(crate) min_datetime: Option<DateTime<Utc>>,
    pub(crate) max_datetime: Option<DateTime<Utc>>,
    pub(crate) min_selections: Option<u32>,
    pub(crate) max_selections: Option<u32>,
    pub(crate) allowed_option_ids: Option<HashSet<Uuid>>,
    pub(crate) required: bool,
    pub(crate) required_locales: Vec<String>,
    pub(crate) max_json_bytes: Option<u32>,
}

impl ProductAttributeValidationRules {
    /// True when the effective binding declares no rule at all; enforcement is skipped.
    pub(crate) fn is_empty(&self) -> bool {
        self.min_length.is_none()
            && self.max_length.is_none()
            && self.pattern.is_none()
            && self.min_number.is_none()
            && self.max_number.is_none()
            && self.min_date.is_none()
            && self.max_date.is_none()
            && self.min_datetime.is_none()
            && self.max_datetime.is_none()
            && self.min_selections.is_none()
            && self.max_selections.is_none()
            && self.allowed_option_ids.is_none()
            && !self.required
            && self.required_locales.is_empty()
            && self.max_json_bytes.is_none()
    }

    /// Value-level rules (existence is gated separately by `required`/`requiredLocales`).
    #[allow(dead_code)]
    pub(crate) fn declares_value_rules(&self) -> bool {
        self.min_length.is_some()
            || self.max_length.is_some()
            || self.pattern.is_some()
            || self.min_number.is_some()
            || self.max_number.is_some()
            || self.min_date.is_some()
            || self.max_date.is_some()
            || self.min_datetime.is_some()
            || self.max_datetime.is_some()
            || self.min_selections.is_some()
            || self.max_selections.is_some()
            || self.allowed_option_ids.is_some()
            || self.max_json_bytes.is_some()
    }
}

fn schema_failure(rule: &str, detail: impl std::fmt::Display) -> CommerceError {
    CommerceError::Validation(format!(
        "{ATTRIBUTE_VALIDATION_RULE_PREFIX} schema is invalid: rule `{rule}` {detail}"
    ))
}

pub(crate) fn rule_failure(
    attribute_id: Uuid,
    rule: &str,
    detail: impl std::fmt::Display,
) -> CommerceError {
    CommerceError::Validation(format!(
        "{ATTRIBUTE_VALIDATION_RULE_PREFIX} `{rule}` failed for attribute {attribute_id}: {detail}"
    ))
}

/// Rejects keys outside the documented rule schema. Called by every authoring command so a typo
/// (`maxlength`) fails the write instead of leaving data unconstrained.
pub(crate) fn validate_declared_rule_keys(value: &Value) -> CommerceResult<()> {
    let Some(object) = value.as_object() else {
        return Err(CommerceError::Validation(format!(
            "{ATTRIBUTE_VALIDATION_RULE_PREFIX} must be a JSON object"
        )));
    };
    for key in object.keys() {
        if !DECLARED_RULE_KEYS.contains(&key.as_str()) {
            return Err(CommerceError::Validation(format!(
                "{ATTRIBUTE_VALIDATION_RULE_PREFIX} `{key}` is not supported; supported rules are {}",
                DECLARED_RULE_KEYS.join(", ")
            )));
        }
    }
    Ok(())
}

/// Merges the attribute-level base rules with the effective binding overrides.
///
/// Later layers win key by key; an explicit `null` override removes an inherited rule, so a
/// category can drop a schema-level constraint without redeclaring the whole object.
pub(crate) fn merge_product_attribute_validation(base: &Value, overrides: &Value) -> Value {
    let mut merged = base.as_object().cloned().unwrap_or_default();
    if let Some(overrides) = overrides.as_object() {
        for (key, value) in overrides {
            if value.is_null() {
                merged.remove(key);
            } else {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    Value::Object(merged)
}

/// Parses the merged effective rule object. Malformed known rules fail closed: a declared but
/// broken constraint must never silently admit data.
pub(crate) fn parse_product_attribute_validation(
    value: &Value,
) -> CommerceResult<ProductAttributeValidationRules> {
    let Some(object) = value.as_object() else {
        return Err(CommerceError::Validation(format!(
            "{ATTRIBUTE_VALIDATION_RULE_PREFIX} must be a JSON object"
        )));
    };

    let min_length = bounded_rule(object, "minLength")?;
    let max_length = bounded_rule(object, "maxLength")?;
    if let (Some(min), Some(max)) = (min_length, max_length) {
        if min > max {
            return Err(schema_failure(
                "minLength",
                "must not exceed maxLength".to_string(),
            ));
        }
    }

    let min_number = decimal_rule(object, "min")?;
    let max_number = decimal_rule(object, "max")?;
    if let (Some(min), Some(max)) = (min_number, max_number) {
        if min > max {
            return Err(schema_failure("min", "must not exceed max".to_string()));
        }
    }

    let min_date = date_rule(object, "minDate")?;
    let max_date = date_rule(object, "maxDate")?;
    if let (Some(min), Some(max)) = (min_date, max_date) {
        if min > max {
            return Err(schema_failure(
                "minDate",
                "must not be later than maxDate".to_string(),
            ));
        }
    }

    let min_datetime = datetime_rule(object, "minDatetime")?;
    let max_datetime = datetime_rule(object, "maxDatetime")?;
    if let (Some(min), Some(max)) = (min_datetime, max_datetime) {
        if min > max {
            return Err(schema_failure(
                "minDatetime",
                "must not be later than maxDatetime".to_string(),
            ));
        }
    }

    let min_selections = bounded_rule(object, "minSelections")?;
    let max_selections = bounded_rule(object, "maxSelections")?;
    if let (Some(min), Some(max)) = (min_selections, max_selections) {
        if min > max {
            return Err(schema_failure(
                "minSelections",
                "must not exceed maxSelections".to_string(),
            ));
        }
    }

    let pattern = match object.get("pattern") {
        None | Some(Value::Null) => None,
        Some(Value::String(pattern)) => {
            if pattern.len() > MAX_RULE_PATTERN_BYTES {
                return Err(schema_failure(
                    "pattern",
                    format!("must not exceed {MAX_RULE_PATTERN_BYTES} bytes"),
                ));
            }
            Some(
                Regex::new(pattern)
                    .map_err(|error| schema_failure("pattern", format!("is not valid: {error}")))?,
            )
        }
        Some(_) => return Err(schema_failure("pattern", "must be a string".to_string())),
    };

    let allowed_option_ids = match object.get("options") {
        None | Some(Value::Null) => None,
        Some(Value::Array(options)) => {
            if options.is_empty() {
                return Err(schema_failure(
                    "options",
                    "must declare at least one allowed option".to_string(),
                ));
            }
            if options.len() > MAX_RULE_LIST_ITEMS {
                return Err(schema_failure(
                    "options",
                    format!("must not declare more than {MAX_RULE_LIST_ITEMS} options"),
                ));
            }
            let mut allowed = HashSet::with_capacity(options.len());
            for option in options {
                let Some(raw) = option.as_str() else {
                    return Err(schema_failure(
                        "options",
                        "must contain option identifiers as strings".to_string(),
                    ));
                };
                let option_id = Uuid::parse_str(raw).map_err(|_| {
                    schema_failure("options", format!("`{raw}` is not a valid identifier"))
                })?;
                allowed.insert(option_id);
            }
            Some(allowed)
        }
        Some(_) => {
            return Err(schema_failure(
                "options",
                "must be an array of option identifiers".to_string(),
            ));
        }
    };

    let required = match object.get("required") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(required)) => *required,
        Some(_) => return Err(schema_failure("required", "must be a boolean".to_string())),
    };

    let required_locales = match object.get("requiredLocales") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(locales)) => {
            if locales.len() > MAX_RULE_LIST_ITEMS {
                return Err(schema_failure(
                    "requiredLocales",
                    format!("must not declare more than {MAX_RULE_LIST_ITEMS} locales"),
                ));
            }
            let mut required_locales = Vec::with_capacity(locales.len());
            for locale in locales {
                let Some(raw) = locale.as_str() else {
                    return Err(schema_failure(
                        "requiredLocales",
                        "must contain locale codes as strings".to_string(),
                    ));
                };
                if validate_locale(raw).is_err() {
                    return Err(schema_failure(
                        "requiredLocales",
                        format!("`{raw}` is not a valid locale"),
                    ));
                }
                let canonical = raw.trim().to_string();
                if !required_locales.contains(&canonical) {
                    required_locales.push(canonical);
                }
            }
            required_locales
        }
        Some(_) => {
            return Err(schema_failure(
                "requiredLocales",
                "must be an array of locale codes".to_string(),
            ));
        }
    };

    let max_json_bytes = bounded_rule(object, "maxJsonBytes")?;

    Ok(ProductAttributeValidationRules {
        min_length,
        max_length,
        pattern,
        min_number,
        max_number,
        min_date,
        max_date,
        min_datetime,
        max_datetime,
        min_selections,
        max_selections,
        allowed_option_ids,
        required,
        required_locales,
        max_json_bytes,
    })
}

fn bounded_rule(
    object: &serde_json::Map<String, Value>,
    rule: &str,
) -> CommerceResult<Option<u32>> {
    let Some(value) = object.get(rule) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(number) = value.as_u64() else {
        return Err(schema_failure(
            rule,
            "must be a non-negative integer".to_string(),
        ));
    };
    if number > MAX_RULE_BOUND {
        return Err(schema_failure(
            rule,
            format!("must not exceed {MAX_RULE_BOUND}"),
        ));
    }
    Ok(Some(number as u32))
}

fn decimal_rule(
    object: &serde_json::Map<String, Value>,
    rule: &str,
) -> CommerceResult<Option<Decimal>> {
    let Some(value) = object.get(rule) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let raw = match value {
        Value::String(raw) => raw.clone(),
        Value::Number(number) => number.to_string(),
        _ => return Err(schema_failure(rule, "must be a number".to_string())),
    };
    Decimal::from_str(&raw)
        .map(Some)
        .map_err(|_| schema_failure(rule, "must be a number".to_string()))
}

fn date_rule(
    object: &serde_json::Map<String, Value>,
    rule: &str,
) -> CommerceResult<Option<NaiveDate>> {
    let Some(value) = object.get(rule) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(raw) = value.as_str() else {
        return Err(schema_failure(
            rule,
            "must be a date string in YYYY-MM-DD".to_string(),
        ));
    };
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| schema_failure(rule, "must be a date string in YYYY-MM-DD".to_string()))
}

fn datetime_rule(
    object: &serde_json::Map<String, Value>,
    rule: &str,
) -> CommerceResult<Option<DateTime<Utc>>> {
    let Some(value) = object.get(rule) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(raw) = value.as_str() else {
        return Err(schema_failure(
            rule,
            "must be an RFC 3339 timestamp".to_string(),
        ));
    };
    DateTime::parse_from_rfc3339(raw)
        .map(|value| Some(value.with_timezone(&Utc)))
        .map_err(|_| schema_failure(rule, "must be an RFC 3339 timestamp".to_string()))
}

/// Enforces every applicable rule of the merged effective rule object against one patch value.
///
/// Rules of a different value-type group stay inert, so one attribute can keep a single declared
/// object even when the schema is edited. `Clear` and empty multiselect payloads delete the value
/// and are governed by `required`/`requiredLocales` at publish time instead.
pub(crate) fn validate_attribute_value_rules(
    attribute_id: Uuid,
    rules: &ProductAttributeValidationRules,
    value_type: AttributeValueType,
    value: &ProductAttributeValuePatchValue,
) -> CommerceResult<()> {
    if rules.is_empty() || matches!(value, ProductAttributeValuePatchValue::Clear) {
        return Ok(());
    }

    match value_type {
        AttributeValueType::Text | AttributeValueType::Textarea | AttributeValueType::Richtext => {
            let ProductAttributeValuePatchValue::Text(text) = value else {
                return Ok(());
            };
            let length = text.chars().count() as u64;
            if let Some(min) = rules.min_length {
                if length < u64::from(min) {
                    return Err(rule_failure(
                        attribute_id,
                        "minLength",
                        format!("minimum length is {min} characters"),
                    ));
                }
            }
            if let Some(max) = rules.max_length {
                if length > u64::from(max) {
                    return Err(rule_failure(
                        attribute_id,
                        "maxLength",
                        format!("maximum length is {max} characters"),
                    ));
                }
            }
            if let Some(pattern) = &rules.pattern {
                if !pattern.is_match(text) {
                    return Err(rule_failure(
                        attribute_id,
                        "pattern",
                        "value does not match the declared pattern".to_string(),
                    ));
                }
            }
        }
        AttributeValueType::Integer => {
            let ProductAttributeValuePatchValue::Integer(number) = value else {
                return Ok(());
            };
            check_numeric_bounds(attribute_id, rules, Decimal::from(*number))?;
        }
        AttributeValueType::Decimal => {
            let ProductAttributeValuePatchValue::Decimal(number) = value else {
                return Ok(());
            };
            check_numeric_bounds(attribute_id, rules, *number)?;
        }
        AttributeValueType::Date => {
            let ProductAttributeValuePatchValue::Date(date) = value else {
                return Ok(());
            };
            if let Some(min) = rules.min_date {
                if *date < min {
                    return Err(rule_failure(
                        attribute_id,
                        "minDate",
                        format!("earliest allowed date is {min}"),
                    ));
                }
            }
            if let Some(max) = rules.max_date {
                if *date > max {
                    return Err(rule_failure(
                        attribute_id,
                        "maxDate",
                        format!("latest allowed date is {max}"),
                    ));
                }
            }
        }
        AttributeValueType::Datetime => {
            let ProductAttributeValuePatchValue::Datetime(datetime) = value else {
                return Ok(());
            };
            if let Some(min) = rules.min_datetime {
                if *datetime < min {
                    return Err(rule_failure(
                        attribute_id,
                        "minDatetime",
                        format!("earliest allowed instant is {}", min.to_rfc3339()),
                    ));
                }
            }
            if let Some(max) = rules.max_datetime {
                if *datetime > max {
                    return Err(rule_failure(
                        attribute_id,
                        "maxDatetime",
                        format!("latest allowed instant is {}", max.to_rfc3339()),
                    ));
                }
            }
        }
        AttributeValueType::Select | AttributeValueType::Multiselect => {
            let option_ids: Vec<Uuid> = match value {
                ProductAttributeValuePatchValue::Select(option_id) => vec![*option_id],
                ProductAttributeValuePatchValue::Multiselect(option_ids) => option_ids.clone(),
                _ => return Ok(()),
            };
            if option_ids.is_empty() {
                return Ok(());
            }
            if let Some(allowed) = &rules.allowed_option_ids {
                if let Some(option_id) = option_ids
                    .iter()
                    .find(|option_id| !allowed.contains(option_id))
                {
                    return Err(rule_failure(
                        attribute_id,
                        "options",
                        format!("option {option_id} is not a declared allowed value"),
                    ));
                }
            }
            let count = option_ids.len() as u64;
            if let Some(min) = rules.min_selections {
                if count < u64::from(min) {
                    return Err(rule_failure(
                        attribute_id,
                        "minSelections",
                        format!("at least {min} option(s) must be selected"),
                    ));
                }
            }
            if let Some(max) = rules.max_selections {
                if count > u64::from(max) {
                    return Err(rule_failure(
                        attribute_id,
                        "maxSelections",
                        format!("at most {max} option(s) can be selected"),
                    ));
                }
            }
        }
        AttributeValueType::Json => {
            let ProductAttributeValuePatchValue::Json(json) = value else {
                return Ok(());
            };
            if let Some(max) = rules.max_json_bytes {
                let bytes = serde_json::to_vec(json)
                    .map(|encoded| encoded.len())
                    .unwrap_or(usize::MAX);
                if bytes > max as usize {
                    return Err(rule_failure(
                        attribute_id,
                        "maxJsonBytes",
                        format!("maximum serialized size is {max} bytes"),
                    ));
                }
            }
        }
        AttributeValueType::Boolean => {}
    }

    Ok(())
}

fn check_numeric_bounds(
    attribute_id: Uuid,
    rules: &ProductAttributeValidationRules,
    number: Decimal,
) -> CommerceResult<()> {
    if let Some(min) = rules.min_number {
        if number < min {
            return Err(rule_failure(
                attribute_id,
                "min",
                format!("minimum allowed value is {min}"),
            ));
        }
    }
    if let Some(max) = rules.max_number {
        if number > max {
            return Err(rule_failure(
                attribute_id,
                "max",
                format!("maximum allowed value is {max}"),
            ));
        }
    }
    Ok(())
}

/// Extracts the failing rule name from a canonical enforcement failure message.
pub(crate) fn attribute_validation_rule_of(message: &str) -> Option<&str> {
    let rest = message.strip_prefix(ATTRIBUTE_VALIDATION_RULE_PREFIX)?;
    let rest = rest.strip_prefix(" `")?;
    let (rule, rest) = rest.split_once('`')?;
    rest.starts_with(" failed for attribute ").then_some(rule)
}

/// Bounded public copy for one failing rule.
///
/// Only the closed rule set is reflected, so no tenant-authored text can reach a public error.
pub(crate) fn attribute_validation_public_message(rule: &str) -> &'static str {
    match rule {
        "minLength" | "maxLength" => "Product attribute value has an unsupported length",
        "pattern" => "Product attribute value does not match the required format",
        "min" | "max" => "Product attribute value is outside the allowed range",
        "minDate" | "maxDate" | "minDatetime" | "maxDatetime" => {
            "Product attribute value is outside the allowed date range"
        }
        "minSelections" | "maxSelections" | "options" => {
            "Product attribute value is not an allowed selection"
        }
        "maxJsonBytes" => "Product attribute value exceeds the allowed size",
        "required" => "Required product attribute value is missing",
        "requiredLocales" => "Product attribute value is missing a required locale",
        _ => "Product attribute value violates the schema validation rules",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn attribute_id() -> Uuid {
        Uuid::parse_str("6f3a5db8-1f5c-4e34-9d2f-6f0e5f1d3b2a").expect("fixture uuid")
    }

    fn rules(value: Value) -> ProductAttributeValidationRules {
        parse_product_attribute_validation(&value).expect("fixture rules must parse")
    }

    fn text(value: &str) -> ProductAttributeValuePatchValue {
        ProductAttributeValuePatchValue::Text(value.to_string())
    }

    fn assert_rules_pass(
        rules: &ProductAttributeValidationRules,
        value_type: AttributeValueType,
        value: &ProductAttributeValuePatchValue,
    ) {
        let outcome = validate_attribute_value_rules(attribute_id(), rules, value_type, value);
        assert!(outcome.is_ok());
    }

    fn assert_rules_fail(
        rules: &ProductAttributeValidationRules,
        value_type: AttributeValueType,
        value: &ProductAttributeValuePatchValue,
    ) {
        let outcome = validate_attribute_value_rules(attribute_id(), rules, value_type, value);
        assert!(outcome.is_err());
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("fixture date")
    }

    #[test]
    fn declared_rule_keys_reject_typos_and_non_objects() {
        let exact = validate_declared_rule_keys(&json!({ "maxLength": 12 }));
        assert!(exact.is_ok());

        let typo = validate_declared_rule_keys(&json!({ "maxlength": 12 }));
        assert!(typo.is_err());

        let not_an_object = validate_declared_rule_keys(&json!([]));
        assert!(not_an_object.is_err());
    }

    #[test]
    fn parse_rejects_malformed_known_rules() {
        let invalid = [
            json!({ "minLength": "12" }),
            json!({ "minLength": -1 }),
            json!({ "minLength": 12, "maxLength": 4 }),
            json!({ "min": 10, "max": 1 }),
            json!({ "pattern": "(" }),
            json!({ "pattern": 12 }),
            json!({ "pattern": "a".repeat(MAX_RULE_PATTERN_BYTES + 1) }),
            json!({ "minDate": "2026-13-01" }),
            json!({ "minDate": "2026-01-02", "maxDate": "2026-01-01" }),
            json!({ "minDatetime": "not-a-timestamp" }),
            json!({ "minSelections": 3, "maxSelections": 1 }),
            json!({ "options": [] }),
            json!({ "options": ["not-a-uuid"] }),
            json!({ "required": "yes" }),
            json!({ "requiredLocales": "ru" }),
            json!({ "maxJsonBytes": -5 }),
        ];

        for candidate in invalid {
            let parsed = parse_product_attribute_validation(&candidate);
            assert!(parsed.is_err(), "expected rejection of {candidate}");
        }
    }

    #[test]
    fn merge_layers_later_wins_and_null_clears() {
        let base = json!({ "maxLength": 8, "pattern": "^[A-Z]+$", "required": true });
        let overrides = json!({ "maxLength": 12, "pattern": null });

        let merged = merge_product_attribute_validation(&base, &overrides);

        assert_eq!(merged["maxLength"], json!(12));
        assert!(merged.get("pattern").is_none());
        assert_eq!(merged["required"], json!(true));

        let parsed = rules(merged);
        assert_eq!(parsed.max_length, Some(12));
        assert!(parsed.pattern.is_none());
        assert!(parsed.required);
    }

    #[test]
    fn text_rules_enforce_length_and_pattern() {
        let declared = rules(json!({ "minLength": 2, "maxLength": 4, "pattern": "^[A-Z]+$" }));
        let value_type = AttributeValueType::Text;

        assert_rules_pass(&declared, value_type, &text("AB"));
        assert_rules_fail(&declared, value_type, &text("A"));
        assert_rules_fail(&declared, value_type, &text("ABCDE"));
        assert_rules_fail(&declared, value_type, &text("abc"));
        assert_rules_pass(
            &declared,
            value_type,
            &ProductAttributeValuePatchValue::Clear,
        );
    }

    #[test]
    fn numeric_and_date_rules_enforce_declared_bounds() {
        let numeric = rules(json!({ "min": 10, "max": 20 }));

        assert_rules_pass(
            &numeric,
            AttributeValueType::Integer,
            &ProductAttributeValuePatchValue::Integer(15),
        );
        assert_rules_fail(
            &numeric,
            AttributeValueType::Integer,
            &ProductAttributeValuePatchValue::Integer(9),
        );
        assert_rules_fail(
            &numeric,
            AttributeValueType::Decimal,
            &ProductAttributeValuePatchValue::Decimal(Decimal::from(21)),
        );

        let dates = rules(json!({ "minDate": "2026-01-01", "maxDate": "2026-12-31" }));

        assert_rules_pass(
            &dates,
            AttributeValueType::Date,
            &ProductAttributeValuePatchValue::Date(date(2026, 6, 1)),
        );
        assert_rules_fail(
            &dates,
            AttributeValueType::Date,
            &ProductAttributeValuePatchValue::Date(date(2025, 12, 31)),
        );
    }

    #[test]
    fn datetime_rules_enforce_declared_bounds() {
        let declared = rules(json!({
            "minDatetime": "2026-01-01T00:00:00Z",
            "maxDatetime": "2026-12-31T23:59:59Z",
        }));
        let inside = DateTime::parse_from_rfc3339("2026-06-01T12:00:00Z")
            .expect("fixture instant")
            .with_timezone(&Utc);
        let outside = DateTime::parse_from_rfc3339("2025-06-01T12:00:00Z")
            .expect("fixture instant")
            .with_timezone(&Utc);

        assert_rules_pass(
            &declared,
            AttributeValueType::Datetime,
            &ProductAttributeValuePatchValue::Datetime(inside),
        );
        assert_rules_fail(
            &declared,
            AttributeValueType::Datetime,
            &ProductAttributeValuePatchValue::Datetime(outside),
        );
    }

    #[test]
    fn selection_and_json_rules_enforce_declared_bounds() {
        let allowed = Uuid::new_v4();
        let rejected = Uuid::new_v4();
        let declared = rules(json!({
            "minSelections": 1,
            "maxSelections": 2,
            "options": [allowed],
        }));
        let value_type = AttributeValueType::Multiselect;

        assert_rules_pass(
            &declared,
            value_type,
            &ProductAttributeValuePatchValue::Multiselect(vec![allowed]),
        );
        assert_rules_fail(
            &declared,
            value_type,
            &ProductAttributeValuePatchValue::Multiselect(vec![allowed, rejected]),
        );
        assert_rules_fail(
            &declared,
            value_type,
            &ProductAttributeValuePatchValue::Multiselect(vec![allowed, allowed, allowed]),
        );
        assert_rules_pass(
            &declared,
            value_type,
            &ProductAttributeValuePatchValue::Multiselect(Vec::new()),
        );

        let declared_json = rules(json!({ "maxJsonBytes": 8 }));

        assert_rules_pass(
            &declared_json,
            AttributeValueType::Json,
            &ProductAttributeValuePatchValue::Json(json!({ "a": 1 })),
        );
        assert_rules_fail(
            &declared_json,
            AttributeValueType::Json,
            &ProductAttributeValuePatchValue::Json(json!({ "key": "0123456789" })),
        );
    }

    #[test]
    fn failure_messages_expose_the_failing_rule_to_the_public_mapper() {
        let declared = rules(json!({ "maxLength": 1 }));
        let outcome = validate_attribute_value_rules(
            attribute_id(),
            &declared,
            AttributeValueType::Text,
            &text("AB"),
        );
        let error = outcome.expect_err("length rule must fail");
        let CommerceError::Validation(message) = error else {
            panic!("rule failures are validation errors");
        };

        assert_eq!(attribute_validation_rule_of(&message), Some("maxLength"));
        assert_eq!(
            attribute_validation_public_message("maxLength"),
            "Product attribute value has an unsupported length"
        );
        assert_eq!(
            attribute_validation_public_message("unknown-rule"),
            "Product attribute value violates the schema validation rules"
        );
        assert!(attribute_validation_rule_of("attribute is not available").is_none());
    }

    #[test]
    fn required_rules_parse_for_publish_gating() {
        let declared = rules(json!({
            "required": true,
            "requiredLocales": [" ru ", "en", "en"],
        }));

        assert!(declared.required);
        assert_eq!(
            declared.required_locales,
            vec!["ru".to_string(), "en".to_string()]
        );
        assert!(!declared.declares_value_rules());
        assert!(!declared.is_empty());

        let optional = rules(json!({ "minLength": 1 }));
        assert!(!optional.required);
        assert!(optional.required_locales.is_empty());
    }

    #[test]
    fn empty_rules_skip_enforcement_entirely() {
        let declared = rules(json!({}));

        assert!(declared.is_empty());
        assert!(!declared.declares_value_rules());
        assert_rules_pass(&declared, AttributeValueType::Text, &text("anything"));
    }
}
