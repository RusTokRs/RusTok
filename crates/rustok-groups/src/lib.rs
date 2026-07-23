use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use rustok_notifications_api::register_notification_source_provider_factory;
use sea_orm_migration::MigrationTrait;

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
pub mod graphql_policy_history;
pub mod group_event_entities;
pub mod invitation_entities;
mod invitations_legacy {
    include!("invitations.rs");
    include!("invitations_transactional.rs");
}
pub mod localization;
pub mod membership_enforcement;
mod membership_enforcement_transaction;
pub mod membership_enforcement_entities;
pub mod migrations;
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
        BulkReviewGroupMembershipApplicationsRequest,
        BulkReviewGroupMembershipApplicationsResult, CancelGroupMembershipApplicationRequest,
        GROUP_APPLICATION_POLICY_CHANGED_CODE, GroupApplicationBulkReviewCommandPort,
        GroupApplicationCasCommandPort, GroupApplicationCommandPort,
        GroupApplicationLifecycleCommandPort, GroupApplicationLifecycleReadPort,
        GroupApplicationLifecycleResult, GroupApplicationPolicy,
        GroupApplicationPolicyLocaleCatalog, GroupApplicationPolicyManagementReadPort,
        GroupApplicationPolicyManagementView, GroupApplicationPolicyPrecondition,
        GroupApplicationQuestion, GroupApplicationReadPort, GroupApplicationReviewCommandPort,
        GroupApplicationReviewDecision, GroupApplicationRule, GroupApplicationStatus,
        GroupMembershipApplication, GroupMembershipApplicationConnection,
        ListGroupApplicationPolicyLocalesRequest, ListGroupMembershipApplicationsRequest,
        ReadGroupApplicationPolicyForManagementRequest, ReadGroupApplicationPolicyRequest,
        ReadMyGroupMembershipApplicationRequest, ReopenGroupMembershipApplicationRequest,
        ReviewGroupMembershipApplicationRequest, ReviewGroupMembershipApplicationResult,
        SubmitGroupMembershipApplicationIfCurrentRequest,
        SubmitGroupMembershipApplicationRequest, SubmitGroupMembershipApplicationResult,
        UpsertGroupApplicationPolicyIfCurrentRequest, UpsertGroupApplicationPolicyRequest,
        UpsertGroupApplicationPolicyResult,
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

pub use applications::*;
pub use domain::*;
pub use dto::*;
pub use effective_service::GroupsService;
pub use error::{GroupsError, GroupsResult};
pub use governance::*;
pub use invitations::*;
pub use localization::GroupLocalizationService;
pub use membership_enforcement::GroupMembershipEnforcementService;
pub use policy_history::*;
pub use ports::*;
pub use targeted_invitations::*;

/// Social group identity, membership, privacy, and modular feature owner.
pub struct GroupsModule;

#[async_trait]
impl RusToKModule for GroupsModule {
    fn slug(&self) -> &'static str {
        "groups"
    }

    fn name(&self) -> &'static str {
        "Groups"
    }

    fn description(&self) -> &'static str {
        "Social groups, memberships, local roles, privacy, and feature bindings"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::GROUPS_CREATE,
            Permission::GROUPS_READ,
            Permission::GROUPS_UPDATE,
            Permission::GROUPS_DELETE,
            Permission::GROUPS_LIST,
            Permission::GROUPS_MODERATE,
            Permission::GROUPS_MANAGE,
        ]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        extensions.insert(GroupCapabilityDescriptor::default());
        register_notification_source_provider_factory(
            extensions,
            notification_source::GroupsNotificationSourceProviderFactory,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "groups notification source factory registration failed: {error}"
            ))
        })?;
        Ok(())
    }
}

impl MigrationSource for GroupsModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_metadata_matches_manifest_contract() {
        let module = GroupsModule;
        assert_eq!(module.slug(), "groups");
        assert_eq!(module.name(), "Groups");
        assert!(module.dependencies().is_empty());
        assert_eq!(module.migrations().len(), 8);
        assert_eq!(module.permissions().len(), 7);
    }

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
    }
}
