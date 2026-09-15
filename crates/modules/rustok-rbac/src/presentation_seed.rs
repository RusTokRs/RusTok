use chrono::Utc;
use rustok_api::{Permission, RuntimeLocale};
use rustok_core::UserRole;
use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement};
use uuid::Uuid;

use crate::catalog::{permission_display_name, role_display_name};

const ROLE_KIND: &str = "role";
const PERMISSION_KIND: &str = "permission";
const BUILTIN_SOURCE_LOCALE: &str = "en";

pub(crate) async fn seed_builtin_role_presentation_on<C>(
    db: &C,
    tenant_id: Uuid,
    role: &UserRole,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    insert_source_if_missing(
        db,
        tenant_id,
        ROLE_KIND,
        &role.to_string(),
        role_display_name(role),
    )
    .await
}

pub(crate) async fn seed_builtin_permission_presentation_on<C>(
    db: &C,
    tenant_id: Uuid,
    permission: &Permission,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let resource_key = permission.to_string();
    let display_name = permission_display_name(&resource_key);
    insert_source_if_missing(
        db,
        tenant_id,
        PERMISSION_KIND,
        &resource_key,
        &display_name,
    )
    .await
}

async fn insert_source_if_missing<C>(
    db: &C,
    tenant_id: Uuid,
    resource_kind: &'static str,
    resource_key: &str,
    name: &str,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let source_locale = RuntimeLocale::new(BUILTIN_SOURCE_LOCALE)
        .expect("RBAC built-in source locale must remain concrete");
    let now = Utc::now().fixed_offset();
    let sql = match db.get_database_backend() {
        DbBackend::Postgres => {
            "INSERT INTO rbac_localized_presentations (tenant_id, resource_kind, resource_key, locale, name, description, copy_revision, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, NULL, 1, $6, $6) ON CONFLICT (tenant_id, resource_kind, resource_key, locale) DO NOTHING"
        }
        DbBackend::Sqlite => {
            "INSERT INTO rbac_localized_presentations (tenant_id, resource_kind, resource_key, locale, name, description, copy_revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, NULL, 1, ?6, ?6) ON CONFLICT (tenant_id, resource_kind, resource_key, locale) DO NOTHING"
        }
        backend => {
            return Err(DbErr::Custom(format!(
                "RBAC built-in presentation seeding does not support database backend {backend:?}"
            )));
        }
    };

    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        sql,
        vec![
            tenant_id.into(),
            resource_kind.into(),
            resource_key.into(),
            source_locale.as_str().into(),
            name.into(),
            now.into(),
        ],
    ))
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::BUILTIN_SOURCE_LOCALE;
    use rustok_api::RuntimeLocale;

    #[test]
    fn built_in_source_locale_is_concrete() {
        assert!(RuntimeLocale::new(BUILTIN_SOURCE_LOCALE).is_ok());
        assert!(RuntimeLocale::new("und").is_err());
    }
}
