use super::super::test_support::{execute, fixture};
use axum::http::StatusCode;
use leptos::server_fn::ServerFn;
use rustok_api::Permission;
use serde_json::json;

#[tokio::test]
async fn native_category_commands_preserve_clear_and_keep_without_read_authority() {
    let fixture = fixture().await;
    let mut auth = fixture.auth.clone();
    auth.permissions = vec![Permission::BLOG_CATEGORIES_CREATE];
    let (status, created) = execute(
        &fixture.host,
        &fixture.tenant,
        &auth,
        super::BlogAdminCreateCategoryNative::PATH,
        super::BlogAdminCreateCategoryNative {
            draft: serde_json::from_value(json!({
                "locale":"en", "name":"News", "slug":"news", "description":"Original"
            }))
            .unwrap(),
        },
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created.as_str().unwrap().to_owned();
    auth.permissions = vec![Permission::BLOG_CATEGORIES_UPDATE];
    for (patch, expected) in [
        (json!({"description":null}), None),
        (json!({"description":"123"}), Some("123")),
        (json!({}), Some("123")),
    ] {
        let mut update = patch;
        update["locale"] = "en".into();
        let (status, response) = execute(
            &fixture.host,
            &fixture.tenant,
            &auth,
            super::BlogAdminUpdateCategoryNative::PATH,
            super::BlogAdminUpdateCategoryNative {
                id: id.clone(),
                update: serde_json::from_value(update).unwrap(),
            },
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{response}");
        assert_eq!(response["description"].as_str(), expected);
        assert_eq!(response["name"], "News");
        assert_eq!(response["slug"], "news");
    }
    let (status, _) = execute(
        &fixture.host,
        &fixture.tenant,
        &auth,
        super::BlogAdminCategoryDetailsNative::PATH,
        super::BlogAdminCategoryDetailsNative {
            locale: Some("en".into()),
        },
    )
    .await;
    assert_ne!(status, StatusCode::OK);
}
