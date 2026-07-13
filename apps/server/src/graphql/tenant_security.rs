use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::{FieldError, Pos, Request, Response, ServerResult};
use rustok_api::TenantContext;
use uuid::Uuid;

use rustok_api::graphql::GraphQLError;

const TENANT_ARGUMENT_NAMES: [&str; 2] = ["tenantId", "tenant_id"];

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

fn classify_tenant_arguments(request: &mut Request) {
    if request.query.trim().is_empty() {
        return;
    }

    let variables = serde_json::to_value(&request.variables).unwrap_or_default();
    let mut policy = GraphqlTenantArgumentPolicy::default();

    collect_query_tenant_arguments(&request.query, &variables, &mut policy);
    if policy.invalid_argument.is_none() {
        collect_nested_tenant_variables(&variables, "$variables", &mut policy);
    }

    policy.requested_tenant_ids.sort_unstable();
    policy.requested_tenant_ids.dedup();
    if policy.invalid_argument.is_some() || !policy.requested_tenant_ids.is_empty() {
        request.data.insert(policy);
    }
}

fn collect_query_tenant_arguments(
    query: &str,
    variables: &serde_json::Value,
    policy: &mut GraphqlTenantArgumentPolicy,
) {
    let bytes = query.as_bytes();
    let mut cursor = 0;

    while cursor < bytes.len() && policy.invalid_argument.is_none() {
        let Some((name_start, name)) = find_next_tenant_argument_name(query, cursor) else {
            break;
        };
        let mut value_start = name_start + name.len();
        value_start = skip_ascii_whitespace(bytes, value_start);
        if bytes.get(value_start) != Some(&b':') {
            cursor = name_start + name.len();
            continue;
        }
        value_start = skip_ascii_whitespace(bytes, value_start + 1);

        match bytes.get(value_start).copied() {
            Some(b'"') => {
                let content_start = value_start + 1;
                let Some(relative_end) = query[content_start..].find('"') else {
                    policy.invalid_argument = Some(format!(
                        "GraphQL tenant argument `{name}` contains an unterminated string"
                    ));
                    break;
                };
                let content_end = content_start + relative_end;
                add_tenant_id(
                    &query[content_start..content_end],
                    &format!("GraphQL argument `{name}`"),
                    policy,
                );
                cursor = content_end + 1;
            }
            Some(b'$') => {
                let variable_start = value_start + 1;
                let variable_end = read_graphql_identifier_end(bytes, variable_start);
                if variable_end == variable_start {
                    policy.invalid_argument = Some(format!(
                        "GraphQL tenant argument `{name}` references an invalid variable"
                    ));
                    break;
                }
                let variable_name = &query[variable_start..variable_end];
                match variables.get(variable_name) {
                    Some(serde_json::Value::String(value)) => add_tenant_id(
                        value,
                        &format!("GraphQL tenant variable `${variable_name}`"),
                        policy,
                    ),
                    Some(serde_json::Value::Null) | None => {}
                    Some(_) => {
                        policy.invalid_argument = Some(format!(
                            "GraphQL tenant variable `${variable_name}` must be a UUID string"
                        ));
                    }
                }
                cursor = variable_end;
            }
            _ if query[value_start..].starts_with("null") => {
                cursor = value_start + 4;
            }
            _ => {
                policy.invalid_argument = Some(format!(
                    "GraphQL tenant argument `{name}` must be a UUID string or variable"
                ));
            }
        }
    }
}

fn find_next_tenant_argument_name(query: &str, from: usize) -> Option<(usize, &'static str)> {
    TENANT_ARGUMENT_NAMES
        .iter()
        .filter_map(|name| {
            query[from..]
                .find(name)
                .map(|relative| (from + relative, *name))
        })
        .filter(|(start, name)| is_identifier_boundary(query.as_bytes(), *start, name.len()))
        .min_by_key(|(start, _)| *start)
}

fn is_identifier_boundary(bytes: &[u8], start: usize, len: usize) -> bool {
    let before_ok = start == 0 || !is_graphql_identifier_byte(bytes[start - 1]);
    let end = start + len;
    let after_ok = end >= bytes.len() || !is_graphql_identifier_byte(bytes[end]);
    before_ok && after_ok
}

fn skip_ascii_whitespace(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes
        .get(cursor)
        .is_some_and(|value| value.is_ascii_whitespace())
    {
        cursor += 1;
    }
    cursor
}

fn read_graphql_identifier_end(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes
        .get(cursor)
        .is_some_and(|value| is_graphql_identifier_byte(*value))
    {
        cursor += 1;
    }
    cursor
}

fn is_graphql_identifier_byte(value: u8) -> bool {
    value == b'_' || value.is_ascii_alphanumeric()
}

fn collect_nested_tenant_variables(
    value: &serde_json::Value,
    path: &str,
    policy: &mut GraphqlTenantArgumentPolicy,
) {
    if policy.invalid_argument.is_some() {
        return;
    }

    match value {
        serde_json::Value::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{path}.{key}");
                if TENANT_ARGUMENT_NAMES.contains(&key.as_str()) {
                    match child {
                        serde_json::Value::String(raw) => {
                            add_tenant_id(raw, &child_path, policy)
                        }
                        serde_json::Value::Null => {}
                        _ => {
                            policy.invalid_argument = Some(format!(
                                "GraphQL tenant variable `{child_path}` must be a UUID string"
                            ));
                        }
                    }
                } else {
                    collect_nested_tenant_variables(child, &child_path, policy);
                }
                if policy.invalid_argument.is_some() {
                    return;
                }
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_nested_tenant_variables(
                    child,
                    &format!("{path}[{index}]"),
                    policy,
                );
                if policy.invalid_argument.is_some() {
                    return;
                }
            }
        }
        _ => {}
    }
}

fn add_tenant_id(raw: &str, source: &str, policy: &mut GraphqlTenantArgumentPolicy) {
    match Uuid::parse_str(raw.trim()) {
        Ok(tenant_id) => policy.requested_tenant_ids.push(tenant_id),
        Err(_) => {
            policy.invalid_argument = Some(format!(
                "{source} value `{raw}` is not a valid UUID"
            ));
        }
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
    use async_graphql::{
        EmptyMutation, EmptySubscription, InputObject, Object, Request, Schema, Variables,
    };
    use rustok_api::TenantContext;
    use serde_json::json;
    use uuid::Uuid;

    #[derive(InputObject)]
    struct TenantInput {
        tenant_id: Option<Uuid>,
    }

    struct Query;

    #[Object]
    impl Query {
        async fn echo_tenant(&self, tenant_id: Option<Uuid>) -> Option<Uuid> {
            tenant_id
        }

        async fn echo_input(&self, input: TenantInput) -> Option<Uuid> {
            input.tenant_id
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
    async fn rejects_nested_input_variable_cross_tenant_argument() {
        let tenant_id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(GraphqlTenantPolicy)
            .finish();
        let response = schema
            .execute(
                Request::new(
                    "query Tenant($input: TenantInput!) { echoInput(input: $input) }",
                )
                .variables(Variables::from_json(json!({
                    "input": { "tenantId": other }
                })))
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