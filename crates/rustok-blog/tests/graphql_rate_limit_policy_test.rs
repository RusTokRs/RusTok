use std::sync::{Arc, Mutex};

use async_graphql::{EmptySubscription, Object, Request, Schema};
use async_trait::async_trait;
use axum::http::HeaderMap;
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_blog::graphql::{
    BlogGraphqlRateLimitError, BlogGraphqlRateLimitExceeded, BlogGraphqlRateLimitPolicy,
    BlogGraphqlRateLimiter, BlogGraphqlRateLimiterHandle,
};
use uuid::Uuid;

#[derive(Clone, Debug)]
enum LimiterOutcome {
    Allow,
    Exceeded { limit: usize, retry_after: u64 },
    BackendUnavailable(&'static str),
}

#[derive(Debug)]
struct ScriptedLimiter {
    outcome: LimiterOutcome,
    keys: Mutex<Vec<String>>,
}

impl ScriptedLimiter {
    fn new(outcome: LimiterOutcome) -> Self {
        Self {
            outcome,
            keys: Mutex::new(Vec::new()),
        }
    }

    fn keys(&self) -> Vec<String> {
        self.keys.lock().expect("keys lock poisoned").clone()
    }
}

#[async_trait]
impl BlogGraphqlRateLimiter for ScriptedLimiter {
    fn namespace(&self) -> &str {
        "blog-graphql-runtime-test"
    }

    async fn check_rate_limit(&self, key: &str) -> Result<(), BlogGraphqlRateLimitError> {
        self.keys
            .lock()
            .expect("keys lock poisoned")
            .push(key.to_string());

        match self.outcome {
            LimiterOutcome::Allow => Ok(()),
            LimiterOutcome::Exceeded { limit, retry_after } => {
                Err(BlogGraphqlRateLimitError::Exceeded(
                    BlogGraphqlRateLimitExceeded { limit, retry_after },
                ))
            }
            LimiterOutcome::BackendUnavailable(reason) => {
                Err(BlogGraphqlRateLimitError::BackendUnavailable(
                    reason.to_string(),
                ))
            }
        }
    }
}

struct TestQuery;

#[Object]
impl TestQuery {
    async fn posts(&self) -> bool {
        true
    }

    async fn post_by_slug(&self, slug: String) -> String {
        slug
    }
}

struct TestMutation;

#[Object]
impl TestMutation {
    async fn create_post(&self) -> bool {
        true
    }

    async fn moderate_comment(&self) -> bool {
        true
    }
}

type TestSchema = Schema<TestQuery, TestMutation, EmptySubscription>;

fn tenant(id: Uuid) -> TenantContext {
    TenantContext {
        id,
        name: "Tenant".to_string(),
        slug: "tenant".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}

fn auth(tenant_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
    AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions,
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    }
}

fn schema(
    tenant: TenantContext,
    auth: Option<AuthContext>,
    headers: Option<HeaderMap>,
    limiter: Arc<ScriptedLimiter>,
) -> TestSchema {
    let handle = BlogGraphqlRateLimiterHandle(limiter);
    let mut builder = Schema::build(TestQuery, TestMutation, EmptySubscription)
        .extension(BlogGraphqlRateLimitPolicy::new(Some(handle)))
        .data(tenant);

    if let Some(auth) = auth {
        builder = builder.data(auth);
    }
    if let Some(headers) = headers {
        builder = builder.data(headers);
    }

    builder.finish()
}

fn response_json(response: async_graphql::Response) -> serde_json::Value {
    serde_json::to_value(response).expect("GraphQL response should serialize")
}

#[tokio::test]
async fn allowed_public_read_records_tenant_scoped_trusted_ip_key() {
    let tenant_id = Uuid::new_v4();
    let limiter = Arc::new(ScriptedLimiter::new(LimiterOutcome::Allow));
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-rustok-trusted-client-ip",
        "203.0.113.7".parse().expect("valid header"),
    );
    let schema = schema(tenant(tenant_id), None, Some(headers), limiter.clone());

    let response = schema.execute(Request::new("{ posts }")).await;

    assert!(response.errors.is_empty());
    assert_eq!(
        limiter.keys(),
        vec![format!(
            "tenant:{tenant_id}:blog:graphql:read:posts:ip:203.0.113.7"
        )]
    );
}

#[tokio::test]
async fn exceeded_read_returns_structured_graphql_extensions() {
    let tenant_id = Uuid::new_v4();
    let limiter = Arc::new(ScriptedLimiter::new(LimiterOutcome::Exceeded {
        limit: 12,
        retry_after: 9,
    }));
    let schema = schema(tenant(tenant_id), None, None, limiter.clone());

    let response = schema
        .execute(Request::new(r#"{ postBySlug(slug: "hello") }"#))
        .await;
    let json = response_json(response);

    assert_eq!(json["errors"][0]["extensions"]["code"], "BLOG_RATE_LIMITED");
    assert_eq!(
        json["errors"][0]["extensions"]["surface"],
        "post_by_slug"
    );
    assert_eq!(json["errors"][0]["extensions"]["limit"], 12);
    assert_eq!(json["errors"][0]["extensions"]["retryAfter"], 9);
    assert_eq!(
        limiter.keys(),
        vec![format!(
            "tenant:{tenant_id}:blog:graphql:read:post_by_slug:anonymous"
        )]
    );
}

#[tokio::test]
async fn backend_unavailable_returns_fail_closed_graphql_error() {
    let tenant_id = Uuid::new_v4();
    let limiter = Arc::new(ScriptedLimiter::new(
        LimiterOutcome::BackendUnavailable("redis unavailable"),
    ));
    let schema = schema(tenant(tenant_id), None, None, limiter.clone());

    let response = schema.execute(Request::new("{ posts }")).await;
    let json = response_json(response);

    assert_eq!(
        json["errors"][0]["extensions"]["code"],
        "BLOG_RATE_LIMIT_BACKEND_UNAVAILABLE"
    );
    assert_eq!(json["errors"][0]["extensions"]["surface"], "posts");
    assert_eq!(limiter.keys().len(), 1);
}

#[tokio::test]
async fn authorized_write_is_rate_limited_by_user_identity() {
    let tenant_id = Uuid::new_v4();
    let actor = auth(tenant_id, vec![Permission::BLOG_POSTS_CREATE]);
    let actor_id = actor.user_id;
    let limiter = Arc::new(ScriptedLimiter::new(LimiterOutcome::Exceeded {
        limit: 3,
        retry_after: 30,
    }));
    let schema = schema(tenant(tenant_id), Some(actor), None, limiter.clone());

    let response = schema
        .execute(Request::new("mutation { createPost }"))
        .await;
    let json = response_json(response);

    assert_eq!(json["errors"][0]["extensions"]["code"], "BLOG_RATE_LIMITED");
    assert_eq!(json["errors"][0]["extensions"]["surface"], "create_post");
    assert_eq!(
        limiter.keys(),
        vec![format!(
            "tenant:{tenant_id}:blog:graphql:write:create_post:user:{actor_id}"
        )]
    );
}

#[tokio::test]
async fn unauthorized_write_skips_module_limiter_and_reaches_resolver() {
    let tenant_id = Uuid::new_v4();
    let actor = auth(tenant_id, vec![Permission::BLOG_POSTS_READ]);
    let limiter = Arc::new(ScriptedLimiter::new(LimiterOutcome::Exceeded {
        limit: 1,
        retry_after: 60,
    }));
    let schema = schema(tenant(tenant_id), Some(actor), None, limiter.clone());

    let response = schema
        .execute(Request::new("mutation { createPost }"))
        .await;

    assert!(response.errors.is_empty());
    assert!(limiter.keys().is_empty());
}

#[tokio::test]
async fn moderation_uses_manage_permission_and_dedicated_surface() {
    let tenant_id = Uuid::new_v4();
    let actor = auth(tenant_id, vec![Permission::BLOG_POSTS_MANAGE]);
    let actor_id = actor.user_id;
    let limiter = Arc::new(ScriptedLimiter::new(LimiterOutcome::Allow));
    let schema = schema(tenant(tenant_id), Some(actor), None, limiter.clone());

    let response = schema
        .execute(Request::new("mutation { moderateComment }"))
        .await;

    assert!(response.errors.is_empty());
    assert_eq!(
        limiter.keys(),
        vec![format!(
            "tenant:{tenant_id}:blog:graphql:write:moderate_comment:user:{actor_id}"
        )]
    );
}

#[tokio::test]
async fn selected_operation_keeps_document_wide_fail_closed_accounting() {
    let tenant_id = Uuid::new_v4();
    let actor = auth(tenant_id, vec![Permission::BLOG_POSTS_CREATE]);
    let actor_id = actor.user_id;
    let limiter = Arc::new(ScriptedLimiter::new(LimiterOutcome::Allow));
    let schema = schema(tenant(tenant_id), Some(actor), None, limiter.clone());

    let response = schema
        .execute(
            Request::new(
                r#"
                    query PublicBlog {
                        posts
                    }

                    mutation HiddenCreate {
                        createPost
                    }
                "#,
            )
            .operation_name("PublicBlog"),
        )
        .await;

    assert!(response.errors.is_empty());
    assert_eq!(
        limiter.keys(),
        vec![
            format!("tenant:{tenant_id}:blog:graphql:read:posts:user:{actor_id}"),
            format!(
                "tenant:{tenant_id}:blog:graphql:write:create_post:user:{actor_id}"
            ),
        ]
    );
}
