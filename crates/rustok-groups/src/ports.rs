use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use std::sync::Arc;

use crate::dto::{
    CreateGroupInput, GroupAccessDecision, GroupAccessRequest, GroupConnection, GroupDetails,
    GroupFeatureBinding, GroupMembership, GroupMembershipConnection, JoinGroupRequest,
    LeaveGroupRequest, ListGroupMembershipsRequest, ListGroupsRequest,
    ReadGroupMembershipRequest, ReadGroupRequest, SetGroupFeatureRequest,
};

#[async_trait]
pub trait GroupSummaryReadPort: Send + Sync {
    async fn read_group(
        &self,
        context: PortContext,
        request: ReadGroupRequest,
    ) -> Result<GroupDetails, PortError>;

    async fn list_groups(
        &self,
        context: PortContext,
        request: ListGroupsRequest,
    ) -> Result<GroupConnection, PortError>;
}

#[async_trait]
pub trait GroupMembershipReadPort: Send + Sync {
    async fn read_membership(
        &self,
        context: PortContext,
        request: ReadGroupMembershipRequest,
    ) -> Result<Option<GroupMembership>, PortError>;

    async fn list_memberships(
        &self,
        context: PortContext,
        request: ListGroupMembershipsRequest,
    ) -> Result<GroupMembershipConnection, PortError>;
}

#[async_trait]
pub trait GroupAccessReadPort: Send + Sync {
    async fn decide_group_access(
        &self,
        context: PortContext,
        request: GroupAccessRequest,
    ) -> Result<GroupAccessDecision, PortError>;

    async fn enabled_group_features(
        &self,
        context: PortContext,
        group_id: uuid::Uuid,
    ) -> Result<Vec<GroupFeatureBinding>, PortError>;
}

#[async_trait]
pub trait GroupCommandPort: Send + Sync {
    async fn create_group(
        &self,
        context: PortContext,
        input: CreateGroupInput,
    ) -> Result<GroupDetails, PortError>;

    async fn join_group(
        &self,
        context: PortContext,
        request: JoinGroupRequest,
    ) -> Result<GroupMembership, PortError>;

    async fn leave_group(
        &self,
        context: PortContext,
        request: LeaveGroupRequest,
    ) -> Result<GroupMembership, PortError>;

    async fn set_group_feature(
        &self,
        context: PortContext,
        request: SetGroupFeatureRequest,
    ) -> Result<GroupFeatureBinding, PortError>;
}

pub type SharedGroupSummaryReadPort = Arc<dyn GroupSummaryReadPort>;
pub type SharedGroupMembershipReadPort = Arc<dyn GroupMembershipReadPort>;
pub type SharedGroupAccessReadPort = Arc<dyn GroupAccessReadPort>;
pub type SharedGroupCommandPort = Arc<dyn GroupCommandPort>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupCapabilityDescriptor {
    pub owner_module: &'static str,
    pub contract_version: &'static str,
    pub ports: &'static [&'static str],
    pub private_content_fallback: &'static str,
    pub implicit_transport_fallback: bool,
}

impl Default for GroupCapabilityDescriptor {
    fn default() -> Self {
        Self {
            owner_module: "groups",
            contract_version: "groups.access.v1",
            ports: &[
                "GroupSummaryReadPort",
                "GroupMembershipReadPort",
                "GroupAccessReadPort",
                "GroupCommandPort",
            ],
            private_content_fallback: "deny",
            implicit_transport_fallback: false,
        }
    }
}
