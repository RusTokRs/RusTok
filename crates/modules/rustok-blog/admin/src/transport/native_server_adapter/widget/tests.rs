use super::super::test_support::{Fixture, execute, fixture};
use axum::http::StatusCode;
use leptos::server_fn::ServerFn;
use rustok_core::{SecurityContext, UserRole};
use sea_orm::{ConnectionTrait, Statement};
use serde_json::json;

#[tokio::test]
async fn native_dispatch_executes_schema_validation_preview_and_denials() {
    let Fixture {
        db,
        bus,
        host,
        tenant,
        auth,
    } = fixture().await;
    let id = rustok_blog::PostService::new(db.clone(), bus.clone())
        .create_post(
            tenant.id,
            SecurityContext::new(UserRole::Admin, Some(auth.user_id)),
            rustok_blog::CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".into(),
                title: "Native owner preview".into(),
                content: rustok_api::RichTextDocument::single_paragraph("Native body"),
                excerpt: None,
                slug: None,
                publish: false,
                tags: vec![],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
                scheduled_at: None,
            },
        )
        .await
        .unwrap();
    let editor = crate::blog_widget_contribution()
        .property_editors
        .into_iter()
        .find(|editor| editor.component_type == "blog.post_detail")
        .unwrap();
    let schema_request = super::BlogWidgetSchemaNative {
        request: crate::model::BlogWidgetPropertyRequest {
            widget_type: editor.component_type.clone(),
            property_schema: editor.property_schema.clone(),
        },
    };
    let (status, response) = execute(
        &host,
        &tenant,
        &auth,
        super::BlogWidgetSchemaNative::PATH,
        &schema_request,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["schema_id"], "blog.post_detail");
    let validation = super::BlogWidgetValidateNative {
        request: crate::model::BlogWidgetValidationRequest {
            widget_type: editor.component_type,
            property_schema: editor.property_schema,
            props: json!({"post_id": id}),
        },
    };
    let (status, response) = execute(
        &host,
        &tenant,
        &auth,
        super::BlogWidgetValidateNative::PATH,
        validation,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["valid"], true);
    let list_editor = crate::blog_widget_contribution()
        .property_editors
        .into_iter()
        .find(|editor| editor.component_type == "blog.post_list")
        .unwrap();
    for (props, valid) in [
        (json!({"per_page":7}), true),
        (json!({"per_page":"7"}), false),
    ] {
        let (status, response) = execute(
            &host,
            &tenant,
            &auth,
            super::BlogWidgetValidateNative::PATH,
            super::BlogWidgetValidateNative {
                request: crate::model::BlogWidgetValidationRequest {
                    widget_type: list_editor.component_type.clone(),
                    property_schema: list_editor.property_schema.clone(),
                    props,
                },
            },
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{response}");
        assert_eq!(
            response["valid"], valid,
            "numeric strings and integers must remain distinct: {response}"
        );
        if valid {
            assert_eq!(response["normalized_props"]["per_page"], 7);
        }
    }
    let preview = super::BlogWidgetPreviewNative {
        request: crate::model::BlogWidgetPreviewRequest {
            widget_type: "blog.post_detail".into(),
            props: json!({"post_id": id}),
        },
    };
    let (status, response) = execute(
        &host,
        &tenant,
        &auth,
        super::BlogWidgetPreviewNative::PATH,
        &preview,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["payload"]["title"], "Native owner preview");
    assert_eq!(response["payload"]["requested_locale"], "fr");
    let mut denied = auth.clone();
    denied.permissions.clear();
    assert_ne!(
        execute(
            &host,
            &tenant,
            &denied,
            super::BlogWidgetSchemaNative::PATH,
            &schema_request
        )
        .await
        .0,
        StatusCode::OK
    );
    db.execute_raw(Statement::from_string(
        db.get_database_backend(),
        "UPDATE tenant_modules SET enabled = false WHERE module_slug = 'blog'".to_string(),
    ))
    .await
    .unwrap();
    assert_ne!(
        execute(
            &host,
            &tenant,
            &auth,
            super::BlogWidgetPreviewNative::PATH,
            &preview
        )
        .await
        .0,
        StatusCode::OK
    );
}
