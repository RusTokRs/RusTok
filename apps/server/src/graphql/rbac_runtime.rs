use std::sync::Arc;

use async_trait::async_trait;
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, UpdateUserCommand, UserAdminMutationRuntime,
};
use rustok_cache::CacheService;
use rustok_core::{ModuleRuntimeExtensions, UserRole};
use rustok_rbac::graphql::{
    RbacGraphqlRoleWriteError, RbacGraphqlRoleWriter, RbacGraphqlRoleWriterHandle,
};
use uuid::Uuid;

use crate::services::server_runtime_context::ServerRuntimeContext;

struct ServerRbacGraphqlRoleWriter {
    runtime: UserAdminMutationRuntime,
    db: sea_orm::DatabaseConnection,
    cache_service: CacheService,
}

#[async_trait]
impl RbacGraphqlRoleWriter for ServerRbacGraphqlRoleWriter {
    async fn replace_user_role(
        &self,
        tenant_id: &Uuid,
        actor_id: &Uuid,
        user_id: &Uuid,
        role: UserRole,
    ) -> Result<(), RbacGraphqlRoleWriteError> {
        self.runtime
            .port()
            .update_user(
                &AuthAdminMutationContext {
                    actor_id: *actor_id,
                    tenant_id: *tenant_id,
                    request_id: None,
                    locale: None,
                    // Host-internal RBAC composition invokes the port
                    // without a request principal; self-service operations
                    // that require DirectUser fail closed on None.
                    principal_kind: None,
                },
                UpdateUserCommand {
                    id: *user_id,
                    email: None,
                    password: None,
                    name: None,
                    role: Some(role.to_string()),
                    status: None,
                    custom_fields: None,
                },
            )
            .await
            .map(|_| ())
            .map_err(map_auth_admin_error)
    }

    async fn replace_user_role_by_slug(
        &self,
        tenant_id: &Uuid,
        actor_id: &Uuid,
        user_id: &Uuid,
        role_slug: &str,
    ) -> Result<(), RbacGraphqlRoleWriteError> {
        // The user-administration port accepts canonical built-in roles and
        // tenant-persisted custom role slugs through the same command; the
        // owner planning inside it enforces hierarchy for built-ins and the
        // actor permission ceiling for custom roles.
        self.runtime
            .port()
            .update_user(
                &AuthAdminMutationContext {
                    actor_id: *actor_id,
                    tenant_id: *tenant_id,
                    request_id: None,
                    locale: None,
                    // Host-internal RBAC composition invokes the port
                    // without a request principal; self-service operations
                    // that require DirectUser fail closed on None.
                    principal_kind: None,
                },
                UpdateUserCommand {
                    id: *user_id,
                    email: None,
                    password: None,
                    name: None,
                    role: Some(role_slug.to_string()),
                    status: None,
                    custom_fields: None,
                },
            )
            .await
            .map(|_| ())
            .map_err(map_auth_admin_error)
    }

    async fn role_permissions_committed(
        &self,
        _tenant_id: &Uuid,
        _role_id: &Uuid,
        generation: u64,
    ) -> Result<(), RbacGraphqlRoleWriteError> {
        // Clear locally and expire the caller's scope before any fallible fan-out.
        // The committed global watermark remains available to all watchdogs.
        crate::services::rbac_runtime::invalidate_all_user_permissions_cache().await;
        crate::services::rbac_cache_invalidation::publish_all_rbac_invalidation(
            &self.cache_service,
            generation,
        )
        .await
        .map_err(|error| RbacGraphqlRoleWriteError::Internal(error.to_string()))
    }

    async fn invalidate_role_users(
        &self,
        tenant_id: &Uuid,
        role_id: &Uuid,
    ) -> Result<(), RbacGraphqlRoleWriteError> {
        use crate::services::rbac_service::RbacService;
        use sea_orm::FromQueryResult;

        // The grant update has committed. Expire the caller's snapshot before
        // the fallible membership lookup: an error must not leave subsequent
        // fields authorized by a potentially stale AuthContext. Conservatively
        // expire it even when the changed role has no membership for the actor.
        crate::services::rbac_request_scope::invalidate_current_rbac_request_scope_for_all();

        #[derive(Debug, FromQueryResult)]
        struct UserRoleMembership {
            user_id: Uuid,
        }

        let backend = self.db.get_database_backend();
        let statement = sea_orm::Statement::from_sql_and_values(
            backend,
            role_membership_invalidation_sql(backend),
            [(*tenant_id).into(), (*role_id).into()],
        );

        let memberships = UserRoleMembership::find_by_statement(statement)
            .all(&self.db)
            .await
            .map_err(|error| {
                RbacGraphqlRoleWriteError::Internal(format!(
                    "failed to load role memberships for cache invalidation: {error}"
                ))
            })?;
        for membership in memberships {
            RbacService::invalidate_user_rbac_caches(tenant_id, &membership.user_id).await;
        }
        Ok(())
    }
}

fn role_membership_invalidation_sql(backend: sea_orm::DbBackend) -> &'static str {
    match backend {
        sea_orm::DbBackend::Postgres => {
            "SELECT ur.user_id FROM user_roles AS ur INNER JOIN roles AS r ON r.id = ur.role_id INNER JOIN users AS u ON u.id = ur.user_id AND u.tenant_id = r.tenant_id WHERE r.tenant_id = $1 AND r.id = $2"
        }
        _ => {
            "SELECT ur.user_id FROM user_roles AS ur INNER JOIN roles AS r ON r.id = ur.role_id INNER JOIN users AS u ON u.id = ur.user_id AND u.tenant_id = r.tenant_id WHERE r.tenant_id = ? AND r.id = ?"
        }
    }
}

fn map_auth_admin_error(error: AuthAdminMutationError) -> RbacGraphqlRoleWriteError {
    match error {
        AuthAdminMutationError::Unauthorized => RbacGraphqlRoleWriteError::Forbidden(
            "authentication context is unavailable".to_string(),
        ),
        AuthAdminMutationError::Forbidden(message) => RbacGraphqlRoleWriteError::Forbidden(message),
        AuthAdminMutationError::NotFound(message) => RbacGraphqlRoleWriteError::NotFound(message),
        AuthAdminMutationError::Validation(message) | AuthAdminMutationError::Conflict(message) => {
            RbacGraphqlRoleWriteError::Conflict(message)
        }
        AuthAdminMutationError::CustomFieldsValidation(fields) => {
            RbacGraphqlRoleWriteError::Conflict(fields.to_string())
        }
        AuthAdminMutationError::Internal(message) => RbacGraphqlRoleWriteError::Internal(message),
    }
}

pub fn rbac_graphql_role_writer_from_context(
    ctx: &ServerRuntimeContext,
) -> RbacGraphqlRoleWriterHandle {
    // INVARIANT: ModuleRuntimeExtensions is registered during bootstrap_app_runtime before GraphQL construction.
    let extensions = ctx
        .shared_get::<Arc<ModuleRuntimeExtensions>>()
        .expect("ModuleRuntimeExtensions must be initialized before GraphQL schema construction");
    // INVARIANT: UserAdminMutationRuntime is registered during module runtime extensions bootstrap.
    let runtime = extensions
        .get::<UserAdminMutationRuntime>()
        .cloned()
        .expect("UserAdminMutationRuntime must be registered before GraphQL schema construction");

    RbacGraphqlRoleWriterHandle(Arc::new(ServerRbacGraphqlRoleWriter {
        runtime,
        db: ctx.db().clone(),
        cache_service: crate::services::cache_runtime::ensure_cache_service(ctx),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_admin_mutation_provider::ServerAuthAdminMutationProvider;
    use crate::services::rbac_request_scope::{
        RbacRequestScope, ensure_scope_not_invalidated, permissions_for, with_rbac_request_scope,
    };
    use rustok_api::Permission;

    #[tokio::test]
    async fn role_membership_invalidation_is_scoped_to_the_role_and_user_tenant() {
        use sea_orm::{ConnectionTrait, FromQueryResult};

        #[derive(Debug, FromQueryResult)]
        struct UserRoleMembership {
            user_id: Uuid,
        }

        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        db.execute_unprepared("CREATE TABLE roles (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)")
            .await
            .expect("create roles table");
        db.execute_unprepared("CREATE TABLE users (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)")
            .await
            .expect("create users table");
        db.execute_unprepared(
            "CREATE TABLE user_roles (id TEXT PRIMARY KEY, user_id TEXT NOT NULL, role_id TEXT NOT NULL)",
        )
        .await
        .expect("create user_roles table");

        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let role_a = Uuid::new_v4();
        let role_b = Uuid::new_v4();
        let user_a = Uuid::new_v4();
        let user_b = Uuid::new_v4();
        for (role, tenant) in [(role_a, tenant_a), (role_b, tenant_b)] {
            db.execute_raw(sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Sqlite,
                "INSERT INTO roles (id, tenant_id) VALUES (?, ?)",
                [role.to_string().into(), tenant.to_string().into()],
            ))
            .await
            .expect("insert role");
        }
        for (user, tenant) in [(user_a, tenant_a), (user_b, tenant_b)] {
            db.execute_raw(sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Sqlite,
                "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
                [user.to_string().into(), tenant.to_string().into()],
            ))
            .await
            .expect("insert user");
        }
        for (user, role) in [(user_a, role_a), (user_b, role_a), (user_a, role_b)] {
            db.execute_raw(sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Sqlite,
                "INSERT INTO user_roles (id, user_id, role_id) VALUES (?, ?, ?)",
                [
                    Uuid::new_v4().to_string().into(),
                    user.to_string().into(),
                    role.to_string().into(),
                ],
            ))
            .await
            .expect("insert role membership");
        }

        let statement = sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            role_membership_invalidation_sql(sea_orm::DbBackend::Sqlite),
            [tenant_a.to_string().into(), role_a.to_string().into()],
        );
        let members = UserRoleMembership::find_by_statement(statement)
            .all(&db)
            .await
            .expect("load only tenant A role members");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].user_id, user_a);

        let foreign_scope = sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            role_membership_invalidation_sql(sea_orm::DbBackend::Sqlite),
            [tenant_b.to_string().into(), role_a.to_string().into()],
        );
        assert!(
            UserRoleMembership::find_by_statement(foreign_scope)
                .all(&db)
                .await
                .expect("foreign tenant lookup")
                .is_empty()
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn committed_grant_notification_clears_cache_and_calling_scope() {
        use crate::services::rbac_runtime::MokaPermissionCache;
        use rustok_rbac::PermissionCache;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let cache_service = CacheService::from_url(None);
        let provider = ServerAuthAdminMutationProvider::new(
            db.clone(),
            crate::auth::test_auth_config(
                "test-secret-key-for-unit-tests-only-32bytes!".to_string(),
            ),
            cache_service.clone(),
        );
        let writer = ServerRbacGraphqlRoleWriter {
            runtime: UserAdminMutationRuntime::new(Arc::new(provider)),
            db,
            cache_service,
        };
        let tenant = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let cache = MokaPermissionCache;
        cache
            .insert(&tenant, &actor, vec![Permission::USERS_MANAGE])
            .await;
        assert!(cache.get(&tenant, &actor).await.is_some());
        let scope = RbacRequestScope::new(
            tenant,
            actor,
            vec![Permission::USERS_MANAGE],
            UserRole::Admin,
        );
        with_rbac_request_scope(Some(scope), async {
            writer
                .role_permissions_committed(&tenant, &Uuid::new_v4(), 1)
                .await
                .unwrap();
            assert!(permissions_for(&tenant, &actor).is_none());
            assert!(ensure_scope_not_invalidated(&tenant, &actor).is_err());
            assert!(cache.get(&tenant, &actor).await.is_none());
        })
        .await;
    }

    #[tokio::test]
    async fn role_membership_lookup_failure_still_invalidates_current_scope() {
        // No tables: the membership query must fail after local invalidation.
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let cache_service = CacheService::from_url(None);
        let provider = ServerAuthAdminMutationProvider::new(
            db.clone(),
            crate::auth::test_auth_config(
                "test-secret-key-for-unit-tests-only-32bytes!".to_string(),
            ),
            cache_service.clone(),
        );
        let writer = ServerRbacGraphqlRoleWriter {
            runtime: UserAdminMutationRuntime::new(Arc::new(provider)),
            db,
            cache_service,
        };
        let tenant = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let scope = RbacRequestScope::new(
            tenant,
            actor,
            vec![Permission::USERS_MANAGE],
            UserRole::Admin,
        );
        with_rbac_request_scope(Some(scope), async {
            assert!(permissions_for(&tenant, &actor).is_some());
            assert!(
                writer
                    .invalidate_role_users(&tenant, &Uuid::new_v4())
                    .await
                    .is_err()
            );
            assert!(permissions_for(&tenant, &actor).is_none());
            assert!(ensure_scope_not_invalidated(&tenant, &actor).is_err());
        })
        .await;
    }
}
