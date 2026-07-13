use std::sync::{Arc, OnceLock};

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::{FieldError, Pos, Request, Response, ServerResult};
use regex::Regex;
use rustok_api::TenantContext;
use uuid::Uuid;

use rustok_api::graphql::GraphQLError;

#[derive(Clone, Debug, Default)]
struct GraphqlTenantArgumentPolicy {
    requested_tenant_ids: Vec<Uuid>,
    invalid_argument: Option<String>,
}

#[derive(Default)]
pub struct GraphqlTenantPolicy;

impl ExtensionFactory for GraphqlTenantPolicy {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(GraphqlTenantPolicyExtension)
    }
}

struct GraphqlTenantPolicyExtension;

fn tenant_argument_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?x)
                \b(?:tenantId|tenant_id)\s*:\s*
                (?:
                    \"(?P<literal>[0-9a-fA-F-]{36})\"
                    |
                    \$(?P<variable>[_A-Za-z][_0-9A-Za-z]*)
                )
            "#,
        )
        .expect("GraphQL tenant argument regex must compile")
    })
}

fn classify_tenant_arguments(request: &mut Request) {
    if request.query.trim().is_empty() {
        return;
    }

    let variables = serde_json::to_value(&request.variables).unwrap_or_default();
    let mut policy = GraphqlTenantArgumentPolicy::default();

    for captures in tenant_argument_regex().captures_iter(&request.query) {
        let raw = if let Some(literal) = captures.name("literal") {
            Some(literal.as_str().to_string())
        } else if let Some(variable) = captures.name("variable") {
            match variables.get(variable.as_str()) {
                Some(serde_json::Value::String(value)) => Some(value.clone()),
                Some(serde_json::Value::Null) | None => None,
                Some(_) => {
                    policy.invalid_argument = Some(format!(
                        "GraphQL tenant variable `${}` must be a UUID string",
                        variable.as_str()
                    ));
                    break;
                }
            }
        } else {
            None
        };

        let Some(raw) = raw else {
            continue;
        };
        match Uuid::parse_str(raw.trim()) {
            Ok(tenant_id) => policy.requested_tenant_ids.push(tenant_id),
            Err(_) => {
                policy.invalid_argument = Some(format!(
                    "GraphQL tenant argument `{raw}` is not a valid UUID"
                ));
                break;
            }
        }
    }

    policy.requested_tenant_ids.sort_unstable();
    policy.requested_tenant_ids.dedup();
    if policy.invalid_argument.is_some() || !policy.requested_tenant_ids.is_empty() {
        request.data.insert(policy);
    }
}

fn tenant_policy_error(message: &str) -> Response {
    Response::from_errors(vec![<FieldError as GraphQLError>::permission_denied(message)
        .into_server_error(Pos::default())])
}

#[async_trait::async_trait]
impl Extension for GraphqlTenantPolicyExtension {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        let mut request = next.run(ctx, request).await?;
        classify_tenant_arguments(&mut request);
        Ok(request)
    }

    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> Response {
        if let Some(policy) = ctx.data_opt::<GraphqlTenantArgumentPolicy>() {
            if let Some(message) = policy.invalid_argument.as_deref() {
                tracing::warn!(operation_name = ?operation_name, message, "Rejected invalid GraphQL tenant argument");
                return tenant_policy_error(message);
            }

            let Some(tenant) = ctx.data_opt::<TenantContext>() else {
                return tenant_policy_error("GraphQL request tenant context is unavailable");
            };
            if policy
                .requested_tenant_ids
                .iter()
                .any(|requested| requested != &tenant.id)
            {
                tracing::warn!(
                    operation_name = ?operation_name,
                    request_tenant_id = %tenant.id,
                    requested_tenant_ids = ?policy.requested_tenant_ids,
                    "Rejected cross-tenant GraphQL argument before resolver execution"
                );
                return tenant_policy_error(
                    "tenantId does not match the authenticated request tenant",
                );
            }
        }

        next.run(ctx, operation_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::GraphqlTenantPolicy;
    use async_graphql::{EmptyMutation, EmptySubscription, Object, Request, Schema, Variables};
    use rustok_api::TenantContext;
    use serde_json::json;
    use uuid::Uuid;

    struct Query;

    #[Object]
    impl Query {
        async fn echo_tenant(&self, tenant_id: Option<Uuid>) -> Option<Uuid> {
            tenant_id
        }
    }

    fn tenant(id: Uuid) -> TenantContext {
        TenantContext {
            id,
            name: "Tenant".to_string(),
            slug: "tenant".to_string(),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    #[tokio::test]
    async fn rejects_literal_cross_tenant_argument() {
        let tenant_id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(GraphqlTenantPolicy)
            .finish();
        let response = schema
            .execute(
                Request::new(format!(
                    "query {{ echoTenant(tenantId: \"{other}\") }}"
                ))
                .data(tenant(tenant_id)),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
    }

    #[tokio::test]
    async fn rejects_variable_cross_tenant_argument() {
        let tenant_id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(GraphqlTenantPolicy)
            .finish();
        let response = schema
            .execute(
                Request::new(
                    "query Tenant($target: UUID) { echoTenant(tenantId: $target) }",
                )
                .variables(Variables::from_json(json!({ "target": other })))
                .data(tenant(tenant_id)),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
    }

    #[tokio::test]
    async fn allows_matching_request_tenant() {
        let tenant_id = Uuid::new_v4();
        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(GraphqlTenantPolicy)
            .finish();
        let response = schema
            .execute(
                Request::new(format!(
                    "query {{ echoTenant(tenantId: \"{tenant_id}\") }}"
                ))
                .data(tenant(tenant_id)),
            )
            .await;

        assert!(response.errors.is_empty());
    }
}