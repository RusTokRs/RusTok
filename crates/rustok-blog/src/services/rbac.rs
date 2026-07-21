use uuid::Uuid;

use rustok_core::{PermissionScope, SecurityContext};

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

pub(crate) fn enforce_any_scope(
    security: &SecurityContext,
    resources: &[Resource],
    action: Action,
) -> BlogResult<()> {
    if resources
        .iter()
        .all(|resource| matches!(security.get_scope(*resource, action), PermissionScope::None))
    {
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

/// Draft and archived posts require tenant-wide read permission. The canonical
/// role is descriptive only and cannot restore authority removed by OAuth
/// scopes or the request-effective permission snapshot.
pub(crate) fn can_read_non_public_posts(security: &SecurityContext) -> bool {
    matches!(
        security.get_scope(Resource::Posts, Action::Read),
        PermissionScope::All
    )
}

#[cfg(test)]
mod tests {
    use super::{can_read_non_public_posts, enforce_any_scope};
    use rustok_api::{Action, Permission, Resource};
    use rustok_core::{SecurityContext, UserRole};

    #[test]
    fn role_name_cannot_restore_non_public_post_access() {
        let restricted = SecurityContext::from_permissions(
            UserRole::Admin,
            Some(uuid::Uuid::new_v4()),
            Vec::<Permission>::new(),
        );
        assert!(!can_read_non_public_posts(&restricted));

        let tenant_wide = SecurityContext::from_permissions(
            UserRole::Manager,
            Some(uuid::Uuid::new_v4()),
            [Permission::POSTS_READ],
        );
        assert!(can_read_non_public_posts(&tenant_wide));
    }

    #[test]
    fn any_scope_accepts_primary_or_legacy_resource() {
        let actor_id = Some(uuid::Uuid::new_v4());
        let primary = SecurityContext::from_permissions(
            UserRole::Manager,
            actor_id,
            [Permission::BLOG_POSTS_UPDATE],
        );
        enforce_any_scope(
            &primary,
            &[Resource::BlogPosts, Resource::Categories],
            Action::Update,
        )
        .expect("primary Blog permission should authorize category update");

        let legacy = SecurityContext::from_permissions(
            UserRole::Manager,
            actor_id,
            [Permission::new(Resource::Categories, Action::Update)],
        );
        enforce_any_scope(
            &legacy,
            &[Resource::BlogPosts, Resource::Categories],
            Action::Update,
        )
        .expect("legacy category permission should remain accepted during migration");
    }
}
