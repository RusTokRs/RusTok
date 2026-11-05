//! Exercise the HTTP dispatcher rather than calling generated server functions
//! outside their request context. Fixture ownership is shared with widget tests.
use axum::http::StatusCode;
use leptos::server_fn::ServerFn;
use rustok_api::Permission;
use rustok_blog::{CreatePostInput, PostService};
use rustok_core::{SecurityContext, UserRole};
use serde_json::json;

use super::test_support::{execute, fixture};

#[tokio::test]
async fn native_writes_acknowledge_write_only_actors_without_detail_read() {
    let fixture = fixture().await;
    let service = PostService::new(fixture.db.clone(), fixture.bus.clone());
    let admin = SecurityContext::new(UserRole::Admin, Some(fixture.auth.user_id));
    let id = service
        .create_post(
            fixture.tenant.id,
            admin.clone(),
            CreatePostInput {
                locale: "en".into(),
                title: "Acknowledged owner write".into(),
                content: rustok_api::RichTextDocument::single_paragraph("Body"),
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
    let before = service
        .get_post(fixture.tenant.id, admin.clone(), id, "en")
        .await
        .unwrap();
    let mut writer = fixture.auth.clone();
    writer.permissions = vec![Permission::BLOG_POSTS_UPDATE];
    let update = super::BlogAdminUpdatePostNative {
        id: id.to_string(),
        update: serde_json::from_value(json!({
            "version": before.version,
            "locale": "en",
            "title": "Write-only edit"
        }))
        .unwrap(),
    };
    let (status, response) = execute(
        &fixture.host,
        &fixture.tenant,
        &writer,
        super::BlogAdminUpdatePostNative::PATH,
        &update,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(response.is_null(), "unit acknowledgement: {response}");
    let updated = service
        .get_post(fixture.tenant.id, admin.clone(), id, "en")
        .await
        .unwrap();
    assert_eq!(updated.title, "Write-only edit");
    assert_eq!(updated.version, before.version + 1);
    // A separate refresh still enforces Read. Its rejection cannot retroactively
    // turn the successful owner command into a failed save acknowledgement.
    let (status, _) = execute(
        &fixture.host,
        &fixture.tenant,
        &writer,
        super::BlogAdminPostNative::PATH,
        &super::BlogAdminPostNative {
            id: id.to_string(),
            locale: Some("en".into()),
        },
    )
    .await;
    assert_ne!(status, StatusCode::OK);

    let mut publisher = fixture.auth.clone();
    publisher.permissions = vec![Permission::BLOG_POSTS_PUBLISH];
    for path in [
        super::BlogAdminPublishPostNative::PATH,
        super::BlogAdminPinPostNative::PATH,
        super::BlogAdminUnpinPostNative::PATH,
        super::BlogAdminArchivePostNative::PATH,
        super::BlogAdminRestorePostNative::PATH,
        super::BlogAdminPublishPostNative::PATH,
        super::BlogAdminUnpublishPostNative::PATH,
    ] {
        let (status, response) = execute(
            &fixture.host,
            &fixture.tenant,
            &publisher,
            path,
            &json!({"id":id}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{path}: {response}");
        assert!(response.is_null(), "{path}: {response}");
    }
    let final_post = service
        .get_post(fixture.tenant.id, admin, id, "en")
        .await
        .unwrap();
    assert_eq!(final_post.title, "Write-only edit");
    assert_eq!(final_post.status, rustok_blog::BlogPostStatus::Draft);
}
