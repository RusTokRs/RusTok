use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{Permission, has_effective_permission};
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, AuthorizedOAuthAppRecord,
    CreateOAuthAppCommand, OAuthAdminPort, OAuthAppMutationRecord, OAuthAppSecretResult,
    UpdateOAuthAppCommand,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::rbac_request_scope::permissions_for;

const MAX_OAUTH_GUARD_LIST_LIMIT: u64 = 100;

fn clamp_oauth_guard_list_limit(limit: u64) -> u64 {
    limit.clamp(1, MAX_OAUTH_GUARD_LIST_LIMIT)
}

/// Transport preflight only: all mutations go through the owner port.
pub struct GuardedOAuthAdminProvider {
    inner: Arc<dyn OAuthAdminPort>,
}

impl GuardedOAuthAdminProvider {
    pub fn new(_db: DatabaseConnection, inner: Arc<dyn OAuthAdminPort>) -> Self {
        Self { inner }
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
}

fn require_direct_user_consent(
    context: &AuthAdminMutationContext,
) -> Result<(), AuthAdminMutationError> {
    match context.principal_kind {
        Some(rustok_api::AuthPrincipalKind::DirectUser) => Ok(()),
        _ => Err(AuthAdminMutationError::Forbidden(
            "oauth consent operations require a direct user session".to_string(),
        )),
    }
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
        require_direct_user_consent(context)?;
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
        self.require_settings_manage(context)?;
        self.inner.rotate_oauth_app_secret(context, app_id).await
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
        // Consent is the user's own decision, not authority delegated to an
        // OAuth client. Enforce this at the outer provider boundary.
        require_direct_user_consent(context)?;
        self.inner
            .grant_oauth_app_consent(context, app_id, scopes)
            .await
    }

    async fn revoke_oauth_app_consent(
        &self,
        context: &AuthAdminMutationContext,
        app_id: Uuid,
    ) -> Result<(), AuthAdminMutationError> {
        require_direct_user_consent(context)?;
        self.inner.revoke_oauth_app_consent(context, app_id).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        GuardedOAuthAdminProvider, clamp_oauth_guard_list_limit, require_direct_user_consent,
        validate_grant_dependencies,
    };
    use crate::models::oauth_apps::scope_runtime_copy_locale;
    use crate::models::{oauth_apps, tenants};
    use crate::services::auth_admin_mutation_provider::ServerAuthAdminMutationProvider;
    use crate::services::oauth_app::{CreateOAuthAppInput, OAuthAppService};
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
    use crate::services::server_runtime_context::ServerRuntimeContext;
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
    async fn guarded_consent_mutations_require_direct_user_principal() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new(
            "OAuth consent guard tenant",
            &format!("oauth-consent-guard-{}", Uuid::new_v4()),
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
                    name: "Consent target".to_string(),
                    slug: "consent-target".to_string(),
                    description: None,
                    app_type: "third_party".to_string(),
                    icon_url: None,
                    redirect_uris: vec!["https://client.example/callback".to_string()],
                    scopes: vec!["profile".to_string()],
                    grant_types: vec!["authorization_code".to_string()],
                    granted_permissions: Vec::new(),
                },
            ),
        )
        .await
        .expect("create OAuth app");
        let inner = Arc::new(ServerAuthAdminMutationProvider::new(
            &ServerRuntimeContext::new(db.clone(), Default::default()),
            crate::auth::test_auth_config(
                "test-secret-key-for-unit-tests-only-32bytes!".to_string(),
            ),
        ));
        let guarded = GuardedOAuthAdminProvider::new(db.clone(), inner);

        // Delegated and service credentials cannot grant or revoke consent
        // through the registered (guarded) provider composition.
        for principal_kind in [
            Some(rustok_api::AuthPrincipalKind::DelegatedUser),
            Some(rustok_api::AuthPrincipalKind::Service),
            None,
        ] {
            let context = AuthAdminMutationContext {
                actor_id: Uuid::new_v4(),
                tenant_id: tenant.id,
                request_id: None,
                locale: Some("en".to_string()),
                principal_kind,
            };
            let grant_result = guarded
                .grant_oauth_app_consent(&context, created.app.id, vec!["profile".to_string()])
                .await;
            assert!(
                matches!(grant_result, Err(AuthAdminMutationError::Forbidden(_))),
                "principal_kind {principal_kind:?} must not grant consent, got {grant_result:?}"
            );

            let revoke_result = guarded
                .revoke_oauth_app_consent(&context, created.app.id)
                .await;
            assert!(
                matches!(revoke_result, Err(AuthAdminMutationError::Forbidden(_))),
                "principal_kind {principal_kind:?} must not revoke consent, got {revoke_result:?}"
            );

            let list_result = guarded.list_authorized_oauth_apps(&context, 10).await;
            assert!(
                matches!(list_result, Err(AuthAdminMutationError::Forbidden(_))),
                "principal_kind {principal_kind:?} must not enumerate consent, got {list_result:?}"
            );
        }
    }

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
        let mut actor = crate::models::users::ActiveModel::new(
            tenant.id,
            "oauth-rotation-admin@example.test",
            "hash",
        );
        actor.status = sea_orm::Set(rustok_core::UserStatus::Active);
        let actor = actor.insert(&db).await.unwrap();
        let actor_id = actor.id;
        rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_on(
            &db,
            tenant.id,
            actor_id,
            UserRole::SuperAdmin,
        )
        .await
        .unwrap();
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
            &ServerRuntimeContext::new(db.clone(), Default::default()),
            crate::auth::test_auth_config(
                "test-secret-key-for-unit-tests-only-32bytes!".to_string(),
            ),
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
        let mut actor = crate::models::users::ActiveModel::new(
            tenant.id,
            "oauth-rotation-admin@example.test",
            "hash",
        );
        actor.status = sea_orm::Set(rustok_core::UserStatus::Active);
        let actor = actor.insert(&db).await.unwrap();
        let actor_id = actor.id;
        rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_on(
            &db,
            tenant.id,
            actor_id,
            UserRole::SuperAdmin,
        )
        .await
        .unwrap();
        let context = AuthAdminMutationContext {
            actor_id,
            tenant_id: tenant.id,
            request_id: None,
            locale: Some("en".to_string()),
            principal_kind: Some(rustok_api::AuthPrincipalKind::DirectUser),
        };
        let provider = ServerAuthAdminMutationProvider::new(
            &ServerRuntimeContext::new(db.clone(), Default::default()),
            crate::auth::test_auth_config(
                "test-secret-key-for-oauth-rotation-race-at-least-32-bytes".to_string(),
            ),
        );
        let authority = vec![Permission::SETTINGS_MANAGE, Permission::MODULES_MANAGE];

        // Password hashing has completed without holding a transaction. The
        // app can be updated independently before the commit phase starts.
        let prepared = OAuthAppService::prepare_secret(&created.app)
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

        let scope = RbacRequestScope::new(tenant.id, actor_id, authority, UserRole::SuperAdmin);
        let error = match with_rbac_request_scope(
            Some(scope),
            provider.commit_prepared_oauth_secret_rotation(&context, app_id, prepared),
        )
        .await
        {
            Ok(_) => panic!("stale prepared secret must never overwrite new secret"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            AuthAdminMutationError::Conflict(message)
                if message.contains("changed during rotation")
        ));
        let current = oauth_apps::Entity::find_by_id(app_id)
            .one(&db)
            .await
            .expect("reload app")
            .expect("app still exists");
        assert_eq!(
            current.client_secret_hash.as_deref(),
            Some(competing_hash.as_str())
        );
    }

    #[test]
    fn direct_user_consent_guard_rejects_delegated_service_and_unknown_principals() {
        let context = |principal_kind| AuthAdminMutationContext {
            actor_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            request_id: None,
            locale: Some("en".to_string()),
            principal_kind,
        };

        assert!(
            require_direct_user_consent(&context(Some(rustok_api::AuthPrincipalKind::DirectUser)))
                .is_ok()
        );
        for principal_kind in [
            Some(rustok_api::AuthPrincipalKind::DelegatedUser),
            Some(rustok_api::AuthPrincipalKind::Service),
            None,
        ] {
            assert!(matches!(
                require_direct_user_consent(&context(principal_kind)),
                Err(AuthAdminMutationError::Forbidden(_))
            ));
        }
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
    fn non_refresh_grants_remain_independent() {
        assert!(validate_grant_dependencies(&["client_credentials".to_string()]).is_ok());
    }
}
