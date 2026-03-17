use std::collections::HashSet;
use uuid::Uuid;

use once_cell::sync::Lazy;

use crate::permissions::{Action, Permission, Resource};
use crate::types::UserRole;

// Pre-computed permission sets (lazy initialized, zero allocation on lookups)
static SUPER_ADMIN_PERMISSIONS: Lazy<HashSet<Permission>> = Lazy::new(|| {
    [
        Resource::Users,
        Resource::Tenants,
        Resource::Modules,
        Resource::Settings,
        Resource::Products,
        Resource::Categories,
        Resource::Orders,
        Resource::Customers,
        Resource::Inventory,
        Resource::Discounts,
        Resource::Posts,
        Resource::Pages,
        Resource::Nodes,
        Resource::Media,
        Resource::Comments,
        Resource::Analytics,
        Resource::Logs,
        Resource::Webhooks,
        Resource::Scripts,
        Resource::BlogPosts,
        Resource::Tags,
        Resource::ForumCategories,
        Resource::ForumTopics,
        Resource::ForumReplies,
    ]
    .into_iter()
    .map(|resource| Permission::new(resource, Action::Manage))
    .collect()
});

static ADMIN_PERMISSIONS: Lazy<HashSet<Permission>> = Lazy::new(|| {
    let mut permissions = HashSet::new();

    let managed_resources = [
        Resource::Users,
        Resource::Settings,
        Resource::Products,
        Resource::Categories,
        Resource::Orders,
        Resource::Customers,
        Resource::Inventory,
        Resource::Discounts,
        Resource::Posts,
        Resource::Pages,
        Resource::Nodes,
        Resource::Media,
        Resource::Comments,
        Resource::Analytics,
        Resource::Webhooks,
    ];

    for resource in managed_resources {
        permissions.insert(Permission::new(resource, Action::Manage));
    }

    permissions.insert(Permission::new(Resource::Modules, Action::Read));
    permissions.insert(Permission::new(Resource::Modules, Action::List));
    permissions.insert(Permission::new(Resource::Scripts, Action::Manage));
    permissions.insert(Permission::new(Resource::Logs, Action::Read));
    permissions.insert(Permission::new(Resource::Logs, Action::List));

    permissions.insert(Permission::new(Resource::BlogPosts, Action::Manage));
    permissions.insert(Permission::new(Resource::Tags, Action::Manage));
    permissions.insert(Permission::new(Resource::ForumCategories, Action::Manage));
    permissions.insert(Permission::new(Resource::ForumTopics, Action::Manage));
    permissions.insert(Permission::new(Resource::ForumReplies, Action::Manage));

    permissions
});

static MANAGER_PERMISSIONS: Lazy<HashSet<Permission>> = Lazy::new(|| {
    let mut permissions = HashSet::new();

    permissions.insert(Permission::PRODUCTS_CREATE);
    permissions.insert(Permission::PRODUCTS_READ);
    permissions.insert(Permission::PRODUCTS_UPDATE);
    permissions.insert(Permission::PRODUCTS_DELETE);
    permissions.insert(Permission::PRODUCTS_LIST);

    for action in [
        Action::Create,
        Action::Read,
        Action::Update,
        Action::Delete,
        Action::List,
    ] {
        permissions.insert(Permission::new(Resource::Categories, action));
    }

    permissions.insert(Permission::ORDERS_READ);
    permissions.insert(Permission::ORDERS_UPDATE);
    permissions.insert(Permission::ORDERS_LIST);

    permissions.insert(Permission::new(Resource::Customers, Action::Read));
    permissions.insert(Permission::new(Resource::Customers, Action::List));

    for action in [Action::Create, Action::Read, Action::Update, Action::List] {
        permissions.insert(Permission::new(Resource::Inventory, action));
    }

    permissions.insert(Permission::POSTS_CREATE);
    permissions.insert(Permission::POSTS_READ);
    permissions.insert(Permission::POSTS_UPDATE);
    permissions.insert(Permission::POSTS_DELETE);
    permissions.insert(Permission::POSTS_LIST);

    // Node permissions for managers
    permissions.insert(Permission::NODES_CREATE);
    permissions.insert(Permission::NODES_READ);
    permissions.insert(Permission::NODES_UPDATE);
    permissions.insert(Permission::NODES_DELETE);
    permissions.insert(Permission::NODES_LIST);

    for action in [
        Action::Create,
        Action::Read,
        Action::Update,
        Action::Delete,
        Action::List,
    ] {
        permissions.insert(Permission::new(Resource::Media, action));
    }

    permissions.insert(Permission::ANALYTICS_READ);

    permissions.insert(Permission::PAGES_CREATE);
    permissions.insert(Permission::PAGES_READ);
    permissions.insert(Permission::PAGES_UPDATE);
    permissions.insert(Permission::PAGES_DELETE);
    permissions.insert(Permission::PAGES_LIST);

    permissions.insert(Permission::BLOG_POSTS_CREATE);
    permissions.insert(Permission::BLOG_POSTS_READ);
    permissions.insert(Permission::BLOG_POSTS_UPDATE);
    permissions.insert(Permission::BLOG_POSTS_DELETE);
    permissions.insert(Permission::BLOG_POSTS_LIST);
    permissions.insert(Permission::BLOG_POSTS_PUBLISH);

    for action in [Action::Create, Action::Read, Action::Update, Action::List] {
        permissions.insert(Permission::new(Resource::ForumCategories, action));
    }
    for action in [
        Action::Create,
        Action::Read,
        Action::Update,
        Action::Delete,
        Action::List,
        Action::Moderate,
    ] {
        permissions.insert(Permission::new(Resource::ForumTopics, action));
        permissions.insert(Permission::new(Resource::ForumReplies, action));
    }

    permissions
});

static CUSTOMER_PERMISSIONS: Lazy<HashSet<Permission>> = Lazy::new(|| {
    let mut permissions = HashSet::new();

    permissions.insert(Permission::PRODUCTS_READ);
    permissions.insert(Permission::PRODUCTS_LIST);

    permissions.insert(Permission::new(Resource::Categories, Action::Read));
    permissions.insert(Permission::new(Resource::Categories, Action::List));

    permissions.insert(Permission::ORDERS_READ);
    permissions.insert(Permission::ORDERS_LIST);
    permissions.insert(Permission::ORDERS_CREATE);

    permissions.insert(Permission::POSTS_READ);
    permissions.insert(Permission::POSTS_LIST);

    // Nodes read permissions for customers
    permissions.insert(Permission::NODES_READ);
    permissions.insert(Permission::NODES_LIST);

    permissions.insert(Permission::PAGES_READ);
    permissions.insert(Permission::PAGES_LIST);

    permissions.insert(Permission::new(Resource::Comments, Action::Create));
    permissions.insert(Permission::new(Resource::Comments, Action::Read));
    permissions.insert(Permission::new(Resource::Comments, Action::List));

    permissions.insert(Permission::BLOG_POSTS_READ);
    permissions.insert(Permission::BLOG_POSTS_LIST);

    permissions.insert(Permission::new(Resource::ForumCategories, Action::Read));
    permissions.insert(Permission::new(Resource::ForumCategories, Action::List));
    permissions.insert(Permission::new(Resource::ForumTopics, Action::Read));
    permissions.insert(Permission::new(Resource::ForumTopics, Action::List));
    permissions.insert(Permission::new(Resource::ForumTopics, Action::Create));
    permissions.insert(Permission::new(Resource::ForumReplies, Action::Read));
    permissions.insert(Permission::new(Resource::ForumReplies, Action::List));
    permissions.insert(Permission::new(Resource::ForumReplies, Action::Create));

    permissions.insert(Permission::INVENTORY_READ);
    permissions.insert(Permission::INVENTORY_LIST);

    permissions
});

pub struct Rbac;

impl Rbac {
    pub fn permissions_for_role(role: &UserRole) -> &'static HashSet<Permission> {
        match role {
            UserRole::SuperAdmin => &SUPER_ADMIN_PERMISSIONS,
            UserRole::Admin => &ADMIN_PERMISSIONS,
            UserRole::Manager => &MANAGER_PERMISSIONS,
            UserRole::Customer => &CUSTOMER_PERMISSIONS,
        }
    }

    pub fn has_permission(role: &UserRole, permission: &Permission) -> bool {
        let permissions = Self::permissions_for_role(role);

        if permissions.contains(permission) {
            return true;
        }

        let manage_permission = Permission::new(permission.resource, Action::Manage);
        permissions.contains(&manage_permission)
    }

    pub fn has_any_permission(role: &UserRole, permissions: &[Permission]) -> bool {
        permissions
            .iter()
            .any(|permission| Self::has_permission(role, permission))
    }

    pub fn has_all_permissions(role: &UserRole, permissions: &[Permission]) -> bool {
        permissions
            .iter()
            .all(|permission| Self::has_permission(role, permission))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PermissionScope {
    All,
    Own,
    None,
}

#[derive(Debug, Clone)]
pub struct SecurityContext {
    pub role: UserRole,
    pub user_id: Option<Uuid>,
}

impl SecurityContext {
    pub fn new(role: UserRole, user_id: Option<Uuid>) -> Self {
        Self { role, user_id }
    }

    pub fn get_scope(&self, resource: Resource, action: Action) -> PermissionScope {
        Rbac::get_scope(&self.role, &Permission::new(resource, action))
    }

    pub fn system() -> Self {
        Self {
            role: UserRole::SuperAdmin,
            user_id: None,
        }
    }
}

impl Rbac {
    pub fn get_scope(role: &UserRole, permission: &Permission) -> PermissionScope {
        if matches!(
            role,
            UserRole::SuperAdmin | UserRole::Admin | UserRole::Manager
        ) && Self::has_permission(role, permission)
        {
            return PermissionScope::All;
        }

        if matches!(role, UserRole::Customer) {
            if permission.resource == Resource::Orders && Self::has_permission(role, permission) {
                return PermissionScope::Own;
            }

            if permission.resource == Resource::Comments {
                if matches!(permission.action, Action::Update | Action::Delete) {
                    return PermissionScope::Own;
                }
                if Self::has_permission(role, permission) {
                    return PermissionScope::All;
                }
            }

            if Self::has_permission(role, permission) {
                return PermissionScope::All;
            }
        }

        PermissionScope::None
    }
}
