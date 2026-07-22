use std::collections::HashSet;
use std::str::FromStr;
use uuid::Uuid;

use once_cell::sync::Lazy;

use crate::types::UserRole;
use rustok_api::{Action, Permission, PortActorKind, PortContext, PortError, Resource};

// Pre-computed permission sets (lazy initialized, zero allocation on lookups)
static SUPER_ADMIN_PERMISSIONS: Lazy<HashSet<Permission>> = Lazy::new(|| {
    [
        Resource::Users,
        Resource::Tenants,
        Resource::Modules,
        Resource::Settings,
        Resource::FlexSchemas,
        Resource::FlexEntries,
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
        Resource::Seo,
        Resource::Comments,
        Resource::Taxonomy,
        Resource::Analytics,
        Resource::Logs,
        Resource::Webhooks,
        Resource::Scripts,
        Resource::Mcp,
        Resource::AiProviders,
        Resource::AiTaskProfiles,
        Resource::AiSessions,
        Resource::AiRuns,
        Resource::AiApprovals,
        Resource::AiRouter,
        Resource::AiTextTasks,
        Resource::AiImageTasks,
        Resource::AiCodeTasks,
        Resource::AiAlloyTasks,
        Resource::AiMultimodalTasks,
        Resource::BlogPosts,
        Resource::Tags,
        Resource::ForumCategories,
        Resource::ForumTopics,
        Resource::ForumReplies,
        Resource::Workflows,
        Resource::WorkflowExecutions,
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
        Resource::Seo,
        Resource::Comments,
        Resource::Taxonomy,
        Resource::Analytics,
        Resource::Webhooks,
    ];

    for resource in managed_resources {
        permissions.insert(Permission::new(resource, Action::Manage));
    }

    permissions.insert(Permission::new(Resource::Modules, Action::Read));
    permissions.insert(Permission::new(Resource::Modules, Action::List));
    permissions.insert(Permission::new(Resource::Scripts, Action::Manage));
    permissions.insert(Permission::new(Resource::Mcp, Action::Manage));
    permissions.insert(Permission::new(Resource::AiProviders, Action::Manage));
    permissions.insert(Permission::new(Resource::AiTaskProfiles, Action::Manage));
    permissions.insert(Permission::new(Resource::AiSessions, Action::Manage));
    permissions.insert(Permission::new(Resource::AiRuns, Action::Manage));
    permissions.insert(Permission::new(Resource::AiApprovals, Action::Manage));
    permissions.insert(Permission::new(Resource::AiRouter, Action::Manage));
    permissions.insert(Permission::new(Resource::AiTextTasks, Action::Manage));
    permissions.insert(Permission::new(Resource::AiImageTasks, Action::Manage));
    permissions.insert(Permission::new(Resource::AiCodeTasks, Action::Manage));
    permissions.insert(Permission::new(Resource::AiAlloyTasks, Action::Manage));
    permissions.insert(Permission::new(Resource::AiMultimodalTasks, Action::Manage));
    permissions.insert(Permission::new(Resource::Logs, Action::Read));
    permissions.insert(Permission::new(Resource::Logs, Action::List));
    permissions.insert(Permission::FLEX_SCHEMAS_CREATE);
    permissions.insert(Permission::FLEX_SCHEMAS_READ);
    permissions.insert(Permission::FLEX_SCHEMAS_UPDATE);
    permissions.insert(Permission::FLEX_SCHEMAS_LIST);

    permissions.insert(Permission::new(Resource::BlogPosts, Action::Manage));
    permissions.insert(Permission::new(Resource::Tags, Action::Manage));
    permissions.insert(Permission::new(Resource::ForumCategories, Action::Manage));
    permissions.insert(Permission::new(Resource::ForumTopics, Action::Manage));
    permissions.insert(Permission::new(Resource::ForumReplies, Action::Manage));
    permissions.insert(Permission::new(Resource::Workflows, Action::Manage));
    permissions.insert(Permission::new(
        Resource::WorkflowExecutions,
        Action::Manage,
    ));

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
    permissions.insert(Permission::TAXONOMY_CREATE);
    permissions.insert(Permission::TAXONOMY_READ);
    permissions.insert(Permission::TAXONOMY_UPDATE);
    permissions.insert(Permission::TAXONOMY_DELETE);
    permissions.insert(Permission::TAXONOMY_LIST);

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
    permissions.insert(Permission::SEO_READ);
    permissions.insert(Permission::SEO_UPDATE);
    permissions.insert(Permission::SEO_PUBLISH);
    permissions.insert(Permission::SEO_GENERATE);
    permissions.insert(Permission::AI_PROVIDERS_READ);
    permissions.insert(Permission::AI_TASK_PROFILES_READ);
    permissions.insert(Permission::AI_SESSIONS_READ);
    permissions.insert(Permission::AI_SESSIONS_RUN);
    permissions.insert(Permission::AI_RUNS_CANCEL);
    permissions.insert(Permission::AI_APPROVALS_RESOLVE);
    permissions.insert(Permission::AI_TASKS_TEXT_RUN);
    permissions.insert(Permission::AI_TASKS_IMAGE_RUN);
    permissions.insert(Permission::AI_TASKS_CODE_RUN);
    permissions.insert(Permission::AI_TASKS_ALLOY_RUN);
    permissions.insert(Permission::AI_TASKS_MULTIMODAL_RUN);

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
    permissions.insert(Permission::TAXONOMY_READ);
    permissions.insert(Permission::TAXONOMY_LIST);

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

fn role_matches_permissions(role: UserRole, permissions: &[Permission]) -> bool {
    Rbac::permissions_for_role(&role)
        .iter()
        .all(|permission| permissions.contains(permission))
}

pub fn infer_user_role_from_permissions(permissions: &[Permission]) -> UserRole {
    for role in [UserRole::SuperAdmin, UserRole::Admin, UserRole::Manager] {
        if role_matches_permissions(role.clone(), permissions) {
            return role;
        }
    }

    UserRole::Customer
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PermissionScope {
    All,
    Own,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SecurityActorKind {
    System,
    User,
    Service,
    Public,
}

#[derive(Debug, Clone)]
pub struct SecurityContext {
    pub actor_kind: SecurityActorKind,
    pub role: UserRole,
    pub user_id: Option<Uuid>,
    permissions: HashSet<Permission>,
}

impl SecurityContext {
    pub fn new(role: UserRole, user_id: Option<Uuid>) -> Self {
        Self {
            actor_kind: SecurityActorKind::User,
            permissions: Rbac::permissions_for_role(&role).iter().copied().collect(),
            role,
            user_id,
        }
    }

    pub fn from_permissions(
        role: UserRole,
        user_id: Option<Uuid>,
        permissions: impl IntoIterator<Item = Permission>,
    ) -> Self {
        Self {
            actor_kind: SecurityActorKind::User,
            role,
            user_id,
            permissions: permissions.into_iter().collect(),
        }
    }

    pub fn service(role: UserRole, permissions: impl IntoIterator<Item = Permission>) -> Self {
        Self {
            actor_kind: SecurityActorKind::Service,
            role,
            user_id: None,
            permissions: permissions.into_iter().collect(),
        }
    }

    pub fn from_permission_snapshot(user_id: Option<Uuid>, permissions: &[Permission]) -> Self {
        Self::from_permissions(
            infer_user_role_from_permissions(permissions),
            user_id,
            permissions.iter().copied(),
        )
    }

    pub fn try_from_port_context(context: &PortContext) -> Result<Self, PortError> {
        if context.actor.kind == PortActorKind::System {
            return Ok(Self::system());
        }

        let actor_id = Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
            PortError::validation(
                "port.actor_id_invalid",
                "user and service actors require a UUID actor id",
            )
        })?;

        if context.claims.is_empty() {
            return Err(PortError::forbidden(
                "port.permission_claims_required",
                "user and service actors require permission claims",
            ));
        }

        let permissions = context
            .claims
            .iter()
            .map(|claim| {
                Permission::from_str(claim).map_err(|_| {
                    PortError::validation(
                        "port.permission_claim_invalid",
                        format!("invalid permission claim: {claim}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let role = match context.roles.as_slice() {
            [] => {
                return Err(PortError::forbidden(
                    "port.role_required",
                    "user and service actors require a role claim",
                ));
            }
            [role] => UserRole::from_str(role).map_err(|_| {
                PortError::validation("port.role_invalid", format!("invalid role claim: {role}"))
            })?,
            _ => {
                return Err(PortError::validation(
                    "port.role_ambiguous",
                    "at most one role claim is allowed",
                ));
            }
        };

        let mut security = Self::from_permissions(
            role,
            (context.actor.kind == PortActorKind::User).then_some(actor_id),
            permissions,
        );
        if context.actor.kind == PortActorKind::Service {
            security.actor_kind = SecurityActorKind::Service;
        }

        Ok(security)
    }

    pub fn get_scope(&self, resource: Resource, action: Action) -> PermissionScope {
        permission_scope_for_set(
            &self.permissions,
            &Permission::new(resource, action),
            self.role.clone(),
        )
    }

    pub fn permissions(&self) -> &HashSet<Permission> {
        &self.permissions
    }

    pub fn public_read() -> Self {
        let permissions = [
            Permission::POSTS_READ,
            Permission::POSTS_LIST,
            Permission::BLOG_POSTS_READ,
            Permission::BLOG_POSTS_LIST,
            Permission::PAGES_READ,
            Permission::PAGES_LIST,
            Permission::NODES_READ,
            Permission::NODES_LIST,
            Permission::TAXONOMY_READ,
            Permission::TAXONOMY_LIST,
            Permission::new(Resource::ForumCategories, Action::Read),
            Permission::new(Resource::ForumCategories, Action::List),
            Permission::new(Resource::ForumTopics, Action::Read),
            Permission::new(Resource::ForumTopics, Action::List),
            Permission::new(Resource::ForumReplies, Action::Read),
            Permission::new(Resource::ForumReplies, Action::List),
        ];

        Self {
            actor_kind: SecurityActorKind::Public,
            role: UserRole::Customer,
            user_id: None,
            permissions: permissions.into_iter().collect(),
        }
    }

    pub fn is_public_read(&self) -> bool {
        self.actor_kind == SecurityActorKind::Public
    }

    /// Trusted platform runtime authority only. Public/storefront reads must use
    /// `SecurityContext::public_read()` instead of system authority.
    pub fn system() -> Self {
        Self {
            actor_kind: SecurityActorKind::System,
            role: UserRole::SuperAdmin,
            user_id: None,
            permissions: Rbac::permissions_for_role(&UserRole::SuperAdmin)
                .iter()
                .copied()
                .collect(),
        }
    }
}

fn has_effective_permission_in_set(
    permissions: &HashSet<Permission>,
    permission: &Permission,
) -> bool {
    permissions.contains(permission)
        || permissions.contains(&Permission::new(permission.resource, Action::Manage))
}

fn permission_scope_for_set(
    permissions: &HashSet<Permission>,
    permission: &Permission,
    role: UserRole,
) -> PermissionScope {
    if matches!(
        role,
        UserRole::SuperAdmin | UserRole::Admin | UserRole::Manager
    ) && has_effective_permission_in_set(permissions, permission)
    {
        return PermissionScope::All;
    }

    if matches!(role, UserRole::Customer) {
        if permission.resource == Resource::Orders
            && has_effective_permission_in_set(permissions, permission)
        {
            return PermissionScope::Own;
        }

        if permission.resource == Resource::Comments {
            if matches!(permission.action, Action::Update | Action::Delete) {
                return PermissionScope::Own;
            }
            if has_effective_permission_in_set(permissions, permission) {
                return PermissionScope::All;
            }
        }

        if has_effective_permission_in_set(permissions, permission) {
            return PermissionScope::All;
        }
    }

    PermissionScope::None
}

impl Rbac {
    pub fn get_scope(role: &UserRole, permission: &Permission) -> PermissionScope {
        permission_scope_for_set(Self::permissions_for_role(role), permission, role.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn super_admin_can_manage_workflow_module_surface() {
        assert!(Rbac::has_permission(
            &UserRole::SuperAdmin,
            &Permission::WORKFLOWS_LIST
        ));
        assert!(Rbac::has_permission(
            &UserRole::SuperAdmin,
            &Permission::WORKFLOW_EXECUTIONS_LIST
        ));
    }

    #[test]
    fn admin_can_manage_workflow_module_surface() {
        assert!(Rbac::has_permission(
            &UserRole::Admin,
            &Permission::WORKFLOWS_LIST
        ));
        assert!(Rbac::has_permission(
            &UserRole::Admin,
            &Permission::WORKFLOW_EXECUTIONS_LIST
        ));
    }

    #[test]
    fn port_context_rejects_untrusted_actor_without_permissions() {
        let context = PortContext::new(
            Uuid::new_v4().to_string(),
            rustok_api::PortActor::user(Uuid::new_v4().to_string()),
            "en",
            "corr",
        );

        assert_eq!(
            SecurityContext::try_from_port_context(&context)
                .unwrap_err()
                .code,
            "port.permission_claims_required"
        );
    }

    #[test]
    fn port_context_system_actor_creates_system_authority() {
        let context = PortContext::new(
            Uuid::new_v4().to_string(),
            rustok_api::PortActor::system(),
            "en",
            "corr",
        );

        let security = SecurityContext::try_from_port_context(&context).unwrap();
        assert_eq!(security.actor_kind, SecurityActorKind::System);
        assert_eq!(security.role, UserRole::SuperAdmin);
    }

    #[test]
    fn port_context_preserves_explicit_permission_snapshot() {
        let user_id = Uuid::new_v4();
        let context = PortContext::new(
            Uuid::new_v4().to_string(),
            rustok_api::PortActor::user(user_id.to_string()),
            "en",
            "corr",
        )
        .with_claim(Permission::PRODUCTS_READ.to_string())
        .with_role("customer");

        let security = SecurityContext::try_from_port_context(&context).unwrap();
        assert_eq!(security.actor_kind, SecurityActorKind::User);
        assert_eq!(security.user_id, Some(user_id));
        assert_eq!(security.role, UserRole::Customer);
        assert!(security.permissions().contains(&Permission::PRODUCTS_READ));
    }

    #[test]
    fn permission_snapshot_creates_user_actor() {
        let user_id = Uuid::new_v4();
        let security = SecurityContext::from_permission_snapshot(
            Some(user_id),
            &[Permission::BLOG_POSTS_READ, Permission::BLOG_POSTS_LIST],
        );

        assert_eq!(security.actor_kind, SecurityActorKind::User);
        assert_eq!(security.user_id, Some(user_id));
        assert!(
            security
                .permissions()
                .contains(&Permission::BLOG_POSTS_READ)
        );
    }

    #[test]
    fn public_read_has_limited_anonymous_read_authority() {
        let security = SecurityContext::public_read();

        assert_eq!(security.actor_kind, SecurityActorKind::Public);
        assert!(security.is_public_read());
        assert_eq!(security.user_id, None);
        assert_eq!(security.role, UserRole::Customer);
        assert!(
            security
                .permissions()
                .contains(&Permission::BLOG_POSTS_READ)
        );
        assert!(security.permissions().contains(&Permission::PAGES_LIST));
        assert!(
            security
                .permissions()
                .contains(&Permission::new(Resource::ForumTopics, Action::Read,))
        );
        assert!(
            !security
                .permissions()
                .contains(&Permission::new(Resource::BlogPosts, Action::Manage))
        );
        assert!(
            !security
                .permissions()
                .contains(&Permission::BLOG_POSTS_UPDATE)
        );
    }

    #[test]
    fn system_context_remains_trusted_runtime_authority() {
        let security = SecurityContext::system();

        assert_eq!(security.actor_kind, SecurityActorKind::System);
        assert_eq!(security.user_id, None);
        assert_eq!(security.role, UserRole::SuperAdmin);
        assert!(
            security
                .permissions()
                .contains(&Permission::new(Resource::BlogPosts, Action::Manage))
        );
    }

    #[test]
    fn port_context_rejects_actor_without_role_claim() {
        let context = PortContext::new(
            Uuid::new_v4().to_string(),
            rustok_api::PortActor::service(Uuid::new_v4().to_string()),
            "en",
            "corr",
        )
        .with_claim(Permission::PRODUCTS_READ.to_string());

        assert_eq!(
            SecurityContext::try_from_port_context(&context)
                .unwrap_err()
                .code,
            "port.role_required"
        );
    }
}
