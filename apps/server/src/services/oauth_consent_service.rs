use rustok_api::context::scope_matches;
use sea_orm::TransactionTrait;
use uuid::Uuid;

use crate::error::{Error, Result};

use super::oauth_app::{
    GrantConsentAttempt, OAuthAppService, lock_oauth_app_for_mutation,
    lock_oauth_user_for_mutation,
};

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
        if scopes.is_empty() {
            return Err(Error::BadRequest(
                "OAuth consent must include at least one scope".to_string(),
            ));
        }

        // Token issuance takes these same locks in app-then-user order before
        // checking consent. Sharing that order makes grant/revoke linearizable
        // with token exchange and with concurrent app/user deactivation.
        let tx = db.begin().await?;
        let app = lock_oauth_app_for_mutation(&tx, app_id, tenant_id)
            .await?
            .filter(|app| app.is_active())
            .ok_or(Error::NotFound)?;
        let user = lock_oauth_user_for_mutation(&tx, user_id, tenant_id)
            .await?
            .filter(|user| user.is_active())
            .ok_or_else(|| {
                Error::Unauthorized("OAuth consent subject is missing or inactive".to_string())
            })?;

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

        let scopes_json = serde_json::to_value(&normalized)
            .map_err(|_| Error::InternalServerError)?;
        OAuthAppService::grant_consent_in_transaction(
            &tx,
            app.id,
            user.id,
            tenant_id,
            &scopes_json,
        )
        .await
        .map_err(|attempt| match attempt {
            GrantConsentAttempt::UniqueRace => Error::Conflict(
                "OAuth consent changed concurrently; retry the operation".to_string(),
            ),
            GrantConsentAttempt::Fatal(error) => error,
        })?;
        tx.commit().await?;
        Ok(())
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
