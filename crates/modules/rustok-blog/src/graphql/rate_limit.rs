use std::collections::BTreeSet;
use std::net::IpAddr;
use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::parser::types::{ExecutableDocument, OperationType, Selection, SelectionSet};
use async_graphql::{ErrorExtensions, FieldError, Pos, Request, Response, ServerResult};
use async_trait::async_trait;
use axum::http::{HeaderMap, header};
use rustok_api::{
    Action, AuthContext, Permission, Resource, TenantContext, has_any_effective_permission,
};
use rustok_telemetry::metrics;

const TRUSTED_CLIENT_IP_HEADER: &str = "x-rustok-trusted-client-ip";

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

/// Every Blog root field has a rate-limit surface.
///
/// The set is a closed list on purpose: `classify_blog_graphql_document` inserts a
/// policy only for classified fields, and an unclassified Blog operation would run
/// with no limit at all. `blog_schema_root_fields_are_fully_classified` enumerates
/// the Blog schema and fails when a root field is added here without a surface.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum BlogGraphqlSurface {
    Post,
    PostBySlug,
    Posts,
    PublicBlogPosts,
    PostByPreviewToken,
    BlogCategory,
    BlogCategories,
    BlogTags,
    CreatePost,
    UpdatePost,
    DeletePost,
    PublishPost,
    UnpublishPost,
    ArchivePost,
    RestorePost,
    CreateBlogComment,
    ModerateComment,
    CreateBlogCategory,
    UpdateBlogCategory,
    DeleteBlogCategory,
    CreatePreviewToken,
    RevokePreviewToken,
    BulkPublishPosts,
    BulkUnpublishPosts,
    BulkArchivePosts,
    BulkDeletePosts,
    PinPost,
    UnpinPost,
}

impl BlogGraphqlSurface {
    fn from_root_field(operation_type: OperationType, field_name: &str) -> Option<Self> {
        match (operation_type, field_name) {
            (OperationType::Query, "post") => Some(Self::Post),
            (OperationType::Query, "postBySlug") => Some(Self::PostBySlug),
            (OperationType::Query, "posts") => Some(Self::Posts),
            (OperationType::Query, "publicBlogPosts") => Some(Self::PublicBlogPosts),
            (OperationType::Query, "postByPreviewToken") => Some(Self::PostByPreviewToken),
            (OperationType::Query, "blogCategory") => Some(Self::BlogCategory),
            (OperationType::Query, "blogCategories") => Some(Self::BlogCategories),
            (OperationType::Query, "blogTags") => Some(Self::BlogTags),
            (OperationType::Mutation, "createPost") => Some(Self::CreatePost),
            (OperationType::Mutation, "updatePost") => Some(Self::UpdatePost),
            (OperationType::Mutation, "deletePost") => Some(Self::DeletePost),
            (OperationType::Mutation, "publishPost") => Some(Self::PublishPost),
            (OperationType::Mutation, "unpublishPost") => Some(Self::UnpublishPost),
            (OperationType::Mutation, "archivePost") => Some(Self::ArchivePost),
            (OperationType::Mutation, "restorePost") => Some(Self::RestorePost),
            (OperationType::Mutation, "createBlogComment") => Some(Self::CreateBlogComment),
            (OperationType::Mutation, "moderateComment") => Some(Self::ModerateComment),
            (OperationType::Mutation, "createBlogCategory") => Some(Self::CreateBlogCategory),
            (OperationType::Mutation, "updateBlogCategory") => Some(Self::UpdateBlogCategory),
            (OperationType::Mutation, "deleteBlogCategory") => Some(Self::DeleteBlogCategory),
            (OperationType::Mutation, "createPreviewToken") => Some(Self::CreatePreviewToken),
            (OperationType::Mutation, "revokePreviewToken") => Some(Self::RevokePreviewToken),
            (OperationType::Mutation, "bulkPublishPosts") => Some(Self::BulkPublishPosts),
            (OperationType::Mutation, "bulkUnpublishPosts") => Some(Self::BulkUnpublishPosts),
            (OperationType::Mutation, "bulkArchivePosts") => Some(Self::BulkArchivePosts),
            (OperationType::Mutation, "bulkDeletePosts") => Some(Self::BulkDeletePosts),
            (OperationType::Mutation, "pinPost") => Some(Self::PinPost),
            (OperationType::Mutation, "unpinPost") => Some(Self::UnpinPost),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Post => "post",
            Self::PostBySlug => "post_by_slug",
            Self::Posts => "posts",
            Self::PublicBlogPosts => "public_blog_posts",
            Self::PostByPreviewToken => "post_by_preview_token",
            Self::BlogCategory => "blog_category",
            Self::BlogCategories => "blog_categories",
            Self::BlogTags => "blog_tags",
            Self::CreatePost => "create_post",
            Self::UpdatePost => "update_post",
            Self::DeletePost => "delete_post",
            Self::PublishPost => "publish_post",
            Self::UnpublishPost => "unpublish_post",
            Self::ArchivePost => "archive_post",
            Self::RestorePost => "restore_post",
            Self::CreateBlogComment => "create_blog_comment",
            Self::ModerateComment => "moderate_comment",
            Self::CreateBlogCategory => "create_blog_category",
            Self::UpdateBlogCategory => "update_blog_category",
            Self::DeleteBlogCategory => "delete_blog_category",
            Self::CreatePreviewToken => "create_preview_token",
            Self::RevokePreviewToken => "revoke_preview_token",
            Self::BulkPublishPosts => "bulk_publish_posts",
            Self::BulkUnpublishPosts => "bulk_unpublish_posts",
            Self::BulkArchivePosts => "bulk_archive_posts",
            Self::BulkDeletePosts => "bulk_delete_posts",
            Self::PinPost => "pin_post",
            Self::UnpinPost => "unpin_post",
        }
    }

    fn operation(self) -> &'static str {
        match self {
            Self::Post
            | Self::PostBySlug
            | Self::Posts
            | Self::PublicBlogPosts
            | Self::PostByPreviewToken
            | Self::BlogCategory
            | Self::BlogCategories
            | Self::BlogTags => "read",
            _ => "write",
        }
    }

    fn is_write(self) -> bool {
        self.operation() == "write"
    }

    /// The permission the resolver itself requires. The limiter only counts actors
    /// that would reach the resolver: an unauthorized write keeps its canonical
    /// unauthenticated/forbidden answer and is never charged a rate-limit key.
    fn required_permission(self) -> Option<Permission> {
        match self {
            Self::Post
            | Self::PostBySlug
            | Self::Posts
            | Self::PublicBlogPosts
            | Self::PostByPreviewToken
            | Self::BlogCategory
            | Self::BlogCategories
            | Self::BlogTags => None,
            Self::CreatePost => Some(Permission::BLOG_POSTS_CREATE),
            Self::UpdatePost | Self::CreatePreviewToken | Self::RevokePreviewToken => {
                Some(Permission::BLOG_POSTS_UPDATE)
            }
            Self::PinPost | Self::UnpinPost => Some(Permission::BLOG_POSTS_UPDATE),
            Self::DeletePost | Self::BulkDeletePosts => Some(Permission::BLOG_POSTS_DELETE),
            Self::PublishPost
            | Self::UnpublishPost
            | Self::ArchivePost
            | Self::RestorePost
            | Self::BulkPublishPosts
            | Self::BulkUnpublishPosts
            | Self::BulkArchivePosts => Some(Permission::BLOG_POSTS_PUBLISH),
            Self::CreateBlogComment => Some(Permission::new(Resource::Comments, Action::Create)),
            Self::ModerateComment => Some(Permission::BLOG_POSTS_MANAGE),
            Self::CreateBlogCategory => Some(Permission::BLOG_CATEGORIES_CREATE),
            Self::UpdateBlogCategory => Some(Permission::BLOG_CATEGORIES_UPDATE),
            Self::DeleteBlogCategory => Some(Permission::BLOG_CATEGORIES_DELETE),
        }
    }

    fn actor_is_authorized(self, auth: &AuthContext) -> bool {
        match self.required_permission() {
            Some(permission) => has_any_effective_permission(&auth.permissions, &[permission]),
            None => true,
        }
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
    headers?
        .get(TRUSTED_CLIENT_IP_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .and_then(|value| value.parse().ok())
}

fn build_rate_limit_key(
    tenant: &TenantContext,
    auth: Option<&AuthContext>,
    headers: Option<&HeaderMap>,
    surface: BlogGraphqlSurface,
) -> String {
    let actor = auth
        .map(|auth| {
            let actor = auth.port_actor();
            let kind = match actor.kind {
                rustok_api::PortActorKind::User => "user",
                rustok_api::PortActorKind::Service => "service",
                rustok_api::PortActorKind::System => "system",
            };
            format!("{kind}:{}", actor.id)
        })
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

fn rate_limited_error_response(error: FieldError, retry_after: u64) -> Response {
    let mut headers = HeaderMap::new();
    if let Ok(value) = retry_after.to_string().parse() {
        headers.insert(header::RETRY_AFTER, value);
    }
    error_response(error).http_headers(headers)
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
            if surface.is_write() && auth.is_none_or(|auth| !surface.actor_is_authorized(auth)) {
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
                    return rate_limited_error_response(
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
                        exceeded.retry_after,
                    );
                }
                Err(BlogGraphqlRateLimitError::BackendUnavailable(reason)) => {
                    metrics::record_rate_limit_backend_unavailable(limiter.0.namespace());
                    metrics::record_module_error("blog", "rate_limit_backend_unavailable", "error");
                    tracing::error!(
                        operation_name = ?operation_name,
                        surface = surface.name(),
                        tenant_id = %tenant.id,
                        %reason,
                        "Blog GraphQL rate limit backend unavailable"
                    );
                    return error_response(
                        FieldError::new("Blog rate limit backend unavailable").extend_with(
                            |_, ext| {
                                ext.set("code", "BLOG_RATE_LIMIT_BACKEND_UNAVAILABLE");
                                ext.set("surface", surface.name());
                            },
                        ),
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

    /// The classifier is fail-open by construction: `classify_blog_graphql_document`
    /// inserts a policy only for classified fields, and an unclassified Blog operation
    /// runs with no limit check at all. Enumerating the Blog schema here makes any new
    /// root field fail this test until it is given a surface.
    #[tokio::test]
    async fn blog_schema_root_fields_are_fully_classified() {
        let schema =
            async_graphql::Schema::build(
                crate::graphql::BlogQuery,
                crate::graphql::BlogMutation,
                async_graphql::EmptySubscription,
            )
            .finish();
        let response = schema
            .execute(
                "{ __schema { queryType { fields { name } } mutationType { fields { name } } } }",
            )
            .await;
        assert!(
            response.errors.is_empty(),
            "Blog schema introspection must succeed: {:?}",
            response.errors
        );
        let data = response
            .data
            .into_json()
            .expect("Blog introspection must serialize to JSON");

        let mut classified = 0;
        for (operation, pointer) in [
            (OperationType::Query, "/__schema/queryType/fields"),
            (OperationType::Mutation, "/__schema/mutationType/fields"),
        ] {
            let fields = data
                .pointer(pointer)
                .and_then(serde_json::Value::as_array)
                .unwrap_or_else(|| panic!("Blog schema must expose {operation:?} root fields"));
            assert!(
                !fields.is_empty(),
                "Blog schema must expose {operation:?} root fields"
            );
            for field in fields {
                let name = field
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .expect("introspected root field must carry a name");
                assert!(
                    BlogGraphqlSurface::from_root_field(operation, name).is_some(),
                    "Blog {operation:?} root field `{name}` has no rate-limit surface"
                );
                classified += 1;
            }
        }
        assert!(
            classified >= 28,
            "the Blog schema root surface unexpectedly shrank to {classified} fields"
        );
    }

    #[test]
    fn classifies_blog_fields_in_fragments_and_all_operations() {
        let document = parse_query(
            r#"
                query PublicBlog { ...BlogReads }
                mutation ManageBlog {
                    publishPost(id: "00000000-0000-0000-0000-000000000001")
                    moderateComment(
                        id: "00000000-0000-0000-0000-000000000002"
                        status: APPROVED
                    )
                }
                fragment BlogReads on Query { postBySlug(slug: "hello") { id } posts { nextCursor } }
            "#,
        )
        .expect("document should parse");

        assert_eq!(
            blog_graphql_fields(&document),
            vec![
                BlogGraphqlSurface::PostBySlug,
                BlogGraphqlSurface::Posts,
                BlogGraphqlSurface::PublishPost,
                BlogGraphqlSurface::ModerateComment,
            ]
        );
    }

    #[test]
    fn builds_tenant_scoped_user_and_trusted_ip_keys() {
        let tenant_id = Uuid::new_v4();
        let tenant = tenant(tenant_id);
        let auth = auth(tenant_id, vec![Permission::BLOG_POSTS_CREATE]);
        let user_key =
            build_rate_limit_key(&tenant, Some(&auth), None, BlogGraphqlSurface::CreatePost);
        assert_eq!(
            user_key,
            format!(
                "tenant:{tenant_id}:blog:graphql:write:create_post:user:{}",
                auth.user_id
            )
        );

        let mut headers = HeaderMap::new();
        headers.insert(TRUSTED_CLIENT_IP_HEADER, "203.0.113.7".parse().unwrap());
        let ip_key = build_rate_limit_key(&tenant, None, Some(&headers), BlogGraphqlSurface::Posts);
        assert_eq!(
            ip_key,
            format!("tenant:{tenant_id}:blog:graphql:read:posts:ip:203.0.113.7")
        );
    }

    #[test]
    fn service_principal_rate_limit_key_is_not_encoded_as_a_user() {
        let tenant_id = Uuid::new_v4();
        let tenant = tenant(tenant_id);
        let client_id = Uuid::new_v4();
        let auth = AuthContext {
            user_id: client_id,
            session_id: Uuid::nil(),
            tenant_id,
            permissions: vec![Permission::BLOG_POSTS_READ],
            client_id: Some(client_id),
            scopes: vec![],
            grant_type: "client_credentials".to_string(),
        };

        let key = build_rate_limit_key(&tenant, Some(&auth), None, BlogGraphqlSurface::Posts);

        assert_eq!(
            key,
            format!("tenant:{tenant_id}:blog:graphql:read:posts:service:{client_id}")
        );
    }

    #[test]
    fn raw_forwarded_headers_do_not_define_the_actor_key() {
        let tenant_id = Uuid::new_v4();
        let tenant = tenant(tenant_id);
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.99".parse().unwrap());

        assert_eq!(
            build_rate_limit_key(&tenant, None, Some(&headers), BlogGraphqlSurface::Posts,),
            format!("tenant:{tenant_id}:blog:graphql:read:posts:anonymous")
        );
    }

    #[test]
    fn rate_limited_response_sets_retry_after_header() {
        let response = rate_limited_error_response(FieldError::new("limited"), 17);

        assert_eq!(
            response
                .http_headers
                .get(header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok()),
            Some("17")
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
        assert!(!BlogGraphqlSurface::ModerateComment.actor_is_authorized(&update));

        let publish = auth(tenant_id, vec![Permission::BLOG_POSTS_PUBLISH]);
        assert!(!BlogGraphqlSurface::UpdatePost.actor_is_authorized(&publish));
        assert!(BlogGraphqlSurface::UnpublishPost.actor_is_authorized(&publish));
        assert!(BlogGraphqlSurface::ArchivePost.actor_is_authorized(&publish));
        assert!(!BlogGraphqlSurface::ModerateComment.actor_is_authorized(&publish));

        let manage = auth(tenant_id, vec![Permission::BLOG_POSTS_MANAGE]);
        assert!(BlogGraphqlSurface::ModerateComment.actor_is_authorized(&manage));
    }
}
