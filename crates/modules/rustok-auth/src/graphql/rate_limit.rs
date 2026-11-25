use std::collections::BTreeSet;
use std::net::IpAddr;
use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::parser::types::{ExecutableDocument, OperationType, Selection, SelectionSet};
use async_graphql::{ErrorExtensions, FieldError, Pos, Request, Response, ServerResult};
use async_trait::async_trait;
use http::{HeaderMap, header};
use rustok_api::{AuthContext, TenantContext};
use rustok_telemetry::metrics;

const TRUSTED_CLIENT_IP_HEADER: &str = "x-rustok-trusted-client-ip";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthGraphqlRateLimitExceeded {
    pub limit: usize,
    pub retry_after: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthGraphqlRateLimitError {
    Exceeded(AuthGraphqlRateLimitExceeded),
    BackendUnavailable(String),
}

#[async_trait]
pub trait AuthGraphqlRateLimiter: Send + Sync {
    fn namespace(&self) -> &str;

    async fn check_rate_limit(&self, key: &str) -> Result<(), AuthGraphqlRateLimitError>;
}

#[derive(Clone)]
pub struct AuthGraphqlRateLimiterHandle(pub Arc<dyn AuthGraphqlRateLimiter>);

/// Credential surfaces reachable without an authenticated session.
///
/// The set is a closed list on purpose: these are the GraphQL counterparts of
/// the REST endpoints behind the dedicated auth rate limiter, and leaving one
/// out would reopen a per-IP credential-guessing path that bypasses it.
/// Authenticated session-management fields are covered by the account lockout
/// and the general API limiter, so they are not charged here.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum AuthGraphqlSurface {
    SignIn,
    SignUp,
    RefreshToken,
    ForgotPassword,
    ResetPassword,
    AcceptInvite,
}

impl AuthGraphqlSurface {
    fn from_root_field(operation_type: OperationType, field_name: &str) -> Option<Self> {
        match (operation_type, field_name) {
            (OperationType::Mutation, "signIn") => Some(Self::SignIn),
            (OperationType::Mutation, "signUp") => Some(Self::SignUp),
            (OperationType::Mutation, "refreshToken") => Some(Self::RefreshToken),
            (OperationType::Mutation, "forgotPassword") => Some(Self::ForgotPassword),
            (OperationType::Mutation, "resetPassword") => Some(Self::ResetPassword),
            (OperationType::Mutation, "acceptInvite") => Some(Self::AcceptInvite),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::SignIn => "sign_in",
            Self::SignUp => "sign_up",
            Self::RefreshToken => "refresh_token",
            Self::ForgotPassword => "forgot_password",
            Self::ResetPassword => "reset_password",
            Self::AcceptInvite => "accept_invite",
        }
    }
}

#[derive(Clone, Debug)]
struct AuthGraphqlDocumentPolicy(Vec<AuthGraphqlSurface>);

// Classification runs before GraphQL validation. Expand each named fragment
// once per operation with an explicit worklist to avoid cycles and exponential
// expansion. Distinct response keys represent separate resolver invocations.
fn auth_graphql_fields(
    document: &ExecutableDocument,
    operation_name: Option<&str>,
) -> Vec<AuthGraphqlSurface> {
    let mut result = Vec::new();
    for (name, operation) in document.operations.iter() {
        if operation_name.is_some_and(|requested| name.map(|n| n.as_str()) != Some(requested)) {
            continue;
        }
        let mut fields = BTreeSet::new();
        let mut visited_fragments = BTreeSet::new();
        let mut pending: Vec<&SelectionSet> = vec![&operation.node.selection_set.node];
        while let Some(selection_set) = pending.pop() {
            for selection in &selection_set.items {
                match &selection.node {
                    Selection::Field(field) => {
                        if let Some(surface) = AuthGraphqlSurface::from_root_field(
                            operation.node.ty,
                            field.node.name.node.as_str(),
                        ) {
                            let response_key = field
                                .node
                                .alias
                                .as_ref()
                                .unwrap_or(&field.node.name)
                                .node
                                .as_str();
                            fields.insert((surface, response_key));
                        }
                    }
                    Selection::FragmentSpread(fragment) => {
                        let name = &fragment.node.fragment_name.node;
                        if visited_fragments.insert(name.as_str()) {
                            if let Some(definition) = document.fragments.get(name) {
                                pending.push(&definition.node.selection_set.node);
                            }
                        }
                    }
                    Selection::InlineFragment(fragment) => {
                        pending.push(&fragment.node.selection_set.node);
                    }
                }
            }
        }
        result.extend(fields.into_iter().map(|(surface, _)| surface));
    }
    result
}

fn classify_auth_graphql_document(request: &mut Request) -> ServerResult<()> {
    // A request may already carry a parsed document (e.g. persisted queries).
    let operation_name = request.operation_name.clone();
    let document = request.parsed_query()?;
    let fields = auth_graphql_fields(document, operation_name.as_deref());
    if !fields.is_empty() {
        request.data.insert(AuthGraphqlDocumentPolicy(fields));
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
    surface: AuthGraphqlSurface,
) -> String {
    let actor = auth
        .map(|auth| format!("user:{}", auth.user_id))
        .or_else(|| extract_client_ip(headers).map(|ip| format!("ip:{ip}")))
        .unwrap_or_else(|| "anonymous".to_string());

    format!(
        "tenant:{}:auth:graphql:{}:{}",
        tenant.id,
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
pub struct AuthGraphqlRateLimitPolicy {
    limiter: Option<AuthGraphqlRateLimiterHandle>,
}

impl AuthGraphqlRateLimitPolicy {
    pub fn new(limiter: Option<AuthGraphqlRateLimiterHandle>) -> Self {
        Self { limiter }
    }
}

impl ExtensionFactory for AuthGraphqlRateLimitPolicy {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(AuthGraphqlRateLimitPolicyExtension {
            limiter: self.limiter.clone(),
        })
    }
}

struct AuthGraphqlRateLimitPolicyExtension {
    limiter: Option<AuthGraphqlRateLimiterHandle>,
}

#[async_trait]
impl Extension for AuthGraphqlRateLimitPolicyExtension {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        let mut request = next.run(ctx, request).await?;
        classify_auth_graphql_document(&mut request)?;
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
        let Some(policy) = ctx.data_opt::<AuthGraphqlDocumentPolicy>() else {
            return next.run(ctx, operation_name).await;
        };
        let Some(tenant) = ctx.data_opt::<TenantContext>() else {
            return error_response(
                FieldError::new("Auth rate limit tenant context is unavailable")
                    .extend_with(|_, ext| ext.set("code", "INTERNAL_SERVER_ERROR")),
            );
        };
        let auth = ctx.data_opt::<AuthContext>();
        let headers = ctx.data_opt::<HeaderMap>();

        for surface in &policy.0 {
            let key = build_rate_limit_key(tenant, auth, headers, *surface);
            match limiter.0.check_rate_limit(&key).await {
                Ok(()) => {}
                Err(AuthGraphqlRateLimitError::Exceeded(exceeded)) => {
                    metrics::record_rate_limit_exceeded(limiter.0.namespace());
                    metrics::record_module_error("auth", "rate_limit_exceeded", "warning");
                    tracing::warn!(
                        operation_name = ?operation_name,
                        surface = surface.name(),
                        tenant_id = %tenant.id,
                        retry_after = exceeded.retry_after,
                        "Rejected rate-limited Auth GraphQL operation"
                    );
                    return rate_limited_error_response(
                        FieldError::new(format!(
                            "Auth rate limit exceeded. Retry after {} seconds",
                            exceeded.retry_after
                        ))
                        .extend_with(|_, ext| {
                            ext.set("code", "AUTH_RATE_LIMITED");
                            ext.set("surface", surface.name());
                            ext.set("limit", exceeded.limit as i64);
                            ext.set("retryAfter", exceeded.retry_after as i64);
                        }),
                        exceeded.retry_after,
                    );
                }
                Err(AuthGraphqlRateLimitError::BackendUnavailable(reason)) => {
                    metrics::record_rate_limit_backend_unavailable(limiter.0.namespace());
                    metrics::record_module_error("auth", "rate_limit_backend_unavailable", "error");
                    tracing::error!(
                        operation_name = ?operation_name,
                        surface = surface.name(),
                        tenant_id = %tenant.id,
                        %reason,
                        "Auth GraphQL rate limit backend unavailable"
                    );
                    return error_response(
                        FieldError::new("Auth rate limit backend unavailable").extend_with(
                            |_, ext| {
                                ext.set("code", "AUTH_RATE_LIMIT_BACKEND_UNAVAILABLE");
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

    #[test]
    fn classifies_every_credential_surface_and_ignores_session_management() {
        let document = parse_query(
            r#"
                mutation AuthFlow {
                    signIn(input: { email: "a@example.com", password: "x" }) { accessToken }
                    refreshToken(refreshToken: "t") { accessToken }
                    logout { success }
                    changePassword(currentPassword: "a", newPassword: "b") { success }
                }
            "#,
        )
        .expect("document should parse");

        assert_eq!(
            auth_graphql_fields(&document, None),
            vec![AuthGraphqlSurface::SignIn, AuthGraphqlSurface::RefreshToken]
        );
    }

    #[test]
    fn classifies_credential_fields_inside_fragments() {
        let document = parse_query(
            r#"
                mutation InviteFlow { ...InviteOps }
                fragment InviteOps on Mutation {
                    acceptInvite(token: "t", password: "x") { email }
                    forgotPassword(email: "a@example.com") { success }
                }
            "#,
        )
        .expect("document should parse");

        assert_eq!(
            auth_graphql_fields(&document, None),
            vec![
                AuthGraphqlSurface::ForgotPassword,
                AuthGraphqlSurface::AcceptInvite
            ]
        );
    }

    #[test]
    fn charges_distinct_aliases_but_not_merged_selections() {
        let document = parse_query(
            r#"
                mutation AuthFlow {
                    first: signIn(input: { email: "a@example.com", password: "x" }) { accessToken }
                    ...Shared
                    ...Shared
                    ... on Mutation { ...Shared }
                    second: signIn(input: { email: "b@example.com", password: "x" }) { accessToken }
                }
                fragment Shared on Mutation {
                    first: signIn(input: { email: "a@example.com", password: "x" }) { accessToken }
                }
            "#,
        )
        .expect("document should parse");

        assert_eq!(
            auth_graphql_fields(&document, Some("AuthFlow")),
            vec![AuthGraphqlSurface::SignIn, AuthGraphqlSurface::SignIn]
        );
    }

    #[test]
    fn cyclic_fragments_terminate_before_graphql_validation() {
        let document = parse_query(
            r#"
                mutation AuthFlow { ...First }
                fragment First on Mutation { ...Second }
                fragment Second on Mutation {
                    ...First
                    signIn(input: { email: "a@example.com", password: "x" }) { accessToken }
                }
            "#,
        )
        .expect("document should parse before validation");

        assert_eq!(
            auth_graphql_fields(&document, Some("AuthFlow")),
            vec![AuthGraphqlSurface::SignIn]
        );
    }

    #[test]
    fn only_selected_operation_is_charged() {
        let document = parse_query(
            r#"
                mutation Login {
                    signIn(input: { email: "a@example.com", password: "x" }) { accessToken }
                }
                mutation Recovery {
                    forgotPassword(input: { email: "a@example.com" }) { success }
                }
                query Read { __typename }
            "#,
        )
        .expect("document should parse");

        assert_eq!(
            auth_graphql_fields(&document, Some("Recovery")),
            vec![AuthGraphqlSurface::ForgotPassword]
        );
        assert!(auth_graphql_fields(&document, Some("Read")).is_empty());
    }

    #[test]
    fn builds_tenant_scoped_ip_and_user_keys() {
        let tenant_id = Uuid::new_v4();
        let tenant = tenant(tenant_id);

        let mut headers = HeaderMap::new();
        headers.insert(TRUSTED_CLIENT_IP_HEADER, "203.0.113.7".parse().unwrap());
        let ip_key =
            build_rate_limit_key(&tenant, None, Some(&headers), AuthGraphqlSurface::SignIn);
        assert_eq!(
            ip_key,
            format!("tenant:{tenant_id}:auth:graphql:sign_in:ip:203.0.113.7")
        );

        let auth = AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: Vec::new(),
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        };
        let user_key = build_rate_limit_key(
            &tenant,
            Some(&auth),
            None,
            AuthGraphqlSurface::ResetPassword,
        );
        assert_eq!(
            user_key,
            format!(
                "tenant:{tenant_id}:auth:graphql:reset_password:user:{}",
                auth.user_id
            )
        );
    }

    #[test]
    fn raw_forwarded_headers_do_not_define_the_actor_key() {
        let tenant_id = Uuid::new_v4();
        let tenant = tenant(tenant_id);
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.99".parse().unwrap());

        assert_eq!(
            build_rate_limit_key(&tenant, None, Some(&headers), AuthGraphqlSurface::SignUp),
            format!("tenant:{tenant_id}:auth:graphql:sign_up:anonymous")
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
}
