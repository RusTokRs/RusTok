use async_graphql::{Enum, InputObject, SimpleObject};
use rustok_core::UserRole;

use crate::RbacPresentationResourceKind;

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum RbacGraphqlUserRole {
    SuperAdmin,
    Admin,
    Manager,
    Customer,
}

impl From<RbacGraphqlUserRole> for UserRole {
    fn from(role: RbacGraphqlUserRole) -> Self {
        match role {
            RbacGraphqlUserRole::SuperAdmin => UserRole::SuperAdmin,
            RbacGraphqlUserRole::Admin => UserRole::Admin,
            RbacGraphqlUserRole::Manager => UserRole::Manager,
            RbacGraphqlUserRole::Customer => UserRole::Customer,
        }
    }
}

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum RbacGraphqlPresentationResourceKind {
    Role,
    Permission,
}

impl From<RbacGraphqlPresentationResourceKind> for RbacPresentationResourceKind {
    fn from(kind: RbacGraphqlPresentationResourceKind) -> Self {
        match kind {
            RbacGraphqlPresentationResourceKind::Role => Self::Role,
            RbacGraphqlPresentationResourceKind::Permission => Self::Permission,
        }
    }
}

#[derive(InputObject, Debug, Clone)]
pub struct WriteRbacPresentationInput {
    pub resource_kind: RbacGraphqlPresentationResourceKind,
    pub resource_key: String,
    pub locale: String,
    pub expected_copy_revision: Option<i64>,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct RbacPresentationPayload {
    pub resource_kind: RbacGraphqlPresentationResourceKind,
    pub resource_key: String,
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
    pub copy_revision: i64,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct RoleInfo {
    /// Role slug, e.g. "super_admin", "admin", "manager", "customer"
    pub slug: String,
    /// Human-readable display name
    pub display_name: String,
    /// All permissions granted to this role (e.g. "users:create")
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AssignUserRolePayload {
    pub success: bool,
    pub user_id: String,
    pub role: String,
}
