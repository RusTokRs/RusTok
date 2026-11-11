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

/// Page actions that author pages. `pages:manage` implies each of them.
const PAGE_AUTHORING_ACTIONS: [Action; 4] = [
    Action::Create,
    Action::Update,
    Action::Delete,
    Action::Publish,
];

/// A page that is not published is authoring state, so its record is readable only by a
/// principal that holds a page authoring permission (create, update, delete or publish).
///
/// `pages:read` and `pages:list` do not qualify: public readers and customers hold both, and
/// to them an unpublished page does not exist.
///
/// The role label is not consulted. It is inferred from the permission snapshot for callers
/// without a role claim, so it can neither restore access that the snapshot removed (an
/// administrator reduced by OAuth scopes) nor grant it (the default customer permissions
/// satisfy the staff heuristic).
pub(crate) fn can_read_non_public_pages(security: &SecurityContext) -> bool {
    PAGE_AUTHORING_ACTIONS.into_iter().any(|action| {
        !matches!(
            security.get_scope(Resource::Pages, action),
            PermissionScope::None
        )
    })
}

/// The page source is authoring state: the working copy of a page (its current body, or its
/// draft while the page is published). It is Page Builder input that the publish sanitizer has
/// not processed, so it is not render authority for a storefront.
///
/// A principal may read the source of exactly the pages whose document it may save or
/// publish: the rule is the write rule of those commands ([`enforce_owned_scope`] for
/// `pages:update` and `pages:publish`), applied to the same page author. A publish folds the
/// working copy into the current body, so whoever may publish must be able to review it. Read
/// authority alone, which public readers and customers hold, never grants it; everyone else
/// renders the published landing artifact.
///
/// No role label is tested here. The label is inferred from the permission snapshot for
/// callers without a role claim, so on its own it neither grants the source (the default
/// customer permissions can be labelled as staff) nor withholds it from a principal that holds
/// the permission. It still takes part where the write rule uses it: `get_scope` narrows
/// `pages:update` of a principal labelled non-staff to its own pages, and the source follows
/// that scope so that reading and saving a document never disagree.
pub(crate) fn can_read_page_source(security: &SecurityContext, author_id: Option<Uuid>) -> bool {
    [Action::Update, Action::Publish]
        .into_iter()
        .any(|action| enforce_owned_scope(security, Resource::Pages, action, author_id).is_ok())
}

#[cfg(test)]
mod tests {
    use super::{can_read_non_public_pages, can_read_page_source};
    use rustok_api::{Action, Permission, Resource};
    use rustok_core::{SecurityContext, UserRole};

    #[test]
    fn non_public_pages_require_an_authoring_permission_not_a_role_label() {
        let user = || Some(uuid::Uuid::new_v4());

        // A role label never restores what the permission snapshot removed.
        let restricted =
            SecurityContext::from_permissions(UserRole::Admin, user(), Vec::<Permission>::new());
        assert!(!can_read_non_public_pages(&restricted));

        // Read authority is what public readers and customers hold; a staff label on top of
        // it changes nothing.
        for role in [UserRole::Manager, UserRole::Customer] {
            let reader = SecurityContext::from_permissions(
                role,
                user(),
                [Permission::PAGES_READ, Permission::PAGES_LIST],
            );
            assert!(!can_read_non_public_pages(&reader));
        }
        assert!(!can_read_non_public_pages(&SecurityContext::public_read()));

        for authoring in [
            Permission::PAGES_CREATE,
            Permission::PAGES_UPDATE,
            Permission::PAGES_DELETE,
            Permission::new(Resource::Pages, Action::Publish),
            Permission::new(Resource::Pages, Action::Manage),
        ] {
            // The label is ignored in both directions.
            for role in [UserRole::Manager, UserRole::Customer] {
                let author = SecurityContext::from_permissions(
                    role,
                    user(),
                    [Permission::PAGES_READ, authoring],
                );
                assert!(can_read_non_public_pages(&author), "{authoring:?}");
            }
        }

        assert!(can_read_non_public_pages(&SecurityContext::system()));
    }

    #[test]
    fn the_page_source_follows_the_document_write_rules() {
        let author = Some(uuid::Uuid::new_v4());
        let other = Some(uuid::Uuid::new_v4());

        assert!(!can_read_page_source(
            &SecurityContext::public_read(),
            author
        ));

        let read_only = SecurityContext::from_permissions(
            UserRole::Manager,
            Some(uuid::Uuid::new_v4()),
            [Permission::PAGES_READ],
        );
        assert!(!can_read_page_source(&read_only, author));

        let editor = SecurityContext::from_permissions(
            UserRole::Manager,
            Some(uuid::Uuid::new_v4()),
            [Permission::PAGES_READ, Permission::PAGES_UPDATE],
        );
        assert!(can_read_page_source(&editor, author));

        // Whoever may publish the working copy may review it.
        let publisher = SecurityContext::from_permissions(
            UserRole::Manager,
            Some(uuid::Uuid::new_v4()),
            [
                Permission::PAGES_READ,
                Permission::new(Resource::Pages, Action::Publish),
            ],
        );
        assert!(can_read_page_source(&publisher, author));

        // A role label never restores what the permission snapshot removed.
        let restricted = SecurityContext::from_permissions(
            UserRole::Admin,
            Some(uuid::Uuid::new_v4()),
            Vec::<Permission>::new(),
        );
        assert!(!can_read_page_source(&restricted, author));

        assert!(can_read_page_source(&SecurityContext::system(), author));
        assert!(can_read_page_source(&SecurityContext::system(), None));

        // A non-staff principal holding `pages:update` saves, and therefore reads, only the
        // documents of its own pages.
        let own_pages_editor = SecurityContext::from_permissions(
            UserRole::Customer,
            author,
            [Permission::PAGES_READ, Permission::PAGES_UPDATE],
        );
        assert!(can_read_page_source(&own_pages_editor, author));
        assert!(!can_read_page_source(&own_pages_editor, other));
        assert!(!can_read_page_source(&own_pages_editor, None));
    }

    /// The role label of a request is inferred from its permission snapshot, and the default
    /// customer permission set satisfies the staff heuristic. Authoring state (unpublished
    /// page records and the page source) must therefore follow the permissions, not the
    /// label.
    #[test]
    fn default_customer_permissions_never_grant_authoring_state() {
        let permissions = rustok_core::Rbac::permissions_for_role(&UserRole::Customer)
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let user_id = Some(uuid::Uuid::new_v4());
        let customer = SecurityContext::from_permission_snapshot(user_id, &permissions);
        assert!(!can_read_non_public_pages(&customer));
        assert!(!can_read_page_source(&customer, user_id));
        assert!(!can_read_page_source(&customer, None));
    }
}
