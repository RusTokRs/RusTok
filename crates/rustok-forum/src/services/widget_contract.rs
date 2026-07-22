use std::collections::BTreeSet;

use serde_json::{Map, Value};
use uuid::Uuid;

use crate::dto::{
    ForumWidgetCapabilityRequirements, ForumWidgetCatalogItem, ForumWidgetCatalogResponse,
    ForumWidgetCompatibilityEntry, ForumWidgetErrorMapping, ForumWidgetPropsValidationResponse,
    ForumWidgetValidationIssue, ValidateForumWidgetPropsInput,
};

pub const FORUM_WIDGET_CATALOG_VERSION: &str = "v1";
pub const FORUM_WIDGET_CONTRACT_VERSION: &str = "1.0";
pub const FORUM_WIDGET_TYPE_TOPIC_LIST: &str = "forum.topic_list";
pub const FORUM_WIDGET_TYPE_TOPIC_DETAIL: &str = "forum.topic_detail";
pub const FORUM_WIDGET_TYPE_REPLY_STREAM: &str = "forum.reply_stream";

pub const FORUM_WIDGET_ERROR_VALIDATION: &str = "forum.widget.validation";
pub const FORUM_WIDGET_ERROR_SANITIZE: &str = "forum.widget.sanitize";
pub const FORUM_WIDGET_ERROR_RBAC: &str = "forum.widget.rbac";
pub const FORUM_WIDGET_ERROR_RUNTIME: &str = "forum.widget.runtime";

#[derive(Default)]
pub struct ForumWidgetContractService;

impl ForumWidgetContractService {
    pub fn catalog() -> ForumWidgetCatalogResponse {
        ForumWidgetCatalogResponse {
            catalog_version: FORUM_WIDGET_CATALOG_VERSION.to_string(),
            builder_contract_version: FORUM_WIDGET_CONTRACT_VERSION.to_string(),
            consumer_min_version: FORUM_WIDGET_CONTRACT_VERSION.to_string(),
            compatibility_matrix: vec![ForumWidgetCompatibilityEntry {
                provider_contract_version: FORUM_WIDGET_CONTRACT_VERSION.to_string(),
                consumer_min_version: FORUM_WIDGET_CONTRACT_VERSION.to_string(),
            }],
            items: vec![
                topic_list_catalog_item(),
                topic_detail_catalog_item(),
                reply_stream_catalog_item(),
            ],
        }
    }

    pub fn validate_props(
        input: ValidateForumWidgetPropsInput,
    ) -> ForumWidgetPropsValidationResponse {
        let widget_type = input.widget_type.trim().to_ascii_lowercase();
        let mut issues = Vec::new();
        let mut normalized = Map::new();

        if widget_type.is_empty() {
            issues.push(validation_issue(
                "widget_type",
                "missing_widget_type",
                "Widget type is required",
            ));
            return validation_response(widget_type, normalized, issues);
        }

        match widget_type.as_str() {
            FORUM_WIDGET_TYPE_TOPIC_LIST => {
                validate_topic_list_props(&input.props, &mut normalized, &mut issues)
            }
            FORUM_WIDGET_TYPE_TOPIC_DETAIL => {
                validate_topic_detail_props(&input.props, &mut normalized, &mut issues)
            }
            FORUM_WIDGET_TYPE_REPLY_STREAM => {
                validate_reply_stream_props(&input.props, &mut normalized, &mut issues)
            }
            _ => issues.push(validation_issue(
                "widget_type",
                "unknown_widget_type",
                "Widget type is not part of forum catalog v1",
            )),
        }

        validation_response(widget_type, normalized, issues)
    }
}

fn validation_response(
    widget_type: String,
    normalized: Map<String, Value>,
    issues: Vec<ForumWidgetValidationIssue>,
) -> ForumWidgetPropsValidationResponse {
    let valid = issues.iter().all(|issue| issue.class != "validation");
    ForumWidgetPropsValidationResponse {
        widget_type,
        valid,
        normalized_props: Value::Object(normalized),
        issues,
    }
}

fn validate_topic_list_props(
    props: &Value,
    normalized: &mut Map<String, Value>,
    issues: &mut Vec<ForumWidgetValidationIssue>,
) {
    let object = expect_object(props, "props", issues);

    validate_optional_uuid(
        object,
        "category_id",
        normalized,
        issues,
        "category_id must be a valid UUID",
    );
    validate_u64_with_bounds(object, "page", 1, 100_000, 1, normalized, issues);
    validate_u64_with_bounds(object, "per_page", 1, 100, 20, normalized, issues);
    validate_bool(object, "include_pinned", true, normalized, issues);

    let allowed_sorts = ["activity", "newest", "top"]
        .into_iter()
        .collect::<BTreeSet<_>>();
    validate_optional_string_enum(
        object,
        "sort",
        &allowed_sorts,
        "activity",
        normalized,
        issues,
    );
}

fn validate_topic_detail_props(
    props: &Value,
    normalized: &mut Map<String, Value>,
    issues: &mut Vec<ForumWidgetValidationIssue>,
) {
    let object = expect_object(props, "props", issues);

    validate_required_uuid(
        object,
        "topic_id",
        normalized,
        issues,
        "topic_id must be a valid UUID",
    );
    validate_bool(object, "include_replies", true, normalized, issues);
    validate_optional_locale(object, "locale", normalized, issues);
}

fn validate_reply_stream_props(
    props: &Value,
    normalized: &mut Map<String, Value>,
    issues: &mut Vec<ForumWidgetValidationIssue>,
) {
    let object = expect_object(props, "props", issues);

    validate_required_uuid(
        object,
        "topic_id",
        normalized,
        issues,
        "topic_id must be a valid UUID",
    );
    validate_u64_with_bounds(object, "page", 1, 100_000, 1, normalized, issues);
    validate_u64_with_bounds(object, "per_page", 1, 100, 20, normalized, issues);
    validate_bool(object, "approved_only", true, normalized, issues);
}

fn expect_object<'a>(
    value: &'a Value,
    path: &str,
    issues: &mut Vec<ForumWidgetValidationIssue>,
) -> Option<&'a Map<String, Value>> {
    match value.as_object() {
        Some(object) => Some(object),
        None => {
            issues.push(validation_issue(
                path,
                "props_must_be_object",
                "Widget props payload must be a JSON object",
            ));
            None
        }
    }
}

fn validate_required_uuid(
    object: Option<&Map<String, Value>>,
    field: &str,
    normalized: &mut Map<String, Value>,
    issues: &mut Vec<ForumWidgetValidationIssue>,
    invalid_message: &str,
) {
    let Some(object) = object else {
        return;
    };

    match object.get(field).and_then(Value::as_str) {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed != raw {
                issues.push(sanitize_issue(
                    field,
                    "trimmed_string",
                    "String value was trimmed",
                ));
            }
            if Uuid::parse_str(trimmed).is_err() {
                issues.push(validation_issue(field, "invalid_uuid", invalid_message));
                return;
            }
            normalized.insert(field.to_string(), Value::String(trimmed.to_string()));
        }
        None => issues.push(validation_issue(
            field,
            "missing_field",
            "Required field is missing",
        )),
    }
}

fn validate_optional_uuid(
    object: Option<&Map<String, Value>>,
    field: &str,
    normalized: &mut Map<String, Value>,
    issues: &mut Vec<ForumWidgetValidationIssue>,
    invalid_message: &str,
) {
    let Some(object) = object else {
        return;
    };

    let Some(raw) = object.get(field) else {
        return;
    };

    let Some(raw) = raw.as_str() else {
        issues.push(validation_issue(
            field,
            "invalid_type",
            "Field must be a string",
        ));
        return;
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }
    if trimmed != raw {
        issues.push(sanitize_issue(
            field,
            "trimmed_string",
            "String value was trimmed",
        ));
    }
    if Uuid::parse_str(trimmed).is_err() {
        issues.push(validation_issue(field, "invalid_uuid", invalid_message));
        return;
    }
    normalized.insert(field.to_string(), Value::String(trimmed.to_string()));
}

fn validate_u64_with_bounds(
    object: Option<&Map<String, Value>>,
    field: &str,
    min: u64,
    max: u64,
    default: u64,
    normalized: &mut Map<String, Value>,
    issues: &mut Vec<ForumWidgetValidationIssue>,
) {
    let Some(object) = object else {
        normalized.insert(field.to_string(), Value::from(default));
        return;
    };

    let value = object.get(field).and_then(Value::as_u64).unwrap_or(default);
    if value < min || value > max {
        issues.push(validation_issue(
            field,
            "out_of_range",
            "Numeric value is outside allowed bounds",
        ));
        normalized.insert(field.to_string(), Value::from(default));
        return;
    }
    normalized.insert(field.to_string(), Value::from(value));
}

fn validate_bool(
    object: Option<&Map<String, Value>>,
    field: &str,
    default: bool,
    normalized: &mut Map<String, Value>,
    issues: &mut Vec<ForumWidgetValidationIssue>,
) {
    let Some(object) = object else {
        normalized.insert(field.to_string(), Value::Bool(default));
        return;
    };

    match object.get(field) {
        Some(value) => match value.as_bool() {
            Some(value) => {
                normalized.insert(field.to_string(), Value::Bool(value));
            }
            None => {
                issues.push(validation_issue(
                    field,
                    "invalid_type",
                    "Field must be boolean",
                ));
                normalized.insert(field.to_string(), Value::Bool(default));
            }
        },
        None => {
            normalized.insert(field.to_string(), Value::Bool(default));
        }
    }
}

fn validate_optional_locale(
    object: Option<&Map<String, Value>>,
    field: &str,
    normalized: &mut Map<String, Value>,
    issues: &mut Vec<ForumWidgetValidationIssue>,
) {
    let Some(object) = object else {
        return;
    };

    let Some(raw) = object.get(field).and_then(Value::as_str) else {
        return;
    };

    let trimmed = raw.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return;
    }
    if trimmed != raw {
        issues.push(sanitize_issue(
            field,
            "normalized_locale",
            "Locale code was normalized",
        ));
    }
    normalized.insert(field.to_string(), Value::String(trimmed));
}

fn validate_optional_string_enum(
    object: Option<&Map<String, Value>>,
    field: &str,
    allowed: &BTreeSet<&str>,
    default: &str,
    normalized: &mut Map<String, Value>,
    issues: &mut Vec<ForumWidgetValidationIssue>,
) {
    let Some(object) = object else {
        normalized.insert(field.to_string(), Value::String(default.to_string()));
        return;
    };

    let Some(raw) = object.get(field).and_then(Value::as_str) else {
        normalized.insert(field.to_string(), Value::String(default.to_string()));
        return;
    };

    let candidate = raw.trim().to_ascii_lowercase();
    if candidate != raw {
        issues.push(sanitize_issue(
            field,
            "trimmed_string",
            "String value was trimmed",
        ));
    }

    if allowed.contains(candidate.as_str()) {
        normalized.insert(field.to_string(), Value::String(candidate));
    } else {
        issues.push(validation_issue(
            field,
            "invalid_value",
            "Unsupported enum value",
        ));
        normalized.insert(field.to_string(), Value::String(default.to_string()));
    }
}

fn validation_issue(path: &str, code: &str, message: &str) -> ForumWidgetValidationIssue {
    ForumWidgetValidationIssue {
        class: "validation".to_string(),
        code: format!("{FORUM_WIDGET_ERROR_VALIDATION}.{code}"),
        message: message.to_string(),
        path: Some(path.to_string()),
    }
}

fn sanitize_issue(path: &str, code: &str, message: &str) -> ForumWidgetValidationIssue {
    ForumWidgetValidationIssue {
        class: "sanitize".to_string(),
        code: format!("{FORUM_WIDGET_ERROR_SANITIZE}.{code}"),
        message: message.to_string(),
        path: Some(path.to_string()),
    }
}

fn default_error_mapping() -> ForumWidgetErrorMapping {
    ForumWidgetErrorMapping {
        validation: FORUM_WIDGET_ERROR_VALIDATION.to_string(),
        sanitize: FORUM_WIDGET_ERROR_SANITIZE.to_string(),
        rbac: FORUM_WIDGET_ERROR_RBAC.to_string(),
        runtime: FORUM_WIDGET_ERROR_RUNTIME.to_string(),
    }
}

fn topic_list_catalog_item() -> ForumWidgetCatalogItem {
    ForumWidgetCatalogItem {
        widget_type: FORUM_WIDGET_TYPE_TOPIC_LIST.to_string(),
        data_contract_version: FORUM_WIDGET_CONTRACT_VERSION.to_string(),
        props_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "category_id": { "type": "string", "format": "uuid" },
                "page": { "type": "integer", "minimum": 1, "maximum": 100000, "default": 1 },
                "per_page": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 },
                "include_pinned": { "type": "boolean", "default": true },
                "sort": { "type": "string", "enum": ["activity", "newest", "top"], "default": "activity" }
            },
            "additionalProperties": false
        }),
        capability_requirements: ForumWidgetCapabilityRequirements {
            preview: true,
            publish: false,
            moderation_view: false,
        },
        fallback_mode: "readonly".to_string(),
        error_mapping: default_error_mapping(),
    }
}

fn topic_detail_catalog_item() -> ForumWidgetCatalogItem {
    ForumWidgetCatalogItem {
        widget_type: FORUM_WIDGET_TYPE_TOPIC_DETAIL.to_string(),
        data_contract_version: FORUM_WIDGET_CONTRACT_VERSION.to_string(),
        props_schema: serde_json::json!({
            "type": "object",
            "required": ["topic_id"],
            "properties": {
                "topic_id": { "type": "string", "format": "uuid" },
                "locale": { "type": "string", "minLength": 2, "maxLength": 16 },
                "include_replies": { "type": "boolean", "default": true }
            },
            "additionalProperties": false
        }),
        capability_requirements: ForumWidgetCapabilityRequirements {
            preview: true,
            publish: false,
            moderation_view: true,
        },
        fallback_mode: "degraded".to_string(),
        error_mapping: default_error_mapping(),
    }
}

fn reply_stream_catalog_item() -> ForumWidgetCatalogItem {
    ForumWidgetCatalogItem {
        widget_type: FORUM_WIDGET_TYPE_REPLY_STREAM.to_string(),
        data_contract_version: FORUM_WIDGET_CONTRACT_VERSION.to_string(),
        props_schema: serde_json::json!({
            "type": "object",
            "required": ["topic_id"],
            "properties": {
                "topic_id": { "type": "string", "format": "uuid" },
                "page": { "type": "integer", "minimum": 1, "maximum": 100000, "default": 1 },
                "per_page": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 },
                "approved_only": { "type": "boolean", "default": true }
            },
            "additionalProperties": false
        }),
        capability_requirements: ForumWidgetCapabilityRequirements {
            preview: true,
            publish: false,
            moderation_view: true,
        },
        fallback_mode: "hidden".to_string(),
        error_mapping: default_error_mapping(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        FORUM_WIDGET_TYPE_REPLY_STREAM, FORUM_WIDGET_TYPE_TOPIC_DETAIL,
        FORUM_WIDGET_TYPE_TOPIC_LIST, ForumWidgetContractService,
    };
    use crate::ValidateForumWidgetPropsInput;

    #[test]
    fn catalog_keeps_widget_types_stable() {
        let catalog = ForumWidgetContractService::catalog();
        let widget_types = catalog
            .items
            .iter()
            .map(|item| item.widget_type.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            widget_types,
            vec![
                FORUM_WIDGET_TYPE_TOPIC_LIST,
                FORUM_WIDGET_TYPE_TOPIC_DETAIL,
                FORUM_WIDGET_TYPE_REPLY_STREAM
            ]
        );
    }

    #[test]
    fn validate_topic_list_props_normalizes_sanitized_values() {
        let response = ForumWidgetContractService::validate_props(ValidateForumWidgetPropsInput {
            widget_type: FORUM_WIDGET_TYPE_TOPIC_LIST.to_string(),
            props: json!({
                "category_id": " 550e8400-e29b-41d4-a716-446655440000 ",
                "page": 3,
                "per_page": 15,
                "include_pinned": true,
                "sort": " Newest "
            }),
        });

        assert!(response.valid, "validation issues: {:?}", response.issues);
        assert_eq!(
            response.normalized_props["category_id"],
            json!("550e8400-e29b-41d4-a716-446655440000")
        );
        assert_eq!(response.normalized_props["sort"], json!("newest"));
        assert!(
            response
                .issues
                .iter()
                .any(|issue| issue.class == "sanitize")
        );
    }

    #[test]
    fn validate_topic_detail_requires_topic_id() {
        let response = ForumWidgetContractService::validate_props(ValidateForumWidgetPropsInput {
            widget_type: FORUM_WIDGET_TYPE_TOPIC_DETAIL.to_string(),
            props: json!({ "include_replies": true }),
        });

        assert!(!response.valid);
        assert!(
            response
                .issues
                .iter()
                .any(|issue| issue.code.ends_with("missing_field")),
            "missing-field validation issue expected, got {:?}",
            response.issues
        );
    }

    #[test]
    fn validate_reply_stream_rejects_out_of_range_page_size() {
        let response = ForumWidgetContractService::validate_props(ValidateForumWidgetPropsInput {
            widget_type: FORUM_WIDGET_TYPE_REPLY_STREAM.to_string(),
            props: json!({
                "topic_id": "550e8400-e29b-41d4-a716-446655440000",
                "per_page": 200
            }),
        });

        assert!(!response.valid);
        assert!(
            response
                .issues
                .iter()
                .any(|issue| issue.code.ends_with("out_of_range"))
        );
    }
}
