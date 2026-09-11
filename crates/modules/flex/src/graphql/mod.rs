//! Owner-owned GraphQL surface for the Flex capability.

mod mutation;
mod query;
mod runtime;
mod types;

use async_graphql::{Context, FieldError, Result};
use rustok_api::{
    AuthContext, Permission, TenantContext, graphql::GraphQLError, has_effective_permission,
};

pub use mutation::FlexMutation;
pub use query::FlexQuery;
pub use runtime::{AttachedValuesGraphqlPort, FlexGraphqlRuntime};
pub use types::*;

fn require_access(
    ctx: &Context<'_>,
    permission: Permission,
) -> Result<(TenantContext, AuthContext)> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    if !has_effective_permission(&auth.permissions, &permission) {
        return Err(<FieldError as GraphQLError>::permission_denied(&format!(
            "{permission} required"
        )));
    }

    let tenant = ctx.data::<TenantContext>()?;
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Authenticated tenant does not match request tenant",
        ));
    }

    Ok((tenant.clone(), auth.clone()))
}

fn map_flex_error(error: rustok_core::field_schema::FlexError) -> FieldError {
    let mapped = crate::map_flex_error(error);
    match mapped.kind {
        crate::FlexMappedErrorKind::Internal => {
            <FieldError as GraphQLError>::internal_error(&mapped.message)
        }
        crate::FlexMappedErrorKind::NotFound => {
            <FieldError as GraphQLError>::not_found(&mapped.message)
        }
        crate::FlexMappedErrorKind::BadUserInput => {
            <FieldError as GraphQLError>::bad_user_input(&mapped.message)
        }
    }
}

fn map_attached_field_policy_error(error: crate::FlexAttachedFieldPolicyError) -> FieldError {
    match error {
        crate::FlexAttachedFieldPolicyError::Invalid(message) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        crate::FlexAttachedFieldPolicyError::Storage(_) => {
            <FieldError as GraphQLError>::internal_error(
                "Flex attached field policy storage failed",
            )
        }
    }
}

fn bad_user_input(message: impl AsRef<str>) -> FieldError {
    <FieldError as GraphQLError>::bad_user_input(message.as_ref())
}

fn resolve_entity_type(entity_type: Option<String>) -> Result<String> {
    let raw = entity_type.unwrap_or_else(|| "user".to_string());
    crate::normalize_flex_entity_type(&raw).ok_or_else(|| {
        bad_user_input(
            "entity_type must be a dot-namespaced identifier such as user or taxonomy.category",
        )
    })
}

#[cfg(test)]
mod tests {
    use async_graphql::{EmptySubscription, ErrorExtensions, Request, Response, Schema};
    use rustok_api::{AuthContext, Permission, TenantContext};
    use uuid::Uuid;

    use super::{FlexMutation, FlexQuery, resolve_entity_type};

    fn error_code(error: &async_graphql::Error) -> Option<String> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
    }

    fn response_error_code(response: &Response) -> Option<String> {
        response
            .errors
            .first()
            .and_then(|error| error.extensions.as_ref())
            .and_then(|extensions| extensions.get("code"))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
    }

    fn tenant_context(tenant_id: Uuid) -> TenantContext {
        TenantContext {
            id: tenant_id,
            name: "Flex policy test".to_string(),
            slug: "flex-policy-test".to_string(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn auth_context(tenant_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
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

    async fn execute_policy_request(
        tenant_id: Uuid,
        permissions: Vec<Permission>,
        document: &str,
    ) -> Response {
        Schema::build(
            FlexQuery::default(),
            FlexMutation::default(),
            EmptySubscription,
        )
        .finish()
        .execute(
            Request::new(document)
                .data(tenant_context(tenant_id))
                .data(auth_context(tenant_id, permissions)),
        )
        .await
    }

    fn assert_single_response_error(
        response: &Response,
        expected_code: &str,
        expected_message_fragment: &str,
    ) {
        assert_eq!(response.errors.len(), 1, "unexpected response: {response:?}");
        assert_eq!(
            response_error_code(response).as_deref(),
            Some(expected_code),
            "unexpected GraphQL error code: {response:?}"
        );
        assert!(
            response.errors[0]
                .message
                .contains(expected_message_fragment),
            "unexpected GraphQL error message: {response:?}"
        );
    }

    #[test]
    fn resolve_entity_type_defaults_to_user() {
        assert_eq!(resolve_entity_type(None).expect("default"), "user");
    }

    #[test]
    fn resolve_entity_type_normalizes_input() {
        assert_eq!(
            resolve_entity_type(Some(" Product ".to_string())).expect("normalize"),
            "product"
        );
        assert_eq!(
            resolve_entity_type(Some(" Taxonomy.Category ".to_string())).expect("namespace"),
            "taxonomy.category"
        );
    }

    #[test]
    fn resolve_entity_type_rejects_invalid_format() {
        for invalid in ["product-type", "taxonomy..category", ".category"] {
            let gql = resolve_entity_type(Some(invalid.to_string()))
                .expect_err("invalid entity type should fail")
                .extend();
            assert_eq!(error_code(&gql).as_deref(), Some("BAD_USER_INPUT"));
        }
    }

    #[tokio::test]
    async fn attached_field_policy_graphql_requires_schema_list_and_update_permissions() {
        let tenant_id = Uuid::new_v4();
        let query =
            "{ attachedFieldPolicies(entityType: \"taxonomy.category\") { fieldKey } }";

        let denied_query = execute_policy_request(
            tenant_id,
            vec![Permission::FLEX_SCHEMAS_READ],
            query,
        )
        .await;
        assert_single_response_error(&denied_query, "PERMISSION_DENIED", "required");

        let allowed_query = execute_policy_request(
            tenant_id,
            vec![Permission::FLEX_SCHEMAS_LIST],
            query,
        )
        .await;
        assert_single_response_error(
            &allowed_query,
            "INTERNAL_ERROR",
            "FlexGraphqlRuntime is not registered",
        );

        let mutation = "mutation { resetAttachedFieldPolicy(input: { entityType: \"taxonomy.category\", fieldKey: \"tagline\" }) { fieldKey } }";
        let denied_mutation = execute_policy_request(
            tenant_id,
            vec![Permission::FLEX_SCHEMAS_LIST],
            mutation,
        )
        .await;
        assert_single_response_error(&denied_mutation, "PERMISSION_DENIED", "required");

        let allowed_mutation = execute_policy_request(
            tenant_id,
            vec![Permission::FLEX_SCHEMAS_UPDATE],
            mutation,
        )
        .await;
        assert_single_response_error(
            &allowed_mutation,
            "INTERNAL_ERROR",
            "FlexGraphqlRuntime is not registered",
        );
    }
}
