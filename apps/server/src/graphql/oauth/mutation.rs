//! GraphQL mutations for OAuth App management

use async_graphql::{Context, FieldError, Object, Result};
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

use crate::context::AuthContext;
use crate::graphql::errors::GraphQLError;
use crate::services::oauth_app::{self, OAuthAppService};

use super::ensure_oauth_admin;

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

        ensure_oauth_admin(auth, db).await?;

        let service_input = oauth_app::CreateOAuthAppInput {
            name: input.name,
            slug: input.slug,
            description: input.description,
            app_type: input.app_type.as_str().to_string(),
            icon_url: input.icon_url,
            redirect_uris: input.redirect_uris.unwrap_or_default(),
            scopes: input.scopes,
            grant_types: input.grant_types,
            granted_permissions: input.granted_permissions,
        };

        let result = OAuthAppService::create_app(db, auth.tenant_id, service_input)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to create app: {e}")))?;

        Ok(CreateOAuthAppResultGql {
            app: OAuthAppGql(result.app),
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

        ensure_oauth_admin(auth, db).await?;

        let app = crate::models::oauth_apps::Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {e}")))?
            .ok_or_else(|| async_graphql::Error::new("App not found"))?;

        if app.tenant_id != auth.tenant_id {
            return Err("App not found".into());
        }

        let updated = OAuthAppService::update_app(
            db,
            auth.tenant_id,
            id,
            oauth_app::UpdateOAuthAppInput {
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
        .map_err(|e| async_graphql::Error::new(format!("Failed to update app: {e}")))?;

        Ok(OAuthAppGql(updated))
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

        ensure_oauth_admin(auth, db).await?;

        // Verify tenant ownership
        let app = crate::models::oauth_apps::Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {e}")))?
            .ok_or_else(|| async_graphql::Error::new("App not found"))?;

        if app.tenant_id != auth.tenant_id {
            return Err("App not found".into());
        }

        let result = OAuthAppService::rotate_secret(db, id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to rotate secret: {e}")))?;

        Ok(RotateSecretResultGql {
            app: OAuthAppGql(result.app),
            client_secret: result.client_secret,
        })
    }

    /// Revoke an OAuth app — deactivates the app and all its tokens (admin only).
    async fn revoke_oauth_app(&self, ctx: &Context<'_>, id: Uuid) -> Result<OAuthAppGql> {
        let auth = require_auth_context(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;

        ensure_oauth_admin(auth, db).await?;

        // Verify tenant ownership
        let app = crate::models::oauth_apps::Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {e}")))?
            .ok_or_else(|| async_graphql::Error::new("App not found"))?;

        if app.tenant_id != auth.tenant_id {
            return Err("App not found".into());
        }

        let revoked = OAuthAppService::revoke_app(db, id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to revoke app: {e}")))?;

        Ok(OAuthAppGql(revoked))
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
