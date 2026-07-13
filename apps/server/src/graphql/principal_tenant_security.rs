use std::sync::Arc;

use async_graphql::extensions::{Extension, ExtensionContext, ExtensionFactory, NextExecute};
use async_graphql::{FieldError, Pos, Response};
use rustok_api::graphql::GraphQLError;

use crate::context::{AuthContext, TenantContext};

#[derive(Default)]
pub struct GraphqlPrincipalTenantPolicy;

impl ExtensionFactory for GraphqlPrincipalTenantPolicy {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(GraphqlPrincipalTenantPolicyExtension)
    }
}

struct GraphqlPrincipalTenantPolicyExtension;

#[async_trait::async_trait]
impl Extension for GraphqlPrincipalTenantPolicyExtension {
    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> Response {
        if let (Some(auth), Some(tenant)) = (
            ctx.data_opt::<AuthContext>(),
            ctx.data_opt::<TenantContext>(),
        ) {
            if auth.tenant_id != tenant.id {
                tracing::warn!(
                    operation_name = ?operation_name,
                    principal_tenant_id = %auth.tenant_id,
                    request_tenant_id = %tenant.id,
                    user_id = %auth.user_id,
                    client_id = ?auth.client_id,
                    "Rejected GraphQL principal bound to another tenant"
                );
                return Response::from_errors(vec![
                    <FieldError as GraphQLError>::permission_denied(
                        "Authenticated principal is not bound to the request tenant",
                    )
                    .into_server_error(Pos::default()),
                ]);
            }
        }

        next.run(ctx, operation_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::GraphqlPrincipalTenantPolicy;
    use async_graphql::{EmptyMutation, EmptySubscription, Object, Request, Schema};
    use rustok_api::{AuthContext, TenantContext};
    use uuid::Uuid;

    struct Query;

    #[Object]
    impl Query {
        async fn ping(&self) -> &str {
            "pong"
        }
    }

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

    fn auth(tenant_id: Uuid) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: Vec::new(),
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    #[tokio::test]
    async fn rejects_principal_from_another_tenant() {
        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(GraphqlPrincipalTenantPolicy)
            .finish();
        let response = schema
            .execute(
                Request::new("{ ping }")
                    .data(tenant(Uuid::new_v4()))
                    .data(auth(Uuid::new_v4())),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
    }

    #[tokio::test]
    async fn allows_matching_or_anonymous_principals() {
        let current = Uuid::new_v4();
        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(GraphqlPrincipalTenantPolicy)
            .finish();

        let authenticated = schema
            .execute(
                Request::new("{ ping }")
                    .data(tenant(current))
                    .data(auth(current)),
            )
            .await;
        assert!(authenticated.errors.is_empty());

        let anonymous = schema
            .execute(Request::new("{ ping }").data(tenant(current)))
            .await;
        assert!(anonymous.errors.is_empty());
    }
}