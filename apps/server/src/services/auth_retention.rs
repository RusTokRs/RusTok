//! Bounded retention for authentication artifacts.
//!
//! Sessions and invite bookkeeping only need to outlive their security
//! purpose: a session row and its cascading refresh-token history are
//! worthless once the whole lineage has expired, and an
//! invite digest only has to survive until the signed token expires.
//! Rows are purged 30 days past expiry, so storage stays bounded
//! without weakening exactly-once acceptance or theft detection. Pending
//! mailbox-change challenges are deleted as soon as expired: they have no
//! replay-evidence role after expiry and retain both email addresses. The
//! host runs this daily via the auth retention worker; the
//! `auth sessions-cleanup` CLI remains available for one-off runs.

use chrono::{Duration, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter, Statement, TransactionTrait};

use crate::error::Result;
use crate::models::{
    auth_email_change_requests, auth_invite_consumptions, auth_invites, oauth_authorization_codes,
    sessions,
};

/// How long expired auth artifacts are kept for audit before deletion.
pub const AUTH_ARTIFACT_RETENTION_DAYS: i64 = 30;

/// OAuth token rows are retained longer: a rotated lineage can live
/// up to the 90-day family cap, and the revoked rows are the reuse
/// evidence (plus the root row anchors the family deadline). They
/// are purged as a complete family 120 days past its latest member's
/// expiry — family cap plus the usual 30-day audit window.
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
    pub email_change_requests: u64,
    pub oauth_tokens: u64,
    pub oauth_authorization_codes: u64,
}

/// Delete expired authentication artifacts in one transaction. A failed purge
/// must not leave some artifact families deleted while later cleanup fails.
/// Returns per-table deletion counts only after the transaction commits.
pub async fn purge_expired_auth_artifacts(db: &DatabaseConnection) -> Result<AuthRetentionPurge> {
    let now = Utc::now();
    let cutoff = now - Duration::days(AUTH_ARTIFACT_RETENTION_DAYS);
    let oauth_cutoff = now - Duration::days(OAUTH_TOKEN_RETENTION_DAYS);
    let oauth_code_cutoff = now - Duration::days(OAUTH_AUTHORIZATION_CODE_RETENTION_DAYS);
    let tx = db.begin().await?;

    let result: Result<AuthRetentionPurge> = async {
        let sessions = sessions::Entity::delete_many()
            .filter(sessions::Column::ExpiresAt.lt(cutoff))
            .exec(&tx)
            .await?;

        let invite_consumptions = auth_invite_consumptions::Entity::delete_many()
            .filter(auth_invite_consumptions::Column::ExpiresAt.lt(cutoff))
            .exec(&tx)
            .await?;

        let invites = auth_invites::Entity::delete_many()
            .filter(auth_invites::Column::ExpiresAt.lt(cutoff))
            .exec(&tx)
            .await?;

        // Match the on-demand cleanup at email-change issuance. Expired
        // challenges no longer reserve an address or authorize confirmation.
        let email_change_requests = auth_email_change_requests::Entity::delete_many()
            .filter(auth_email_change_requests::Column::ExpiresAt.lte(now))
            .exec(&tx)
            .await?;

        // Keep every generation and its root until the entire family's
        // retention window closes. Descendants go first for SQLite's
        // immediate root guard; PostgreSQL also has a deferred root FK.
        let backend = tx.get_database_backend();
        let parameter = match backend {
            DbBackend::Postgres => "$1",
            DbBackend::Sqlite => "?",
            _ => return Err(sea_orm::DbErr::Custom("Unsupported OAuth retention database".into()).into()),
        };
        let descendants = tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("DELETE FROM oauth_tokens WHERE id <> family_id AND family_id IN (SELECT family_id FROM oauth_tokens GROUP BY family_id HAVING MAX(expires_at) < {parameter})"),
            [oauth_cutoff.into()],
        )).await?;
        let roots = tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("DELETE FROM oauth_tokens WHERE id = family_id AND family_id IN (SELECT family_id FROM oauth_tokens GROUP BY family_id HAVING MAX(expires_at) < {parameter}) AND NOT EXISTS (SELECT 1 FROM oauth_tokens member WHERE member.family_id = oauth_tokens.id AND member.id <> oauth_tokens.id)"),
            [oauth_cutoff.into()],
        )).await?;

        // A code is replay evidence as long as any token from its exchange
        // remains. Do not purge it solely because its own TTL is older.
        let oauth_authorization_codes = tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("DELETE FROM oauth_authorization_codes WHERE expires_at < {parameter} AND NOT EXISTS (SELECT 1 FROM oauth_tokens token WHERE token.authorization_code_id = oauth_authorization_codes.id)"),
            [oauth_code_cutoff.into()],
        )).await?;

        Ok(AuthRetentionPurge {
            sessions: sessions.rows_affected,
            invite_consumptions: invite_consumptions.rows_affected,
            invites: invites.rows_affected,
            email_change_requests: email_change_requests.rows_affected,
            oauth_tokens: descendants.rows_affected() + roots.rows_affected(),
            oauth_authorization_codes: oauth_authorization_codes.rows_affected(),
        })
    }
    .await;

    match result {
        Ok(report) => {
            tx.commit().await?;
            Ok(report)
        }
        Err(error) => {
            // Explicit rollback keeps the failure visible and prevents later
            // runs from observing a partially committed retention sweep.
            tx.rollback().await?;
            Err(error)
        }
    }
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
    async fn purge_rolls_back_earlier_deletes_when_a_later_table_fails() {
        use sea_orm::ConnectionTrait;

        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new(
            "Rollback tenant",
            &format!("retention-rollback-{}", Uuid::new_v4()),
        )
        .insert(&db)
        .await
        .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "rollback@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        let session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            format!("expired-{}", Uuid::new_v4()),
            Utc::now() - Duration::days(AUTH_ARTIFACT_RETENTION_DAYS + 1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("expired session");

        db.execute_unprepared("DROP TABLE auth_invite_consumptions")
            .await
            .expect("break second retention delete");
        assert!(purge_expired_auth_artifacts(&db).await.is_err());
        assert!(
            sessions::Entity::find_by_id(session.id)
                .one(&db)
                .await
                .expect("session lookup after rollback")
                .is_some(),
            "the first delete must roll back if a later delete fails"
        );
    }

    #[tokio::test]
    async fn purge_removes_expired_email_change_requests_across_tenants_only() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let now = Utc::now();
        let mut ids = Vec::new();
        for (index, expired) in [true, true, false].into_iter().enumerate() {
            let tenant = tenants::ActiveModel::new(
                "Email retention tenant",
                &format!("email-retention-{index}-{}", Uuid::new_v4()),
            )
            .insert(&db)
            .await
            .expect("tenant");
            let user = users::ActiveModel::new(tenant.id, "old@example.com", "hash")
                .insert(&db)
                .await
                .expect("user");
            let expiry = if expired {
                now - Duration::minutes(1)
            } else {
                now + Duration::hours(1)
            };
            let pending = auth_email_change_requests::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant.id),
                user_id: Set(user.id),
                current_email: Set(user.email),
                new_email: Set("new@example.com".to_string()),
                password_changed_at: Set(user.password_changed_at),
                token_hash: Set(format!("{:064x}", index + 1)),
                expires_at: Set(expiry.into()),
                created_at: Set((now - Duration::minutes(30)).into()),
            }
            .insert(&db)
            .await
            .expect("pending email change");
            ids.push(pending.id);
        }

        let purge = purge_expired_auth_artifacts(&db).await.expect("purge");
        assert_eq!(purge.email_change_requests, 2);
        for (index, id) in ids.into_iter().enumerate() {
            let remaining = auth_email_change_requests::Entity::find_by_id(id)
                .one(&db)
                .await
                .expect("lookup after purge");
            assert_eq!(remaining.is_some(), index == 2);
        }
        let second = purge_expired_auth_artifacts(&db).await.expect("repeat purge");
        assert_eq!(second.email_change_requests, 0);
    }

    #[tokio::test]
    async fn purge_removes_only_oauth_codes_past_the_lineage_retention_window() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant =
            tenants::ActiveModel::new("Retention tenant", &format!("retention-{}", Uuid::new_v4()))
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
        let old_code = old_code.insert(&db).await.expect("old code");
        let root_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();
        let rotated_at = old_cutoff - Duration::days(1);
        for (id, expires, revoked_at) in [
            (root_id, old_cutoff - Duration::minutes(1), Some(rotated_at)),
            (child_id, old_cutoff + Duration::days(1), None),
        ] {
            crate::models::oauth_tokens::ActiveModel {
                id: Set(id),
                app_id: Set(created.app.id),
                tenant_id: Set(tenant.id),
                user_id: Set(Some(user.id)),
                token_hash: Set(format!("retention-{id}")),
                family_id: Set(root_id),
                authorization_code_id: Set(Some(old_code.id)),
                grant_type: Set("authorization_code".into()),
                token_kind: Set(crate::models::oauth_tokens::TOKEN_KIND_REFRESH.into()),
                consent_granted_at: Set(None),
                scopes: Set(serde_json::json!(["openid"])),
                expires_at: Set(expires.into()),
                revoked_at: Set(revoked_at.map(Into::into)),
                last_used_at: Set(revoked_at.map(Into::into)),
                created_at: Set((old_cutoff - Duration::days(2)).into()),
                updated_at: Set(rotated_at.into()),
            }.insert(&db).await.expect("retention family member");
        }

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
        assert_eq!(purge.oauth_authorization_codes, 0);
        assert_eq!(purge.oauth_tokens, 0);
        for id in [root_id, child_id] {
            assert!(crate::models::oauth_tokens::Entity::find_by_id(id)
                .one(&db).await.unwrap().is_some());
        }
        let mut child: crate::models::oauth_tokens::ActiveModel =
            crate::models::oauth_tokens::Entity::find_by_id(child_id)
                .one(&db).await.unwrap().unwrap().into();
        child.expires_at = Set((old_cutoff - Duration::minutes(1)).into());
        child.update(&db).await.expect("age remaining family member");
        let purge = purge_expired_auth_artifacts(&db).await.expect("purge whole family");
        assert_eq!(purge.oauth_tokens, 2);
        assert_eq!(purge.oauth_authorization_codes, 1);

        let remaining = oauth_authorization_codes::Entity::find()
            .filter(oauth_authorization_codes::Column::TenantId.eq(tenant.id))
            .all(&db)
            .await
            .expect("remaining codes");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, recent.id);

        let second = purge_expired_auth_artifacts(&db)
            .await
            .expect("second purge");
        assert_eq!(second.oauth_authorization_codes, 0);
    }
}
