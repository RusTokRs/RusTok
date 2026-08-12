use axum::{
    extract::{Path, Query, Request, State},
    http::{
        StatusCode,
        header::{CACHE_CONTROL, LOCATION},
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use rustok_api::{
    AuthContext, Permission, RequestContext, TenantContext, has_any_effective_permission,
};
use rustok_web::HttpResult;
use uuid::Uuid;

use crate::ListTopicsFilter;

use super::ForumHttpRuntime;

#[utoipa::path(
    get,
    path = "/api/forum/topics/{id}",
    tag = "forum",
    params(
        ("id" = Uuid, Path, description = "Topic ID"),
        ("locale" = Option<String>, Query, description = "Locale")
    ),
    responses(
        (status = 200, description = "Topic details", body = crate::TopicResponse),
        (
            status = 308,
            description = "Merged source topic redirects to the retained canonical topic",
            headers(
                ("Location" = String, description = "Tenant-relative canonical topic API path"),
                ("Cache-Control" = String, description = "Private no-store redirect policy")
            )
        ),
        (status = 404, description = "Topic not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(crate) async fn redirect_merged_topic(
    State(runtime): State<ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(topic_id): Path<Uuid>,
    Query(filter): Query<ListTopicsFilter>,
    request: Request,
    next: Next,
) -> HttpResult<Response> {
    if !has_any_effective_permission(&auth.permissions, &[Permission::FORUM_TOPICS_READ]) {
        return Ok(next.run(request).await);
    }

    let security = rustok_core::SecurityContext::from_permission_snapshot(
        Some(auth.user_id),
        &auth.permissions,
    );
    let service = runtime.topic_service();
    let resolution = service
        .resolve_canonical_topic(tenant.id, security.clone(), topic_id)
        .await
        .map_err(super::map_forum_error)?;

    if !resolution.redirected {
        return Ok(next.run(request).await);
    }

    let locale = filter
        .locale
        .as_deref()
        .unwrap_or(request_context.locale.as_str());
    let canonical_topic = service
        .get_with_locale_fallback(
            tenant.id,
            security,
            resolution.canonical_topic_id,
            locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(super::map_forum_error)?;

    let location = canonical_topic_location(canonical_topic.id, filter.locale.as_deref());
    Ok((
        StatusCode::PERMANENT_REDIRECT,
        [
            (LOCATION, location),
            (CACHE_CONTROL, "private, no-store".to_string()),
        ],
    )
        .into_response())
}

fn canonical_topic_location(topic_id: Uuid, locale: Option<&str>) -> String {
    let path = format!("/api/forum/topics/{topic_id}");
    let Some(locale) = locale.filter(|locale| !locale.is_empty()) else {
        return path;
    };

    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query.append_pair("locale", locale);
    format!("{path}?{}", query.finish())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{
            Method, Request, StatusCode,
            header::{CACHE_CONTROL, LOCATION},
        },
        middleware,
        routing::get,
    };
    use rustok_api::{
        AuthContext, AuthContextExtension, Permission, TenantContext, TenantContextExtension,
    };
    use rustok_core::{MigrationSource, SecurityContext, UserRole};
    use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
    use rustok_taxonomy::TaxonomyModule;
    use sea_orm::{
        ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    };
    use sea_orm_migration::SchemaManager;
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::{
        CategoryService, CreateCategoryInput, CreateTopicInput, ForumModule,
        ForumTopicMergeService, MergeForumTopicInput, TopicResponse, TopicService,
    };

    use super::{ForumHttpRuntime, canonical_topic_location, redirect_merged_topic};

    type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

    async fn setup() -> TestResult<(DatabaseConnection, TransactionalEventBus)> {
        let db_url = format!(
            "sqlite:file:forum_topic_http_redirect_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let mut options = ConnectOptions::new(db_url);
        options
            .max_connections(5)
            .min_connections(1)
            .sqlx_logging(false);
        let db = Database::connect(options).await?;
        db.execute_unprepared(
            "CREATE TABLE users (\
                id TEXT NOT NULL PRIMARY KEY, \
                tenant_id TEXT NOT NULL, \
                UNIQUE (tenant_id, id)\
            )",
        )
        .await?;
        let schema = SchemaManager::new(&db);
        for migration in OutboxModule.migrations() {
            migration.up(&schema).await?;
        }
        for migration in TaxonomyModule.migrations() {
            migration.up(&schema).await?;
        }
        for migration in ForumModule.migrations() {
            migration.up(&schema).await?;
        }
        let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
        Ok((db, event_bus))
    }

    async fn insert_user(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> TestResult<()> {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
            vec![user_id.into(), tenant_id.into()],
        ))
        .await?;
        Ok(())
    }

    async fn create_category(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        security: SecurityContext,
    ) -> TestResult<Uuid> {
        Ok(CategoryService::new(db.clone())
            .create(
                tenant_id,
                security,
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "HTTP redirects".to_string(),
                    slug: "http-redirects".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await?
            .id)
    }

    async fn create_topic(
        db: &DatabaseConnection,
        event_bus: &TransactionalEventBus,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        key: &str,
    ) -> TestResult<Uuid> {
        Ok(TopicService::new(db.clone(), event_bus.clone())
            .create(
                tenant_id,
                security,
                CreateTopicInput {
                    locale: "en".to_string(),
                    category_id,
                    title: format!("Redirect {key}"),
                    slug: Some(format!("redirect-{key}")),
                    body: rustok_api::RichTextDocument::single_paragraph(format!(
                        "Redirect {key} body"
                    )),
                    metadata: serde_json::json!({}),
                    tags: Vec::new(),
                    channel_slugs: None,
                },
            )
            .await?
            .id)
    }

    fn tenant_context(tenant_id: Uuid) -> TenantContext {
        TenantContext {
            id: tenant_id,
            name: "HTTP redirect tenant".to_string(),
            slug: "http-redirect-tenant".to_string(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn auth_context(tenant_id: Uuid, user_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    fn request(uri: String, tenant: TenantContext, auth: AuthContext) -> Request<Body> {
        let mut request = Request::builder()
            .uri(uri)
            .body(Body::empty())
            .expect("redirect request");
        request
            .extensions_mut()
            .insert(TenantContextExtension(tenant));
        request.extensions_mut().insert(AuthContextExtension(auth));
        request
    }

    async fn put_passthrough() -> StatusCode {
        StatusCode::NO_CONTENT
    }

    #[test]
    fn canonical_location_encodes_explicit_locale() {
        let topic_id = Uuid::new_v4();
        assert_eq!(
            canonical_topic_location(topic_id, Some("en/US")),
            format!("/api/forum/topics/{topic_id}?locale=en%2FUS")
        );
        assert_eq!(
            canonical_topic_location(topic_id, None),
            format!("/api/forum/topics/{topic_id}")
        );
    }

    #[tokio::test]
    async fn merged_source_redirects_privately_while_target_uses_existing_handler() -> TestResult<()>
    {
        let (db, event_bus) = setup().await?;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        insert_user(&db, tenant_id, actor_id).await?;
        let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
        let category_id = create_category(&db, tenant_id, admin.clone()).await?;
        let source_topic_id = create_topic(
            &db,
            &event_bus,
            tenant_id,
            category_id,
            admin.clone(),
            "source",
        )
        .await?;
        let target_topic_id = create_topic(
            &db,
            &event_bus,
            tenant_id,
            category_id,
            admin.clone(),
            "target",
        )
        .await?;
        ForumTopicMergeService::new(db.clone(), event_bus.clone())
            .merge_topic(
                tenant_id,
                target_topic_id,
                admin,
                MergeForumTopicInput {
                    operation_id: Uuid::new_v4(),
                    source_topic_id,
                    reason: "Redirect the merged source".to_string(),
                },
            )
            .await?;

        let runtime = ForumHttpRuntime {
            db,
            event_bus,
            audience_facts: None,
        };
        let app = Router::new()
            .route(
                "/api/forum/topics/{id}",
                get(crate::controllers::topics::get_topic)
                    .route_layer(middleware::from_fn_with_state(
                        runtime.clone(),
                        redirect_merged_topic,
                    ))
                    .put(put_passthrough),
            )
            .with_state(runtime);
        let tenant = tenant_context(tenant_id);
        let read_auth = auth_context(tenant_id, actor_id, vec![Permission::FORUM_TOPICS_READ]);

        let source_response = app
            .clone()
            .oneshot(request(
                format!("/api/forum/topics/{source_topic_id}?locale=en-US"),
                tenant.clone(),
                read_auth.clone(),
            ))
            .await?;
        assert_eq!(source_response.status(), StatusCode::PERMANENT_REDIRECT);
        let expected_location = format!("/api/forum/topics/{target_topic_id}?locale=en-US");
        assert_eq!(
            source_response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some(expected_location.as_str())
        );
        assert_eq!(
            source_response
                .headers()
                .get(CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("private, no-store")
        );

        let target_response = app
            .clone()
            .oneshot(request(
                format!("/api/forum/topics/{target_topic_id}"),
                tenant.clone(),
                read_auth.clone(),
            ))
            .await?;
        assert_eq!(target_response.status(), StatusCode::OK);
        assert!(target_response.headers().get(LOCATION).is_none());
        let target_body = to_bytes(target_response.into_body(), usize::MAX).await?;
        let target: TopicResponse = serde_json::from_slice(&target_body)?;
        assert_eq!(target.id, target_topic_id);

        let missing_response = app
            .clone()
            .oneshot(request(
                format!("/api/forum/topics/{}", Uuid::new_v4()),
                tenant.clone(),
                read_auth,
            ))
            .await?;
        assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);
        assert!(missing_response.headers().get(LOCATION).is_none());

        let forbidden_response = app
            .clone()
            .oneshot(request(
                format!("/api/forum/topics/{source_topic_id}"),
                tenant,
                auth_context(tenant_id, actor_id, Vec::new()),
            ))
            .await?;
        assert_eq!(forbidden_response.status(), StatusCode::FORBIDDEN);
        assert!(forbidden_response.headers().get(LOCATION).is_none());

        let put_response = app
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(format!("/api/forum/topics/{source_topic_id}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(put_response.status(), StatusCode::NO_CONTENT);
        assert!(put_response.headers().get(LOCATION).is_none());

        Ok(())
    }
}
