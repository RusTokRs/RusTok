//! Bounded retention for authentication artifacts.
//!
//! Sessions and invite bookkeeping only need to outlive their security
//! purpose: a session row (with the previous-token hash used for reuse
//! detection) is worthless once the whole lineage has expired, and an
//! invite digest only has to survive until the signed token expires.
//! Rows are purged 30 days past expiry, so storage stays bounded
//! without weakening exactly-once acceptance or theft detection. The
//! host runs this daily via the auth retention worker; the
//! `auth sessions-cleanup` CLI remains available for one-off runs.

use chrono::{Duration, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::error::Result;
use crate::models::{
    auth_invite_consumptions, auth_invites, oauth_authorization_codes, oauth_tokens, sessions,
};

/// How long expired auth artifacts are kept for audit before deletion.
pub const AUTH_ARTIFACT_RETENTION_DAYS: i64 = 30;

/// OAuth token rows are retained longer: a rotated lineage can live
/// up to the 90-day family cap, and the revoked rows are the reuse
/// evidence (plus the root row anchors the family deadline). They
/// are purged 120 days past expiry — family cap plus the usual
/// 30-day audit window.
pub const OAUTH_TOKEN_RETENTION_DAYS: i64 = 120;

/// Authorization codes are short-lived, but a used code is the replay anchor
/// for the delegated refresh lineage it issued. Keep that evidence for the
/// same outer window as the token family itself; purging it earlier would let
/// a late code replay fail closed without revoking the still-live lineage.
pub const OAUTH_AUTHORIZATION_CODE_RETENTION_DAYS: i64 = OAUTH_TOKEN_RETENTION_DAYS;

#[derive(Debug, Default, Clone, Copy)]
pub struct AuthRetentionPurge {
    pub sessions: u64,
    pub invite_consumptions: u64,
    pub invites: u64,
    pub oauth_tokens: u64,
    pub oauth_authorization_codes: u64,
}

/// Delete sessions and invite bookkeeping whose expiry is older than
/// the retention window. Returns per-table deletion counts.
pub async fn purge_expired_auth_artifacts(db: &DatabaseConnection) -> Result<AuthRetentionPurge> {
    let cutoff = Utc::now() - Duration::days(AUTH_ARTIFACT_RETENTION_DAYS);

    let sessions = sessions::Entity::delete_many()
        .filter(sessions::Column::ExpiresAt.lt(cutoff))
        .exec(db)
        .await?;

    let invite_consumptions = auth_invite_consumptions::Entity::delete_many()
        .filter(auth_invite_consumptions::Column::ExpiresAt.lt(cutoff))
        .exec(db)
        .await?;

    let invites = auth_invites::Entity::delete_many()
        .filter(auth_invites::Column::ExpiresAt.lt(cutoff))
        .exec(db)
        .await?;

    let oauth_cutoff = Utc::now() - Duration::days(OAUTH_TOKEN_RETENTION_DAYS);
    let oauth_tokens = oauth_tokens::Entity::delete_many()
        .filter(oauth_tokens::Column::ExpiresAt.lt(oauth_cutoff))
        .exec(db)
        .await?;

    let oauth_code_cutoff =
        Utc::now() - Duration::days(OAUTH_AUTHORIZATION_CODE_RETENTION_DAYS);
    let oauth_authorization_codes = oauth_authorization_codes::Entity::delete_many()
        .filter(oauth_authorization_codes::Column::ExpiresAt.lt(oauth_code_cutoff))
        .exec(db)
        .await?;

    Ok(AuthRetentionPurge {
        sessions: sessions.rows_affected,
        invite_consumptions: invite_consumptions.rows_affected,
        invites: invites.rows_affected,
        oauth_tokens: oauth_tokens.rows_affected,
        oauth_authorization_codes: oauth_authorization_codes.rows_affected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{oauth_apps, oauth_authorization_codes, tenants, users};
    use crate::services::oauth_app::{CreateOAuthAppInput, OAuthAppService};
    use chrono::Duration;
    use rustok_migrations::SqliteTestMigrator as Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
    use uuid::Uuid;

    #[tokio::test]
    async fn purge_removes_only_oauth_codes_past_the_lineage_retention_window() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Retention tenant", &format!("retention-{}", Uuid::new_v4()))
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "retention@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        let created = oauth_apps::scope_runtime_copy_locale(
            "en".to_string(),
            OAuthAppService::create_app(
                &db,
                tenant.id,
                CreateOAuthAppInput {
                    name: "Retention app".to_string(),
                    slug: format!("retention-app-{}", Uuid::new_v4()),
                    description: None,
                    app_type: "third_party".to_string(),
                    icon_url: None,
                    redirect_uris: vec!["https://client.example.com/callback".to_string()],
                    scopes: vec!["openid".to_string()],
                    grant_types: vec![
                        "authorization_code".to_string(),
                        "refresh_token".to_string(),
                    ],
                    granted_permissions: vec![],
                },
            ),
        )
        .await
        .expect("oauth app");

        let now = Utc::now();
        let old_cutoff = now - Duration::days(OAUTH_AUTHORIZATION_CODE_RETENTION_DAYS);
        let old_code = oauth_authorization_codes::ActiveModel {
            id: Set(Uuid::new_v4()),
            app_id: Set(created.app.id),
            user_id: Set(user.id),
            tenant_id: Set(tenant.id),
            code_hash: Set(format!("old-{}", Uuid::new_v4())),
            redirect_uri: Set("https://client.example.com/callback".to_string()),
            scopes: Set(serde_json::json!(["openid"])),
            code_challenge: Set("challenge".to_string()),
            code_challenge_method: Set("S256".to_string()),
            expires_at: Set((old_cutoff - Duration::minutes(1)).into()),
            used_at: Set(Some(now.into())),
            created_at: Set((old_cutoff - Duration::minutes(10)).into()),
        };
        old_code.insert(&db).await.expect("old code");

        let recent = oauth_authorization_codes::ActiveModel {
            id: Set(Uuid::new_v4()),
            app_id: Set(created.app.id),
            user_id: Set(user.id),
            tenant_id: Set(tenant.id),
            code_hash: Set(format!("recent-{}", Uuid::new_v4())),
            redirect_uri: Set("https://client.example.com/callback".to_string()),
            scopes: Set(serde_json::json!(["openid"])),
            code_challenge: Set("challenge".to_string()),
            code_challenge_method: Set("S256".to_string()),
            expires_at: Set((now - Duration::minutes(5)).into()),
            used_at: Set(Some(now.into())),
            created_at: Set((now - Duration::minutes(10)).into()),
        }
        .insert(&db)
        .await
        .expect("recent code");

        let purge = purge_expired_auth_artifacts(&db).await.expect("purge");
        assert_eq!(purge.oauth_authorization_codes, 1);

        let remaining = oauth_authorization_codes::Entity::find()
            .filter(oauth_authorization_codes::Column::TenantId.eq(tenant.id))
            .all(&db)
            .await
            .expect("remaining codes");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, recent.id);

        let second = purge_expired_auth_artifacts(&db).await.expect("second purge");
        assert_eq!(second.oauth_authorization_codes, 0);
    }
}
