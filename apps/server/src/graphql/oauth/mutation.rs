//! GraphQL mutations for OAuth App management

use async_graphql::{Context, Object, Result};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::context::AuthContext;
use crate::services::oauth_app::{self, OAuthAppService};

use super::types::{
    CreateOAuthAppInput, CreateOAuthAppResultGql, OAuthAppGql, RotateSecretResultGql,
};

#[derive(Default)]
pub struct OAuthMutation;

#[Object]
impl OAuthMutation {
    /// Create a new OAuth app (admin only).
    /// Returns the client_secret ONCE — it cannot be retrieved later.
    async fn create_oauth_app(
        &self,
        ctx: &Context<'_>,
        input: CreateOAuthAppInput,
    ) -> Result<CreateOAuthAppResultGql> {
        let auth = ctx.data::<AuthContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;

        let sc = auth.security_context();
        if !sc.is_admin() {
            return Err("Admin access required".into());
        }

        let service_input = oauth_app::CreateOAuthAppInput {
            name: input.name,
            slug: input.slug,
            description: input.description,
            app_type: input.app_type.as_str().to_string(),
            redirect_uris: input.redirect_uris.unwrap_or_default(),
            scopes: input.scopes,
            grant_types: input.grant_types,
        };

        let result = OAuthAppService::create_app(db, auth.tenant_id, service_input)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to create app: {e}")))?;

        Ok(CreateOAuthAppResultGql {
            app: OAuthAppGql(result.app),
            client_secret: result.client_secret,
        })
    }

    /// Rotate client_secret for an OAuth app (admin only).
    /// Returns the new secret ONCE.
    async fn rotate_oauth_app_secret(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<RotateSecretResultGql> {
        let auth = ctx.data::<AuthContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;

        let sc = auth.security_context();
        if !sc.is_admin() {
            return Err("Admin access required".into());
        }

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
        let auth = ctx.data::<AuthContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;

        let sc = auth.security_context();
        if !sc.is_admin() {
            return Err("Admin access required".into());
        }

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
        let auth = ctx.data::<AuthContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;

        // Require authenticated user
        if auth.user_id.is_none() {
            return Err("Authentication required".into());
        }
        let user_id = auth.user_id.unwrap();

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
        let auth = ctx.data::<AuthContext>()?;
        let db = ctx.data::<DatabaseConnection>()?;

        // Require authenticated user
        if auth.user_id.is_none() {
            return Err("Authentication required".into());
        }
        let user_id = auth.user_id.unwrap();

        OAuthAppService::revoke_user_consent(db, app_id, user_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to revoke consent: {e}")))?;

        Ok(true)
    }
}
