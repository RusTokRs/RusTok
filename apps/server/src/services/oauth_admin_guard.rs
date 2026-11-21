use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{Permission, has_effective_permission};
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, AuthorizedOAuthAppRecord,
    CreateOAuthAppCommand, OAuthAdminPort, OAuthAppMutationRecord, OAuthAppSecretResult,
    UpdateOAuthAppCommand, generate_refresh_token,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter,
    QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
use uuid::Uuid;

use crate::models::oauth_apps;

use super::oauth_app::OAuthAppService;
use super::rbac_request_scope::permissions_for;
use crate::services::auth_admin_mutation_provider::build_oauth_app_record;

const MAX_OAUTH_GUARD_LIST_LIMIT: u64 = 100;

fn clamp_oauth_guard_list_limit(limit: u64) -> u64 {
    limit.clamp(1, MAX_OAUTH_GUARD_LIST_LIMIT)
}

fn internal_oauth_guard_error<E>(error: E) -> AuthAdminMutationError
where
    E: std::fmt::Display,
{
    tracing::error!(
        error = %error,
        "OAuth administration guard operation failed"
    );
    AuthAdminMutationError::Internal("OAuth administration guard operation failed".to_string())
}

/// A secret prepared without holding a database transaction. Keep it private
/// and never derive Debug: the plaintext must not enter diagnostics.
struct PreparedOAuthSecretRotation {
    client_secret: String,
    secret_hash: String,
    prior_secret_hash: Option<String>,
}

pub struct GuardedOAuthAdminProvider {
    db: DatabaseConnection,
    inner: Arc<dyn OAuthAdminPort>,
}

impl GuardedOAuthAdminProvider {
    pub fn new(db: DatabaseConnection, inner: Arc<dyn OAuthAdminPort>) -> Self {
        Self { db, inner }
    }

    fn request_permissions(
        &self,
        context: &AuthAdminMutationContext,
    ) -> Result<Vec<Permission>, AuthAdminMutationError> {
        permissions_for(&context.tenant_id, &context.actor_id).ok_or_else(|| {
            AuthAdminMutationError::Forbidden(
                "OAuth administration requires a request-bound effective permission snapshot"
                    .to_string(),
            )
        })
    }

    fn require_settings_manage(
        &self,
        context: &AuthAdminMutationContext,
    ) -> Result<Vec<Permission>, AuthAdminMutationError> {
        let authority = self.request_permissions(context)?;
        if has_effective_permission(&authority, &Permission::SETTINGS_MANAGE) {
            Ok(authority)
        } else {
            Err(AuthAdminMutationError::Forbidden(
                "settings:manage required for OAuth application administration".to_string(),
            ))
        }
    }

    fn validate_permission_strings(
        &self,
        authority: &[Permission],
        granted_permissions: &[String],
    ) -> Result<(), AuthAdminMutationError> {
        for raw in granted_permissions {
            let permission = Permission::from_str(raw.trim()).map_err(|error| {
                AuthAdminMutationError::Validation(format!(
                    "OAuth app contains invalid delegated permission `{raw}`: {error}"
                ))
            })?;
            if !has_effective_permission(authority, &permission) {
                return Err(AuthAdminMutationError::Forbidden(format!(
                    "OAuth app permission `{permission}` exceeds the current request authority"
                )));
            }
        }
        Ok(())
    }

    /// Preflight is intentionally lock-free: generating and Argon2-hashing a
    /// new client secret is expensive and must never hold an app mutation lock.
    /// The commit phase repeats all security checks against the locked row.
    async fn prepare_oauth_secret_rotation(
        &self,
        tenant_id: Uuid,
        app_id: Uuid,
        authority: &[Permission],
    ) -> Result<PreparedOAuthSecretRotation, AuthAdminMutationError> {
        let app = oauth_apps::Entity::find_by_id(app_id)
            .filter(oauth_apps::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(internal_oauth_guard_error)?
            .ok_or_else(|| AuthAdminMutationError::NotFound("oauth app".to_string()))?;
        self.validate_permission_strings(authority, &app.granted_permissions_list())?;
        if !app.can_rotate_secret() {
            return Err(AuthAdminMutationError::Validation(
                "This OAuth app does not support client secret rotation".to_string(),
            ));
        }

        let first_token = generate_refresh_token().map_err(internal_oauth_guard_error)?;
        let second_token = generate_refresh_token().map_err(internal_oauth_guard_error)?;
        let client_secret = format!("sk_live_{first_token}{second_token}");
        let secret_hash = crate::auth::hash_password_async(client_secret.clone())
            .await
            .map_err(internal_oauth_guard_error)?;
        Ok(PreparedOAuthSecretRotation {
            client_secret,
            secret_hash,
            prior_secret_hash: app.client_secret_hash,
        })
    }

    async fn commit_prepared_oauth_secret_rotation(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
        authority: &[Permission],
        prepared: PreparedOAuthSecretRotation,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        let tx = self.db.begin().await.map_err(internal_oauth_guard_error)?;
        let app = lock_oauth_app(&tx, context.tenant_id, app_id).await?;
        self.validate_permission_strings(authority, &app.granted_permissions_list())?;
        if !app.can_rotate_secret() {
            return Err(AuthAdminMutationError::Validation(
                "This OAuth app does not support client secret rotation".to_string(),
            ));
        }
        // If a second rotation committed while Argon2 was running, return a
        // retryable conflict instead of returning a secret that has already
        // been superseded by the other successful request.
        if app.client_secret_hash != prepared.prior_secret_hash {
            return Err(AuthAdminMutationError::Validation(
                "OAuth client secret changed during rotation; retry".to_string(),
            ));
        }

        let mut active: oauth_apps::ActiveModel = app.into();
        active.client_secret_hash = Set(Some(prepared.secret_hash));
        active.updated_at = Set(Utc::now().into());
        let updated_app = active
            .update(&tx)
            .await
            .map_err(internal_oauth_guard_error)?;
        let response_record = build_oauth_app_record(&tx, context, updated_app).await?;
        tx.commit().await.map_err(internal_oauth_guard_error)?;

        Ok(OAuthAppSecretResult {
            app: response_record,
            client_secret: prepared.client_secret,
        })
    }

    async fn rotate_secret_transactionally(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
        authority: &[Permission],
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        let prepared = self
            .prepare_oauth_secret_rotation(context.tenant_id, app_id, authority)
            .await?;
        self.commit_prepared_oauth_secret_rotation(context, app_id, authority, prepared)
            .await
    }
}

async fn lock_oauth_app<C>(
    db: &C,
    tenant_id: Uuid,
    app_id: Uuid,
) -> Result<oauth_apps::Model, AuthAdminMutationError>
where
    C: ConnectionTrait,
{
    let query = || {
        oauth_apps::Entity::find_by_id(app_id).filter(oauth_apps::Column::TenantId.eq(tenant_id))
    };
    let app = match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => query()
            .lock_exclusive()
            .one(db)
            .await
            .map_err(internal_oauth_guard_error)?,
        DbBackend::Sqlite => {
            // app_id and tenant_id are known. Reserve SQLite's writer before
            // reading the app so a concurrent token issue/revoke cannot race a
            // deferred read-to-write lock upgrade.
            let result = oauth_apps::Entity::update_many()
                .col_expr(
                    oauth_apps::Column::UpdatedAt,
                    Expr::col(oauth_apps::Column::UpdatedAt),
                )
                .filter(oauth_apps::Column::Id.eq(app_id))
                .filter(oauth_apps::Column::TenantId.eq(tenant_id))
                .exec(db)
                .await
                .map_err(internal_oauth_guard_error)?;
            if result.rows_affected != 1 {
                return Err(AuthAdminMutationError::NotFound("oauth app".to_string()));
            }
            query().one(db).await.map_err(internal_oauth_guard_error)?
        }
        _ => {
            return Err(AuthAdminMutationError::Internal(
                "unsupported SeaORM database backend".to_string(),
            ));
        }
    };

    app.ok_or_else(|| AuthAdminMutationError::NotFound("oauth app".to_string()))
}

fn validate_grant_dependencies(grant_types: &[String]) -> Result<(), AuthAdminMutationError> {
    let has_authorization_code = grant_types
        .iter()
        .any(|grant| grant.trim() == "authorization_code");
    let has_refresh_token = grant_types
        .iter()
        .any(|grant| grant.trim() == "refresh_token");

    if has_refresh_token && !has_authorization_code {
        return Err(AuthAdminMutationError::Validation(
            "refresh_token grant requires authorization_code grant".to_string(),
        ));
    }

    Ok(())
}

#[async_trait]
impl OAuthAdminPort for GuardedOAuthAdminProvider {
    async fn list_oauth_apps(
        &self,
        context: &AuthAdminMutationContext,
        app_type: Option<String>,
        limit: u64,
    ) -> Result<Vec<OAuthAppMutationRecord>, AuthAdminMutationError> {
        self.require_settings_manage(context)?;
        self.inner
            .list_oauth_apps(context, app_type, clamp_oauth_guard_list_limit(limit))
            .await
    }

    async fn get_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<Option<OAuthAppMutationRecord>, AuthAdminMutationError> {
        self.require_settings_manage(context)?;
        self.inner.get_oauth_app(context, app_id).await
    }

    async fn list_authorized_oauth_apps(
        &self,
        context: &AuthAdminMutationContext,
        limit: u64,
    ) -> Result<Vec<AuthorizedOAuthAppRecord>, AuthAdminMutationError> {
        self.inner
            .list_authorized_oauth_apps(context, clamp_oauth_guard_list_limit(limit))
            .await
    }

    async fn create_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        command: CreateOAuthAppCommand,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        let authority = self.require_settings_manage(context)?;
        validate_grant_dependencies(&command.grant_types)?;
        self.validate_permission_strings(&authority, &command.granted_permissions)?;
        self.inner.create_oauth_app(context, command).await
    }

    async fn update_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        command: UpdateOAuthAppCommand,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        let authority = self.require_settings_manage(context)?;
        validate_grant_dependencies(&command.grant_types)?;
        self.validate_permission_strings(&authority, &command.granted_permissions)?;
        self.inner.update_oauth_app(context, command).await
    }

    async fn rotate_oauth_app_secret(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<OAuthAppSecretResult, AuthAdminMutationError> {
        let authority = self.require_settings_manage(context)?;
        self.rotate_secret_transactionally(context, app_id, &authority)
            .await
    }

    async fn revoke_oauth_app(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<OAuthAppMutationRecord, AuthAdminMutationError> {
        self.require_settings_manage(context)?;
        self.inner.revoke_oauth_app(context, app_id).await
    }

    async fn grant_oauth_app_consent(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
        scopes: Vec<String>,
    ) -> Result<(), AuthAdminMutationError> {
        OAuthAppService::grant_consent_strict(
            &self.db,
            app_id,
            context.actor_id,
            context.tenant_id,
            scopes,
        )
        .await
        .map_err(map_consent_error)
    }

    async fn revoke_oauth_app_consent(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<(), AuthAdminMutationError> {
        self.inner.revoke_oauth_app_consent(context, app_id).await
    }
}

fn map_consent_error(error: crate::error::Error) -> AuthAdminMutationError {
    match error {
        crate::error::Error::NotFound => AuthAdminMutationError::NotFound("oauth app".to_string()),
        crate::error::Error::BadRequest(message) => AuthAdminMutationError::Validation(message),
        crate::error::Error::Unauthorized(_) => AuthAdminMutationError::Unauthorized,
        other => internal_oauth_guard_error(other),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        GuardedOAuthAdminProvider, clamp_oauth_guard_list_limit, internal_oauth_guard_error,
        validate_grant_dependencies,
    };
    use crate::models::oauth_apps::scope_runtime_copy_locale;
    use crate::models::{oauth_apps, tenants};
    use crate::services::auth_admin_mutation_provider::ServerAuthAdminMutationProvider;
    use crate::services::oauth_app::{CreateOAuthAppInput, OAuthAppService};
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
    use rustok_api::Permission;
    use rustok_auth::{AuthAdminMutationContext, AuthAdminMutationError, OAuthAdminPort};
    use rustok_core::UserRole;
    use rustok_migrations::SqliteTestMigrator as Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ActiveModelTrait, EntityTrait};
    use serial_test::serial;
    use uuid::Uuid;

    #[tokio::test]
    #[serial]
    async fn guarded_secret_rotation_checks_tenant_and_delegated_permission_ceiling() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new(
            "OAuth guard tenant",
            &format!("oauth-guard-{}", Uuid::new_v4()),
        )
        .insert(&db)
        .await
        .expect("insert tenant");
        let actor_id = Uuid::new_v4();
        let context = AuthAdminMutationContext {
            actor_id,
            tenant_id: tenant.id,
            request_id: None,
            locale: Some("en".to_string()),
            principal_kind: Some(rustok_api::AuthPrincipalKind::DirectUser),
        };
        let created = scope_runtime_copy_locale(
            "en".to_string(),
            OAuthAppService::create_app(
                &db,
                tenant.id,
                CreateOAuthAppInput {
                    name: "Admin client".to_string(),
                    slug: "admin-client".to_string(),
                    description: None,
                    app_type: "third_party".to_string(),
                    icon_url: None,
                    redirect_uris: vec!["https://client.example/callback".to_string()],
                    scopes: vec!["profile".to_string()],
                    grant_types: vec!["authorization_code".to_string()],
                    granted_permissions: vec![Permission::MODULES_MANAGE.to_string()],
                },
            ),
        )
        .await
        .expect("create OAuth app");
        let original_secret = created.client_secret;
        let app_id = created.app.id;
        let inner = Arc::new(ServerAuthAdminMutationProvider::new(
            db.clone(),
            crate::auth::AuthConfig::new("test-secret".to_string()),
        ));
        let guarded = GuardedOAuthAdminProvider::new(db.clone(), inner);

        let no_admin_scope =
            RbacRequestScope::new(tenant.id, actor_id, Vec::new(), UserRole::Customer);
        let no_admin_access = with_rbac_request_scope(Some(no_admin_scope), async {
            guarded.rotate_oauth_app_secret(&context, app_id).await
        })
        .await;
        assert!(matches!(
            no_admin_access,
            Err(AuthAdminMutationError::Forbidden(_))
        ));

        let insufficient_scope = RbacRequestScope::new(
            tenant.id,
            actor_id,
            vec![Permission::SETTINGS_MANAGE],
            UserRole::Admin,
        );
        let denied = with_rbac_request_scope(Some(insufficient_scope), async {
            guarded.rotate_oauth_app_secret(&context, app_id).await
        })
        .await;
        assert!(matches!(denied, Err(AuthAdminMutationError::Forbidden(_))));

        let unchanged = oauth_apps::Entity::find_by_id(app_id)
            .one(&db)
            .await
            .expect("reload OAuth app after denied rotation")
            .expect("OAuth app remains present");
        assert!(
            OAuthAppService::verify_client_secret(
                &original_secret,
                unchanged
                    .client_secret_hash
                    .as_deref()
                    .expect("existing secret hash"),
            )
            .expect("verify original secret after denied attempt")
        );

        let other_tenant =
            tenants::ActiveModel::new("Other tenant", &format!("oauth-guard-{}", Uuid::new_v4()))
                .insert(&db)
                .await
                .expect("insert another tenant");
        let other_context = AuthAdminMutationContext {
            tenant_id: other_tenant.id,
            ..context.clone()
        };
        let other_tenant_scope = RbacRequestScope::new(
            other_tenant.id,
            actor_id,
            vec![Permission::SETTINGS_MANAGE, Permission::MODULES_MANAGE],
            UserRole::Admin,
        );
        let cross_tenant = with_rbac_request_scope(Some(other_tenant_scope), async {
            guarded
                .rotate_oauth_app_secret(&other_context, app_id)
                .await
        })
        .await;
        assert!(matches!(
            cross_tenant,
            Err(AuthAdminMutationError::NotFound(_))
        ));
        let after_cross_tenant_attempt = oauth_apps::Entity::find_by_id(app_id)
            .one(&db)
            .await
            .expect("reload OAuth app after cross-tenant rotation attempt")
            .expect("OAuth app remains present");
        assert!(
            OAuthAppService::verify_client_secret(
                &original_secret,
                after_cross_tenant_attempt
                    .client_secret_hash
                    .as_deref()
                    .expect("existing secret hash"),
            )
            .expect("verify cross-tenant attempt did not rotate the secret")
        );

        let sufficient_scope = RbacRequestScope::new(
            tenant.id,
            actor_id,
            vec![Permission::SETTINGS_MANAGE, Permission::MODULES_MANAGE],
            UserRole::Admin,
        );
        let rotated = with_rbac_request_scope(Some(sufficient_scope), async {
            guarded.rotate_oauth_app_secret(&context, app_id).await
        })
        .await
        .expect("authorized secret rotation");
        let persisted = oauth_apps::Entity::find_by_id(app_id)
            .one(&db)
            .await
            .expect("load rotated OAuth app")
            .expect("OAuth app remains present");
        let persisted_hash = persisted
            .client_secret_hash
            .as_deref()
            .expect("rotated secret hash");
        assert!(
            OAuthAppService::verify_client_secret(&rotated.client_secret, persisted_hash)
                .expect("verify rotated client secret")
        );
        assert!(
            !OAuthAppService::verify_client_secret(&original_secret, persisted_hash)
                .expect("verify old secret is rejected")
        );
    }

    #[tokio::test]
    #[serial]
    async fn prepared_secret_rotation_rejects_a_concurrent_secret_change() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new(
            "OAuth rotation race tenant",
            &format!("oauth-rotation-{}", Uuid::new_v4()),
        )
        .insert(&db)
        .await
        .expect("insert tenant");
        let created = scope_runtime_copy_locale(
            "en".to_string(),
            OAuthAppService::create_app(
                &db,
                tenant.id,
                CreateOAuthAppInput {
                    name: "Rotating client".to_string(),
                    slug: "rotating-client".to_string(),
                    description: None,
                    app_type: "third_party".to_string(),
                    icon_url: None,
                    redirect_uris: vec!["https://client.example/callback".to_string()],
                    scopes: vec!["profile".to_string()],
                    grant_types: vec!["authorization_code".to_string()],
                    granted_permissions: vec![Permission::MODULES_MANAGE.to_string()],
                },
            ),
        )
        .await
        .expect("create OAuth app");
        let app_id = created.app.id;
        let actor_id = Uuid::new_v4();
        let context = AuthAdminMutationContext {
            actor_id,
            tenant_id: tenant.id,
            request_id: None,
            locale: Some("en".to_string()),
            principal_kind: Some(rustok_api::AuthPrincipalKind::DirectUser),
        };
        let guard = GuardedOAuthAdminProvider::new(
            db.clone(),
            Arc::new(ServerAuthAdminMutationProvider::new(
                db.clone(),
                crate::auth::AuthConfig::new("test-secret".to_string()),
            )),
        );
        let authority = vec![Permission::SETTINGS_MANAGE, Permission::MODULES_MANAGE];

        // Password hashing has completed without holding a transaction. The
        // app can be updated independently before the commit phase starts.
        let prepared = guard
            .prepare_oauth_secret_rotation(tenant.id, app_id, &authority)
            .await
            .expect("prepare secret outside a SQL transaction");
        let competing_hash = "secret-hash-set-by-another-rotation".to_string();
        oauth_apps::ActiveModel {
            id: sea_orm::Set(app_id),
            client_secret_hash: sea_orm::Set(Some(competing_hash.clone())),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("concurrent secret update must not be blocked by preparation");

        let error = match guard
            .commit_prepared_oauth_secret_rotation(&context, app_id, &authority, prepared)
            .await
        {
            Ok(_) => panic!("stale prepared secret must never overwrite new secret"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            AuthAdminMutationError::Validation(message)
                if message.contains("changed during rotation")
        ));
        let current = oauth_apps::Entity::find_by_id(app_id)
            .one(&db)
            .await
            .expect("reload app")
            .expect("app still exists");
        assert_eq!(current.client_secret_hash.as_deref(), Some(competing_hash.as_str()));
    }

    #[test]
    fn refresh_grant_requires_authorization_code() {
        assert!(validate_grant_dependencies(&["refresh_token".to_string()]).is_err());
        assert!(
            validate_grant_dependencies(&[
                "authorization_code".to_string(),
                "refresh_token".to_string(),
            ])
            .is_ok()
        );
    }

    #[test]
    fn oauth_guard_list_limit_is_bounded() {
        assert_eq!(clamp_oauth_guard_list_limit(0), 1);
        assert_eq!(clamp_oauth_guard_list_limit(50), 50);
        assert_eq!(clamp_oauth_guard_list_limit(100), 100);
        assert_eq!(clamp_oauth_guard_list_limit(u64::MAX), 100);
    }

    #[test]
    fn internal_guard_errors_are_redacted() {
        let error = internal_oauth_guard_error("database secret leaked");
        assert!(matches!(
            error,
            AuthAdminMutationError::Internal(message)
                if message == "OAuth administration guard operation failed"
        ));
    }

    #[test]
    fn non_refresh_grants_remain_independent() {
        assert!(validate_grant_dependencies(&["client_credentials".to_string()]).is_ok());
    }
}
