use rustok_core::{UserRole, UserStatus};
use rustok_events::RbacRoleMutationEvent;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RbacRoleMutationFacts {
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub actor_tenant_id: Uuid,
    pub actor_role: UserRole,
    pub target_user_id: Uuid,
    pub target_tenant_id: Uuid,
    pub target_role: UserRole,
    pub target_status: UserStatus,
    pub requested_role: UserRole,
    pub resulting_status: UserStatus,
    /// True only when the target has exactly one tenant role assignment and it
    /// is the requested canonical built-in role.
    pub assignment_is_exact: bool,
    /// Active super administrators in the same tenant after excluding the
    /// target user. This fact is consumed only when the mutation would remove
    /// the target from the active super-administrator set.
    pub remaining_active_super_admins: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RbacRoleMutationOutcome {
    Noop,
    Apply(RbacRoleMutationPlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RbacRoleMutationPlan {
    tenant_id: Uuid,
    actor_id: Uuid,
    target_user_id: Uuid,
    previous_role: UserRole,
    new_role: UserRole,
    change: RbacRoleMutationChange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RbacRoleMutationChange {
    RoleReplaced,
    AssignmentRepaired,
}

impl RbacRoleMutationPlan {
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }

    pub fn target_user_id(&self) -> Uuid {
        self.target_user_id
    }

    pub fn previous_role(&self) -> &UserRole {
        &self.previous_role
    }

    pub fn new_role(&self) -> &UserRole {
        &self.new_role
    }

    pub fn change(&self) -> RbacRoleMutationChange {
        self.change
    }

    pub fn integration_event(
        &self,
        durable_generation: u64,
    ) -> Result<RbacRoleMutationEvent, RbacRoleMutationPolicyError> {
        if durable_generation == 0 {
            return Err(RbacRoleMutationPolicyError::InvalidDurableGeneration);
        }

        Ok(match self.change {
            RbacRoleMutationChange::RoleReplaced => RbacRoleMutationEvent::user_role_replaced(
                self.target_user_id,
                self.previous_role.to_string(),
                self.new_role.to_string(),
                durable_generation,
            ),
            RbacRoleMutationChange::AssignmentRepaired => {
                RbacRoleMutationEvent::user_role_assignment_repaired(
                    self.target_user_id,
                    self.new_role.to_string(),
                    durable_generation,
                )
            }
        })
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RbacRoleMutationPolicyError {
    #[error("RBAC role mutation identity `{0}` must not be nil")]
    NilIdentity(&'static str),
    #[error("RBAC role mutation actor tenant does not match the requested tenant")]
    ActorTenantMismatch,
    #[error("RBAC role mutation target tenant does not match the requested tenant")]
    TargetTenantMismatch,
    #[error("cannot assign a peer or higher-privileged role")]
    CannotAssignPeerOrHigherRole,
    #[error("cannot modify a peer or higher-privileged user")]
    CannotManagePeerOrHigherUser,
    #[error("cannot remove, demote, or deactivate the last active super administrator")]
    LastActiveSuperAdmin,
    #[error("RBAC role mutation durable generation must be greater than zero")]
    InvalidDurableGeneration,
}

pub fn plan_user_role_mutation(
    facts: RbacRoleMutationFacts,
) -> Result<RbacRoleMutationOutcome, RbacRoleMutationPolicyError> {
    validate_identity("tenant_id", facts.tenant_id)?;
    validate_identity("actor_id", facts.actor_id)?;
    validate_identity("actor_tenant_id", facts.actor_tenant_id)?;
    validate_identity("target_user_id", facts.target_user_id)?;
    validate_identity("target_tenant_id", facts.target_tenant_id)?;

    if facts.actor_tenant_id != facts.tenant_id {
        return Err(RbacRoleMutationPolicyError::ActorTenantMismatch);
    }
    if facts.target_tenant_id != facts.tenant_id {
        return Err(RbacRoleMutationPolicyError::TargetTenantMismatch);
    }
    if !facts.actor_role.can_assign_role(&facts.requested_role) {
        return Err(RbacRoleMutationPolicyError::CannotAssignPeerOrHigherRole);
    }
    if facts.actor_id != facts.target_user_id
        && !facts.actor_role.can_manage_role(&facts.target_role)
    {
        return Err(RbacRoleMutationPolicyError::CannotManagePeerOrHigherUser);
    }

    let removes_active_super_admin = facts.target_role == UserRole::SuperAdmin
        && facts.target_status == UserStatus::Active
        && (facts.requested_role != UserRole::SuperAdmin
            || facts.resulting_status != UserStatus::Active);
    if removes_active_super_admin && facts.remaining_active_super_admins == 0 {
        return Err(RbacRoleMutationPolicyError::LastActiveSuperAdmin);
    }

    if facts.assignment_is_exact && facts.target_role == facts.requested_role {
        return Ok(RbacRoleMutationOutcome::Noop);
    }

    let change = if facts.target_role == facts.requested_role {
        RbacRoleMutationChange::AssignmentRepaired
    } else {
        RbacRoleMutationChange::RoleReplaced
    };
    Ok(RbacRoleMutationOutcome::Apply(RbacRoleMutationPlan {
        tenant_id: facts.tenant_id,
        actor_id: facts.actor_id,
        target_user_id: facts.target_user_id,
        previous_role: facts.target_role,
        new_role: facts.requested_role,
        change,
    }))
}

fn validate_identity(field: &'static str, value: Uuid) -> Result<(), RbacRoleMutationPolicyError> {
    if value.is_nil() {
        Err(RbacRoleMutationPolicyError::NilIdentity(field))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_events::{RBAC_EVENT_USER_ROLE_ASSIGNMENT_REPAIRED, RBAC_EVENT_USER_ROLE_REPLACED};

    fn facts() -> RbacRoleMutationFacts {
        let tenant_id = Uuid::new_v4();
        RbacRoleMutationFacts {
            tenant_id,
            actor_id: Uuid::new_v4(),
            actor_tenant_id: tenant_id,
            actor_role: UserRole::Admin,
            target_user_id: Uuid::new_v4(),
            target_tenant_id: tenant_id,
            target_role: UserRole::Customer,
            target_status: UserStatus::Active,
            requested_role: UserRole::Manager,
            resulting_status: UserStatus::Active,
            assignment_is_exact: false,
            remaining_active_super_admins: 0,
        }
    }

    #[test]
    fn approved_replacement_builds_typed_event() {
        let plan = match plan_user_role_mutation(facts()).unwrap() {
            RbacRoleMutationOutcome::Apply(plan) => plan,
            RbacRoleMutationOutcome::Noop => panic!("replacement must apply"),
        };

        assert_eq!(plan.change(), RbacRoleMutationChange::RoleReplaced);
        assert_eq!(
            plan.integration_event(4).unwrap().event_type(),
            RBAC_EVENT_USER_ROLE_REPLACED
        );
    }

    #[test]
    fn exact_same_role_is_noop_but_malformed_same_role_is_repair() {
        let mut exact = facts();
        exact.target_role = UserRole::Manager;
        exact.assignment_is_exact = true;
        assert_eq!(
            plan_user_role_mutation(exact).unwrap(),
            RbacRoleMutationOutcome::Noop
        );

        let mut malformed = facts();
        malformed.target_role = UserRole::Manager;
        let plan = match plan_user_role_mutation(malformed).unwrap() {
            RbacRoleMutationOutcome::Apply(plan) => plan,
            RbacRoleMutationOutcome::Noop => panic!("malformed assignment must repair"),
        };
        assert_eq!(plan.change(), RbacRoleMutationChange::AssignmentRepaired);
        assert_eq!(
            plan.integration_event(5).unwrap().event_type(),
            RBAC_EVENT_USER_ROLE_ASSIGNMENT_REPAIRED
        );
    }

    #[test]
    fn hierarchy_and_tenant_scope_fail_closed() {
        let mut peer = facts();
        peer.requested_role = UserRole::Admin;
        assert_eq!(
            plan_user_role_mutation(peer).unwrap_err(),
            RbacRoleMutationPolicyError::CannotAssignPeerOrHigherRole
        );

        let mut foreign = facts();
        foreign.target_tenant_id = Uuid::new_v4();
        assert_eq!(
            plan_user_role_mutation(foreign).unwrap_err(),
            RbacRoleMutationPolicyError::TargetTenantMismatch
        );
    }

    #[test]
    fn last_active_super_admin_removal_is_rejected() {
        let tenant_id = Uuid::new_v4();
        let error = plan_user_role_mutation(RbacRoleMutationFacts {
            tenant_id,
            actor_id: Uuid::new_v4(),
            actor_tenant_id: tenant_id,
            actor_role: UserRole::SuperAdmin,
            target_user_id: Uuid::new_v4(),
            target_tenant_id: tenant_id,
            target_role: UserRole::SuperAdmin,
            target_status: UserStatus::Active,
            requested_role: UserRole::Admin,
            resulting_status: UserStatus::Active,
            assignment_is_exact: false,
            remaining_active_super_admins: 0,
        })
        .unwrap_err();

        assert_eq!(error, RbacRoleMutationPolicyError::LastActiveSuperAdmin);
    }

    #[test]
    fn self_demotion_still_requires_assignable_target_role() {
        let mut self_demotion = facts();
        self_demotion.target_user_id = self_demotion.actor_id;
        self_demotion.target_role = UserRole::Admin;
        self_demotion.requested_role = UserRole::Manager;
        assert!(matches!(
            plan_user_role_mutation(self_demotion).unwrap(),
            RbacRoleMutationOutcome::Apply(_)
        ));
    }
}
