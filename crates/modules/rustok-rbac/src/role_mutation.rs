use rustok_core::{UserRole, UserStatus};
use rustok_events::RbacRoleMutationEvent;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RbacRoleMutationFacts {
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub actor_tenant_id: Uuid,
    pub actor_role: UserRole,
    pub target_user_id: Uuid,
    pub target_tenant_id: Uuid,
    pub target_role: UserRole,
    pub target_status: UserStatus,
    pub requested_role: UserRole,
    pub resulting_status: UserStatus,
    /// True only when the target has exactly one tenant role assignment and it
    /// is the requested canonical built-in role.
    pub assignment_is_exact: bool,
    /// Active super administrators in the same tenant after excluding the
    /// target user. This fact is consumed only when the mutation would remove
    /// the target from the active super-administrator set.
    pub remaining_active_super_admins: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RbacRoleMutationOutcome {
    Noop,
    Apply(RbacRoleMutationPlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RbacRoleMutationPlan {
    tenant_id: Uuid,
    actor_id: Uuid,
    target_user_id: Uuid,
    previous_role: UserRole,
    new_role: UserRole,
    change: RbacRoleMutationChange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RbacRoleMutationChange {
    RoleReplaced,
    AssignmentRepaired,
}

impl RbacRoleMutationPlan {
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }

    pub fn target_user_id(&self) -> Uuid {
        self.target_user_id
    }

    pub fn previous_role(&self) -> &UserRole {
        &self.previous_role
    }

    pub fn new_role(&self) -> &UserRole {
        &self.new_role
    }

    pub fn change(&self) -> RbacRoleMutationChange {
        self.change
    }

    pub fn integration_event(
        &self,
        durable_generation: u64,
    ) -> Result<RbacRoleMutationEvent, RbacRoleMutationPolicyError> {
        if durable_generation == 0 {
            return Err(RbacRoleMutationPolicyError::InvalidDurableGeneration);
        }

        Ok(match self.change {
            RbacRoleMutationChange::RoleReplaced => RbacRoleMutationEvent::user_role_replaced(
                self.target_user_id,
                self.previous_role.to_string(),
                self.new_role.to_string(),
                durable_generation,
            ),
            RbacRoleMutationChange::AssignmentRepaired => {
                RbacRoleMutationEvent::user_role_assignment_repaired(
                    self.target_user_id,
                    self.new_role.to_string(),
                    durable_generation,
                )
            }
        })
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RbacRoleMutationPolicyError {
    #[error("RBAC role mutation identity `{0}` must not be nil")]
    NilIdentity(&'static str),
    #[error("RBAC role mutation actor tenant does not match the requested tenant")]
    ActorTenantMismatch,
    #[error("RBAC role mutation target tenant does not match the requested tenant")]
    TargetTenantMismatch,
    #[error("cannot assign a peer or higher-privileged role")]
    CannotAssignPeerOrHigherRole,
    #[error("cannot modify a peer or higher-privileged user")]
    CannotManagePeerOrHigherUser,
    #[error(
        "cannot assign role `{role}` because permission `{permission}` exceeds the current request authority"
    )]
    RequestAuthorityCeiling {
        role: UserRole,
        permission: rustok_api::Permission,
    },
    #[error("cannot remove, demote, or deactivate the last active super administrator")]
    LastActiveSuperAdmin,
    #[error("RBAC role mutation durable generation must be greater than zero")]
    InvalidDurableGeneration,
}

pub fn plan_user_role_mutation(
    facts: RbacRoleMutationFacts,
) -> Result<RbacRoleMutationOutcome, RbacRoleMutationPolicyError> {
    validate_identity("tenant_id", facts.tenant_id)?;
    validate_identity("actor_id", facts.actor_id)?;
    validate_identity("actor_tenant_id", facts.actor_tenant_id)?;
    validate_identity("target_user_id", facts.target_user_id)?;
    validate_identity("target_tenant_id", facts.target_tenant_id)?;

    if facts.actor_tenant_id != facts.tenant_id {
        return Err(RbacRoleMutationPolicyError::ActorTenantMismatch);
    }
    if facts.target_tenant_id != facts.tenant_id {
        return Err(RbacRoleMutationPolicyError::TargetTenantMismatch);
    }
    require_role_assignment(&facts.actor_role, &facts.requested_role)?;
    require_user_management(
        facts.actor_id,
        facts.target_user_id,
        &facts.actor_role,
        &facts.target_role,
    )?;

    let removes_active_super_admin = facts.target_role == UserRole::SuperAdmin
        && facts.target_status == UserStatus::Active
        && (facts.requested_role != UserRole::SuperAdmin
            || facts.resulting_status != UserStatus::Active);
    if removes_active_super_admin && facts.remaining_active_super_admins == 0 {
        return Err(RbacRoleMutationPolicyError::LastActiveSuperAdmin);
    }

    if facts.assignment_is_exact && facts.target_role == facts.requested_role {
        return Ok(RbacRoleMutationOutcome::Noop);
    }

    let change = if facts.target_role == facts.requested_role {
        RbacRoleMutationChange::AssignmentRepaired
    } else {
        RbacRoleMutationChange::RoleReplaced
    };
    Ok(RbacRoleMutationOutcome::Apply(RbacRoleMutationPlan {
        tenant_id: facts.tenant_id,
        actor_id: facts.actor_id,
        target_user_id: facts.target_user_id,
        previous_role: facts.target_role,
        new_role: facts.requested_role,
        change,
    }))
}

fn validate_identity(field: &'static str, value: Uuid) -> Result<(), RbacRoleMutationPolicyError> {
    if value.is_nil() {
        Err(RbacRoleMutationPolicyError::NilIdentity(field))
    } else {
        Ok(())
    }
}

pub fn require_role_assignment(
    actor_role: &UserRole,
    requested_role: &UserRole,
) -> Result<(), RbacRoleMutationPolicyError> {
    if actor_role.can_assign_role(requested_role) {
        Ok(())
    } else {
        Err(RbacRoleMutationPolicyError::CannotAssignPeerOrHigherRole)
    }
}

pub fn require_user_management(
    actor_id: Uuid,
    target_user_id: Uuid,
    actor_role: &UserRole,
    target_role: &UserRole,
) -> Result<(), RbacRoleMutationPolicyError> {
    validate_identity("actor_id", actor_id)?;
    validate_identity("target_user_id", target_user_id)?;
    if actor_id == target_user_id || actor_role.can_manage_role(target_role) {
        Ok(())
    } else {
        Err(RbacRoleMutationPolicyError::CannotManagePeerOrHigherUser)
    }
}

/// A privileged role must fit inside the token's effective permission ceiling.
/// Customer provisioning is the baseline users:create/users:manage operation.
pub fn require_request_role_grant(
    actor_role: &UserRole,
    authority: &[rustok_api::Permission],
    requested_role: &UserRole,
) -> Result<(), RbacRoleMutationPolicyError> {
    require_role_assignment(actor_role, requested_role)?;
    if requested_role != &UserRole::Customer {
        for permission in rustok_core::Rbac::permissions_for_role(requested_role) {
            if !crate::has_effective_permission_in_set(authority, permission) {
                return Err(RbacRoleMutationPolicyError::RequestAuthorityCeiling {
                    role: requested_role.clone(),
                    permission: *permission,
                });
            }
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum RbacRolePersistenceError {
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    Assignment(#[from] crate::RbacRoleAssignmentError),
    #[error(transparent)]
    Policy(#[from] RbacRoleMutationPolicyError),
    #[error("RBAC target user was not found in the requested tenant")]
    TargetNotFound,
    #[error("active super administrator role assignment is inconsistent")]
    InconsistentAuthority,
}

/// The requested change to a user's participation in the active administrator set.
#[derive(Clone, Debug, Default)]
pub struct RbacUserAuthorityChange {
    pub role: Option<UserRole>,
    pub status: Option<UserStatus>,
    pub deleting: bool,
}

fn role_statement<C: sea_orm::ConnectionTrait>(
    db: &C,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> Result<sea_orm::Statement, sea_orm::DbErr> {
    let backend = db.get_database_backend();
    if !matches!(
        backend,
        sea_orm::DbBackend::Postgres | sea_orm::DbBackend::Sqlite
    ) {
        return Err(sea_orm::DbErr::Custom(
            "RBAC role mutation requires PostgreSQL or SQLite".to_string(),
        ));
    }
    let mut index = 0;
    let sql = sql
        .chars()
        .map(|ch| {
            if ch == '?' {
                index += 1;
                match backend {
                    sea_orm::DbBackend::Postgres => format!("${}", index),
                    _ => format!("?{}", index),
                }
            } else {
                ch.to_string()
            }
        })
        .collect::<String>();
    Ok(sea_orm::Statement::from_sql_and_values(
        backend, sql, values,
    ))
}

/// Read the complete tenant membership set, rather than only its effective role.
pub async fn has_exact_tenant_role_assignment_on<C: sea_orm::ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
    requested_role: &UserRole,
) -> Result<bool, RbacRolePersistenceError> {
    validate_identity("tenant_id", tenant_id)?;
    validate_identity("target_user_id", user_id)?;
    let rows = db.query_all_raw(role_statement(
        db,
        "SELECT r.slug, r.is_system FROM user_roles ur JOIN roles r ON r.id = ur.role_id JOIN users u ON u.id = ur.user_id AND u.tenant_id = r.tenant_id WHERE ur.user_id = ? AND r.tenant_id = ?",
        vec![user_id.into(), tenant_id.into()],
    )?).await?;
    Ok(rows.len() == 1
        && rows[0].try_get::<String>("", "slug")? == requested_role.to_string()
        && rows[0].try_get::<bool>("", "is_system")?)
}

/// Serialize continuity checks on the tenant's canonical administrator role.
/// The transaction owner must retain this lock through its user/relation write.
pub async fn count_remaining_active_super_admins_on(
    db: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    target_user_id: Uuid,
) -> Result<u64, RbacRolePersistenceError> {
    use sea_orm::ConnectionTrait;
    validate_identity("tenant_id", tenant_id)?;
    validate_identity("target_user_id", target_user_id)?;
    let sql = match db.get_database_backend() {
        sea_orm::DbBackend::Postgres => {
            "SELECT id FROM roles WHERE tenant_id = ? AND slug = ? AND is_system = TRUE FOR UPDATE"
        }
        sea_orm::DbBackend::Sqlite => {
            "UPDATE roles SET updated_at = updated_at WHERE tenant_id = ? AND slug = ? AND is_system = TRUE RETURNING id"
        }
        _ => {
            return Err(sea_orm::DbErr::Custom(
                "RBAC continuity requires PostgreSQL or SQLite".to_string(),
            )
            .into());
        }
    };
    let role = db
        .query_one_raw(role_statement(
            db,
            sql,
            vec![tenant_id.into(), UserRole::SuperAdmin.to_string().into()],
        )?)
        .await?
        .ok_or(RbacRolePersistenceError::InconsistentAuthority)?;
    let role_id: Uuid = role.try_get("", "id")?;
    let row = db.query_one_raw(role_statement(
        db,
        "SELECT COUNT(*) AS remaining FROM users u WHERE u.tenant_id = ? AND u.id <> ? AND u.status = 'active' AND EXISTS (SELECT 1 FROM user_roles ur WHERE ur.user_id = u.id AND ur.role_id = ?)",
        vec![tenant_id.into(), target_user_id.into(), role_id.into()],
    )?).await?.ok_or(RbacRolePersistenceError::InconsistentAuthority)?;
    let remaining: i64 = row.try_get("", "remaining")?;
    u64::try_from(remaining).map_err(|_| {
        sea_orm::DbErr::Custom("RBAC administrator count is negative".to_string()).into()
    })
}

#[derive(Clone, Debug)]
pub struct RbacUserRoleMutationRequest {
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub actor_tenant_id: Uuid,
    pub actor_role: UserRole,
    pub target_user_id: Uuid,
    pub requested_role: UserRole,
    pub resulting_status: Option<UserStatus>,
}

/// Build the role plan from owner-read facts under the host's transaction.
/// Actor authority is bound to the authenticated request by the host.
pub async fn plan_persisted_user_role_mutation_on(
    db: &sea_orm::DatabaseTransaction,
    request: RbacUserRoleMutationRequest,
) -> Result<RbacRoleMutationOutcome, RbacRolePersistenceError> {
    validate_identity("actor_id", request.actor_id)?;
    validate_identity("actor_tenant_id", request.actor_tenant_id)?;
    if request.actor_tenant_id != request.tenant_id {
        return Err(RbacRoleMutationPolicyError::ActorTenantMismatch.into());
    }
    let target_status =
        lock_target_authority_on(db, request.tenant_id, request.target_user_id).await?;
    let permissions =
        crate::resolve_persisted_permissions_on(db, &request.tenant_id, &request.target_user_id)
            .await?;
    let target_role = rustok_core::infer_user_role_from_permissions(&permissions);
    let resulting_status = request
        .resulting_status
        .unwrap_or_else(|| target_status.clone());
    let removes_active_super_admin = target_role == UserRole::SuperAdmin
        && target_status == UserStatus::Active
        && (request.requested_role != UserRole::SuperAdmin
            || resulting_status != UserStatus::Active);
    let remaining_active_super_admins = if removes_active_super_admin {
        count_remaining_active_super_admins_on(db, request.tenant_id, request.target_user_id)
            .await?
    } else {
        0
    };
    let assignment_is_exact = has_exact_tenant_role_assignment_on(
        db,
        request.tenant_id,
        request.target_user_id,
        &request.requested_role,
    )
    .await?;
    Ok(plan_user_role_mutation(RbacRoleMutationFacts {
        tenant_id: request.tenant_id,
        actor_id: request.actor_id,
        actor_tenant_id: request.actor_tenant_id,
        actor_role: request.actor_role,
        target_user_id: request.target_user_id,
        target_tenant_id: request.tenant_id,
        target_role,
        target_status,
        requested_role: request.requested_role,
        resulting_status,
        assignment_is_exact,
        remaining_active_super_admins,
    })?)
}

async fn lock_target_authority_on(
    db: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<UserStatus, RbacRolePersistenceError> {
    use sea_orm::ConnectionTrait;
    validate_identity("tenant_id", tenant_id)?;
    validate_identity("target_user_id", user_id)?;
    let sql = match db.get_database_backend() {
        sea_orm::DbBackend::Postgres => {
            "SELECT status FROM users WHERE id = ? AND tenant_id = ? FOR UPDATE"
        }
        sea_orm::DbBackend::Sqlite => {
            "UPDATE users SET updated_at = updated_at WHERE id = ? AND tenant_id = ? RETURNING status"
        }
        _ => {
            return Err(sea_orm::DbErr::Custom(
                "RBAC user mutation requires PostgreSQL or SQLite".to_string(),
            )
            .into());
        }
    };
    let row = db
        .query_one_raw(role_statement(
            db,
            sql,
            vec![user_id.into(), tenant_id.into()],
        )?)
        .await?
        .ok_or(RbacRolePersistenceError::TargetNotFound)?;
    match row.try_get::<String>("", "status")?.as_str() {
        "active" => Ok(UserStatus::Active),
        "inactive" => Ok(UserStatus::Inactive),
        "banned" => Ok(UserStatus::Banned),
        _ => Err(RbacRolePersistenceError::InconsistentAuthority),
    }
}

/// Validate removal, role demotion, and status changes against current persisted
/// authority inside the same transaction that applies the host-owned user change.
pub async fn ensure_user_authority_continuity_on(
    db: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    user_id: Uuid,
    change: &RbacUserAuthorityChange,
) -> Result<(), RbacRolePersistenceError> {
    use sea_orm::ConnectionTrait;
    let active = lock_target_authority_on(db, tenant_id, user_id).await? == UserStatus::Active;
    let remains = !change.deleting
        && change
            .role
            .as_ref()
            .is_none_or(|role| role == &UserRole::SuperAdmin)
        && change
            .status
            .as_ref()
            .is_none_or(|status| status == &UserStatus::Active);
    if !active || remains {
        return Ok(());
    }
    let membership = db.query_one_raw(role_statement(
        db,
        "SELECT r.id FROM roles r JOIN user_roles ur ON ur.role_id = r.id WHERE r.tenant_id = ? AND r.slug = ? AND r.is_system = TRUE AND ur.user_id = ?",
        vec![tenant_id.into(), UserRole::SuperAdmin.to_string().into(), user_id.into()],
    )?).await?;
    if membership.is_some()
        && count_remaining_active_super_admins_on(db, tenant_id, user_id).await? == 0
    {
        return Err(RbacRoleMutationPolicyError::LastActiveSuperAdmin.into());
    }
    Ok(())
}

/// Replace a persisted role within the caller's transaction. Returns false only
/// for an exact canonical single-role assignment. Host commit and invalidation
/// remain outside this owner operation.
pub async fn replace_persisted_user_role_on(
    db: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    user_id: Uuid,
    role: UserRole,
) -> Result<bool, RbacRolePersistenceError> {
    lock_target_authority_on(db, tenant_id, user_id).await?;
    if has_exact_tenant_role_assignment_on(db, tenant_id, user_id, &role).await? {
        return Ok(false);
    }
    ensure_user_authority_continuity_on(
        db,
        tenant_id,
        user_id,
        &RbacUserAuthorityChange {
            role: Some(role.clone()),
            ..Default::default()
        },
    )
    .await?;
    crate::RbacRoleAssignmentDbWriter::replace_role_on(db, tenant_id, user_id, role).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_events::{RBAC_EVENT_USER_ROLE_ASSIGNMENT_REPAIRED, RBAC_EVENT_USER_ROLE_REPLACED};

    #[tokio::test]
    async fn persisted_role_mutations_preserve_tenant_and_active_admin_authority() {
        use sea_orm::{ConnectionTrait, Database, TransactionTrait};
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .unwrap();
        // These are the host-owned relation parents. UUID values use the same
        // native SeaORM codec as the live server models.
        for ddl in [
            "CREATE TABLE users (id BLOB PRIMARY KEY, tenant_id BLOB NOT NULL, status TEXT NOT NULL, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE roles (id BLOB PRIMARY KEY, tenant_id BLOB NOT NULL, name TEXT NOT NULL, slug TEXT NOT NULL, description TEXT, is_system BOOLEAN NOT NULL, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, slug))",
            "CREATE TABLE user_roles (id BLOB PRIMARY KEY, user_id BLOB NOT NULL REFERENCES users(id), role_id BLOB NOT NULL REFERENCES roles(id), UNIQUE (user_id, role_id))",
            "CREATE TABLE permissions (id BLOB PRIMARY KEY, tenant_id BLOB NOT NULL, resource TEXT NOT NULL, action TEXT NOT NULL, description TEXT, UNIQUE (tenant_id, resource, action))",
            "CREATE TABLE role_permissions (id BLOB PRIMARY KEY, role_id BLOB NOT NULL REFERENCES roles(id), permission_id BLOB NOT NULL REFERENCES permissions(id), UNIQUE (role_id, permission_id))",
        ] {
            db.execute_unprepared(ddl).await.unwrap();
        }
        let tenant = Uuid::new_v4();
        let other_tenant = Uuid::new_v4();
        let admin = Uuid::new_v4();
        let successor = Uuid::new_v4();
        let foreign_admin = Uuid::new_v4();
        for (id, tenant_id) in [
            (admin, tenant),
            (successor, tenant),
            (foreign_admin, other_tenant),
        ] {
            db.execute_raw(
                role_statement(
                    &db,
                    "INSERT INTO users (id, tenant_id, status) VALUES (?, ?, 'active')",
                    vec![id.into(), tenant_id.into()],
                )
                .unwrap(),
            )
            .await
            .unwrap();
        }
        let tx = db.begin().await.unwrap();
        for (id, tenant_id) in [(admin, tenant), (foreign_admin, other_tenant)] {
            crate::RbacRoleAssignmentDbWriter::assign_role_on(
                &tx,
                tenant_id,
                id,
                UserRole::SuperAdmin,
            )
            .await
            .unwrap();
        }
        tx.commit().await.unwrap();

        let tx = db.begin().await.unwrap();
        assert_eq!(
            count_remaining_active_super_admins_on(&tx, tenant, admin)
                .await
                .unwrap(),
            0
        );
        let request = RbacUserRoleMutationRequest {
            tenant_id: tenant,
            actor_id: admin,
            actor_tenant_id: tenant,
            actor_role: UserRole::SuperAdmin,
            target_user_id: admin,
            requested_role: UserRole::SuperAdmin,
            resulting_status: None,
        };
        assert_eq!(
            plan_persisted_user_role_mutation_on(&tx, request.clone())
                .await
                .unwrap(),
            RbacRoleMutationOutcome::Noop,
        );
        let mut demotion = request.clone();
        demotion.requested_role = UserRole::Customer;
        assert!(matches!(
            plan_persisted_user_role_mutation_on(&tx, demotion).await,
            Err(RbacRolePersistenceError::Policy(
                RbacRoleMutationPolicyError::LastActiveSuperAdmin
            ))
        ));
        let mut escalation = request.clone();
        escalation.actor_id = successor;
        escalation.actor_role = UserRole::Admin;
        assert!(matches!(
            plan_persisted_user_role_mutation_on(&tx, escalation).await,
            Err(RbacRolePersistenceError::Policy(
                RbacRoleMutationPolicyError::CannotAssignPeerOrHigherRole
            ))
        ));
        let mut mismatch = request;
        mismatch.actor_tenant_id = other_tenant;
        assert!(matches!(
            plan_persisted_user_role_mutation_on(&tx, mismatch).await,
            Err(RbacRolePersistenceError::Policy(
                RbacRoleMutationPolicyError::ActorTenantMismatch
            ))
        ));
        assert!(
            !replace_persisted_user_role_on(&tx, tenant, admin, UserRole::SuperAdmin)
                .await
                .unwrap()
        );
        assert!(matches!(
            replace_persisted_user_role_on(&tx, tenant, admin, UserRole::Customer).await,
            Err(RbacRolePersistenceError::Policy(
                RbacRoleMutationPolicyError::LastActiveSuperAdmin
            ))
        ));
        for change in [
            RbacUserAuthorityChange {
                status: Some(UserStatus::Inactive),
                ..Default::default()
            },
            RbacUserAuthorityChange {
                deleting: true,
                ..Default::default()
            },
        ] {
            assert!(matches!(
                ensure_user_authority_continuity_on(&tx, tenant, admin, &change).await,
                Err(RbacRolePersistenceError::Policy(
                    RbacRoleMutationPolicyError::LastActiveSuperAdmin
                ))
            ));
        }
        assert!(matches!(
            replace_persisted_user_role_on(&tx, other_tenant, admin, UserRole::Customer).await,
            Err(RbacRolePersistenceError::TargetNotFound)
        ));
        tx.rollback().await.unwrap();
        assert!(
            has_exact_tenant_role_assignment_on(&db, tenant, admin, &UserRole::SuperAdmin)
                .await
                .unwrap()
        );

        let tx = db.begin().await.unwrap();
        crate::RbacRoleAssignmentDbWriter::assign_role_on(
            &tx,
            tenant,
            successor,
            UserRole::SuperAdmin,
        )
        .await
        .unwrap();
        assert_eq!(
            count_remaining_active_super_admins_on(&tx, tenant, admin)
                .await
                .unwrap(),
            1
        );
        assert!(
            replace_persisted_user_role_on(&tx, tenant, admin, UserRole::Customer)
                .await
                .unwrap()
        );
        tx.commit().await.unwrap();
        assert!(
            has_exact_tenant_role_assignment_on(&db, tenant, admin, &UserRole::Customer)
                .await
                .unwrap()
        );

        let tx = db.begin().await.unwrap();
        crate::RbacRoleAssignmentDbWriter::assign_role_on(&tx, tenant, admin, UserRole::Manager)
            .await
            .unwrap();
        assert!(
            !has_exact_tenant_role_assignment_on(&tx, tenant, admin, &UserRole::Customer)
                .await
                .unwrap()
        );
        assert!(
            replace_persisted_user_role_on(&tx, tenant, admin, UserRole::Customer)
                .await
                .unwrap()
        );
        tx.commit().await.unwrap();
        assert!(
            has_exact_tenant_role_assignment_on(&db, tenant, admin, &UserRole::Customer)
                .await
                .unwrap()
        );

        let tx = db.begin().await.unwrap();
        db_status_update(&tx, successor, "inactive").await;
        // Removing an inactive administrator cannot reduce the active set.
        ensure_user_authority_continuity_on(
            &tx,
            tenant,
            successor,
            &RbacUserAuthorityChange {
                deleting: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        assert!(
            has_exact_tenant_role_assignment_on(
                &db,
                other_tenant,
                foreign_admin,
                &UserRole::SuperAdmin
            )
            .await
            .unwrap()
        );
    }

    async fn db_status_update(db: &sea_orm::DatabaseTransaction, user: Uuid, status: &str) {
        use sea_orm::ConnectionTrait;
        db.execute_raw(
            role_statement(
                db,
                "UPDATE users SET status = ? WHERE id = ?",
                vec![status.into(), user.into()],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    }

    fn facts() -> RbacRoleMutationFacts {
        let tenant_id = Uuid::new_v4();
        RbacRoleMutationFacts {
            tenant_id,
            actor_id: Uuid::new_v4(),
            actor_tenant_id: tenant_id,
            actor_role: UserRole::Admin,
            target_user_id: Uuid::new_v4(),
            target_tenant_id: tenant_id,
            target_role: UserRole::Customer,
            target_status: UserStatus::Active,
            requested_role: UserRole::Manager,
            resulting_status: UserStatus::Active,
            assignment_is_exact: false,
            remaining_active_super_admins: 0,
        }
    }

    #[test]
    fn approved_replacement_builds_typed_event() {
        let plan = match plan_user_role_mutation(facts()).unwrap() {
            RbacRoleMutationOutcome::Apply(plan) => plan,
            RbacRoleMutationOutcome::Noop => panic!("replacement must apply"),
        };

        assert_eq!(plan.change(), RbacRoleMutationChange::RoleReplaced);
        assert_eq!(
            plan.integration_event(4).unwrap().event_type(),
            RBAC_EVENT_USER_ROLE_REPLACED
        );
    }

    #[test]
    fn exact_same_role_is_noop_but_malformed_same_role_is_repair() {
        let mut exact = facts();
        exact.target_role = UserRole::Manager;
        exact.assignment_is_exact = true;
        assert_eq!(
            plan_user_role_mutation(exact).unwrap(),
            RbacRoleMutationOutcome::Noop
        );

        let mut malformed = facts();
        malformed.target_role = UserRole::Manager;
        let plan = match plan_user_role_mutation(malformed).unwrap() {
            RbacRoleMutationOutcome::Apply(plan) => plan,
            RbacRoleMutationOutcome::Noop => panic!("malformed assignment must repair"),
        };
        assert_eq!(plan.change(), RbacRoleMutationChange::AssignmentRepaired);
        assert_eq!(
            plan.integration_event(5).unwrap().event_type(),
            RBAC_EVENT_USER_ROLE_ASSIGNMENT_REPAIRED
        );
    }

    #[test]
    fn hierarchy_and_tenant_scope_fail_closed() {
        assert!(require_request_role_grant(&UserRole::Admin, &[], &UserRole::Customer).is_ok());
        assert!(matches!(
            require_request_role_grant(
                &UserRole::Admin,
                &[rustok_api::Permission::USERS_MANAGE],
                &UserRole::Manager,
            ),
            Err(RbacRoleMutationPolicyError::RequestAuthorityCeiling { .. })
        ));
        let mut peer = facts();
        peer.requested_role = UserRole::Admin;
        assert_eq!(
            plan_user_role_mutation(peer).unwrap_err(),
            RbacRoleMutationPolicyError::CannotAssignPeerOrHigherRole
        );

        let mut foreign = facts();
        foreign.target_tenant_id = Uuid::new_v4();
        assert_eq!(
            plan_user_role_mutation(foreign).unwrap_err(),
            RbacRoleMutationPolicyError::TargetTenantMismatch
        );
    }

    #[test]
    fn last_active_super_admin_removal_is_rejected() {
        let tenant_id = Uuid::new_v4();
        let error = plan_user_role_mutation(RbacRoleMutationFacts {
            tenant_id,
            actor_id: Uuid::new_v4(),
            actor_tenant_id: tenant_id,
            actor_role: UserRole::SuperAdmin,
            target_user_id: Uuid::new_v4(),
            target_tenant_id: tenant_id,
            target_role: UserRole::SuperAdmin,
            target_status: UserStatus::Active,
            requested_role: UserRole::Admin,
            resulting_status: UserStatus::Active,
            assignment_is_exact: false,
            remaining_active_super_admins: 0,
        })
        .unwrap_err();

        assert_eq!(error, RbacRoleMutationPolicyError::LastActiveSuperAdmin);
    }

    #[test]
    fn self_demotion_still_requires_assignable_target_role() {
        let mut self_demotion = facts();
        self_demotion.target_user_id = self_demotion.actor_id;
        self_demotion.target_role = UserRole::Admin;
        self_demotion.requested_role = UserRole::Manager;
        assert!(matches!(
            plan_user_role_mutation(self_demotion).unwrap(),
            RbacRoleMutationOutcome::Apply(_)
        ));
    }
}
