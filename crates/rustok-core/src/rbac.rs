use std::collections::HashSet;
use std::str::FromStr;

use once_cell::sync::Lazy;
use uuid::Uuid;

use crate::types::UserRole;
use rustok_api::{Action, Permission, PortActorKind, PortContext, PortError, Resource};

fn managed_permissions(resources: impl IntoIterator<Item = Resource>) -> HashSet<Permission> {
    resources
        .into_iter()
        .map(|resource| Permission::new(resource, Action::Manage))
        .collect()
}

fn insert_actions(
    permissions: &mut HashSet<Permission>,
    resource: Resource,
    actions: impl IntoIterator<Item = Action>,
) {
    permissions.extend(
        actions
            .into_iter()
            .map(|action| Permission::new(resource, action)),
    );
}

// Pre-computed permission sets (lazy initialized, zero allocation on lookups)
static SUPER_ADMIN_PERMISSIONS: Lazy<HashSet<Permission>> = Lazy::new(|| {
    managed_permissions([
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
        Resource::BlogCategories,
        Resource::Tags,
        Resource::ForumCategories,
        Resource::ForumTopics,
        Resource::ForumReplies,
        Resource::Workflows,
        Resource::WorkflowExecutions,
    ])
});

static ADMIN_PERMISSIONS: Lazy<HashSet<Permission>> = Lazy::new(|| {
    let mut permissions = managed_permissions([
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
        Resource::BlogPosts,
        Resource::BlogCategories,
        Resource::Tags,
        Resource::ForumCategories,
        Resource::ForumTopics,
        Resource::ForumReplies,
        Resource::Workflows,
        Resource::WorkflowExecutions,
    ]);

    permissions.insert(Permission::MODULES_READ);
    permissions.insert(Permission::MODULES_LIST);
    permissions.insert(Permission::SCRIPTS_MANAGE);
    permissions.insert(Permission::MCP_MANAGE);
    permissions.insert(Permission::AI_PROVIDERS_MANAGE);
    permissions.insert(Permission::AI_TASK_PROFILES_MANAGE);
    permissions.insert(Permission::new(Resource::AiSessions, Action::Manage));
    permissions.insert(Permission::new(Resource::AiRuns, Action::Manage));
    permissions.insert(Permission::new(Resource::AiApprovals, Action::Manage));
    permissions.insert(Permission::new(Resource::AiRouter, Action::Manage));
    permissions.insert(Permission::new(Resource::AiTextTasks, Action::Manage));
    permissions.insert(Permission::new(Resource::AiImageTasks, Action::Manage));
    permissions.insert(Permission::new(Resource::AiCodeTasks, Action::Manage));
    permissions.insert(Permission::new(Resource::AiAlloyTasks, Action::Manage));
    permissions.insert(Permission::new(Resource::AiMultimodalTasks, Action::Manage));
    permissions.insert(Permission::LOGS_READ);
    permissions.insert(Permission::LOGS_LIST);
    permissions.insert(Permission::FLEX_SCHEMAS_CREATE);
    permissions.insert(Permission::FLEX_SCHEMAS_READ);
    permissions.insert(Permission::FLEX_SCHEMAS_UPDATE);
    permissions.insert(Permission::FLEX_SCHEMAS_LIST);

    permissions
});

static MANAGER_PERMISSIONS: Lazy<HashSet<Permission>> = Lazy::new(|| {
    let mut permissions = HashSet::new();

    permissions.extend([
        Permission::PRODUCTS_CREATE,
        Permission::PRODUCTS_READ,
        Permission::PRODUCTS_UPDATE,
        Permission::PRODUCTS_DELETE,
        Permission::PRODUCTS_LIST,
    ]);
    insert_actions(
        &mut permissions,
        Resource::Categories,
        [
            Action::Create,
            Action::Read,
            Action::Update,
            Action::Delete,
            Action::List,
        ],
    );

    permissions.extend([
        Permission::ORDERS_READ,
        Permission::ORDERS_UPDATE,
        Permission::ORDERS_LIST,
        Permission::new(Resource::Customers, Action::Read),
        Permission::new(Resource::Customers, Action::List),
    ]);
    insert_actions(
        &mut permissions,
        Resource::Inventory,
        [Action::Create, Action::Read, Action::Update, Action::List],
    );

    permissions.extend([
        Permission::POSTS_CREATE,
        Permission::POSTS_READ,
        Permission::POSTS_UPDATE,
        Permission::POSTS_DELETE,
        Permission::POSTS_LIST,
        Permission::NODES_CREATE,
        Permission::NODES_READ,
        Permission::NODES_UPDATE,
        Permission::NODES_DELETE,
        Permission::NODES_LIST,
    ]);
    insert_actions(
        &mut permissions,
        Resource::Media,
        [
            Action::Create,
            Action::Read,
            Action::Update,
            Action::Delete,
            Action::List,
        ],
    );

    permissions.extend([
        Permission::ANALYTICS_READ,
        Permission::TAXONOMY_CREATE,
        Permission::TAXONOMY_READ,
        Permission::TAXONOMY_UPDATE,
        Permission::TAXONOMY_DELETE,
        Permission::TAXONOMY_LIST,
        Permission::PAGES_CREATE,
        Permission::PAGES_READ,
        Permission::PAGES_UPDATE,
        Permission::PAGES_DELETE,
        Permission::PAGES_LIST,
        Permission::BLOG_POSTS_CREATE,
        Permission::BLOG_POSTS_READ,
        Permission::BLOG_POSTS_UPDATE,
        Permission::BLOG_POSTS_DELETE,
        Permission::BLOG_POSTS_LIST,
        Permission::BLOG_POSTS_PUBLISH,
        Permission::BLOG_CATEGORIES_CREATE,
        Permission::BLOG_CATEGORIES_READ,
        Permission::BLOG_CATEGORIES_UPDATE,
        Permission::BLOG_CATEGORIES_DELETE,
        Permission::BLOG_CATEGORIES_LIST,
        Permission::SEO_READ,
        Permission::SEO_UPDATE,
        Permission::SEO_PUBLISH,
        Permission::SEO_GENERATE,
        Permission::AI_PROVIDERS_READ,
        Permission::AI_TASK_PROFILES_READ,
        Permission::AI_SESSIONS_READ,
        Permission::AI_SESSIONS_RUN,
        Permission::AI_RUNS_CANCEL,
        Permission::AI_APPROVALS_RESOLVE,
        Permission::AI_TASKS_TEXT_RUN,
        Permission::AI_TASKS_IMAGE_RUN,
        Permission::AI_TASKS_CODE_RUN,
        Permission::AI_TASKS_ALLOY_RUN,
        Permission::AI_TASKS_MULTIMODAL_RUN,
    ]);

    insert_actions(
        &mut permissions,
        Resource::ForumCategories,
        [Action::Create, Action::Read, Action::Update, Action::List],
    );
    for resource in [Resource::ForumTopics, Resource::ForumReplies] {
        insert_actions(
            &mut permissions,
            resource,
            [
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
                Action::Moderate,
            ],
        );
    }

    permissions
});

static CUSTOMER_PERMISSIONS: Lazy<HashSet<Permission>> = Lazy::new(|| {
    let mut permissions = HashSet::new();

    permissions.extend([
        Permission::PRODUCTS_READ,
        Permission::PRODUCTS_LIST,
        Permission::new(Resource::Categories, Action::Read),
        Permission::new(Resource::Categories, Action::List),
        Permission::ORDERS_READ,
        Permission::ORDERS_LIST,
        Permission::ORDERS_CREATE,
        Permission::POSTS_READ,
        Permission::POSTS_LIST,
        Permission::NODES_READ,
        Permission::NODES_LIST,
        Permission::PAGES_READ,
        Permission::PAGES_LIST,
        Permission::new(Resource::Comments, Action::Create),
        Permission::new(Resource::Comments, Action::Read),
        Permission::new(Resource::Comments, Action::List),
        Permission::TAXONOMY_READ,
        Permission::TAXONOMY_LIST,
        Permission::BLOG_POSTS_READ,
        Permission::BLOG_POSTS_LIST,
        Permission::BLOG_CATEGORIES_READ,
        Permission::BLOG_CATEGORIES_LIST,
        Permission::new(Resource::ForumCategories, Action::Read),
        Permission::new(Resource::ForumCategories, Action::List),
        Permission::new(Resource::ForumTopics, Action::Read),
        Permission::new(Resource::ForumTopics, Action::List),
        Permission::new(Resource::ForumTopics, Action::Create),
        Permission::new(Resource::ForumReplies, Action::Read),
        Permission::new(Resource::ForumReplies, Action::List),
        Permission::new(Resource::ForumReplies, Action::Create),
        Permission::INVENTORY_READ,
        Permission::INVENTORY_LIST,
    ]);

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
        permissions.contains(permission)
            || permissions.contains(&Permission::new(permission.resource, Action::Manage))
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

    pub fn get_scope(role: &UserRole, permission: &Permission) -> PermissionScope {
        permission_scope_for_set(Self::permissions_for_role(role), permission, role.clone())
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
            Permission::BLOG_CATEGORIES_READ,
            Permission::BLOG_CATEGORIES_LIST,
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
    fn built_in_roles_use_dedicated_blog_category_permissions() {
        assert!(Rbac::has_permission(
            &UserRole::SuperAdmin,
            &Permission::BLOG_CATEGORIES_MANAGE
        ));
        assert!(Rbac::has_permission(
            &UserRole::Admin,
            &Permission::BLOG_CATEGORIES_UPDATE
        ));
        assert!(Rbac::has_permission(
            &UserRole::Manager,
            &Permission::BLOG_CATEGORIES_DELETE
        ));
        assert!(Rbac::has_permission(
            &UserRole::Customer,
            &Permission::BLOG_CATEGORIES_READ
        ));
    }

    #[test]
    fn catalog_category_permission_does_not_authorize_blog_categories() {
        let security = SecurityContext::from_permissions(
            UserRole::Manager,
            Some(Uuid::new_v4()),
            [Permission::new(Resource::Categories, Action::Update)],
        );
        assert_eq!(
            security.get_scope(Resource::BlogCategories, Action::Update),
            PermissionScope::None
        );
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
        assert!(
            security
                .permissions()
                .contains(&Permission::BLOG_CATEGORIES_LIST)
        );
        assert!(security.permissions().contains(&Permission::PAGES_LIST));
        assert!(
            security
                .permissions()
                .contains(&Permission::new(Resource::ForumTopics, Action::Read))
        );
        assert!(
            !security
                .permissions()
                .contains(&Permission::BLOG_CATEGORIES_UPDATE)
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
                .contains(&Permission::BLOG_CATEGORIES_MANAGE)
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
