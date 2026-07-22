pub mod auth_mutation;
pub mod auth_query;
pub mod auth_types;
pub mod mutation;
pub mod query;
pub mod types;

use std::sync::Arc;

use async_graphql::{Context, ErrorExtensions, FieldError, Result};
use rustok_api::{AuthContext, TenantContext, graphql::GraphQLError};
use rustok_core::{Locale, ModuleRuntimeExtensions, i18n::translate};

use crate::{
    AuthAdminMutationContext, AuthAdminMutationError, AuthLifecycleContext,
    AuthLifecycleMutationError, AuthLifecycleRuntime, OAuthAdminRuntime,
};

pub use auth_mutation::AuthMutation;
pub use auth_query::AuthQuery;
pub use auth_types::*;
pub use mutation::OAuthMutation;
pub use query::OAuthQuery;
pub use types::*;

fn require_auth_context<'a>(ctx: &'a Context<'a>) -> Result<&'a AuthContext> {
    ctx.data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())
}

fn optional_auth_context<'a>(ctx: &'a Context<'a>) -> Option<&'a AuthContext> {
    ctx.data::<AuthContext>().ok()
}

fn runtime(ctx: &Context<'_>) -> Result<OAuthAdminRuntime> {
    ctx.data::<Arc<ModuleRuntimeExtensions>>()?
        .get::<OAuthAdminRuntime>()
        .cloned()
        .ok_or_else(|| {
            <FieldError as GraphQLError>::internal_error(
                "OAuthAdminRuntime is not registered; initialize shared host runtime providers",
            )
        })
}

fn auth_runtime(ctx: &Context<'_>) -> Result<AuthLifecycleRuntime> {
    ctx.data::<Arc<ModuleRuntimeExtensions>>()?
        .get::<AuthLifecycleRuntime>()
        .cloned()
        .ok_or_else(|| {
            <FieldError as GraphQLError>::internal_error(
                "AuthLifecycleRuntime is not registered; initialize shared host runtime providers",
            )
        })
}

fn mutation_context(auth: &AuthContext) -> AuthAdminMutationContext {
    AuthAdminMutationContext {
        actor_id: auth.user_id,
        tenant_id: auth.tenant_id,
        request_id: None,
        locale: None,
    }
}

fn auth_lifecycle_context(
    ctx: &Context<'_>,
    auth: Option<&AuthContext>,
) -> Result<AuthLifecycleContext> {
    let tenant = ctx.data::<TenantContext>()?;
    let locale = ctx.data::<Locale>().copied().unwrap_or_default();
    let user_auth = auth.filter(|auth| auth.grant_type != "client_credentials");
    Ok(AuthLifecycleContext {
        tenant_id: tenant.id,
        user_id: user_auth.map(|auth| auth.user_id),
        session_id: user_auth.map(|auth| auth.session_id),
        permissions: user_auth
            .map(|auth| auth.permissions.clone())
            .unwrap_or_default(),
        locale,
    })
}

fn map_error(error: AuthAdminMutationError) -> FieldError {
    match error {
        AuthAdminMutationError::Unauthorized => <FieldError as GraphQLError>::unauthenticated(),
        AuthAdminMutationError::Forbidden(message) => {
            <FieldError as GraphQLError>::permission_denied(&message)
        }
        AuthAdminMutationError::Validation(message) | AuthAdminMutationError::Conflict(message) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        AuthAdminMutationError::CustomFieldsValidation(fields) => {
            <FieldError as GraphQLError>::bad_user_input(&fields.to_string())
        }
        AuthAdminMutationError::NotFound(message) => {
            <FieldError as GraphQLError>::not_found(&message)
        }
        AuthAdminMutationError::Internal(message) => {
            <FieldError as GraphQLError>::internal_error(&message)
        }
    }
}

fn unauthenticated_auth_error(message: &str) -> FieldError {
    FieldError::new(message).extend_with(|_, e| {
        e.set(
            "code",
            rustok_api::graphql::ErrorCode::Unauthenticated.as_str(),
        );
    })
}

fn map_auth_lifecycle_error(ctx: &Context<'_>, error: AuthLifecycleMutationError) -> FieldError {
    let locale = ctx.data::<Locale>().copied().unwrap_or_default();
    let t = |key: &str| translate(locale, key);
    match error {
        AuthLifecycleMutationError::EmailAlreadyExists => {
            FieldError::new(t("auth.email_already_exists"))
        }
        AuthLifecycleMutationError::InvalidCredentials => {
            unauthenticated_auth_error(&t("auth.invalid_credentials"))
        }
        AuthLifecycleMutationError::UserInactive => {
            unauthenticated_auth_error(&t("auth.user_inactive"))
        }
        AuthLifecycleMutationError::InvalidRefreshToken => {
            unauthenticated_auth_error(&t("auth.invalid_refresh_token"))
        }
        AuthLifecycleMutationError::SessionExpired => {
            unauthenticated_auth_error(&t("auth.session_expired"))
        }
        AuthLifecycleMutationError::UserNotFound | AuthLifecycleMutationError::Unauthorized => {
            unauthenticated_auth_error(&t("auth.user_not_found"))
        }
        AuthLifecycleMutationError::InvalidResetToken => {
            unauthenticated_auth_error(&t("auth.invalid_reset_token"))
        }
        AuthLifecycleMutationError::InvalidInviteToken => {
            FieldError::new(t("auth.invalid_invite_token"))
        }
        AuthLifecycleMutationError::Validation(message) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        AuthLifecycleMutationError::Internal(message) => {
            <FieldError as GraphQLError>::internal_error(&message)
        }
    }
}
