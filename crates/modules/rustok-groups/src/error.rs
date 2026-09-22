use rustok_api::{PortError, PortErrorKind};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GroupsError {
    #[error("group validation failed: {0}")]
    Validation(String),
    #[error("group was not found")]
    NotFound,
    #[error("group handle already exists")]
    HandleConflict,
    #[error("group operation is forbidden: {0}")]
    Forbidden(String),
    #[error("the group membership is suspended")]
    MembershipSuspended,
    #[error("the group membership is banned")]
    MembershipBanned,
    #[error("group manager authority is required: {0}")]
    ManagerRequired(String),
    #[error("the user is already an active group member")]
    MembershipAlreadyActive,
    #[error("group membership enforcement expected revision is stale")]
    MembershipEnforcementRevisionConflict,
    #[error("the group owner cannot be suspended; transfer ownership first")]
    MembershipEnforcementOwnerProtected,
    #[error("a direct membership enforcement command cannot target its own actor")]
    MembershipEnforcementSelfTarget,
    #[error("the group membership already has an effective suspension")]
    MembershipEnforcementAlreadySuspended,
    #[error("the group membership does not have an effective suspension to revoke")]
    MembershipEnforcementNotActive,
    #[error("direct local enforcement cannot revoke moderation-decision enforcement")]
    MembershipEnforcementSourceConflict,
    #[error("group state conflict: {0}")]
    Conflict(String),
    #[error("group persistence failed: {0}")]
    Persistence(String),
    #[error("group invariant failed: {0}")]
    Invariant(String),
}

pub type GroupsResult<T> = Result<T, GroupsError>;

impl From<sea_orm::DbErr> for GroupsError {
    fn from(value: sea_orm::DbErr) -> Self {
        Self::Persistence(value.to_string())
    }
}

impl From<GroupsError> for PortError {
    fn from(value: GroupsError) -> Self {
        match value {
            GroupsError::Validation(message) => PortError::validation("groups.validation", message),
            GroupsError::NotFound => {
                PortError::not_found("groups.not_found", "group was not found")
            }
            GroupsError::HandleConflict => {
                PortError::conflict("groups.handle_conflict", "group handle already exists")
            }
            GroupsError::Forbidden(message) => PortError::forbidden("groups.forbidden", message),
            GroupsError::MembershipSuspended => PortError::forbidden(
                "groups.membership_suspended",
                "group membership is suspended",
            ),
            GroupsError::MembershipBanned => {
                PortError::forbidden("groups.membership_banned", "group membership is banned")
            }
            GroupsError::ManagerRequired(message) => {
                PortError::forbidden("groups.manager_required", message)
            }
            GroupsError::MembershipAlreadyActive => PortError::conflict(
                "groups.membership_already_active",
                "user is already an active group member",
            ),
            GroupsError::MembershipEnforcementRevisionConflict => PortError::conflict(
                "groups.membership_enforcement_revision_conflict",
                "group membership changed after the enforcement command was prepared",
            ),
            GroupsError::MembershipEnforcementOwnerProtected => PortError::forbidden(
                "groups.membership_enforcement_owner_protected",
                "group owner must transfer ownership before membership enforcement",
            ),
            GroupsError::MembershipEnforcementSelfTarget => PortError::forbidden(
                "groups.membership_enforcement_self_target",
                "direct membership enforcement cannot target the acting user",
            ),
            GroupsError::MembershipEnforcementAlreadySuspended => PortError::conflict(
                "groups.membership_enforcement_already_suspended",
                "group membership already has an effective suspension",
            ),
            GroupsError::MembershipEnforcementNotActive => PortError::conflict(
                "groups.membership_enforcement_not_active",
                "group membership does not have an effective suspension to revoke",
            ),
            GroupsError::MembershipEnforcementSourceConflict => PortError::forbidden(
                "groups.membership_enforcement_source_conflict",
                "direct local enforcement cannot revoke moderation-decision enforcement",
            ),
            GroupsError::Conflict(message) => PortError::conflict("groups.conflict", message),
            GroupsError::Persistence(message) => PortError::new(
                PortErrorKind::Unavailable,
                "groups.persistence_unavailable",
                message,
                true,
            ),
            GroupsError::Invariant(message) => {
                PortError::invariant_violation("groups.invariant", message)
            }
        }
    }
}
