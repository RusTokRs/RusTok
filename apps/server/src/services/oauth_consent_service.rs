use rustok_api::context::scope_matches;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::models::{oauth_apps::Entity as OAuthApps, users::{self, Entity as Users}};

use super::oauth_app::OAuthAppService;

impl OAuthAppService {
    /// Grant consent only after validating every authority dimension at the
    /// service boundary. Callers cannot persist scopes for another tenant,
    /// inactive user, inactive client, or scopes outside the app policy.
    pub async fn grant_consent_strict(
        db: &sea_orm::DatabaseConnection,
        app_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
        scopes: Vec<String>,
    ) -> Result<()> {
        let app = OAuthApps::find_by_id(app_id)
            .filter(crate::models::oauth_apps::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .filter(|app| app.is_active())
            .ok_or(Error::NotFound)?;

        let user = Users::find_by_id(user_id)
            .filter(users::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .filter(|user| user.is_active())
            .ok_or_else(|| Error::Unauthorized("OAuth consent subject is missing or inactive".to_string()))?;

        if user.id != user_id {
            return Err(Error::Unauthorized(
                "OAuth consent subject mismatch".to_string(),
            ));
        }

        let allowed_scopes = app.scopes_list();
        let mut normalized = Vec::with_capacity(scopes.len());
        for scope in scopes {
            let scope = scope.trim().to_string();
            if scope.is_empty() {
                return Err(Error::BadRequest(
                    "OAuth consent contains an empty scope".to_string(),
                ));
            }
            if !scope_matches(&allowed_scopes, &scope) {
                return Err(Error::BadRequest(format!(
                    "OAuth consent scope `{scope}` is not allowed by the application"
                )));
            }
            if !normalized.contains(&scope) {
                normalized.push(scope);
            }
        }

        Self::grant_consent(db, app.id, user.id, tenant_id, normalized).await
    }
}

#[cfg(test)]
mod tests {
    use rustok_api::context::scope_matches;

    #[test]
    fn nested_app_scope_contains_only_its_namespace() {
        let allowed = vec!["ai:providers:*".to_string()];
        assert!(scope_matches(&allowed, "ai:providers:read"));
        assert!(!scope_matches(&allowed, "ai:tasks:text:read"));
    }
}