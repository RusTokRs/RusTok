use rustok_api::{
    RuntimeLocale, StoredLocale, TenantRbacCatalog, TenantRbacPermission, TenantRbacRole,
    PLATFORM_FALLBACK_LOCALE, build_locale_candidates,
};
use sea_orm::DatabaseConnection;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    BuiltinTenantRbacCatalog, RbacLocalizedPresentation, RbacPresentationResourceKind,
    RbacPresentationStore, RbacPresentationStoreError, SeaOrmRbacPresentationStore,
};

#[derive(Debug, Error)]
pub enum RbacPresentationCommandError {
    #[error("RBAC presentation name must be non-empty, trimmed, control-free, and at most 255 characters")]
    InvalidName,
    #[error("RBAC presentation copy revision must be positive")]
    InvalidRevision,
    #[error(transparent)]
    Store(#[from] RbacPresentationStoreError),
}

/// Canonical owner service for RBAC human-facing presentation copy.
///
/// Authorization identity stays in stable role slugs and permission keys. This
/// service owns source seeding, locale-aware resolution, and copy-only commands.
#[derive(Clone)]
pub struct RbacPresentationService<S = SeaOrmRbacPresentationStore> {
    store: S,
}

impl RbacPresentationService<SeaOrmRbacPresentationStore> {
    pub fn from_database(db: DatabaseConnection) -> Self {
        Self::new(SeaOrmRbacPresentationStore::new(db))
    }
}

impl<S> RbacPresentationService<S>
where
    S: RbacPresentationStore,
{
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Seed the admitted built-in presentation catalog at the explicit platform
    /// source locale. Existing tenant-authored rows are never overwritten.
    pub async fn ensure_builtin_source_presentations(
        &self,
        tenant_id: Uuid,
    ) -> Result<(), RbacPresentationStoreError> {
        let source_locale = RuntimeLocale::new(PLATFORM_FALLBACK_LOCALE)
            .map_err(|error| RbacPresentationStoreError::Storage(error.to_string()))?;
        let stored_source_locale = StoredLocale::from(source_locale.clone());
        let catalog = BuiltinTenantRbacCatalog;

        for role in catalog.roles(tenant_id) {
            self.ensure_source(
                tenant_id,
                RbacPresentationResourceKind::Role,
                &role.slug,
                &source_locale,
                &stored_source_locale,
                role.display_name,
            )
            .await?;
        }

        for permission in catalog.permissions(tenant_id) {
            self.ensure_source(
                tenant_id,
                RbacPresentationResourceKind::Permission,
                &permission.slug,
                &source_locale,
                &stored_source_locale,
                permission.display_name,
            )
            .await?;
        }

        Ok(())
    }

    async fn ensure_source(
        &self,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        source_locale: &RuntimeLocale,
        stored_source_locale: &StoredLocale,
        name: String,
    ) -> Result<(), RbacPresentationStoreError> {
        if self
            .store
            .find_exact(
                tenant_id,
                resource_kind,
                resource_key,
                stored_source_locale,
            )
            .await?
            .is_some()
        {
            return Ok(());
        }

        match self
            .store
            .create_source(
                tenant_id,
                resource_kind,
                resource_key,
                source_locale.clone(),
                name,
                None,
            )
            .await
        {
            Ok(_) | Err(RbacPresentationStoreError::AlreadyExists) => Ok(()),
            Err(error) => {
                // A concurrent bootstrap may win between the exact read and the
                // insert. Treat that race as success only after observing the
                // canonical owner row.
                if self
                    .store
                    .find_exact(
                        tenant_id,
                        resource_kind,
                        resource_key,
                        stored_source_locale,
                    )
                    .await?
                    .is_some()
                {
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }

    /// Create or compare-and-set presentation copy for one admitted stable
    /// authorization identity. The command never changes the role/permission
    /// identity itself or any grant relation.
    pub async fn write_source(
        &self,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        source_locale: RuntimeLocale,
        expected_copy_revision: Option<i64>,
        name: String,
        description: Option<String>,
    ) -> Result<RbacLocalizedPresentation, RbacPresentationCommandError> {
        validate_name(&name)?;
        if expected_copy_revision.is_some_and(|revision| revision <= 0) {
            return Err(RbacPresentationCommandError::InvalidRevision);
        }

        match expected_copy_revision {
            Some(expected_copy_revision) => self
                .store
                .compare_and_set_source(
                    tenant_id,
                    resource_kind,
                    resource_key,
                    &source_locale,
                    expected_copy_revision,
                    name,
                    description,
                )
                .await
                .map_err(Into::into),
            None => self
                .store
                .create_source(
                    tenant_id,
                    resource_kind,
                    resource_key,
                    source_locale,
                    name,
                    description,
                )
                .await
                .map_err(Into::into),
        }
    }

    pub async fn resolve(
        &self,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        runtime_locale: &RuntimeLocale,
    ) -> Result<RbacLocalizedPresentation, RbacPresentationStoreError> {
        for candidate in build_locale_candidates(
            [
                Some(runtime_locale.as_str()),
                Some(PLATFORM_FALLBACK_LOCALE),
            ],
            true,
        ) {
            let stored_locale = StoredLocale::new(&candidate)
                .map_err(|error| RbacPresentationStoreError::Storage(error.to_string()))?;
            if let Some(presentation) = self
                .store
                .find_exact(tenant_id, resource_kind, resource_key, &stored_locale)
                .await?
            {
                return Ok(presentation);
            }
        }

        Err(RbacPresentationStoreError::NotFound)
    }

    pub async fn roles(
        &self,
        tenant_id: Uuid,
        runtime_locale: &RuntimeLocale,
    ) -> Result<Vec<TenantRbacRole>, RbacPresentationStoreError> {
        let catalog = BuiltinTenantRbacCatalog;
        let mut roles = catalog.roles(tenant_id);
        for role in &mut roles {
            role.display_name = self
                .resolve(
                    tenant_id,
                    RbacPresentationResourceKind::Role,
                    &role.slug,
                    runtime_locale,
                )
                .await?
                .name;
        }
        Ok(roles)
    }

    pub async fn permissions(
        &self,
        tenant_id: Uuid,
        runtime_locale: &RuntimeLocale,
    ) -> Result<Vec<TenantRbacPermission>, RbacPresentationStoreError> {
        let catalog = BuiltinTenantRbacCatalog;
        let mut permissions = catalog.permissions(tenant_id);
        for permission in &mut permissions {
            permission.display_name = self
                .resolve(
                    tenant_id,
                    RbacPresentationResourceKind::Permission,
                    &permission.slug,
                    runtime_locale,
                )
                .await?
                .name;
        }
        Ok(permissions)
    }
}

fn validate_name(name: &str) -> Result<(), RbacPresentationCommandError> {
    if name.is_empty()
        || name.trim() != name
        || name.chars().count() > 255
        || name.chars().any(char::is_control)
    {
        return Err(RbacPresentationCommandError::InvalidName);
    }
    Ok(())
}
