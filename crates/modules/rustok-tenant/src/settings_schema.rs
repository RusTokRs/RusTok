use serde_json::Value;

use crate::error::TenantError;

const MAX_SETTINGS_DEPTH: usize = 8;
const MAX_OBJECT_KEYS: usize = 256;
const MAX_ARRAY_ITEMS: usize = 256;
const MAX_KEY_LENGTH: usize = 128;
const MAX_SETTINGS_SIZE_BYTES: usize = 16 * 1024;

pub(crate) fn validate_tenant_settings(settings: &Value) -> Result<(), TenantError> {
    let Some(root) = settings.as_object() else {
        return Err(TenantError::InvalidSettingsSchema(
            "tenant settings must be a JSON object".to_string(),
        ));
    };

    if root.len() > MAX_OBJECT_KEYS {
        return Err(TenantError::InvalidSettingsSchema(format!(
            "tenant settings exceed maximum key count ({MAX_OBJECT_KEYS})",
        )));
    }

    let size = serde_json::to_vec(settings)
        .map_err(|err| {
            TenantError::InvalidSettingsSchema(format!("unable to serialize settings: {err}"))
        })?
        .len();
    if size > MAX_SETTINGS_SIZE_BYTES {
        return Err(TenantError::InvalidSettingsSchema(format!(
            "tenant settings exceed maximum payload size ({MAX_SETTINGS_SIZE_BYTES} bytes)",
        )));
    }

    validate_value(settings, 0)
}

fn validate_value(value: &Value, depth: usize) -> Result<(), TenantError> {
    if depth > MAX_SETTINGS_DEPTH {
        return Err(TenantError::InvalidSettingsSchema(format!(
            "tenant settings exceed maximum nesting depth ({MAX_SETTINGS_DEPTH})",
        )));
    }

    match value {
        Value::Object(map) => {
            if map.len() > MAX_OBJECT_KEYS {
                return Err(TenantError::InvalidSettingsSchema(format!(
                    "tenant settings object exceeds maximum key count ({MAX_OBJECT_KEYS})",
                )));
            }

            for (key, nested) in map {
                if key.trim().is_empty() {
                    return Err(TenantError::InvalidSettingsSchema(
                        "tenant settings contain an empty key".to_string(),
                    ));
                }

                if key.contains('\0') {
                    return Err(TenantError::InvalidSettingsSchema(
                        "tenant settings key contains null byte".to_string(),
                    ));
                }

                if key.len() > MAX_KEY_LENGTH {
                    return Err(TenantError::InvalidSettingsSchema(format!(
                        "tenant settings key exceeds maximum length ({MAX_KEY_LENGTH})",
                    )));
                }

                validate_value(nested, depth + 1)?;
            }
        }
        Value::Array(items) => {
            if items.len() > MAX_ARRAY_ITEMS {
                return Err(TenantError::InvalidSettingsSchema(format!(
                    "tenant settings array exceeds maximum element count ({MAX_ARRAY_ITEMS})",
                )));
            }
            for nested in items {
                validate_value(nested, depth + 1)?;
            }
        }
        Value::String(s) => {
            if s.contains('\0') {
                return Err(TenantError::InvalidSettingsSchema(
                    "tenant settings string value contains null byte".to_string(),
                ));
            }
        }
        Value::Number(num) if num.as_f64().is_some_and(|f| !f.is_finite()) => {
            return Err(TenantError::InvalidSettingsSchema(
                "tenant settings contains non-finite numeric value".to_string(),
            ));
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::validate_tenant_settings;

    #[test]
    fn accepts_object_settings() {
        let settings = json!({
            "branding": {"theme": "dark", "logo": "https://example.com/logo.svg"},
            "features": {"catalog": true, "checkout": false},
            "fallback_locales": ["en", "ru"]
        });

        assert!(validate_tenant_settings(&settings).is_ok());
    }

    #[test]
    fn rejects_non_object_root() {
        let settings = json!(["invalid"]);
        let err =
            validate_tenant_settings(&settings).expect_err("schema must reject non-object root");

        assert!(err.to_string().contains("JSON object"));
    }

    #[test]
    fn rejects_nested_payload_that_is_too_deep() {
        let settings = json!({
            "l1": {"l2": {"l3": {"l4": {"l5": {"l6": {"l7": {"l8": {"l9": true}}}}}}}}
        });

        let err =
            validate_tenant_settings(&settings).expect_err("schema must reject excessive depth");
        assert!(err.to_string().contains("nesting depth"));
    }

    #[test]
    fn rejects_empty_key() {
        let settings = json!({"": "invalid"});

        let err = validate_tenant_settings(&settings).expect_err("schema must reject empty keys");
        assert!(err.to_string().contains("empty key"));
    }

    #[test]
    fn rejects_oversized_payload() {
        let large = "x".repeat(17 * 1024);
        let settings = json!({"blob": large});

        let err =
            validate_tenant_settings(&settings).expect_err("schema must reject oversized payload");
        assert!(err.to_string().contains("payload size"));
    }

    #[test]
    fn rejects_key_with_null_byte() {
        let settings = json!({"bad\0key": "val"});

        let err =
            validate_tenant_settings(&settings).expect_err("schema must reject key with null byte");
        assert!(err.to_string().contains("null byte"));
    }

    #[test]
    fn rejects_string_value_with_null_byte() {
        let settings = json!({"key": "bad\0value"});

        let err = validate_tenant_settings(&settings)
            .expect_err("schema must reject string value with null byte");
        assert!(err.to_string().contains("null byte"));
    }

    #[test]
    fn rejects_oversized_array() {
        let items = vec![1; 300];
        let settings = json!({"list": items});

        let err =
            validate_tenant_settings(&settings).expect_err("schema must reject oversized array");
        assert!(err.to_string().contains("maximum element count"));
    }
}
