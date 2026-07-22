use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use crate::error::{NavigationError, NavigationResult};

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
