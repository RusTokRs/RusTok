use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::parser::types::{
    ExecutableDocument, OperationDefinition, SelectionSet,
};
use async_graphql::{FieldError, Name, Pos, Request, Response, ServerResult, Variables};
use async_graphql_value::{ConstValue, Value};
use rustok_api::TenantContext;
use rustok_api::graphql::GraphQLError;
use uuid::Uuid;

const TENANT_ARGUMENT_NAMES: [&str; 2] = ["tenantId", "tenant_id"];

#[derive(Clone, Debug, Default)]
struct GraphqlTenantArgumentPolicy {
    requested_tenant_ids: BTreeSet<Uuid>,
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
    // Clone variables before borrowing the parsed document from Request.
    let variables = request.variables.clone();
    let operation_name = request.operation_name.clone();
    let policy = {
        let document = request.parsed_query()?;
        tenant_argument_policy(document, &variables, operation_name.as_deref())
    };

    if policy.invalid_argument.is_some() || !policy.requested_tenant_ids.is_empty() {
        request.data.insert(policy);
    }

    Ok(())
}

fn tenant_argument_policy(
    document: &ExecutableDocument,
    variables: &Variables,
    operation_name: Option<&str>,
) -> GraphqlTenantArgumentPolicy {
    let mut policy = GraphqlTenantArgumentPolicy::default();

    for operation in super::selected_operations(document, operation_name) {
        let defaults = operation_variable_defaults(&operation.node);
        collect_selection_set_tenant_arguments(
            &operation.node.selection_set.node,
            document,
            variables,
            &defaults,
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

fn collect_selection_set_tenant_arguments(
    selection_set: &SelectionSet,
    document: &ExecutableDocument,
    variables: &Variables,
    defaults: &HashMap<Name, ConstValue>,
    policy: &mut GraphqlTenantArgumentPolicy,
) {
    if policy.invalid_argument.is_some() {
        return;
    }

    // Nested variable values are immutable within the selected operation. Scan
    // each once, independently of direct tenantId uses of the same variable.
    let mut visited_nested_variables = HashSet::new();
    let _completed = super::visit_selection_fields(document, selection_set, (), |field, ()| {
        for (argument_name, argument_value) in &field.arguments {
            if is_tenant_argument_name(argument_name.node.as_str()) {
                collect_direct_tenant_value(
                    &argument_value.node,
                    variables,
                    defaults,
                    policy,
                );
            } else {
                collect_nested_tenant_value(
                    &argument_value.node,
                    variables,
                    defaults,
                    &mut visited_nested_variables,
                    policy,
                );
            }
            if policy.invalid_argument.is_some() {
                return std::ops::ControlFlow::Break(());
            }
        }
        std::ops::ControlFlow::Continue(Some(()))
    });
}

fn is_tenant_argument_name(name: &str) -> bool {
    TENANT_ARGUMENT_NAMES.contains(&name)
}

fn collect_direct_tenant_value(
    value: &Value,
    variables: &Variables,
    defaults: &HashMap<Name, ConstValue>,
    policy: &mut GraphqlTenantArgumentPolicy,
) {
    match value {
        Value::String(raw) => add_tenant_id(raw, policy),
        Value::Null => {}
        Value::Variable(name) => {
            if let Some(value) = resolve_variable(name, variables, defaults) {
                collect_direct_tenant_const_value(value, policy);
            }
        }
        _ => set_invalid_argument(
            policy,
            "tenantId must be a UUID string, null, or UUID variable".to_string(),
        ),
    }
}

fn collect_direct_tenant_const_value(value: &ConstValue, policy: &mut GraphqlTenantArgumentPolicy) {
    match value {
        ConstValue::String(raw) => add_tenant_id(raw, policy),
        ConstValue::Null => {}
        _ => set_invalid_argument(
            policy,
            "tenantId variable must resolve to a UUID string or null".to_string(),
        ),
    }
}

fn collect_nested_tenant_value(
    value: &Value,
    variables: &Variables,
    defaults: &HashMap<Name, ConstValue>,
    visited_nested_variables: &mut HashSet<Name>,
    policy: &mut GraphqlTenantArgumentPolicy,
) {
    let mut pending = vec![(value, false)];
    while let Some((value, direct)) = pending.pop() {
        if policy.invalid_argument.is_some() {
            return;
        }
        if direct {
            collect_direct_tenant_value(value, variables, defaults, policy);
            continue;
        }
        match value {
            Value::Variable(name) => {
                if visited_nested_variables.insert(name.clone())
                    && let Some(value) = resolve_variable(name, variables, defaults)
                {
                    collect_nested_tenant_const_value(value, policy);
                }
            }
            Value::Object(object) => {
                pending.extend(
                    object
                        .iter()
                        .rev()
                        .map(|(key, child)| (child, is_tenant_argument_name(key.as_str()))),
                );
            }
            Value::List(items) => {
                pending.extend(items.iter().rev().map(|child| (child, false)));
            }
            _ => {}
        }
    }
}

fn collect_nested_tenant_const_value(value: &ConstValue, policy: &mut GraphqlTenantArgumentPolicy) {
    let mut pending = vec![(value, false)];
    while let Some((value, direct)) = pending.pop() {
        if policy.invalid_argument.is_some() {
            return;
        }
        if direct {
            collect_direct_tenant_const_value(value, policy);
            continue;
        }
        match value {
            ConstValue::Object(object) => {
                pending.extend(
                    object
                        .iter()
                        .rev()
                        .map(|(key, child)| (child, is_tenant_argument_name(key.as_str()))),
                );
            }
            ConstValue::List(items) => {
                pending.extend(items.iter().rev().map(|child| (child, false)));
            }
            _ => {}
        }
    }
}

fn resolve_variable<'a>(
    name: &Name,
    variables: &'a Variables,
    defaults: &'a HashMap<Name, ConstValue>,
) -> Option<&'a ConstValue> {
    variables.get(name).or_else(|| defaults.get(name))
}

fn add_tenant_id(raw: &str, policy: &mut GraphqlTenantArgumentPolicy) {
    match Uuid::parse_str(raw.trim()) {
        Ok(tenant_id) => {
            policy.requested_tenant_ids.insert(tenant_id);
        }
        Err(_) => set_invalid_argument(
            policy,
            "tenantId must be a valid UUID string or null".to_string(),
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
                    operation_name_present = operation_name.is_some(),
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
                    operation_name_present = operation_name.is_some(),
                    request_tenant_id = %tenant.id,
                    requested_tenant_count = policy.requested_tenant_ids.len(),
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
    async fn rejects_invalid_tenant_value_without_echoing_input() {
        let tenant_id = Uuid::new_v4();
        let hostile = "not-a-uuid secret=should-not-be-reflected";
        let response = schema()
            .execute(
                Request::new(format!("query {{ echoTenant(tenantId: \"{hostile}\") }}"))
                    .data(tenant(tenant_id)),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
        assert!(
            !response.errors[0]
                .message
                .contains("secret=should-not-be-reflected")
        );
        assert!(response.errors[0].message.contains("valid UUID"));
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
    async fn tenant_guard_checks_only_the_selected_graphql_operation() {
        let tenant_id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let query = format!(
            r#"
                query PublicInfo {{ echoText(text: "ok") }}
                query CrossTenant {{ echoTenant(tenantId: "{other}") }}
            "#
        );

        let public = schema()
            .execute(
                Request::new(&query)
                    .operation_name("PublicInfo")
                    .data(tenant(tenant_id)),
            )
            .await;
        assert!(public.errors.is_empty());
        assert_eq!(
            public
                .data
                .into_json()
                .expect("public result should serialize")["echoText"],
            "ok"
        );

        let cross_tenant = schema()
            .execute(
                Request::new(&query)
                    .operation_name("CrossTenant")
                    .data(tenant(tenant_id)),
            )
            .await;
        assert_eq!(cross_tenant.errors.len(), 1);
    }

    #[test]
    fn repeated_nested_variables_preserve_distinct_ids_and_direct_checks() {
        let tenant_id = Uuid::new_v4();
        let variables = Variables::from_json(json!({
            "input": { "items": [{ "tenantId": tenant_id }, { "tenant_id": tenant_id }] }
        }));
        let document = async_graphql::parser::parse_query(
            "query Tenant($input: TenantInput!) { a: echoInput(input: $input) b: echoInput(input: $input) }",
        )
        .expect("query parses");
        let policy = super::tenant_argument_policy(&document, &variables, Some("Tenant"));
        assert!(policy.invalid_argument.is_none());
        assert_eq!(policy.requested_tenant_ids.len(), 1);
        assert!(policy.requested_tenant_ids.contains(&tenant_id));

        // An object is invalid in a direct tenantId position even if its nested
        // contents have already been classified. Exercise pre-validation directly.
        let document = async_graphql::parser::parse_query(
            "query Tenant($input: TenantInput!) { echoInput(input: $input) echoTenant(tenantId: $input) }",
        )
        .expect("query parses");
        let policy = super::tenant_argument_policy(&document, &variables, Some("Tenant"));
        assert!(policy.invalid_argument.is_some());
    }

    #[test]
    fn deeply_nested_variable_values_preserve_tenant_checks() {
        let tenant_id = Uuid::new_v4();
        let mut value = json!({ "tenantId": tenant_id });
        for _ in 0..64 {
            value = json!([value]);
        }
        let variables = Variables::from_json(json!({ "input": value }));
        let document = async_graphql::parser::parse_query(
            "query Tenant($input: TenantInput!) { echoInput(input: $input) }",
        )
        .expect("query parses");
        let policy = super::tenant_argument_policy(&document, &variables, Some("Tenant"));
        assert!(policy.invalid_argument.is_none());
        assert_eq!(policy.requested_tenant_ids.len(), 1);
        assert!(policy.requested_tenant_ids.contains(&tenant_id));
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
