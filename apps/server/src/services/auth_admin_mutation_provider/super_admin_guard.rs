use rustok_auth::AuthAdminMutationError;
use rustok_core::{UserRole, UserStatus};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect,
    sea_query::Expr,
};
use uuid::Uuid;

use crate::models::{
    _entities::{roles, user_roles},
    users,
};

pub(super) async fn ensure_active_super_admin_continuity<C>(
    db: &C,
    tenant_id: Uuid,
    target_user_id: Uuid,
    current_role: &UserRole,
    resulting_role: Option<&UserRole>,
    resulting_status: Option<&UserStatus>,
    deleting: bool,
) -> Result<(), AuthAdminMutationError>
where
    C: ConnectionTrait,
{
    if current_role != &UserRole::SuperAdmin {
        return Ok(());
    }

    let remains_active_super_admin = !deleting
        && resulting_role.is_none_or(|role| role == &UserRole::SuperAdmin)
        && resulting_status.is_none_or(|status| status == &UserStatus::Active);
    if remains_active_super_admin {
        return Ok(());
    }

    let super_admin_role = lock_super_admin_role(db, tenant_id).await?.ok_or_else(|| {
        AuthAdminMutationError::Conflict(
            "active super administrator role assignment is inconsistent".to_string(),
        )
    })?;

    let super_admin_user_ids = user_roles::Entity::find()
        .select_only()
        .column(user_roles::Column::UserId)
        .filter(user_roles::Column::RoleId.eq(super_admin_role.id))
        .into_tuple::<Uuid>()
        .all(db)
        .await
        .map_err(internal)?;

    let remaining_active = users::Entity::find()
        .filter(users::Column::TenantId.eq(tenant_id))
        .filter(users::Column::Id.is_in(super_admin_user_ids))
        .filter(users::Column::Id.ne(target_user_id))
        .filter(users::Column::Status.eq(UserStatus::Active))
        .count(db)
        .await
        .map_err(internal)?;

    if remaining_active == 0 {
        return Err(AuthAdminMutationError::Conflict(
            "cannot remove, demote, or deactivate the last active super administrator".to_string(),
        ));
    }

    Ok(())
}

async fn lock_super_admin_role<C>(
    db: &C,
    tenant_id: Uuid,
) -> Result<Option<roles::Model>, AuthAdminMutationError>
where
    C: ConnectionTrait,
{
    let role_query = || {
        roles::Entity::find()
            .filter(roles::Column::TenantId.eq(tenant_id))
            .filter(roles::Column::Slug.eq(UserRole::SuperAdmin.to_string()))
    };

    match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => role_query()
            .lock_exclusive()
            .one(db)
            .await
            .map_err(internal),
        DbBackend::Sqlite => {
            let role = role_query().one(db).await.map_err(internal)?;
            if let Some(role) = role.as_ref() {
                // SQLite has no SELECT ... FOR UPDATE. A no-op write obtains the
                // database write lock before the continuity count is read.
                roles::Entity::update_many()
                    .col_expr(
                        roles::Column::UpdatedAt,
                        Expr::col(roles::Column::UpdatedAt).into(),
                    )
                    .filter(roles::Column::Id.eq(role.id))
                    .exec(db)
                    .await
                    .map_err(internal)?;
            }
            Ok(role)
        }
    }
}

fn internal(error: sea_orm::DbErr) -> AuthAdminMutationError {
    AuthAdminMutationError::Internal(error.to_string())
}
