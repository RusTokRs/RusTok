//! # rustok-groups
//!
//! Social groups, memberships, local roles, privacy, and feature bindings.
//!
//! Provides social group identity, multilingual presentation, group memberships,
//! invitations, application policies, and group-level enforcement state.

pub mod application_entities;

mod applications_legacy_module {
    include!("applications.rs");
    include!("applications_transactional.rs");
    include!("applications_transactional_cas.rs");
    include!("applications_transactional_cas_bridge.rs");
    include!("applications_transactional_lifecycle.rs");
}
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
mod invitations_legacy {
    include!("invitations.rs");
    include!("invitations_transactional.rs");
}
pub mod localization;
pub mod membership_enforcement;
mod membership_enforcement_command;
pub mod membership_enforcement_entities;
mod membership_enforcement_transaction;
pub mod migrations;
pub mod module;
mod moderation_subject;
mod notification_source;
pub mod policy_history;
pub mod ports;
// Transitional status-only core implementation delegate. It is crate-private so external
// consumers and module-owned transports cannot bypass the effective membership facade.
mod service;
mod targeted_invitations_legacy {
    include!("targeted_invitations.rs");
    include!("targeted_invitations_transactional.rs");
}

/// Compatibility module preserving the public application types and paths while sealing the
/// status-only owner implementation behind the effective-membership facade.
pub mod applications {
    pub use crate::applications_legacy_module::{
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

/// Compatibility module preserving invitation contracts while routing the service type through
/// effective group-membership authorization.
pub mod invitations {
    pub use crate::effective_invitations::GroupInvitationService;
    pub use crate::invitations_legacy::{
        AcceptGroupInvitationRequest, AcceptGroupInvitationResult, CreateGroupInvitationRequest,
        CreateGroupInvitationResult, GroupInvitation, GroupInvitationCommandPort,
        GroupInvitationConnection, GroupInvitationReadPort, GroupInvitationStatus,
        ListGroupInvitationsRequest, RevokeGroupInvitationRequest, RevokeGroupInvitationResult,
    };
}

/// Compatibility module preserving targeted-invitation contracts while sealing the legacy
/// implementation delegate.
pub mod targeted_invitations {
    pub use crate::effective_invitations::GroupTargetedInvitationService;
    pub use crate::targeted_invitations_legacy::{
        AcceptTargetedGroupInvitationRequest, GroupTargetedInvitationCommandPort,
    };
}

pub use applications::{
    BulkReviewGroupMembershipApplicationItemResult,
    BulkReviewGroupMembershipApplicationsRequest, BulkReviewGroupMembershipApplicationsResult,
    CancelGroupMembershipApplicationRequest, GroupApplicationBulkReviewCommandPort,
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
    UpsertGroupApplicationPolicyResult, GROUP_APPLICATION_POLICY_CHANGED_CODE,
};
pub use domain::{
    normalize_feature_key, normalize_group_handle, GroupAction, GroupFeatureStatus,
    GroupJoinPolicy, GroupMembershipEffectiveStatus, GroupMembershipEnforcementSourceKind,
    GroupMembershipEnforcementState, GroupMembershipStatus, GroupRole, GroupStatus,
    GroupVisibility,
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
    GroupAccessReadPort, GroupCapabilityDescriptor, GroupCommandPort,
    GroupLocalizationCommandPort, GroupLocalizationReadPort,
    GroupMembershipEnforcementCommandPort, GroupMembershipEnforcementReadPort,
    GroupMembershipReadPort, GroupSummaryReadPort, SharedGroupAccessReadPort,
    SharedGroupCommandPort, SharedGroupLocalizationCommandPort, SharedGroupLocalizationReadPort,
    SharedGroupMembershipEnforcementCommandPort, SharedGroupMembershipEnforcementReadPort,
    SharedGroupMembershipReadPort, SharedGroupSummaryReadPort,
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

