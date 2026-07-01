use uuid::Uuid;

use rustok_core::{PermissionScope, SecurityContext};

use rustok_api::{Action, Resource};

use crate::error::{ForumError, ForumResult};

pub(crate) fn enforce_scope(
    security: &SecurityContext,
    resource: Resource,
    action: Action,
) -> ForumResult<()> {
    if matches!(security.get_scope(resource, action), PermissionScope::None) {
        return Err(ForumError::forbidden("Permission denied"));
    }
    Ok(())
}

pub(crate) fn enforce_owned_scope(
    security: &SecurityContext,
    resource: Resource,
    action: Action,
    owner_id: Option<Uuid>,
) -> ForumResult<()> {
    match security.get_scope(resource, action) {
        PermissionScope::All => Ok(()),
        PermissionScope::Own if owner_id.is_some() && security.user_id == owner_id => Ok(()),
        PermissionScope::Own | PermissionScope::None => {
            Err(ForumError::forbidden("Permission denied"))
        }
    }
}
