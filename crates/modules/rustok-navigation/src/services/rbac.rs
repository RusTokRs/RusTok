use crate::error::{NavigationError, NavigationResult};
use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};

pub(crate) fn enforce_scope(
    security: &SecurityContext,
    resource: Resource,
    action: Action,
) -> NavigationResult<()> {
    if matches!(security.get_scope(resource, action), PermissionScope::None) {
        return Err(NavigationError::forbidden("Permission denied"));
    }
    Ok(())
}
