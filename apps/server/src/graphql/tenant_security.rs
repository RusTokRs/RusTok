use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::parser::types::{
    ExecutableDocument, OperationDefinition, Selection, SelectionSet,
};
use async_graphql::{FieldError, Name, Pos, Request, Response, ServerResult, Variables};
use async_graphql_value::{ConstValue, Value};
use rustok_api::TenantContext;
use rustok_api::graphql::GraphQLError;
use uuid::Uuid;

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

fn classify_tenant_arguments(request: &mut Request) -> ServerResult<()> {
    if request.query.trim().is_empty() {
        return Ok(());
    }

    // Clone variables before borrowing the parsed document from Request.
    let variables = request.variables.clone();
    let mut policy = {
        let document = request.parsed_query()?;
        tenant_argument_policy(document, &variables)
    };

    policy.requested_tenant_ids.sort_unstable();
    policy.requested_tenant_ids.dedup();
    if policy.invalid_argument.is_some() || !policy.requested_tenant_ids.is_empty() {
        request.data.insert(policy);
    }

    Ok(())
}

fn tenant_argument_policy(
    document: &ExecutableDocument,
    variables: &Variables,
) -> GraphqlTenantArgumentPolicy {
    let mut policy = GraphqlTenantArgumentPolicy::default();

    for (_, operation) in document.operations.iter() {
        let defaults = operation_variable_defaults(&operation.node);
        let mut visited_fragments = HashSet::new();
        collect_selection_set_tenant_arguments(
            &operation.node.selection_set.node,
            document,
            variables,
            &defaults,
            &mut visited_fragments,
            &mut policy,
        );
        if policy.invalid_argument.is_some() {
            break;
        }
    }

    policy
}

fn operation_variable_defaults(operation: &OperationDefinition) -> HashMap<Name, ConstValue> {
    operation
        .variable_definitions
        .iter()
        .filter_map(|definition| {
            definition
                .node
                .default_value()
                .cloned()
                .map(|value| (definition.node.name.node.clone(), value))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn collect_selection_set_tenant_arguments(
    selection_set: &SelectionSet,
    document: &ExecutableDocument,
    variables: &Variables,
    defaults: &HashMap<Name, ConstValue>,
    visited_fragments: &mut HashSet<Name>,
    policy: &mut GraphqlTenantArgumentPolicy,
) {
    if policy.invalid_argument.is_some() {
        return;
    }

    for selection in &selection_set.items {
        match &selection.node {
            Selection::Field(field) => {
                for (argument_name, argument_value) in &field.node.arguments {
                    let source = format!(
                        "GraphQL field `{}` argument `{}`",
                        field.node.name.node, argument_name.node
                    );
                    if is_tenant_argument_name(argument_name.node.as_str()) {
                        collect_direct_tenant_value(
                            &argument_value.node,
                            variables,
                            defaults,
                            &source,
                            policy,
                        );
                    } else {
                        collect_nested_tenant_value(
                            &argument_value.node,
                            variables,
                            defaults,
                            &source,
                            policy,
                        );
                    }
                    if policy.invalid_argument.is_some() {
                        return;
                    }
                }

                collect_selection_set_tenant_arguments(
                    &field.node.selection_set.node,
                    document,
                    variables,
                    defaults,
                    visited_fragments,
                    policy,
                );
            }
            Selection::FragmentSpread(fragment) => {
                let fragment_name = fragment.node.fragment_name.node.clone();
                if visited_fragments.insert(fragment_name.clone()) {
                    if let Some(definition) = document.fragments.get(&fragment_name) {
                        collect_selection_set_tenant_arguments(
                            &definition.node.selection_set.node,
                            document,
                            variables,
                            defaults,
                            visited_fragments,
                            policy,
                        );
                    }
                }
            }
            Selection::InlineFragment(fragment) => collect_selection_set_tenant_arguments(
                &fragment.node.selection_set.node,
                document,
                variables,
                defaults,
                visited_fragments,
                policy,
            ),
        }

        if policy.invalid_argument.is_some() {
            return;
        }
    }
}

fn is_tenant_argument_name(name: &str) -> bool {
    TENANT_ARGUMENT_NAMES.contains(&name)
}

fn collect_direct_tenant_value(
    value: &Value,
    variables: &Variables,
    defaults: &HashMap<Name, ConstValue>,
    source: &str,
    policy: &mut GraphqlTenantArgumentPolicy,
) {
    match value {
        Value::String(raw) => add_tenant_id(raw, source, policy),
        Value::Null => {}
        Value::Variable(name) => {
            if let Some(value) = resolve_variable(name, variables, defaults) {
                collect_direct_tenant_const_value(value, source, policy);
            }
        }
        _ => set_invalid_argument(
            policy,
            format!("{source} must be a UUID string, null, or UUID variable"),
        ),
    }
}

fn collect_direct_tenant_const_value(
    value: &ConstValue,
    source: &str,
    policy: &mut GraphqlTenantArgumentPolicy,
) {
    match value {
        ConstValue::String(raw) => add_tenant_id(raw, source, policy),
        ConstValue::Null => {}
        _ => set_invalid_argument(
            policy,
            format!("{source} variable must resolve to a UUID string or null"),
        ),
    }
}

fn collect_nested_tenant_value(
    value: &Value,
    variables: &Variables,
    defaults: &HashMap<Name, ConstValue>,
    path: &str,
    policy: &mut GraphqlTenantArgumentPolicy,
) {
    if policy.invalid_argument.is_some() {
        return;
    }

    match value {
        Value::Variable(name) => {
            if let Some(value) = resolve_variable(name, variables, defaults) {
                collect_nested_tenant_const_value(value, &format!("{path}.${name}"), policy);
            }
        }
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{path}.{}", key.as_str());
                if is_tenant_argument_name(key.as_str()) {
                    collect_direct_tenant_value(child, variables, defaults, &child_path, policy);
                } else {
                    collect_nested_tenant_value(child, variables, defaults, &child_path, policy);
                }
                if policy.invalid_argument.is_some() {
                    return;
                }
            }
        }
        Value::List(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_nested_tenant_value(
                    child,
                    variables,
                    defaults,
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

fn collect_nested_tenant_const_value(
    value: &ConstValue,
    path: &str,
    policy: &mut GraphqlTenantArgumentPolicy,
) {
    if policy.invalid_argument.is_some() {
        return;
    }

    match value {
        ConstValue::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{path}.{}", key.as_str());
                if is_tenant_argument_name(key.as_str()) {
                    collect_direct_tenant_const_value(child, &child_path, policy);
                } else {
                    collect_nested_tenant_const_value(child, &child_path, policy);
                }
                if policy.invalid_argument.is_some() {
                    return;
                }
            }
        }
        ConstValue::List(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_nested_tenant_const_value(child, &format!("{path}[{index}]"), policy);
                if policy.invalid_argument.is_some() {
                    return;
                }
            }
        }
        _ => {}
    }
}

fn resolve_variable<'a>(
    name: &Name,
    variables: &'a Variables,
    defaults: &'a HashMap<Name, ConstValue>,
) -> Option<&'a ConstValue> {
    variables.get(name).or_else(|| defaults.get(name))
}

fn add_tenant_id(raw: &str, source: &str, policy: &mut GraphqlTenantArgumentPolicy) {
    match Uuid::parse_str(raw.trim()) {
        Ok(tenant_id) => policy.requested_tenant_ids.push(tenant_id),
        Err(_) => set_invalid_argument(
            policy,
            format!("{source} value `{raw}` is not a valid UUID"),
        ),
    }
}

fn set_invalid_argument(policy: &mut GraphqlTenantArgumentPolicy, message: String) {
    if policy.invalid_argument.is_none() {
        policy.invalid_argument = Some(message);
    }
}

fn tenant_policy_error(message: &str) -> Response {
    Response::from_errors(vec![
        <FieldError as GraphQLError>::permission_denied(message).into_server_error(Pos::default()),
    ])
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
        classify_tenant_arguments(&mut request)?;
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
                tracing::warn!(
                    operation_name = ?operation_name,
                    message,
                    "Rejected invalid GraphQL tenant argument"
                );
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

        async fn echo_text(&self, text: String) -> String {
            text
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

    fn schema() -> Schema<Query, EmptyMutation, EmptySubscription> {
        Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(GraphqlTenantPolicy)
            .finish()
    }

    #[tokio::test]
    async fn rejects_literal_cross_tenant_argument() {
        let tenant_id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let response = schema()
            .execute(
                Request::new(format!("query {{ echoTenant(tenantId: \"{other}\") }}"))
                    .data(tenant(tenant_id)),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
    }

    #[tokio::test]
    async fn rejects_variable_cross_tenant_argument() {
        let tenant_id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let response = schema()
            .execute(
                Request::new("query Tenant($target: UUID) { echoTenant(tenantId: $target) }")
                    .variables(Variables::from_json(json!({ "target": other })))
                    .data(tenant(tenant_id)),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
    }

    #[tokio::test]
    async fn rejects_defaulted_cross_tenant_variable() {
        let tenant_id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let response = schema()
            .execute(
                Request::new(format!(
                    "query Tenant($target: UUID = \"{other}\") {{ echoTenant(tenantId: $target) }}"
                ))
                .data(tenant(tenant_id)),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
    }

    #[tokio::test]
    async fn rejects_nested_input_variable_cross_tenant_argument() {
        let tenant_id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let response = schema()
            .execute(
                Request::new("query Tenant($input: TenantInput!) { echoInput(input: $input) }")
                    .variables(Variables::from_json(json!({
                        "input": { "tenantId": other }
                    })))
                    .data(tenant(tenant_id)),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
    }

    #[tokio::test]
    async fn rejects_nested_literal_cross_tenant_argument() {
        let tenant_id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let response = schema()
            .execute(
                Request::new(format!(
                    "query {{ echoInput(input: {{ tenantId: \"{other}\" }}) }}"
                ))
                .data(tenant(tenant_id)),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
    }

    #[tokio::test]
    async fn follows_fragment_spreads() {
        let tenant_id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let response = schema()
            .execute(
                Request::new(format!(
                    "query Tenant {{ ...TenantField }} fragment TenantField on Query {{ echoTenant(tenantId: \"{other}\") }}"
                ))
                .data(tenant(tenant_id)),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
    }

    #[tokio::test]
    async fn ignores_tenant_text_inside_normal_string_values() {
        let tenant_id = Uuid::new_v4();
        let text = format!("tenantId: \"{}\"", Uuid::new_v4());
        let response = schema()
            .execute(
                Request::new("query Text($text: String!) { echoText(text: $text) }")
                    .variables(Variables::from_json(json!({ "text": text })))
                    .data(tenant(tenant_id)),
            )
            .await;

        assert!(response.errors.is_empty());
    }

    #[tokio::test]
    async fn allows_matching_request_tenant() {
        let tenant_id = Uuid::new_v4();
        let response = schema()
            .execute(
                Request::new(format!("query {{ echoTenant(tenantId: \"{tenant_id}\") }}"))
                    .data(tenant(tenant_id)),
            )
            .await;

        assert!(response.errors.is_empty());
    }
}
