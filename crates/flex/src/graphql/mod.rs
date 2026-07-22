//! Owner-owned GraphQL surface for the Flex capability.

mod mutation;
mod query;
mod runtime;
mod types;

use async_graphql::{Context, FieldError, Result};
use rustok_api::{
    AuthContext, Permission, TenantContext, graphql::GraphQLError, has_effective_permission,
};
use rustok_core::field_schema::is_valid_field_key;

pub use mutation::FlexMutation;
pub use query::FlexQuery;
pub use runtime::FlexGraphqlRuntime;
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

fn bad_user_input(message: impl AsRef<str>) -> FieldError {
    <FieldError as GraphQLError>::bad_user_input(message.as_ref())
}

fn resolve_entity_type(entity_type: Option<String>) -> Result<String> {
    let resolved = entity_type
        .unwrap_or_else(|| "user".to_string())
        .trim()
        .to_ascii_lowercase();

    if resolved.is_empty() {
        return Err(bad_user_input("entity_type must not be empty"));
    }

    if !is_valid_field_key(&resolved) {
        return Err(bad_user_input(
            "entity_type must match ^[a-z][a-z0-9_]{0,127}$",
        ));
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use async_graphql::ErrorExtensions;

    use super::resolve_entity_type;

    fn error_code(error: &async_graphql::Error) -> Option<String> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
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
    }

    #[test]
    fn resolve_entity_type_rejects_invalid_format() {
        let gql = resolve_entity_type(Some("product-type".to_string()))
            .expect_err("invalid entity type should fail")
            .extend();
        assert_eq!(error_code(&gql).as_deref(), Some("BAD_USER_INPUT"));
    }
}
