use std::sync::Arc;

use rustok_core::{MemoryTransport, SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use uuid::Uuid;

use super::BlogWidgetService;
use crate::{BlogError, BlogWidgetInput};

fn validate(widget_type: &str, props: Value) -> crate::BlogWidgetValidation {
    BlogWidgetService::validate(BlogWidgetInput {
        widget_type: widget_type.into(),
        props,
    })
}

#[test]
fn catalog_defaults_are_accepted_and_normalization_is_idempotent() {
    let id = Uuid::new_v4().to_string();
    for schema in BlogWidgetService::catalog().items {
        assert_eq!(schema.schema_id, schema.widget_type);
        assert_eq!(schema.props_schema["additionalProperties"], false);
        let props = if schema.widget_type == "blog.post_detail" {
            json!({"post_id": id})
        } else {
            json!({})
        };
        let first = validate(&schema.widget_type, props);
        assert!(first.valid, "{:?}", first.issues);
        let second = validate(&schema.widget_type, first.normalized_props.clone());
        assert!(second.valid);
        assert_eq!(first.normalized_props, second.normalized_props);
        for (field, descriptor) in schema.props_schema["properties"].as_object().unwrap() {
            if let Some(default) = descriptor.get("default") {
                assert_eq!(&first.normalized_props[field], default);
            }
        }
    }
}

#[test]
fn tenant_principal_channel_and_unknown_configuration_are_rejected() {
    for field in ["tenant_id", "user_id", "channel_slug", "status", "unknown"] {
        let result = validate("blog.post_list", json!({field: Uuid::new_v4().to_string()}));
        assert!(!result.valid);
        assert!(
            result
                .issues
                .iter()
                .any(|issue| issue.code == "unknown_field" && issue.path.as_deref() == Some(field))
        );
    }
    assert!(!validate("other.post_list", json!({})).valid);
    for props in [Value::Null, json!([]), json!(1), json!("props")] {
        assert!(!validate("blog.post_list", props).valid);
    }
}

#[test]
fn bounds_use_strict_integers_and_do_not_silently_clamp() {
    for value in [
        json!(0),
        json!(-1),
        json!(101),
        json!(1.5),
        json!("12"),
        Value::Null,
    ] {
        assert!(!validate("blog.post_list", json!({"per_page": value})).valid);
    }
    assert!(validate("blog.post_list", json!({"per_page": 1})).valid);
    assert!(validate("blog.post_list", json!({"per_page": 100})).valid);
    assert!(!validate("blog.category_list", json!({"page": 100001})).valid);
    assert!(validate("blog.category_list", json!({"page": 100000})).valid);
}

#[test]
fn ids_cursors_and_locale_keep_the_canonical_owner_contract() {
    for props in [
        json!({}),
        json!({"post_id": ""}),
        json!({"post_id": Uuid::nil().to_string()}),
        json!({"post_id": "bad"}),
    ] {
        assert!(!validate("blog.post_detail", props).valid);
    }
    assert!(!validate("blog.post_list", json!({"after": "bad"})).valid);
    let result = validate(
        "blog.post_list",
        json!({"locale": "sl-Latn-SI-rozaj-biske", "after": "", "category_id": ""}),
    );
    assert!(result.valid, "{:?}", result.issues);
    assert_eq!(result.normalized_props["locale"], "sl-Latn-SI-biske-rozaj");
    assert!(result.normalized_props.get("after").is_none());
    assert!(result.normalized_props.get("category_id").is_none());
    assert!(!validate("blog.post_list", json!({"locale": "en-*"})).valid);
    assert!(!validate("blog.post_list", json!({"locale": "a".repeat(33)})).valid);
}

#[test]
fn optional_text_controls_use_omission_and_reject_explicit_null() {
    for field in ["locale", "after", "category_id"] {
        let omitted = validate("blog.post_list", json!({}));
        let cleared = validate("blog.post_list", json!({field: "  "}));
        assert!(cleared.valid, "{:?}", cleared.issues);
        assert_eq!(cleared.normalized_props, omitted.normalized_props);
        assert!(!validate("blog.post_list", json!({field: null})).valid);
    }
}

#[test]
fn oversized_configuration_is_rejected_before_normalization() {
    let result = validate("blog.post_list", json!({"after": "x".repeat(16 * 1024)}));
    assert!(!result.valid);
    assert_eq!(result.issues[0].code, "too_large");
}

#[tokio::test]
async fn invalid_configuration_and_permission_denial_do_not_reach_persistence() {
    let service = BlogWidgetService::new(
        DatabaseConnection::Disconnected,
        TransactionalEventBus::new(Arc::new(MemoryTransport::new())),
    );
    let tenant = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let result = service
        .preview(
            tenant,
            admin,
            "en",
            Some("en"),
            None,
            BlogWidgetInput {
                widget_type: "blog.post_detail".into(),
                props: json!({}),
            },
        )
        .await
        .unwrap();
    assert!(!result.valid);
    assert!(result.payload.is_none());
    for security in [
        SecurityContext::public_read(),
        SecurityContext::from_permissions(UserRole::Admin, Some(Uuid::new_v4()), []),
    ] {
        let result = service
            .preview(
                tenant,
                security,
                "en",
                Some("en"),
                None,
                BlogWidgetInput {
                    widget_type: "blog.post_list".into(),
                    props: json!({}),
                },
            )
            .await;
        assert!(matches!(result, Err(BlogError::Forbidden(_))));
    }
    let posts_only = SecurityContext::from_permissions(
        UserRole::Manager,
        Some(Uuid::new_v4()),
        [rustok_api::Permission::BLOG_POSTS_READ],
    );
    let result = service
        .preview(
            tenant,
            posts_only,
            "en",
            Some("en"),
            None,
            BlogWidgetInput {
                widget_type: "blog.category_list".into(),
                props: json!({}),
            },
        )
        .await;
    assert!(matches!(result, Err(BlogError::Forbidden(_))));
}
