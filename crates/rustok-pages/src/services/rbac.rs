use uuid::Uuid;

use rustok_core::{PermissionScope, SecurityContext, UserRole};

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

pub(crate) fn can_read_non_public_pages(security: &SecurityContext) -> bool {
    !matches!(security.role, UserRole::Customer)
}
