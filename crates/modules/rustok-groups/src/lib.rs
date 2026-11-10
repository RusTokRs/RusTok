//! # rustok-groups
//!
//! Social groups, memberships, local roles, privacy, and feature bindings.
//!
//! Provides social group identity, multilingual presentation, group memberships,
//! invitations, application policies, and group-level enforcement state.

#![doc = include_str!("../CRATE_API.md")]

pub mod application_entities;
mod commands;

#[path = "applications.rs"]
mod application_owner;

pub mod domain;
pub mod dto;
mod effective_applications;
mod effective_invitations;
mod effective_membership_guard;
mod effective_service;
pub mod entities;
pub mod error;
pub mod governance;
pub mod governance_entities;
#[cfg(feature = "graphql")]
pub mod graphql;
#[cfg(feature = "graphql")]
pub mod graphql_application_cas;
#[cfg(feature = "graphql")]
pub mod graphql_application_lifecycle;
#[cfg(feature = "graphql")]
pub mod graphql_application_policy_management;
#[cfg(feature = "graphql")]
pub mod graphql_applications;
#[cfg(feature = "graphql")]
pub mod graphql_governance;
#[cfg(feature = "graphql")]
pub mod graphql_invitations;
#[cfg(feature = "graphql")]
pub mod graphql_localization;
#[cfg(feature = "graphql")]
pub mod graphql_membership_enforcement;
#[cfg(feature = "graphql")]
pub mod graphql_policy_history;
pub mod group_event_entities;
pub mod invitation_entities;
#[path = "invitations.rs"]
mod invitation_owner;
pub mod localization;
pub mod membership_enforcement;
mod membership_enforcement_command;
pub mod membership_enforcement_entities;
mod membership_enforcement_transaction;
pub mod migrations;
mod moderation_subject;
pub mod module;
mod notification_source;
pub mod policy_history;
pub mod ports;
// Core storage and presentation implementation. Access decisions route through the public owner.
mod service;
#[path = "targeted_invitations.rs"]
mod targeted_invitation_owner;

/// Public application contracts with one private transactional owner implementation.
pub mod applications {
    pub use crate::application_owner::{
        BulkReviewGroupMembershipApplicationItemResult,
        BulkReviewGroupMembershipApplicationsRequest, BulkReviewGroupMembershipApplicationsResult,
        CancelGroupMembershipApplicationRequest, GROUP_APPLICATION_POLICY_CHANGED_CODE,
        GroupApplicationBulkReviewCommandPort, GroupApplicationCasCommandPort,
        GroupApplicationCommandPort, GroupApplicationLifecycleCommandPort,
        GroupApplicationLifecycleReadPort, GroupApplicationLifecycleResult, GroupApplicationPolicy,
        GroupApplicationPolicyLocaleCatalog, GroupApplicationPolicyManagementReadPort,
        GroupApplicationPolicyManagementView, GroupApplicationPolicyPrecondition,
        GroupApplicationQuestion, GroupApplicationReadPort, GroupApplicationReviewCommandPort,
        GroupApplicationReviewDecision, GroupApplicationRule, GroupApplicationStatus,
        GroupMembershipApplication, GroupMembershipApplicationConnection,
        ListGroupApplicationPolicyLocalesRequest, ListGroupMembershipApplicationsRequest,
        ReadGroupApplicationPolicyForManagementRequest, ReadGroupApplicationPolicyRequest,
        ReadMyGroupMembershipApplicationRequest, ReopenGroupMembershipApplicationRequest,
        ReviewGroupMembershipApplicationRequest, ReviewGroupMembershipApplicationResult,
        SubmitGroupMembershipApplicationIfCurrentRequest, SubmitGroupMembershipApplicationRequest,
        SubmitGroupMembershipApplicationResult, UpsertGroupApplicationPolicyIfCurrentRequest,
        UpsertGroupApplicationPolicyRequest, UpsertGroupApplicationPolicyResult,
    };
    pub use crate::effective_applications::GroupApplicationService;
}

/// Public invitation contracts using effective group-membership authorization.
pub mod invitations {
    pub use crate::effective_invitations::GroupInvitationService;
    pub use crate::invitation_owner::{
        AcceptGroupInvitationRequest, AcceptGroupInvitationResult, CreateGroupInvitationRequest,
        CreateGroupInvitationResult, GroupInvitation, GroupInvitationCommandPort,
        GroupInvitationConnection, GroupInvitationReadPort, GroupInvitationStatus,
        ListGroupInvitationsRequest, RevokeGroupInvitationRequest, RevokeGroupInvitationResult,
    };
}

/// Public targeted invitation contracts using the transactional owner implementation.
pub mod targeted_invitations {
    pub use crate::effective_invitations::GroupTargetedInvitationService;
    pub use crate::targeted_invitation_owner::{
        AcceptTargetedGroupInvitationRequest, GroupTargetedInvitationCommandPort,
    };
}

pub use applications::{
    BulkReviewGroupMembershipApplicationItemResult, BulkReviewGroupMembershipApplicationsRequest,
    BulkReviewGroupMembershipApplicationsResult, CancelGroupMembershipApplicationRequest,
    GROUP_APPLICATION_POLICY_CHANGED_CODE, GroupApplicationBulkReviewCommandPort,
    GroupApplicationCasCommandPort, GroupApplicationCommandPort,
    GroupApplicationLifecycleCommandPort, GroupApplicationLifecycleReadPort,
    GroupApplicationLifecycleResult, GroupApplicationPolicy, GroupApplicationPolicyLocaleCatalog,
    GroupApplicationPolicyManagementReadPort, GroupApplicationPolicyManagementView,
    GroupApplicationPolicyPrecondition, GroupApplicationQuestion, GroupApplicationReadPort,
    GroupApplicationReviewCommandPort, GroupApplicationReviewDecision, GroupApplicationRule,
    GroupApplicationService, GroupApplicationStatus, GroupMembershipApplication,
    GroupMembershipApplicationConnection, ListGroupApplicationPolicyLocalesRequest,
    ListGroupMembershipApplicationsRequest, ReadGroupApplicationPolicyForManagementRequest,
    ReadGroupApplicationPolicyRequest, ReadMyGroupMembershipApplicationRequest,
    ReopenGroupMembershipApplicationRequest, ReviewGroupMembershipApplicationRequest,
    ReviewGroupMembershipApplicationResult, SubmitGroupMembershipApplicationIfCurrentRequest,
    SubmitGroupMembershipApplicationRequest, SubmitGroupMembershipApplicationResult,
    UpsertGroupApplicationPolicyIfCurrentRequest, UpsertGroupApplicationPolicyRequest,
    UpsertGroupApplicationPolicyResult,
};
pub use domain::{
    GroupAction, GroupFeatureStatus, GroupJoinPolicy, GroupMembershipEffectiveStatus,
    GroupMembershipEnforcementSourceKind, GroupMembershipEnforcementState, GroupMembershipStatus,
    GroupRole, GroupStatus, GroupVisibility, normalize_feature_key, normalize_group_handle,
};
pub use dto::{
    CreateGroupInput, DeleteGroupTranslationRequest, DeleteGroupTranslationResult,
    GroupAccessDecision, GroupAccessRequest, GroupConnection, GroupDetails, GroupFeatureBinding,
    GroupMembership, GroupMembershipConnection, GroupMembershipEffectiveState,
    GroupMembershipEnforcementSummary, GroupSummary, GroupTranslation,
    GroupTranslationMutationResult, JoinGroupRequest, LeaveGroupRequest,
    ListGroupMembershipsRequest, ListGroupTranslationsRequest, ListGroupsRequest,
    ReadGroupMembershipEnforcementRequest, ReadGroupMembershipRequest, ReadGroupRequest,
    SetGroupFeatureRequest, UpsertGroupTranslationRequest,
};
pub use effective_service::GroupsService;
pub use error::{GroupsError, GroupsResult};
pub use governance::{
    ChangeGroupRoleRequest, GroupGovernanceCommandPort, GroupGovernanceResult,
    GroupGovernanceService, TransferGroupOwnershipRequest,
};
pub use invitations::{
    AcceptGroupInvitationRequest, AcceptGroupInvitationResult, CreateGroupInvitationRequest,
    CreateGroupInvitationResult, GroupInvitation, GroupInvitationCommandPort,
    GroupInvitationConnection, GroupInvitationReadPort, GroupInvitationService,
    GroupInvitationStatus, ListGroupInvitationsRequest, RevokeGroupInvitationRequest,
    RevokeGroupInvitationResult,
};
pub use localization::GroupLocalizationService;
pub use membership_enforcement::GroupMembershipEnforcementService;
pub use membership_enforcement_command::{
    GroupMembershipEnforcementCommandService, GroupMembershipEnforcementMutationResult,
    RevokeGroupMembershipSuspensionRequest, SuspendGroupMembershipRequest,
};
pub use moderation_subject::GroupsModerationSubjectAdapterFactory;
pub use policy_history::{
    GroupApplicationPolicyHistoryReadPort, GroupApplicationPolicyHistoryService,
    GroupApplicationPolicyRevision, GroupApplicationPolicyRevisionConnection,
    ListGroupApplicationPolicyRevisionsRequest,
};
pub use ports::{
    GroupAccessReadPort, GroupCapabilityDescriptor, GroupCommandPort, GroupLocalizationCommandPort,
    GroupLocalizationReadPort, GroupMembershipEnforcementCommandPort,
    GroupMembershipEnforcementReadPort, GroupMembershipReadPort, GroupSummaryReadPort,
    SharedGroupAccessReadPort, SharedGroupCommandPort, SharedGroupLocalizationCommandPort,
    SharedGroupLocalizationReadPort, SharedGroupMembershipEnforcementCommandPort,
    SharedGroupMembershipEnforcementReadPort, SharedGroupMembershipReadPort,
    SharedGroupSummaryReadPort,
};
pub use targeted_invitations::{
    AcceptTargetedGroupInvitationRequest, GroupTargetedInvitationCommandPort,
    GroupTargetedInvitationService,
};

pub use module::GroupsModule;

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_descriptor_fails_closed() {
        let descriptor = GroupCapabilityDescriptor::default();
        assert_eq!(descriptor.contract_version, "groups.access.v1");
        assert_eq!(descriptor.private_content_fallback, "deny");
        assert!(!descriptor.implicit_transport_fallback);
        assert!(
            descriptor
                .ports
                .contains(&"GroupMembershipEnforcementReadPort")
        );
        assert!(
            descriptor
                .ports
                .contains(&"GroupMembershipEnforcementCommandPort")
        );
    }
}
