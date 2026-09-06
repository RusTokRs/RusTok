use rustok_core::UserRole;
use serde::{Deserialize, Serialize};

pub const RBAC_EVENT_ROLE_PERMISSIONS_ASSIGNED: &str = "rbac.role_permissions_assigned";
pub const RBAC_EVENT_USER_ROLE_REPLACED: &str = "rbac.user_role_replaced";
pub const RBAC_EVENT_TENANT_ROLE_ASSIGNMENTS_REMOVED: &str = "rbac.tenant_role_assignments_removed";
pub const RBAC_EVENT_USER_ROLE_ASSIGNMENT_REMOVED: &str = "rbac.user_role_assignment_removed";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RbacIntegrationEventKind {
    RolePermissionsAssigned,
    UserRoleReplaced,
    TenantRoleAssignmentsRemoved,
    UserRoleAssignmentRemoved,
}

impl RbacIntegrationEventKind {
    pub fn event_type(self) -> &'static str {
        match self {
            Self::RolePermissionsAssigned => RBAC_EVENT_ROLE_PERMISSIONS_ASSIGNED,
            Self::UserRoleReplaced => RBAC_EVENT_USER_ROLE_REPLACED,
            Self::TenantRoleAssignmentsRemoved => RBAC_EVENT_TENANT_ROLE_ASSIGNMENTS_REMOVED,
            Self::UserRoleAssignmentRemoved => RBAC_EVENT_USER_ROLE_ASSIGNMENT_REMOVED,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacRoleAssignmentEvent {
    pub kind: RbacIntegrationEventKind,
    pub tenant_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub role: Option<UserRole>,
}

impl RbacRoleAssignmentEvent {
    pub fn role_permissions_assigned(
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        role: UserRole,
    ) -> Self {
        Self {
            kind: RbacIntegrationEventKind::RolePermissionsAssigned,
            tenant_id,
            user_id,
            role: Some(role),
        }
    }

    pub fn user_role_replaced(tenant_id: uuid::Uuid, user_id: uuid::Uuid, role: UserRole) -> Self {
        Self {
            kind: RbacIntegrationEventKind::UserRoleReplaced,
            tenant_id,
            user_id,
            role: Some(role),
        }
    }

    pub fn tenant_role_assignments_removed(tenant_id: uuid::Uuid, user_id: uuid::Uuid) -> Self {
        Self {
            kind: RbacIntegrationEventKind::TenantRoleAssignmentsRemoved,
            tenant_id,
            user_id,
            role: None,
        }
    }

    pub fn user_role_assignment_removed(
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        role: UserRole,
    ) -> Self {
        Self {
            kind: RbacIntegrationEventKind::UserRoleAssignmentRemoved,
            tenant_id,
            user_id,
            role: Some(role),
        }
    }

    pub fn event_type(&self) -> &'static str {
        self.kind.event_type()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RBAC_EVENT_ROLE_PERMISSIONS_ASSIGNED, RBAC_EVENT_TENANT_ROLE_ASSIGNMENTS_REMOVED,
        RbacRoleAssignmentEvent,
    };
    use rustok_core::UserRole;

    #[test]
    fn constructors_build_expected_payloads() {
        let tenant_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();

        let assigned =
            RbacRoleAssignmentEvent::role_permissions_assigned(tenant_id, user_id, UserRole::Admin);
        assert_eq!(assigned.tenant_id, tenant_id);
        assert_eq!(assigned.user_id, user_id);
        assert_eq!(assigned.role, Some(UserRole::Admin));
        assert_eq!(assigned.event_type(), RBAC_EVENT_ROLE_PERMISSIONS_ASSIGNED);

        let removed = RbacRoleAssignmentEvent::tenant_role_assignments_removed(tenant_id, user_id);
        assert_eq!(removed.tenant_id, tenant_id);
        assert_eq!(removed.user_id, user_id);
        assert_eq!(removed.role, None);
        assert_eq!(
            removed.event_type(),
            RBAC_EVENT_TENANT_ROLE_ASSIGNMENTS_REMOVED
        );
    }

    #[test]
    fn event_kind_serializes_as_stable_snake_case_tag() {
        let kind = super::RbacIntegrationEventKind::UserRoleAssignmentRemoved;
        let serialized = serde_json::to_string(&kind).expect("serialize kind");

        assert_eq!(serialized, "\"user_role_assignment_removed\"");
    }

    #[test]
    fn role_assignment_event_supports_json_roundtrip() {
        let event = RbacRoleAssignmentEvent::user_role_replaced(
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            UserRole::Manager,
        );

        let serialized = serde_json::to_string(&event).expect("serialize event");
        let decoded: RbacRoleAssignmentEvent =
            serde_json::from_str(&serialized).expect("deserialize event");

        assert_eq!(decoded, event);
    }
}
