use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use utoipa::ToSchema;

#[derive(Debug, thiserror::Error)]
pub enum UserRoleParseError {
    #[error("Invalid user role: {0}")]
    Invalid(String),
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    EnumIter,
    DeriveActiveEnum,
    Default,
    ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum UserRole {
    #[sea_orm(string_value = "super_admin")]
    SuperAdmin,
    #[sea_orm(string_value = "admin")]
    Admin,
    #[sea_orm(string_value = "manager")]
    Manager,
    #[sea_orm(string_value = "customer")]
    #[default]
    Customer,
}

impl UserRole {
    /// Stable privilege ordering used by role-administration policy.
    pub const fn privilege_rank(&self) -> u8 {
        match self {
            Self::Customer => 0,
            Self::Manager => 1,
            Self::Admin => 2,
            Self::SuperAdmin => 3,
        }
    }

    /// Whether this role may grant `target` to another principal.
    ///
    /// Super administrators may grant every role. Other roles may only grant
    /// roles strictly below their own privilege level, preventing peer creation
    /// and upward privilege escalation.
    pub const fn can_assign_role(&self, target: &Self) -> bool {
        matches!(self, Self::SuperAdmin) || self.privilege_rank() > target.privilege_rank()
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::SuperAdmin => "super_admin",
            Self::Admin => "admin",
            Self::Manager => "manager",
            Self::Customer => "customer",
        };
        write!(f, "{value}")
    }
}

impl FromStr for UserRole {
    type Err = UserRoleParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "super_admin" => Ok(Self::SuperAdmin),
            "admin" => Ok(Self::Admin),
            "manager" => Ok(Self::Manager),
            "customer" => Ok(Self::Customer),
            _ => Err(UserRoleParseError::Invalid(value.to_string())),
        }
    }
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    EnumIter,
    DeriveActiveEnum,
    Default,
    ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum UserStatus {
    #[sea_orm(string_value = "active")]
    #[default]
    Active,
    #[sea_orm(string_value = "inactive")]
    Inactive,
    #[sea_orm(string_value = "banned")]
    Banned,
}

impl fmt::Display for UserStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Banned => "banned",
        };
        write!(f, "{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::UserRole;

    #[test]
    fn role_assignment_policy_prevents_peer_and_upward_grants() {
        assert!(UserRole::SuperAdmin.can_assign_role(&UserRole::SuperAdmin));
        assert!(UserRole::SuperAdmin.can_assign_role(&UserRole::Admin));
        assert!(UserRole::Admin.can_assign_role(&UserRole::Manager));
        assert!(UserRole::Manager.can_assign_role(&UserRole::Customer));

        assert!(!UserRole::Admin.can_assign_role(&UserRole::Admin));
        assert!(!UserRole::Admin.can_assign_role(&UserRole::SuperAdmin));
        assert!(!UserRole::Manager.can_assign_role(&UserRole::Admin));
        assert!(!UserRole::Customer.can_assign_role(&UserRole::Manager));
    }
}
