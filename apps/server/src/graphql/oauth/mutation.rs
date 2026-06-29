//! GraphQL mutations for OAuth App management

use async_graphql::{Context, FieldError, Object, Result};
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, CreateOAuthAppCommand,
    OAuthAdminMutationRuntime, UpdateOAuthAppCommand,
};
use rustok_core::ModuleRuntimeExtensions;
use sea_orm::{DatabaseConnection, EntityTrait};
use std::sync::Arc;
use uuid::Uuid;

use crate::context::AuthContext;
use crate::graphql::errors::GraphQLError;
use crate::services::oauth_app::OAuthAppService;

use super::types::{
    CreateOAuthAppInput, CreateOAuthAppResultGql, OAuthAppGql, RotateSecretResultGql,
    UpdateOAuthAppInput,
};

#[derive(Default)]
pub struct OAuthMutation;

fn require_auth_context<'a>(ctx: &'a Context<'a>) -> Result<&'a AuthContext> {
    ctx.data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())
}

fn mutation_runtime(ctx: &Context<'_>) -> Result<OAuthAdminMutationRuntime> {
    ctx.data::<Arc<ModuleRuntimeExtensions>>()?
        .get::<OAuthAdminMutationRuntime>()
        .cloned()
        .ok_or_else(|| {
            <FieldError as GraphQLError>::internal_error(
                "OAuthAdminMutationRuntime is not registered; initialize shared host runtime providers",
            )
            .into()
        })
}

fn mutation_context(auth: &AuthContext) -> AuthAdminMutationContext {
    AuthAdminMutationContext {
        actor_id: auth.user_id,
        tenant_id: auth.tenant_id,
        request_id: None,
    }
}

fn map_mutation_error(error: AuthAdminMutationError) -> FieldError {
    match error {
        AuthAdminMutationError::Unauthorized => <FieldError as GraphQLError>::unauthenticated(),
        AuthAdminMutationError::Forbidden(message) => {
            <FieldError as GraphQLError>::permission_denied(&message)
        }
        AuthAdminMutationError::Validation(message) | AuthAdminMutationError::Conflict(message) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        AuthAdminMutationError::NotFound(message) => {
            <FieldError as GraphQLError>::not_found(&message)
        }
        AuthAdminMutationError::Internal(message) => {
            <FieldError as GraphQLError>::internal_error(&message)
        }
    }
}

async fn load_oauth_app(db: &DatabaseConnection, id: Uuid) -> Result<OAuthAppGql> {
    crate::models::oauth_apps::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|error| <FieldError as GraphQLError>::internal_error(&error.to_string()))?
        .map(OAuthAppGql)
        .ok_or_else(|| <FieldError as GraphQLError>::not_found("OAuth app not found").into())
}

#[Object]
impl OAuthMutation {
    /// Create a new OAuth app (admin only).
    /// Returns the client_secret ONCE — it cannot be retrieved later.
    async fn create_oauth_app(
        &self,
        ctx: &Context<'_>,
        input: CreateOAuthAppInput,
    ) -> Result<CreateOAuthAppResultGql> {
        let auth = require_auth_context(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;

        let runtime = mutation_runtime(ctx)?;
        let result = runtime
            .port()
            .create_oauth_app(
                &mutation_context(auth),
                CreateOAuthAppCommand {
                    name: input.name,
                    slug: input.slug,
                    description: input.description,
                    app_type: input.app_type.as_str().to_string(),
                    icon_url: input.icon_url,
                    redirect_uris: input.redirect_uris.unwrap_or_default(),
                    scopes: input.scopes,
                    grant_types: input.grant_types,
                    granted_permissions: input.granted_permissions,
                },
            )
            .await
            .map_err(map_mutation_error)?;

        Ok(CreateOAuthAppResultGql {
            app: load_oauth_app(db, result.app.id).await?,
            client_secret: result.client_secret,
        })
    }

    /// Update a manual OAuth app (admin only).
    async fn update_oauth_app(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateOAuthAppInput,
    ) -> Result<OAuthAppGql> {
        let auth = require_auth_context(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;

        let runtime = mutation_runtime(ctx)?;
        let updated = runtime
            .port()
            .update_oauth_app(
                &mutation_context(auth),
                UpdateOAuthAppCommand {
                    id,
                    name: input.name,
                    description: input.description,
                    icon_url: input.icon_url,
                    redirect_uris: input.redirect_uris,
                    scopes: input.scopes,
                    grant_types: input.grant_types,
                    granted_permissions: input.granted_permissions,
                },
            )
            .await
            .map_err(map_mutation_error)?;

        load_oauth_app(db, updated.id).await
    }

    /// Rotate client_secret for an OAuth app (admin only).
    /// Returns the new secret ONCE.
    async fn rotate_oauth_app_secret(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<RotateSecretResultGql> {
        let auth = require_auth_context(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;

        let runtime = mutation_runtime(ctx)?;
        let result = runtime
            .port()
            .rotate_oauth_app_secret(&mutation_context(auth), id)
            .await
            .map_err(map_mutation_error)?;

        Ok(RotateSecretResultGql {
            app: load_oauth_app(db, result.app.id).await?,
            client_secret: result.client_secret,
        })
    }

    /// Revoke an OAuth app — deactivates the app and all its tokens (admin only).
    async fn revoke_oauth_app(&self, ctx: &Context<'_>, id: Uuid) -> Result<OAuthAppGql> {
        let auth = require_auth_context(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;

        let runtime = mutation_runtime(ctx)?;
        let revoked = runtime
            .port()
            .revoke_oauth_app(&mutation_context(auth), id)
            .await
            .map_err(map_mutation_error)?;

        load_oauth_app(db, revoked.id).await
    }

    /// Grant consent to an application
    async fn grant_app_consent(
        &self,
        ctx: &Context<'_>,
        app_id: Uuid,
        scopes: Vec<String>,
    ) -> Result<bool> {
        let auth = require_auth_context(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let user_id = auth.user_id;

        // Ensure app belongs to same tenant and is active
        let app = crate::models::oauth_apps::Entity::find_by_id(app_id)
            .one(db)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {e}")))?
            .ok_or_else(|| async_graphql::Error::new("App not found"))?;

        if app.tenant_id != auth.tenant_id || !app.is_active {
            return Err("App not found or inactive".into());
        }

        OAuthAppService::grant_consent(db, app_id, user_id, auth.tenant_id, scopes)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to grant consent: {e}")))?;

        Ok(true)
    }

    /// Revoke consent to an application (also revokes tokens)
    async fn revoke_app_consent(&self, ctx: &Context<'_>, app_id: Uuid) -> Result<bool> {
        let auth = require_auth_context(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let user_id = auth.user_id;

        OAuthAppService::revoke_user_consent(db, app_id, user_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to revoke consent: {e}")))?;

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use async_graphql::{EmptySubscription, Object, Request, Schema, Value};
    use sea_orm::Database;

    use super::OAuthMutation;
    use crate::context::AuthContext;

    struct TestQueryRoot;

    #[Object]
    impl TestQueryRoot {
        async fn health(&self) -> &str {
            "ok"
        }
    }

    fn auth_context() -> AuthContext {
        AuthContext {
            user_id: uuid::Uuid::new_v4(),
            session_id: uuid::Uuid::new_v4(),
            tenant_id: uuid::Uuid::new_v4(),
            permissions: vec![],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        }
    }

    fn error_code(response: &async_graphql::Response) -> Option<&str> {
        response.errors.first().and_then(|error| {
            error
                .extensions
                .as_ref()
                .and_then(|ext| ext.get("code"))
                .and_then(|value| match value {
                    Value::String(code) => Some(code.as_str()),
                    _ => None,
                })
        })
    }

    #[tokio::test]
    async fn revoke_app_consent_requires_auth_context() {
        let schema = Schema::build(TestQueryRoot, OAuthMutation, EmptySubscription).finish();

        let response = schema
            .execute(Request::new(
                "mutation { revokeAppConsent(appId: \"550e8400-e29b-41d4-a716-446655440000\") }",
            ))
            .await;

        assert_eq!(error_code(&response), Some("UNAUTHENTICATED"));
    }

    #[tokio::test]
    async fn revoke_app_consent_with_auth_context_is_not_unauthenticated() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let schema = Schema::build(TestQueryRoot, OAuthMutation, EmptySubscription)
            .data(db)
            .finish();

        let response = schema
            .execute(
                Request::new(
                    "mutation { revokeAppConsent(appId: \"550e8400-e29b-41d4-a716-446655440000\") }",
                )
                .data(auth_context()),
            )
            .await;

        assert!(!response.errors.is_empty());
        assert_ne!(error_code(&response), Some("UNAUTHENTICATED"));
    }
}
