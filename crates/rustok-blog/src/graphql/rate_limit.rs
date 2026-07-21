use std::collections::BTreeSet;
use std::net::IpAddr;
use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::parser::types::{ExecutableDocument, OperationType, Selection, SelectionSet};
use async_graphql::{ErrorExtensions, FieldError, Pos, Request, Response, ServerResult};
use async_trait::async_trait;
use axum::http::HeaderMap;
use rustok_api::{has_any_effective_permission, AuthContext, Permission, TenantContext};
use rustok_telemetry::metrics;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlogGraphqlRateLimitExceeded {
    pub limit: usize,
    pub retry_after: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlogGraphqlRateLimitError {
    Exceeded(BlogGraphqlRateLimitExceeded),
    BackendUnavailable(String),
}

#[async_trait]
pub trait BlogGraphqlRateLimiter: Send + Sync {
    fn namespace(&self) -> &str;

    async fn check_rate_limit(&self, key: &str) -> Result<(), BlogGraphqlRateLimitError>;
}

#[derive(Clone)]
pub struct BlogGraphqlRateLimiterHandle(pub Arc<dyn BlogGraphqlRateLimiter>);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum BlogGraphqlSurface {
    Post,
    PostBySlug,
    Posts,
    CreatePost,
    UpdatePost,
    DeletePost,
    PublishPost,
    UnpublishPost,
    ArchivePost,
}

impl BlogGraphqlSurface {
    fn from_root_field(operation_type: OperationType, field_name: &str) -> Option<Self> {
        match (operation_type, field_name) {
            (OperationType::Query, "post") => Some(Self::Post),
            (OperationType::Query, "postBySlug") => Some(Self::PostBySlug),
            (OperationType::Query, "posts") => Some(Self::Posts),
            (OperationType::Mutation, "createPost") => Some(Self::CreatePost),
            (OperationType::Mutation, "updatePost") => Some(Self::UpdatePost),
            (OperationType::Mutation, "deletePost") => Some(Self::DeletePost),
            (OperationType::Mutation, "publishPost") => Some(Self::PublishPost),
            (OperationType::Mutation, "unpublishPost") => Some(Self::UnpublishPost),
            (OperationType::Mutation, "archivePost") => Some(Self::ArchivePost),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Post => "post",
            Self::PostBySlug => "post_by_slug",
            Self::Posts => "posts",
            Self::CreatePost => "create_post",
            Self::UpdatePost => "update_post",
            Self::DeletePost => "delete_post",
            Self::PublishPost => "publish_post",
            Self::UnpublishPost => "unpublish_post",
            Self::ArchivePost => "archive_post",
        }
    }

    fn operation(self) -> &'static str {
        match self {
            Self::Post | Self::PostBySlug | Self::Posts => "read",
            _ => "write",
        }
    }

    fn is_write(self) -> bool {
        self.operation() == "write"
    }

    fn actor_is_authorized(self, auth: &AuthContext) -> bool {
        let permission = match self {
            Self::CreatePost => Permission::BLOG_POSTS_CREATE,
            Self::UpdatePost => Permission::BLOG_POSTS_UPDATE,
            Self::DeletePost => Permission::BLOG_POSTS_DELETE,
            Self::PublishPost | Self::UnpublishPost | Self::ArchivePost => {
                Permission::BLOG_POSTS_PUBLISH
            }
            Self::Post | Self::PostBySlug | Self::Posts => return true,
        };
        has_any_effective_permission(&auth.permissions, &[permission])
    }
}

#[derive(Clone, Debug)]
struct BlogGraphqlDocumentPolicy(Vec<BlogGraphqlSurface>);

fn collect_blog_fields_from_selection_set(
    operation_type: OperationType,
    selection_set: &SelectionSet,
    document: &ExecutableDocument,
    fields: &mut BTreeSet<BlogGraphqlSurface>,
) {
    for selection in &selection_set.items {
        match &selection.node {
            Selection::Field(field) => {
                if let Some(surface) = BlogGraphqlSurface::from_root_field(
                    operation_type,
                    field.node.name.node.as_str(),
                ) {
                    fields.insert(surface);
                }
            }
            Selection::FragmentSpread(fragment) => {
                if let Some(definition) = document.fragments.get(&fragment.node.fragment_name.node)
                {
                    collect_blog_fields_from_selection_set(
                        operation_type,
                        &definition.node.selection_set.node,
                        document,
                        fields,
                    );
                }
            }
            Selection::InlineFragment(fragment) => collect_blog_fields_from_selection_set(
                operation_type,
                &fragment.node.selection_set.node,
                document,
                fields,
            ),
        }
    }
}

fn blog_graphql_fields(document: &ExecutableDocument) -> Vec<BlogGraphqlSurface> {
    let mut fields = BTreeSet::new();
    for (_, operation) in document.operations.iter() {
        collect_blog_fields_from_selection_set(
            operation.node.ty,
            &operation.node.selection_set.node,
            document,
            &mut fields,
        );
    }
    fields.into_iter().collect()
}

fn classify_blog_graphql_document(request: &mut Request) -> ServerResult<()> {
    if request.query.trim().is_empty() {
        return Ok(());
    }

    let document = request.parsed_query()?;
    let fields = blog_graphql_fields(document);
    if !fields.is_empty() {
        request.data.insert(BlogGraphqlDocumentPolicy(fields));
    }
    Ok(())
}

fn extract_client_ip(headers: Option<&HeaderMap>) -> Option<IpAddr> {
    let headers = headers?;
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .and_then(|value| value.parse().ok())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .and_then(|value| value.parse().ok())
        })
}

fn build_rate_limit_key(
    tenant: &TenantContext,
    auth: Option<&AuthContext>,
    headers: Option<&HeaderMap>,
    surface: BlogGraphqlSurface,
) -> String {
    let actor = auth
        .map(|auth| format!("user:{}", auth.user_id))
        .or_else(|| extract_client_ip(headers).map(|ip| format!("ip:{ip}")))
        .unwrap_or_else(|| "anonymous".to_string());

    format!(
        "tenant:{}:blog:graphql:{}:{}:{}",
        tenant.id,
        surface.operation(),
        surface.name(),
        actor
    )
}

fn error_response(error: FieldError) -> Response {
    Response::from_errors(vec![error.into_server_error(Pos::default())])
}

#[derive(Clone, Default)]
pub struct BlogGraphqlRateLimitPolicy {
    limiter: Option<BlogGraphqlRateLimiterHandle>,
}

impl BlogGraphqlRateLimitPolicy {
    pub fn new(limiter: Option<BlogGraphqlRateLimiterHandle>) -> Self {
        Self { limiter }
    }
}

impl ExtensionFactory for BlogGraphqlRateLimitPolicy {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(BlogGraphqlRateLimitPolicyExtension {
            limiter: self.limiter.clone(),
        })
    }
}

struct BlogGraphqlRateLimitPolicyExtension {
    limiter: Option<BlogGraphqlRateLimiterHandle>,
}

#[async_trait]
impl Extension for BlogGraphqlRateLimitPolicyExtension {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        let mut request = next.run(ctx, request).await?;
        classify_blog_graphql_document(&mut request)?;
        Ok(request)
    }

    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> Response {
        let Some(limiter) = self.limiter.as_ref() else {
            return next.run(ctx, operation_name).await;
        };
        let Some(policy) = ctx.data_opt::<BlogGraphqlDocumentPolicy>() else {
            return next.run(ctx, operation_name).await;
        };
        let Some(tenant) = ctx.data_opt::<TenantContext>() else {
            return error_response(
                FieldError::new("Blog rate limit tenant context is unavailable")
                    .extend_with(|_, ext| ext.set("code", "INTERNAL_SERVER_ERROR")),
            );
        };
        let auth = ctx.data_opt::<AuthContext>();
        let headers = ctx.data_opt::<HeaderMap>();

        for surface in &policy.0 {
            // Authentication and RBAC stay authoritative. Unauthorized writes reach
            // their resolver and preserve the existing unauthenticated/forbidden error.
            if surface.is_write()
                && auth.is_none_or(|auth| !surface.actor_is_authorized(auth))
            {
                continue;
            }

            let key = build_rate_limit_key(tenant, auth, headers, *surface);
            match limiter.0.check_rate_limit(&key).await {
                Ok(()) => {}
                Err(BlogGraphqlRateLimitError::Exceeded(exceeded)) => {
                    metrics::record_rate_limit_exceeded(limiter.0.namespace());
                    metrics::record_module_error("blog", "rate_limit_exceeded", "warning");
                    tracing::warn!(
                        operation_name = ?operation_name,
                        surface = surface.name(),
                        tenant_id = %tenant.id,
                        retry_after = exceeded.retry_after,
                        "Rejected rate-limited Blog GraphQL operation"
                    );
                    return error_response(
                        FieldError::new(format!(
                            "Blog rate limit exceeded. Retry after {} seconds",
                            exceeded.retry_after
                        ))
                        .extend_with(|_, ext| {
                            ext.set("code", "BLOG_RATE_LIMITED");
                            ext.set("surface", surface.name());
                            ext.set("limit", exceeded.limit as i64);
                            ext.set("retryAfter", exceeded.retry_after as i64);
                        }),
                    );
                }
                Err(BlogGraphqlRateLimitError::BackendUnavailable(reason)) => {
                    metrics::record_rate_limit_backend_unavailable(limiter.0.namespace());
                    metrics::record_module_error(
                        "blog",
                        "rate_limit_backend_unavailable",
                        "error",
                    );
                    tracing::error!(
                        operation_name = ?operation_name,
                        surface = surface.name(),
                        tenant_id = %tenant.id,
                        %reason,
                        "Blog GraphQL rate limit backend unavailable"
                    );
                    return error_response(
                        FieldError::new("Blog rate limit backend unavailable")
                            .extend_with(|_, ext| {
                                ext.set("code", "BLOG_RATE_LIMIT_BACKEND_UNAVAILABLE");
                                ext.set("surface", surface.name());
                            }),
                    );
                }
            }
        }

        next.run(ctx, operation_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql::parser::parse_query;
    use uuid::Uuid;

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

    #[test]
    fn classifies_blog_fields_in_fragments_and_all_operations() {
        let document = parse_query(
            r#"
                query PublicBlog { ...BlogReads }
                mutation ManageBlog { publishPost(id: "00000000-0000-0000-0000-000000000001") }
                fragment BlogReads on Query { postBySlug(slug: "hello") { id } posts { total } }
            "#,
        )
        .expect("document should parse");

        assert_eq!(
            blog_graphql_fields(&document),
            vec![
                BlogGraphqlSurface::PostBySlug,
                BlogGraphqlSurface::Posts,
                BlogGraphqlSurface::PublishPost,
            ]
        );
    }

    #[test]
    fn builds_tenant_scoped_user_and_ip_keys() {
        let tenant_id = Uuid::new_v4();
        let tenant = tenant(tenant_id);
        let auth = auth(tenant_id, vec![Permission::BLOG_POSTS_CREATE]);
        let user_key = build_rate_limit_key(
            &tenant,
            Some(&auth),
            None,
            BlogGraphqlSurface::CreatePost,
        );
        assert_eq!(
            user_key,
            format!(
                "tenant:{tenant_id}:blog:graphql:write:create_post:user:{}",
                auth.user_id
            )
        );

        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1".parse().unwrap());
        let ip_key = build_rate_limit_key(
            &tenant,
            None,
            Some(&headers),
            BlogGraphqlSurface::Posts,
        );
        assert_eq!(
            ip_key,
            format!("tenant:{tenant_id}:blog:graphql:read:posts:ip:203.0.113.7")
        );
    }

    #[test]
    fn write_rate_limit_permission_gate_matches_current_resolvers() {
        let tenant_id = Uuid::new_v4();
        let create = auth(tenant_id, vec![Permission::BLOG_POSTS_CREATE]);
        assert!(BlogGraphqlSurface::CreatePost.actor_is_authorized(&create));
        assert!(!BlogGraphqlSurface::DeletePost.actor_is_authorized(&create));

        let update = auth(tenant_id, vec![Permission::BLOG_POSTS_UPDATE]);
        assert!(BlogGraphqlSurface::UpdatePost.actor_is_authorized(&update));
        assert!(!BlogGraphqlSurface::ArchivePost.actor_is_authorized(&update));

        let publish = auth(tenant_id, vec![Permission::BLOG_POSTS_PUBLISH]);
        assert!(!BlogGraphqlSurface::UpdatePost.actor_is_authorized(&publish));
        assert!(BlogGraphqlSurface::UnpublishPost.actor_is_authorized(&publish));
        assert!(BlogGraphqlSurface::ArchivePost.actor_is_authorized(&publish));
    }
}
