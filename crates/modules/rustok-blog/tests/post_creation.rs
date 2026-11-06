mod support;
#[path = "support/transport.rs"]
mod transport;

use async_graphql::{EmptySubscription, Request, Schema};
use axum::http::StatusCode;
use rustok_api::{AuthContext, RichTextDocument};
use rustok_blog::{BlogMutation, BlogQuery};
use sea_orm::{ConnectionTrait, Statement};
use serde_json::{Value, json};
use transport::{Fixture, fixture, fixture_on, http, request_context, router};
use uuid::Uuid;

#[tokio::test]
async fn rest_creation_replays_the_original_id_and_refuses_changed_input() {
    let (db, bus, tenant, mut auth) = fixture().await;
    auth.permissions = vec![rustok_api::Permission::BLOG_POSTS_CREATE];
    let app = router(db.clone(), bus, tenant, Some(auth));
    let mut input = json!({
        "command_id": Uuid::new_v4(), "locale": "en", "title": "REST creation",
        "content": RichTextDocument::single_paragraph("Body"), "publish": false, "tags": []
    });
    let (status, first) = http(app.clone(), "POST", "/api/blog/posts", input.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    assert!(!Uuid::parse_str(first.as_str().unwrap()).unwrap().is_nil());
    let (status, replay) = http(app.clone(), "POST", "/api/blog/posts", input.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{replay}");
    assert_eq!(replay, first);
    input["title"] = json!("Different input");
    let (status, _) = http(app.clone(), "POST", "/api/blog/posts", input.clone()).await;
    assert_eq!(status, StatusCode::CONFLICT);
    input.as_object_mut().unwrap().remove("command_id");
    let (status, _) = http(app, "POST", "/api/blog/posts", input).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let counts = db
        .query_one_raw(Statement::from_string(
            db.get_database_backend(),
            "SELECT (SELECT COUNT(*) FROM blog_posts) AS posts, \
         (SELECT COUNT(*) FROM sys_events) AS events",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(counts.try_get::<i64>("", "posts").unwrap(), 1);
    assert_eq!(counts.try_get::<i64>("", "events").unwrap(), 1);
}

#[tokio::test]
async fn graphql_creation_replays_acknowledgement_and_marks_durable_rejections() {
    graphql_creation_contract(fixture().await).await;
}

async fn graphql_creation_contract((db, bus, tenant, auth): Fixture) {
    let schema = Schema::build(BlogQuery, BlogMutation, EmptySubscription)
        .data(db.clone())
        .data(bus)
        .finish();
    let mut input = json!({
        "commandId": Uuid::new_v4(), "locale": "en", "title": "Replayable creation",
        "content": RichTextDocument::single_paragraph("Body"), "publish": false, "tags": []
    });
    let request = |input: Value, auth: AuthContext| {
        Request::new("mutation($input: CreatePostInput!) { createPost(input: $input) }")
            .variables(async_graphql::Variables::from_json(json!({"input": input})))
            .data(tenant.clone())
            .data(request_context(&tenant, &auth))
            .data(auth)
    };
    let mut creator = auth;
    creator.permissions = vec![rustok_api::Permission::BLOG_POSTS_CREATE];
    let first = schema
        .execute(request(input.clone(), creator.clone()))
        .await;
    assert!(first.errors.is_empty(), "{:?}", first.errors);
    let replay = schema
        .execute(request(input.clone(), creator.clone()))
        .await;
    assert!(replay.errors.is_empty(), "{:?}", replay.errors);
    assert_eq!(replay.data, first.data);
    let count = db
        .query_one_raw(Statement::from_string(
            db.get_database_backend(),
            "SELECT COUNT(*) AS count FROM blog_posts",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "count")
        .unwrap();
    assert_eq!(count, 1);
    let mut denied = creator.clone();
    denied.permissions.clear();
    let denied = schema.execute(request(input.clone(), denied)).await;
    assert!(!denied.errors.is_empty());
    assert!(denied.errors.iter().all(|error| {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("outcome"))
            .is_none()
    }));
    let mut missing_command = input.clone();
    missing_command.as_object_mut().unwrap().remove("commandId");
    let missing = schema
        .execute(request(missing_command, creator.clone()))
        .await;
    assert!(!missing.errors.is_empty());
    assert!(missing.errors.iter().all(|error| {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("outcome"))
            .is_none()
    }));
    input["commandId"] = json!(Uuid::new_v4());
    input["title"] = json!("");
    let rejected = schema
        .execute(request(input.clone(), creator.clone()))
        .await;
    assert_eq!(rejected.errors.len(), 1, "{:?}", rejected.errors);
    assert_eq!(
        rejected.errors[0]
            .extensions
            .as_ref()
            .unwrap()
            .get("outcome"),
        Some(&async_graphql::Value::from("rejected"))
    );
    let replay = schema.execute(request(input, creator)).await;
    assert_eq!(replay.errors, rejected.errors);
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL and disposable-database permission"]
async fn postgres_graphql_creation_contract() {
    use rustok_test_utils::db::{
        connect_postgres, create_postgres_database, drop_postgres_database_if_exists,
        postgres_database_url, unique_postgres_database_name,
    };
    let control_url = support::postgres_database_url().unwrap();
    let control = connect_postgres(&control_url).await.unwrap();
    let name = unique_postgres_database_name("blog_creation_transport");
    create_postgres_database(&control, &name).await.unwrap();
    let url = postgres_database_url(&control_url, &name);
    let result = tokio::spawn(async move {
        graphql_creation_contract(fixture_on(connect_postgres(&url).await.unwrap()).await).await;
    })
    .await;
    drop_postgres_database_if_exists(&control, &name)
        .await
        .unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error.into_panic());
    }
}
