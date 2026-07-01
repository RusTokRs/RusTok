use uuid::Uuid;

use rustok_core::{PermissionScope, SecurityContext, UserRole};

use rustok_api::{Action, Resource};

use crate::error::{BlogError, BlogResult};

pub(crate) fn enforce_scope(
    security: &SecurityContext,
    resource: Resource,
    action: Action,
) -> BlogResult<()> {
    if matches!(security.get_scope(resource, action), PermissionScope::None) {
        return Err(BlogError::forbidden("Permission denied"));
    }
    Ok(())
}

pub(crate) fn enforce_owned_scope(
    security: &SecurityContext,
    resource: Resource,
    action: Action,
    owner_id: Uuid,
) -> BlogResult<()> {
    match security.get_scope(resource, action) {
        PermissionScope::All => Ok(()),
        PermissionScope::Own if security.user_id == Some(owner_id) => Ok(()),
        PermissionScope::Own | PermissionScope::None => {
            Err(BlogError::forbidden("Permission denied"))
        }
    }
}

pub(crate) fn enforce_create_author(
    security: &SecurityContext,
    resource: Resource,
    action: Action,
) -> BlogResult<Uuid> {
    match security.get_scope(resource, action) {
        PermissionScope::All | PermissionScope::Own => {
            security.user_id.ok_or(BlogError::AuthorRequired)
        }
        PermissionScope::None => Err(BlogError::forbidden("Permission denied")),
    }
}

pub(crate) fn can_read_non_public_posts(security: &SecurityContext) -> bool {
    !matches!(security.role, UserRole::Customer)
}
