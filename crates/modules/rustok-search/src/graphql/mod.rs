mod forum_projection_reconciliation;
mod forum_storefront;
mod mutation;
mod query;
mod rate_limit;
mod types;

pub use forum_projection_reconciliation::{
    ForumSearchProjectionReconciliationQuery, GqlForumSearchProjectionDrift,
    GqlForumSearchProjectionReconciliationStatus,
};
pub use forum_storefront::ForumStorefrontSearchQuery;
pub use mutation::SearchMutationRoot;
pub use query::SearchQueryRoot;
pub use rate_limit::{
    SearchGraphqlRateLimitError, SearchGraphqlRateLimitExceeded, SearchGraphqlRateLimiter,
    SearchGraphqlRateLimiterHandle,
};
pub use types::*;

const SEARCH_INTERNAL_ERROR_MESSAGE: &str = "Search service is temporarily unavailable";

pub(super) fn map_search_module_error(error: impl Into<rustok_core::Error>) -> async_graphql::FieldError {
    match error.into() {
        rustok_core::Error::Validation(message) => {
            <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::bad_user_input(
                &message,
            )
        }
        rustok_core::Error::NotFound(message) => {
            <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::not_found(&message)
        }
        rustok_core::Error::InvalidIdFormat(message) => {
            <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::bad_user_input(
                &message,
            )
        }
        rustok_core::Error::Forbidden(_) => {
            <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::permission_denied(
                "Search operation is not permitted",
            )
        }
        rustok_core::Error::Auth(_) => {
            <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::unauthenticated()
        }
        rustok_core::Error::Database(_)
        | rustok_core::Error::Serialization(_)
        | rustok_core::Error::Cache(_)
        | rustok_core::Error::Scripting(_)
        | rustok_core::Error::Internal(_)
        | rustok_core::Error::External(_) => {
            <async_graphql::FieldError as rustok_api::graphql::GraphQLError>::internal_error(
                SEARCH_INTERNAL_ERROR_MESSAGE,
            )
        }
    }
}

async fn ensure_search_admin_permission(
    ctx: &async_graphql::Context<'_>,
    permission: &rustok_api::Permission,
) -> async_graphql::Result<()> {
    use rustok_api::graphql::GraphQLError;

    let auth = ctx
        .data::<rustok_api::AuthContext>()
        .map_err(|_| <async_graphql::FieldError as GraphQLError>::unauthenticated())?;
    let tenant = ctx.data::<rustok_api::TenantContext>()?;

    if auth.tenant_id != tenant.id {
        tracing::warn!(
            auth_tenant_id = %auth.tenant_id,
            resolved_tenant_id = %tenant.id,
            code = "search.graphql_tenant_scope_mismatch",
            boundary = "search_graphql_admin",
            "Search GraphQL admin authority cannot cross the resolved tenant boundary"
        );
        return Err(
            <async_graphql::FieldError as GraphQLError>::permission_denied(
                "Search administration access is denied",
            ),
        );
    }

    if !rustok_api::has_effective_permission(&auth.permissions, permission) {
        return Err(
            <async_graphql::FieldError as GraphQLError>::permission_denied(&format!(
                "{permission} required"
            )),
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use async_graphql::{ErrorExtensions, FieldError};
    use super::map_search_module_error;

    fn error_code(error: &FieldError) -> Option<String> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
            .and_then(|value| value.clone().into_json().ok())
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
    }

    #[test]
    fn internal_search_errors_are_redacted_at_graphql_boundary() {
        let internal = map_search_module_error(rustok_core::Error::Internal(
            "password=super-secret database connection details".to_string(),
        ));
        let external = map_search_module_error(rustok_core::Error::External(
            "upstream secret=driver-details".to_string(),
        ));

        assert_eq!(internal.message, "Search service is temporarily unavailable");
        assert_eq!(external.message, "Search service is temporarily unavailable");
        assert_eq!(error_code(&internal).as_deref(), Some("INTERNAL_ERROR"));
        assert_eq!(error_code(&external).as_deref(), Some("INTERNAL_ERROR"));
        assert!(!internal.message.contains("super-secret"));
        assert!(!external.message.contains("driver-details"));
    }

    #[test]
    fn caller_safe_search_errors_keep_typed_graphql_codes() {
        let validation = map_search_module_error(rustok_core::Error::Validation(
            "query is invalid".to_string(),
        ));
        let not_found =
            map_search_module_error(rustok_core::Error::NotFound("document missing".to_string()));

        assert_eq!(validation.message, "query is invalid");
        assert_eq!(not_found.message, "document missing");
        assert_eq!(error_code(&validation).as_deref(), Some("BAD_USER_INPUT"));
        assert_eq!(error_code(&not_found).as_deref(), Some("NOT_FOUND"));
    }
}
