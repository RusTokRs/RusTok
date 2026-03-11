//! GraphQL queries for OAuth App management

use async_graphql::{Context, FieldError, Object, Result};
use uuid::Uuid;

use crate::context::AuthContext;
use crate::graphql::errors::GraphQLError;
use crate::services::oauth_app::OAuthAppService;
use sea_orm::DatabaseConnection;

use super::{
    ensure_oauth_admin,
    types::{AppType, OAuthAppGql},
};

#[derive(Default)]
pub struct OAuthQuery;

#[Object]
impl OAuthQuery {
    /// List OAuth apps for the current tenant (admin only)
    async fn oauth_apps(
        &self,
        ctx: &Context<'_>,
        app_type: Option<AppType>,
    ) -> Result<Vec<OAuthAppGql>> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let db = ctx.data::<DatabaseConnection>()?;

        // Require admin permissions
        ensure_oauth_admin(auth, db).await?;

        let apps = OAuthAppService::list_by_tenant(db, auth.tenant_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to list apps: {e}")))?;

        let apps = if let Some(app_type) = app_type {
            apps.into_iter()
                .filter(|a| a.app_type == app_type.as_str())
                .map(OAuthAppGql)
                .collect()
        } else {
            apps.into_iter().map(OAuthAppGql).collect()
        };

        Ok(apps)
    }

    /// Get a specific OAuth app by ID (admin only)
    async fn oauth_app(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<OAuthAppGql>> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let db = ctx.data::<DatabaseConnection>()?;

        ensure_oauth_admin(auth, db).await?;

        let app = crate::models::oauth_apps::Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {e}")))?;

        // Verify tenant ownership
        let app = app.filter(|a| a.tenant_id == auth.tenant_id);

        Ok(app.map(OAuthAppGql))
    }

    /// List apps the current user has authorized
    async fn my_authorized_apps(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<super::types::AuthorizedAppGql>> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let db = ctx.data::<DatabaseConnection>()?;

        // Require authenticated user
        if auth.user_id.is_none() {
            return Err("Authentication required".into());
        }
        let user_id = auth.user_id.unwrap();

        // Find active consents joined with apps
        use crate::models::oauth_apps;
        use crate::models::oauth_consents;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        let active_consents: Vec<(oauth_consents::Model, Option<oauth_apps::Model>)> =
            oauth_consents::Entity::find()
                .filter(oauth_consents::Column::UserId.eq(user_id))
                .filter(oauth_consents::Column::TenantId.eq(auth.tenant_id))
                .filter(oauth_consents::Column::RevokedAt.is_null())
                .find_also_related(oauth_apps::Entity)
                .all(db)
                .await
                .map_err(|e| {
                    async_graphql::Error::new(format!("Failed to fetch authorizations: {e}"))
                })?;

        let mut results = Vec::new();
        for (consent, app) in active_consents {
            if let Some(app_model) = app {
                // Ensure app is still active
                if app_model.is_active {
                    results.push(super::types::AuthorizedAppGql {
                        app: app_model,
                        scopes: consent.scopes_list(),
                        granted_at: consent.granted_at.into(),
                    });
                }
            }
        }

        Ok(results)
    }
}
