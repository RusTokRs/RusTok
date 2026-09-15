use rustok_api::{Permission, RuntimeLocale};
use rustok_core::UserRole;
use sea_orm::ConnectionTrait;
use uuid::Uuid;

use crate::catalog::{permission_display_name, role_display_name};
use crate::presentation::{
    RbacPresentationResourceKind, RbacPresentationStoreError, SeaOrmRbacPresentationStore,
};

const BUILTIN_SOURCE_LOCALE: &str = "en";

pub(crate) async fn seed_builtin_role_presentation_on<C>(
    db: &C,
    tenant_id: Uuid,
    role: &UserRole,
) -> Result<(), RbacPresentationStoreError>
where
    C: ConnectionTrait,
{
    insert_source_if_missing(
        db,
        tenant_id,
        RbacPresentationResourceKind::Role,
        &role.to_string(),
        role_display_name(role).to_string(),
    )
    .await
}

pub(crate) async fn seed_builtin_permission_presentation_on<C>(
    db: &C,
    tenant_id: Uuid,
    permission: &Permission,
) -> Result<(), RbacPresentationStoreError>
where
    C: ConnectionTrait,
{
    let resource_key = permission.to_string();
    let display_name = permission_display_name(&resource_key);
    insert_source_if_missing(
        db,
        tenant_id,
        RbacPresentationResourceKind::Permission,
        &resource_key,
        display_name,
    )
    .await
}

async fn insert_source_if_missing<C>(
    db: &C,
    tenant_id: Uuid,
    resource_kind: RbacPresentationResourceKind,
    resource_key: &str,
    name: String,
) -> Result<(), RbacPresentationStoreError>
where
    C: ConnectionTrait,
{
    let source_locale = RuntimeLocale::new(BUILTIN_SOURCE_LOCALE)
        .expect("RBAC built-in source locale must remain concrete");

    match SeaOrmRbacPresentationStore::create_source_on(
        db,
        tenant_id,
        resource_kind,
        resource_key,
        source_locale,
        name,
        None,
    )
    .await
    {
        Ok(_) | Err(RbacPresentationStoreError::AlreadyExists) => Ok(()),
        Err(error) => Err(error),
    }
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
