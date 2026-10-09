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

/// Author self-deletion is a tenant policy (`allow_user_content_deletion`). It limits only the
/// author path, which is the scope `Own`. Callers with scope `All` (moderators and administrators)
/// delete through their own permissions and are not affected.
pub(crate) fn enforce_author_deletion_policy(
    security: &SecurityContext,
    resource: Resource,
    author_deletion_allowed: bool,
) -> ForumResult<()> {
    match security.get_scope(resource, Action::Delete) {
        PermissionScope::Own if !author_deletion_allowed => Err(ForumError::forbidden(
            "Authors cannot delete their own content in this forum",
        )),
        PermissionScope::All | PermissionScope::Own | PermissionScope::None => Ok(()),
    }
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
