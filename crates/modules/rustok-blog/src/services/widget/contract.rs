use rustok_api::locale::MAX_LOCALE_TAG_LEN;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::dto::{
    AdminPostCursor, BlogWidgetCatalog, BlogWidgetInput, BlogWidgetIssue, BlogWidgetSchema,
    BlogWidgetValidation, PostSortField, PostSortOrder,
};

pub(super) const POST_LIST: &str = "blog.post_list";
pub(super) const POST_DETAIL: &str = "blog.post_detail";
pub(super) const CATEGORY_LIST: &str = "blog.category_list";
const MAX_PROPS_BYTES: usize = 16 * 1024;
const MAX_PAGE_SIZE: u64 = 100;
const MAX_CATEGORY_PAGE: u64 = 100_000;
const POST_PAGE_SIZE: u64 = 12;
const CATEGORY_PAGE_SIZE: u64 = 20;

pub(super) fn catalog() -> BlogWidgetCatalog {
    let locale =
        json!({"type": "string", "maxLength": MAX_LOCALE_TAG_LEN, "title": "Content locale"});
    let uuid = json!({"type": "string", "format": "uuid", "maxLength": 36});
    let size = |default| json!({"type": "integer", "minimum": 1, "maximum": MAX_PAGE_SIZE, "default": default});
    let schema = |widget_type: &str, properties: Value, required: Value| BlogWidgetSchema {
        widget_type: widget_type.into(),
        schema_id: widget_type.into(),
        props_schema: json!({"type": "object", "additionalProperties": false, "properties": properties, "required": required}),
    };
    BlogWidgetCatalog {
        items: vec![
            schema(
                POST_LIST,
                json!({"category_id": {"type": "string", "maxLength": 36}, "after": {"type": "string", "maxLength": 512}, "per_page": size(POST_PAGE_SIZE), "locale": locale}),
                json!([]),
            ),
            schema(
                POST_DETAIL,
                json!({"post_id": uuid, "locale": locale}),
                json!(["post_id"]),
            ),
            schema(
                CATEGORY_LIST,
                json!({"page": {"type": "integer", "minimum": 1, "maximum": MAX_CATEGORY_PAGE, "default": 1}, "per_page": size(CATEGORY_PAGE_SIZE), "locale": locale}),
                json!([]),
            ),
        ],
    }
}

pub(super) fn validate(input: BlogWidgetInput) -> BlogWidgetValidation {
    let widget_type = input.widget_type.trim().to_string();
    let mut issues = Vec::new();
    let mut props = Map::new();
    let fields: &[&str] = match widget_type.as_str() {
        POST_LIST => &["category_id", "after", "per_page", "locale"],
        POST_DETAIL => &["post_id", "locale"],
        CATEGORY_LIST => &["page", "per_page", "locale"],
        _ => {
            issue(
                &mut issues,
                "widget_type",
                "unknown_widget_type",
                "Widget is not registered by Blog",
            );
            return result(widget_type, props, issues);
        }
    };
    let Some(object) = input.props.as_object() else {
        issue(
            &mut issues,
            "props",
            "invalid_type",
            "Widget props must be an object",
        );
        return result(widget_type, props, issues);
    };
    if input.props.to_string().len() > MAX_PROPS_BYTES {
        issue(
            &mut issues,
            "props",
            "too_large",
            "Widget configuration exceeds 16 KiB",
        );
        return result(widget_type, props, issues);
    }
    for key in object.keys().filter(|key| !fields.contains(&key.as_str())) {
        issue(
            &mut issues,
            key,
            "unknown_field",
            "Field is not part of this widget configuration",
        );
    }
    if let Some(locale) = optional_text(object, "locale", &mut issues) {
        match rustok_api::normalize_locale_tag(locale) {
            Some(locale) => {
                props.insert("locale".into(), locale.into());
            }
            None => issue(
                &mut issues,
                "locale",
                "invalid_locale",
                "Content locale is invalid",
            ),
        }
    }
    match widget_type.as_str() {
        POST_LIST => {
            uuid_field(object, "category_id", false, &mut props, &mut issues);
            if let Some(cursor) = optional_text(object, "after", &mut issues) {
                if AdminPostCursor::decode(
                    cursor,
                    PostSortField::default(),
                    PostSortOrder::default(),
                )
                .is_ok()
                {
                    props.insert("after".into(), cursor.into());
                } else {
                    issue(
                        &mut issues,
                        "after",
                        "invalid_cursor",
                        "Post list cursor is invalid",
                    );
                }
            }
            integer(
                object,
                "per_page",
                POST_PAGE_SIZE,
                MAX_PAGE_SIZE,
                &mut props,
                &mut issues,
            );
        }
        POST_DETAIL => uuid_field(object, "post_id", true, &mut props, &mut issues),
        CATEGORY_LIST => {
            integer(
                object,
                "page",
                1,
                MAX_CATEGORY_PAGE,
                &mut props,
                &mut issues,
            );
            integer(
                object,
                "per_page",
                CATEGORY_PAGE_SIZE,
                MAX_PAGE_SIZE,
                &mut props,
                &mut issues,
            );
        }
        _ => issue(
            &mut issues,
            "widget_type",
            "unknown_widget_type",
            "Widget is not registered by Blog",
        ),
    }
    result(widget_type, props, issues)
}

fn result(
    widget_type: String,
    props: Map<String, Value>,
    issues: Vec<BlogWidgetIssue>,
) -> BlogWidgetValidation {
    BlogWidgetValidation {
        widget_type,
        valid: issues.is_empty(),
        normalized_props: Value::Object(props),
        issues,
    }
}

fn issue(issues: &mut Vec<BlogWidgetIssue>, path: &str, code: &str, message: &str) {
    issues.push(BlogWidgetIssue {
        class: "validation".into(),
        code: code.into(),
        message: message.into(),
        path: Some(path.into()),
    });
}

fn optional_text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    issues: &mut Vec<BlogWidgetIssue>,
) -> Option<&'a str> {
    match object.get(field) {
        None => None,
        Some(Value::String(value)) => (!value.trim().is_empty()).then(|| value.trim()),
        Some(_) => {
            issue(issues, field, "invalid_type", "Field must be a string");
            None
        }
    }
}

fn uuid_field(
    object: &Map<String, Value>,
    field: &str,
    required: bool,
    props: &mut Map<String, Value>,
    issues: &mut Vec<BlogWidgetIssue>,
) {
    match optional_text(object, field, issues) {
        Some(raw) => match Uuid::parse_str(raw) {
            Ok(id) if !id.is_nil() => {
                props.insert(field.into(), id.to_string().into());
            }
            _ => issue(
                issues,
                field,
                "invalid_uuid",
                "Field must be a non-nil UUID",
            ),
        },
        None if required => issue(issues, field, "required", "Field is required"),
        None => {}
    }
}

fn integer(
    object: &Map<String, Value>,
    field: &str,
    default: u64,
    maximum: u64,
    props: &mut Map<String, Value>,
    issues: &mut Vec<BlogWidgetIssue>,
) {
    let value = match object.get(field) {
        None => Some(default),
        Some(value) => value.as_u64(),
    };
    match value {
        Some(value) if (1..=maximum).contains(&value) => {
            props.insert(field.into(), value.into());
        }
        _ => issue(
            issues,
            field,
            "out_of_bounds",
            "Field must be an integer within its declared bounds",
        ),
    }
}
