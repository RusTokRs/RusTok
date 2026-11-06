mod support;

use std::sync::Arc;

use async_graphql::{EmptySubscription, Request, Schema};
use axum::{Extension, Router, body::Body, http::StatusCode};
use rustok_api::{AuthContext, RequestContext, RichTextDocument, TenantContext};
use rustok_blog::{
    BlogMutation, BlogQuery, BlogWidgetInput, BlogWidgetPayload, BlogWidgetService,
    CategoryService, CreateCategoryInput, CreatePostInput, PostService,
};
use rustok_core::{SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, Statement,
    sea_query::{Alias, Expr, Query},
};
use sea_orm_migration::SchemaManager;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

type Fixture = (
    DatabaseConnection,
    TransactionalEventBus,
    TenantContext,
    AuthContext,
);

async fn fixture() -> Fixture {
    fixture_on(rustok_test_utils::setup_test_db().await).await
}

async fn fixture_on(db: DatabaseConnection) -> Fixture {
    support::apply_blog_schema(&db).await.unwrap();
    rustok_modules::migrations::migrations()
        .into_iter()
        .find(|migration| migration.name() == "m20250101_000060_create_tenant_modules")
        .unwrap()
        .up(&SchemaManager::new(&db))
        .await
        .unwrap();
    let tenant = TenantContext {
        id: Uuid::new_v4(),
        name: "Widget fixture".into(),
        slug: "widgets".into(),
        domain: None,
        settings: json!({}),
        default_locale: "en".into(),
        is_active: true,
    };
    let mut insert = Query::insert();
    insert
        .into_table(Alias::new("tenants"))
        .columns([Alias::new("id"), Alias::new("name"), Alias::new("slug")])
        .values_panic([
            Expr::value(tenant.id),
            Expr::value(&tenant.name),
            Expr::value(&tenant.slug),
        ]);
    db.execute_raw(db.get_database_backend().build(&insert))
        .await
        .unwrap();
    let mut insert = Query::insert();
    insert
        .into_table(Alias::new("tenant_modules"))
        .columns([
            Alias::new("id"),
            Alias::new("tenant_id"),
            Alias::new("module_slug"),
            Alias::new("enabled"),
        ])
        .values_panic([
            Expr::value(Uuid::new_v4()),
            Expr::value(tenant.id),
            Expr::value("blog"),
            Expr::value(true),
        ]);
    db.execute_raw(db.get_database_backend().build(&insert))
        .await
        .unwrap();
    let auth = AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        tenant_id: tenant.id,
        permissions: rustok_core::Rbac::permissions_for_role(&UserRole::Admin)
            .iter()
            .copied()
            .collect(),
        client_id: None,
        scopes: vec![],
        grant_type: "direct".into(),
    };
    let bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (db, bus, tenant, auth)
}

fn input(widget_type: &str, props: Value) -> BlogWidgetInput {
    BlogWidgetInput {
        widget_type: widget_type.into(),
        props,
    }
}

fn request_context(tenant: &TenantContext, auth: &AuthContext) -> RequestContext {
    RequestContext {
        tenant_id: tenant.id,
        user_id: Some(auth.user_id),
        channel_id: None,
        channel_slug: None,
        channel_resolution_source: None,
        locale: "fr".into(),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

fn service_auth(auth: &AuthContext) -> AuthContext {
    AuthContext {
        // OAuth app subject and client identity are distinct from any human author.
        user_id: Uuid::new_v4(),
        session_id: Uuid::nil(),
        client_id: Some(Uuid::new_v4()),
        scopes: vec!["*".into()],
        grant_type: "client_credentials".into(),
        ..auth.clone()
    }
}

async fn create_post(
    posts: &PostService,
    tenant: Uuid,
    author: Uuid,
    title: &str,
    category: Option<Uuid>,
) -> Uuid {
    posts
        .create_post(
            tenant,
            SecurityContext::new(UserRole::Admin, Some(author)),
            CreatePostInput {
                command_id: uuid::Uuid::new_v4(),
                locale: "en".into(),
                title: title.into(),
                content: RichTextDocument::single_paragraph("Owner document"),
                excerpt: None,
                slug: None,
                publish: false,
                tags: vec![],
                category_id: category,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
                scheduled_at: None,
            },
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn preview_uses_migrated_owner_reads_and_never_changes_rows_or_events() {
    owner_preview_contract(fixture().await).await;
}

async fn owner_preview_contract((db, bus, tenant, auth): Fixture) {
    let admin = SecurityContext::new(UserRole::Admin, Some(auth.user_id));
    let category = CategoryService::new(db.clone(), bus.clone())
        .create(
            tenant.id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "Owner category".into(),
                slug: None,
                description: Some("Canonical copy".into()),
                parent_id: None,
                position: None,
                settings: json!({}),
            },
        )
        .await
        .unwrap();
    let posts = PostService::new(db.clone(), bus.clone());
    let first = create_post(&posts, tenant.id, auth.user_id, "First", Some(category)).await;
    create_post(&posts, tenant.id, auth.user_id, "Second", Some(category)).await;
    let before = posts
        .get_post(tenant.id, admin.clone(), first, "en")
        .await
        .unwrap();
    let count = db
        .query_one_raw(Statement::from_string(
            db.get_database_backend(),
            "SELECT COUNT(*) AS count FROM sys_events".to_string(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "count")
        .unwrap();
    let service = BlogWidgetService::new(db.clone(), bus);
    let auth_service = service_auth(&auth);
    let service_security = rustok_core::security_context_from_access_token(
        auth_service.user_id,
        &auth_service.grant_type,
        &auth_service.permissions,
    );
    assert_eq!(service_security.user_id, None);
    let preview = service
        .preview(
            tenant.id,
            service_security,
            "fr",
            Some("en"),
            None,
            input("blog.post_detail", json!({"post_id": first})),
        )
        .await
        .unwrap();
    let Some(BlogWidgetPayload::PostDetail(preview)) = preview.payload else {
        panic!("authenticated service owner detail expected")
    };
    assert_eq!(preview.id, first);
    assert_eq!(preview.title, "First");
    let denied = service
        .preview(
            tenant.id,
            SecurityContext::service(
                UserRole::Customer,
                [rustok_api::Permission::BLOG_POSTS_READ],
            ),
            "en",
            Some("en"),
            None,
            input("blog.post_detail", json!({"post_id": first})),
        )
        .await;
    assert!(matches!(denied, Err(rustok_blog::BlogError::Forbidden(_))));
    let list = service
        .preview(
            tenant.id,
            admin.clone(),
            "fr",
            Some("en"),
            None,
            input(
                "blog.post_list",
                json!({"category_id": category, "per_page": 1}),
            ),
        )
        .await
        .unwrap();
    let Some(BlogWidgetPayload::PostList(page)) = list.payload else {
        panic!("post page expected")
    };
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].effective_locale, "en");
    let next = service
        .preview(
            tenant.id,
            admin.clone(),
            "fr",
            Some("en"),
            None,
            input(
                "blog.post_list",
                json!({"category_id": category, "per_page": 1, "after": page.next_cursor.unwrap()}),
            ),
        )
        .await
        .unwrap();
    let Some(BlogWidgetPayload::PostList(next)) = next.payload else {
        panic!("post page expected")
    };
    assert_eq!(next.items.len(), 1);
    assert_ne!(page.items[0].id, next.items[0].id);
    assert!(next.next_cursor.is_none());
    let detail = service
        .preview(
            tenant.id,
            admin.clone(),
            "fr",
            Some("en"),
            None,
            input("blog.post_detail", json!({"post_id": first})),
        )
        .await
        .unwrap();
    let Some(BlogWidgetPayload::PostDetail(detail)) = detail.payload else {
        panic!("post detail expected")
    };
    assert_eq!(detail.requested_locale, "fr");
    assert_eq!(detail.effective_locale, "en");
    assert_eq!(detail.available_locales, ["en"]);
    let categories = service
        .preview(
            tenant.id,
            admin.clone(),
            "fr",
            Some("en"),
            None,
            input("blog.category_list", json!({})),
        )
        .await
        .unwrap();
    let Some(BlogWidgetPayload::CategoryList { items, total, .. }) = categories.payload else {
        panic!("category page expected")
    };
    assert_eq!(total, 1);
    assert_eq!(items[0].name, "Owner category");
    assert_eq!(items[0].effective_locale, "en");
    let foreign = service
        .preview(
            Uuid::new_v4(),
            admin.clone(),
            "en",
            Some("en"),
            None,
            input("blog.post_detail", json!({"post_id": first})),
        )
        .await;
    assert!(matches!(
        foreign,
        Err(rustok_blog::BlogError::PostNotFound(_))
    ));
    let after = posts.get_post(tenant.id, admin, first, "en").await.unwrap();
    assert_eq!(before.version, after.version);
    assert_eq!(before.updated_at, after.updated_at);
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
    assert_eq!(
        count,
        db.query_one_raw(Statement::from_string(
            db.get_database_backend(),
            "SELECT COUNT(*) AS count FROM sys_events".to_string()
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "count")
        .unwrap()
    );
}

#[tokio::test]
async fn public_scope_preview_applies_channel_visibility_before_keyset_limits() {
    let (db, bus, tenant, auth) = fixture().await;
    for slug in ["web", "mobile"] {
        rustok_channel::ChannelService::new(db.clone())
            .create_channel(rustok_channel::CreateChannelInput {
                tenant_id: tenant.id,
                slug: slug.into(),
                name: slug.into(),
                settings: None,
            })
            .await
            .unwrap();
    }
    let posts = PostService::new(db.clone(), bus.clone());
    let admin = SecurityContext::new(UserRole::Admin, Some(auth.user_id));
    let mut expected = Vec::new();
    let mut hidden = None;
    for (title, channel, published) in [
        ("Visible A", "web", true),
        ("Visible B", "web", true),
        ("Hidden mobile", "mobile", true),
        ("Hidden draft", "web", false),
    ] {
        let id = posts
            .create_post(
                tenant.id,
                admin.clone(),
                CreatePostInput {
                    command_id: uuid::Uuid::new_v4(),
                    locale: "en".into(),
                    title: title.into(),
                    content: RichTextDocument::single_paragraph("Channel body"),
                    excerpt: None,
                    slug: None,
                    publish: published,
                    tags: vec![],
                    category_id: None,
                    featured_image_url: None,
                    seo_title: None,
                    seo_description: None,
                    channel_slugs: Some(vec![channel.into()]),
                    metadata: None,
                    scheduled_at: None,
                },
            )
            .await
            .unwrap();
        if channel == "web" && published {
            expected.push(id);
        }
        if channel == "mobile" {
            hidden = Some(id);
        }
    }
    let service = BlogWidgetService::new(db, bus);
    let viewer = SecurityContext::from_permissions(
        UserRole::Customer,
        Some(Uuid::new_v4()),
        [
            rustok_api::Permission::BLOG_POSTS_READ,
            rustok_api::Permission::BLOG_POSTS_LIST,
        ],
    );
    let mut after = None;
    let mut actual = Vec::new();
    loop {
        let mut props = json!({"per_page": 1});
        if let Some(after) = after {
            props["after"] = Value::String(after);
        }
        let response = service
            .preview(
                tenant.id,
                viewer.clone(),
                "fr",
                Some("en"),
                Some("web"),
                input("blog.post_list", props),
            )
            .await
            .unwrap();
        let Some(BlogWidgetPayload::PostList(page)) = response.payload else {
            panic!("owner post page expected")
        };
        assert_eq!(page.items.len(), 1);
        actual.push(page.items[0].id);
        after = page.next_cursor;
        if after.is_none() {
            break;
        }
        assert!(actual.len() < 3);
    }
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    let denied = service
        .preview(
            tenant.id,
            viewer,
            "en",
            Some("en"),
            Some("web"),
            input("blog.post_detail", json!({"post_id": hidden.unwrap()})),
        )
        .await;
    assert!(matches!(denied, Err(rustok_blog::BlogError::Forbidden(_))));
}

#[tokio::test]
async fn graphql_creation_replays_acknowledgement_and_marks_durable_rejections() {
    let (db, bus, tenant, auth) = fixture().await;
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
    let first = schema.execute(request(input.clone(), creator.clone())).await;
    assert!(first.errors.is_empty(), "{:?}", first.errors);
    let replay = schema.execute(request(input.clone(), creator.clone())).await;
    assert!(replay.errors.is_empty(), "{:?}", replay.errors);
    assert_eq!(replay.data, first.data);
    let count = db.query_one_raw(Statement::from_string(
        db.get_database_backend(), "SELECT COUNT(*) AS count FROM blog_posts",
    )).await.unwrap().unwrap().try_get::<i64>("", "count").unwrap();
    assert_eq!(count, 1);
    let mut denied = creator.clone();
    denied.permissions.clear();
    let denied = schema.execute(request(input.clone(), denied)).await;
    assert!(!denied.errors.is_empty());
    assert!(denied.errors.iter().all(|error| error.extensions.as_ref()
        .and_then(|extensions| extensions.get("outcome")).is_none()));
    input["commandId"] = json!(Uuid::new_v4());
    input["title"] = json!("");
    let rejected = schema.execute(request(input.clone(), creator.clone())).await;
    assert_eq!(rejected.errors.len(), 1, "{:?}", rejected.errors);
    assert_eq!(rejected.errors[0].extensions.as_ref().unwrap().get("outcome"),
        Some(&async_graphql::Value::from("rejected")));
    let replay = schema.execute(request(input, creator)).await;
    assert_eq!(replay.errors, rejected.errors);
}

#[tokio::test]
async fn graphql_executes_the_owner_contract_and_fails_closed_on_context_or_module_denial() {
    let (db, bus, tenant, auth) = fixture().await;
    let posts = PostService::new(db.clone(), bus.clone());
    let id = create_post(&posts, tenant.id, auth.user_id, "GraphQL preview", None).await;
    let schema = Schema::build(BlogQuery, BlogMutation, EmptySubscription)
        .data(db.clone())
        .data(bus)
        .finish();
    let document = "query($id: JSON!) { blogWidgetCatalog validateBlogWidget(widgetType: \"blog.post_list\", props: {}) previewBlogWidget(widgetType: \"blog.post_detail\", props: $id) }";
    let request = || {
        Request::new(document)
            .variables(async_graphql::Variables::from_json(
                json!({"id": {"post_id": id}}),
            ))
            .data(tenant.clone())
            .data(request_context(&tenant, &auth))
    };
    let response = schema.execute(request().data(auth.clone())).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(
        data["blogWidgetCatalog"]["items"].as_array().unwrap().len(),
        3
    );
    assert_eq!(
        data["validateBlogWidget"]["normalized_props"]["per_page"],
        12
    );
    assert_eq!(
        data["previewBlogWidget"]["payload"]["title"],
        "GraphQL preview"
    );
    let auth_service = service_auth(&auth);
    let response = schema
        .execute(
            request()
                .data(request_context(&tenant, &auth_service))
                .data(auth_service),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["previewBlogWidget"]["payload"]["title"],
        "GraphQL preview"
    );
    let anonymous = schema.execute(request()).await;
    assert!(!anonymous.errors.is_empty());
    let mut no_read = auth.clone();
    no_read.permissions.clear();
    assert!(
        !schema
            .execute(request().data(no_read))
            .await
            .errors
            .is_empty()
    );
    for mismatch in ["tenant", "principal"] {
        let mut context = request_context(&tenant, &auth);
        if mismatch == "tenant" {
            context.tenant_id = Uuid::new_v4();
        } else {
            context.user_id = Some(Uuid::new_v4());
        }
        let denied = schema
            .execute(request().data(auth.clone()).data(context))
            .await;
        assert!(
            !denied.errors.is_empty(),
            "{mismatch} mismatch was admitted"
        );
    }
    let mut foreign_auth = auth.clone();
    foreign_auth.tenant_id = Uuid::new_v4();
    assert!(
        !schema
            .execute(request().data(foreign_auth))
            .await
            .errors
            .is_empty()
    );
    db.execute_raw(Statement::from_string(
        db.get_database_backend(),
        "UPDATE tenant_modules SET enabled = false WHERE module_slug = 'blog'".to_string(),
    ))
    .await
    .unwrap();
    let denied = schema.execute(request().data(auth)).await;
    assert!(!denied.errors.is_empty());
}

struct UnusedCategoryCleanup;

#[async_trait::async_trait]
impl rustok_taxonomy::TaxonomyCategoryDeleteCleanupPort for UnusedCategoryCleanup {
    async fn cleanup_in_tx(
        &self,
        _txn: &sea_orm::DatabaseTransaction,
        _tenant: Uuid,
        _category: Uuid,
    ) -> rustok_taxonomy::TaxonomyResult<()> {
        Err(rustok_taxonomy::TaxonomyError::validation(
            "Widget preview must never delete a category",
        ))
    }
}

fn router(
    db: DatabaseConnection,
    bus: TransactionalEventBus,
    tenant: TenantContext,
    auth: Option<AuthContext>,
) -> Router {
    let cleanup: Arc<dyn rustok_taxonomy::TaxonomyCategoryDeleteCleanupPort> =
        Arc::new(UnusedCategoryCleanup);
    let host = rustok_api::HostRuntimeContext::new(db)
        .with_shared_value(bus)
        .with_shared_value(cleanup);
    let router = rustok_blog::controllers::axum_router(&host)
        .unwrap()
        .layer(Extension(rustok_api::context::TenantContextExtension(
            tenant,
        )))
        .layer(Extension(rustok_api::ResolvedRequestLocale {
            requested_locale: Some("fr".into()),
            effective_locale: "fr".into(),
        }));
    match auth {
        Some(auth) => router.layer(Extension(rustok_api::context::AuthContextExtension(auth))),
        None => router,
    }
}

async fn http(router: Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(if method == "GET" {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

#[tokio::test]
async fn real_rest_router_serves_catalog_validation_preview_and_explicit_denials() {
    let (db, bus, tenant, auth) = fixture().await;
    let posts = PostService::new(db.clone(), bus.clone());
    let id = create_post(&posts, tenant.id, auth.user_id, "HTTP preview", None).await;
    let app = router(db.clone(), bus.clone(), tenant.clone(), Some(auth.clone()));
    let (status, catalog) =
        http(app.clone(), "GET", "/api/blog/widgets/catalog", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(catalog["items"].as_array().unwrap().len(), 3);
    let (status, validation) = http(
        app.clone(),
        "POST",
        "/api/blog/widgets/validate",
        json!({"widget_type": "blog.post_list", "props": {"tenant_id": tenant.id}}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(validation["valid"], false);
    let (status, preview) = http(
        app,
        "POST",
        "/api/blog/widgets/preview",
        json!({"widget_type": "blog.post_detail", "props": {"post_id": id}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview["payload"]["title"], "HTTP preview");
    assert_eq!(preview["payload"]["requested_locale"], "fr");
    let auth_service = service_auth(&auth);
    let (status, preview) = http(
        router(
            db.clone(),
            bus.clone(),
            tenant.clone(),
            Some(auth_service.clone()),
        ),
        "POST",
        "/api/blog/widgets/preview",
        json!({"widget_type": "blog.post_detail", "props": {"post_id": id}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["payload"]["title"], "HTTP preview");
    let mut no_service_read = auth_service;
    no_service_read.permissions.clear();
    assert_eq!(
        http(
            router(
                db.clone(),
                bus.clone(),
                tenant.clone(),
                Some(no_service_read)
            ),
            "POST",
            "/api/blog/widgets/preview",
            json!({"widget_type": "blog.post_detail", "props": {"post_id": id}})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let mut no_read = auth.clone();
    no_read.permissions.clear();
    assert_eq!(
        http(
            router(db.clone(), bus.clone(), tenant.clone(), Some(no_read)),
            "GET",
            "/api/blog/widgets/catalog",
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        http(
            router(db.clone(), bus.clone(), tenant.clone(), None),
            "GET",
            "/api/blog/widgets/catalog",
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    db.execute_raw(Statement::from_string(
        db.get_database_backend(),
        "UPDATE tenant_modules SET enabled = false WHERE module_slug = 'blog'".to_string(),
    ))
    .await
    .unwrap();
    assert_eq!(
        http(
            router(db, bus, tenant, Some(auth)),
            "GET",
            "/api/blog/widgets/catalog",
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL and disposable-database permission"]
async fn postgres_widget_owner_preview_contract() {
    use rustok_test_utils::db::{
        connect_postgres, create_postgres_database, drop_postgres_database_if_exists,
        postgres_database_url, unique_postgres_database_name,
    };
    let url = support::postgres_database_url().unwrap();
    let control = connect_postgres(&url).await.unwrap();
    let name = unique_postgres_database_name("blog_widgets");
    create_postgres_database(&control, &name).await.unwrap();
    let url = postgres_database_url(&url, &name);
    let outcome = tokio::spawn(async move {
        owner_preview_contract(fixture_on(connect_postgres(&url).await.unwrap()).await).await;
    })
    .await;
    drop_postgres_database_if_exists(&control, &name)
        .await
        .unwrap();
    if let Err(error) = outcome {
        std::panic::resume_unwind(error.into_panic());
    }
}
