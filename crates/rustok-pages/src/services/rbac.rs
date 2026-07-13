use uuid::Uuid;

use rustok_core::{PermissionScope, SecurityContext};

use rustok_api::{Action, Resource};

use crate::error::{PagesError, PagesResult};

pub(crate) fn enforce_scope(
    security: &SecurityContext,
    resource: Resource,
    action: Action,
) -> PagesResult<()> {
    if matches!(security.get_scope(resource, action), PermissionScope::None) {
        return Err(PagesError::forbidden("Permission denied"));
    }
    Ok(())
}

pub(crate) fn enforce_owned_scope(
    security: &SecurityContext,
    resource: Resource,
    action: Action,
    owner_id: Option<Uuid>,
) -> PagesResult<()> {
    match security.get_scope(resource, action) {
        PermissionScope::All => Ok(()),
        PermissionScope::Own if owner_id.is_some() && security.user_id == owner_id => Ok(()),
        PermissionScope::Own | PermissionScope::None => {
            Err(PagesError::forbidden("Permission denied"))
        }
    }
}

/// Non-public page reads require tenant-wide read authority.
///
/// Role names are intentionally ignored: OAuth scopes and request-effective
/// permission snapshots can reduce an administrator to `None`, and a static
/// role check must never restore access removed by that snapshot.
pub(crate) fn can_read_non_public_pages(security: &SecurityContext) -> bool {
    matches!(
        security.get_scope(Resource::Pages, Action::Read),
        PermissionScope::All
    )
}

#[cfg(test)]
mod tests {
    use super::can_read_non_public_pages;
    use rustok_api::Permission;
    use rustok_core::{SecurityContext, UserRole};

    #[test]
    fn role_name_cannot_restore_non_public_page_access() {
        let restricted = SecurityContext::from_permissions(
            UserRole::Admin,
            Some(uuid::Uuid::new_v4()),
            Vec::<Permission>::new(),
        );
        assert!(!can_read_non_public_pages(&restricted));

        let tenant_wide = SecurityContext::from_permissions(
            UserRole::Manager,
            Some(uuid::Uuid::new_v4()),
            [Permission::PAGES_READ],
        );
        assert!(can_read_non_public_pages(&tenant_wide));
    }
}