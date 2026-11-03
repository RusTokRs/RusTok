use rustok_core::i18n::{translate, Locale};
use serde_json::{json, Value};
/// Validation helpers with i18n support
///
/// Provides utilities to convert validation errors to localized messages
use validator::ValidationErrors;

/// Format validation errors with i18n support
pub fn format_validation_errors(errors: &ValidationErrors, locale: Locale) -> Value {
    let mut error_map = serde_json::Map::new();

    for (field, field_errors) in errors.field_errors() {
        let mut messages = Vec::new();

        for error in field_errors {
            let message = if let Some(msg) = &error.message {
                msg.to_string()
            } else {
                // Translate error code
                translate(locale, error.code.as_ref())
            };

            messages.push(message);
        }

        error_map.insert(field.to_string(), json!(messages));
    }

    json!({
        "validation_errors": error_map
    })
}

/// Format a single validation error message
pub fn format_single_error(error: &validator::ValidationError, locale: Locale) -> String {
    if let Some(msg) = &error.message {
        msg.to_string()
    } else {
        translate(locale, error.code.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::validation::*;
    use validator::Validate;

    #[derive(Validate)]
    struct TestStruct {
        #[validate(custom(function = "validate_kind"))]
        kind: String,
    }

    #[test]
    fn test_format_validation_errors() {
        let test = TestStruct {
            kind: "invalid".to_string(),
        };

        if let Err(errors) = test.validate() {
            let formatted = format_validation_errors(&errors, Locale::En);
            assert!(formatted["validation_errors"]["kind"].is_array());
        }
    }
}
