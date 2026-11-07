//! Exercise the HTTP dispatcher rather than calling generated server functions
//! outside their request context. Fixture ownership is shared with widget tests.
use axum::http::StatusCode;
use leptos::server_fn::ServerFn;
use rustok_api::Permission;
use rustok_blog::{CreatePostInput, PostService};
use rustok_core::{SecurityContext, UserRole};
use sea_orm::{ConnectionTrait, Statement};
use serde_json::json;

use super::test_support::{execute, fixture};

#[tokio::test]
async fn native_creation_replays_the_ack_and_exposes_only_durable_rejection() {
    let fixture = fixture().await;
    let mut creator = fixture.auth.clone();
    creator.permissions = vec![Permission::BLOG_POSTS_CREATE];
    let mut draft = json!({
        "command_id": uuid::Uuid::new_v4(),
        "locale": "en",
        "title": "Replayable native creation",
        "slug": "replayable-native-creation",
        "excerpt": "",
        "content": rustok_api::RichTextDocument::single_paragraph("Body"),
        "publish": false,
        "tags": [],
        "channel_slugs": [],
        "schedule_editable": true
    });
    let path = super::BlogAdminCreatePostNative::PATH;
    let (status, scope) = execute(
        &fixture.host,
        &fixture.tenant,
        &creator,
        super::BlogAdminCreationScopeNative::PATH,
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{scope}");
    assert_eq!(scope["tenant_id"], json!(fixture.tenant.id));
    assert_eq!(scope["actor_id"], json!(creator.user_id));
    for field in ["actor_id", "tenant_id"] {
        let mut changed = scope.clone();
        changed[field] = json!(uuid::Uuid::new_v4());
        let (status, response) = execute(
            &fixture.host,
            &fixture.tenant,
            &creator,
            path,
            json!({"scope": changed, "draft": draft}),
        )
        .await;
        assert_ne!(status, StatusCode::OK, "{response}");
        assert!(
            response.to_string().contains("Blog creation scope changed"),
            "{response}"
        );
    }
    let counts = fixture
        .db
        .query_one_raw(Statement::from_string(
            fixture.db.get_database_backend(),
            "SELECT COUNT(*) AS posts FROM blog_posts",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(counts.try_get::<i64>("", "posts").unwrap(), 0);
    let (status, first) = execute(
        &fixture.host,
        &fixture.tenant,
        &creator,
        path,
        json!({"scope": scope, "draft": draft}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert!(first["Created"].as_str().is_some(), "{first}");
    let (status, replay) = execute(
        &fixture.host,
        &fixture.tenant,
        &creator,
        path,
        json!({"scope": scope, "draft": draft}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(replay, first);
    // An attempt-local permission denial cannot establish the outcome of the
    // original request and must never be encoded as a durable rejection.
    let mut denied = creator.clone();
    denied.permissions.clear();
    let (status, _) = execute(
        &fixture.host,
        &fixture.tenant,
        &denied,
        path,
        json!({"scope": scope, "draft": draft}),
    )
    .await;
    assert_ne!(status, StatusCode::OK);
    draft["command_id"] = json!(uuid::Uuid::new_v4());
    draft["title"] = json!("");
    let (status, rejected) = execute(
        &fixture.host,
        &fixture.tenant,
        &creator,
        path,
        json!({"scope": scope, "draft": draft}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rejected}");
    assert!(rejected["Rejected"].as_str().is_some(), "{rejected}");
    let (_, replay) = execute(
        &fixture.host,
        &fixture.tenant,
        &creator,
        path,
        json!({"scope": scope, "draft": draft}),
    )
    .await;
    assert_eq!(replay, rejected);
}

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
                command_id: uuid::Uuid::new_v4(),
                locale: "en".into(),
                title: "Acknowledged owner write".into(),
                content: rustok_api::RichTextDocument::single_paragraph("Body"),
                excerpt: Some("Clear this excerpt".into()),
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
            "title": "Write-only edit",
            "content": rustok_api::RichTextDocument::single_paragraph("123"),
            "excerpt": null
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
    assert_eq!(updated.content_plain_text, "123");
    assert_eq!(updated.excerpt, None);
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
    for (path, permission) in [
        (
            super::BlogAdminPublishPostNative::PATH,
            Permission::BLOG_POSTS_PUBLISH,
        ),
        (
            super::BlogAdminPinPostNative::PATH,
            Permission::BLOG_POSTS_UPDATE,
        ),
        (
            super::BlogAdminUnpinPostNative::PATH,
            Permission::BLOG_POSTS_UPDATE,
        ),
        (
            super::BlogAdminArchivePostNative::PATH,
            Permission::BLOG_POSTS_PUBLISH,
        ),
        (
            super::BlogAdminRestorePostNative::PATH,
            Permission::BLOG_POSTS_PUBLISH,
        ),
        (
            super::BlogAdminPublishPostNative::PATH,
            Permission::BLOG_POSTS_PUBLISH,
        ),
        (
            super::BlogAdminUnpublishPostNative::PATH,
            Permission::BLOG_POSTS_PUBLISH,
        ),
    ] {
        publisher.permissions = vec![permission];
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
