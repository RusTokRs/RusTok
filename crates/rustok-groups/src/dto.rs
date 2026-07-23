use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::domain::{
    GroupAction, GroupFeatureStatus, GroupJoinPolicy, GroupMembershipEffectiveStatus,
    GroupMembershipEnforcementSourceKind, GroupMembershipEnforcementState,
    GroupMembershipStatus, GroupRole, GroupStatus, GroupVisibility,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupSummary {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub owner_user_id: Uuid,
    pub handle: String,
    pub visibility: GroupVisibility,
    pub join_policy: GroupJoinPolicy,
    pub status: GroupStatus,
    pub title: String,
    pub summary: Option<String>,
    pub avatar_media_id: Option<Uuid>,
    pub cover_media_id: Option<Uuid>,
    pub member_count: u64,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupDetails {
    pub summary: GroupSummary,
    pub body: Option<String>,
    pub viewer_membership: Option<GroupMembership>,
    pub features: Vec<GroupFeatureBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMembership {
    pub id: Uuid,
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub role: GroupRole,
    pub status: GroupMembershipStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMembershipEnforcementSummary {
    pub membership_id: Uuid,
    pub state: GroupMembershipEnforcementState,
    pub reason_code: String,
    pub source_kind: GroupMembershipEnforcementSourceKind,
    pub effective_from: DateTime<Utc>,
    pub effective_until: Option<DateTime<Utc>>,
    pub restore_status: GroupMembershipStatus,
    pub moderation_decision_id: Option<Uuid>,
    pub moderation_decision_hash: Option<String>,
    pub actor_kind: String,
    pub actor_id: String,
    pub revision: i64,
    pub revoked_at: Option<DateTime<Utc>>,
    pub is_effective: bool,
}

/// Owner-clock evaluation of one membership and its Groups-owned current enforcement row.
///
/// `stored_status` remains visible for lifecycle compatibility, while callers must use
/// `effective_status`, `active_member`, and `denied_reentry` for access decisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMembershipEffectiveState {
    pub tenant_id: Uuid,
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub membership_id: Option<Uuid>,
    pub role: Option<GroupRole>,
    pub stored_status: Option<GroupMembershipStatus>,
    pub membership_revision: Option<i64>,
    pub effective_status: GroupMembershipEffectiveStatus,
    pub active_member: bool,
    pub denied_reentry: bool,
    pub enforcement: Option<GroupMembershipEnforcementSummary>,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupFeatureBinding {
    pub id: Uuid,
    pub group_id: Uuid,
    pub feature_key: String,
    pub owner_module: String,
    pub contract_version: String,
    pub status: GroupFeatureStatus,
    pub sort_order: i32,
    pub configuration: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupTranslation {
    pub id: Uuid,
    pub group_id: Uuid,
    pub locale: String,
    pub title: String,
    pub summary: Option<String>,
    pub body: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupTranslationMutationResult {
    pub translation: GroupTranslation,
    pub group_version: u64,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteGroupTranslationResult {
    pub group_id: Uuid,
    pub locale: String,
    pub group_version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupAccessDecision {
    pub group_id: Uuid,
    pub action: GroupAction,
    pub allowed: bool,
    pub reason_code: String,
    pub membership_role: Option<GroupRole>,
    pub membership_status: Option<GroupMembershipStatus>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateGroupInput {
    pub handle: String,
    pub locale: String,
    pub title: String,
    pub summary: Option<String>,
    pub body: Option<String>,
    pub visibility: GroupVisibility,
    pub join_policy: GroupJoinPolicy,
    pub category_id: Option<Uuid>,
    pub avatar_media_id: Option<Uuid>,
    pub cover_media_id: Option<Uuid>,
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadGroupRequest {
    pub group_id: Option<Uuid>,
    pub handle: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListGroupsRequest {
    pub page: u64,
    pub per_page: u64,
    pub search: Option<String>,
    pub include_non_public: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupConnection {
    pub items: Vec<GroupSummary>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinGroupRequest {
    pub group_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaveGroupRequest {
    pub group_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SetGroupFeatureRequest {
    pub group_id: Uuid,
    pub feature_key: String,
    pub contract_version: String,
    pub enabled: bool,
    pub sort_order: i32,
    pub configuration: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListGroupTranslationsRequest {
    pub group_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertGroupTranslationRequest {
    pub group_id: Uuid,
    pub locale: String,
    pub title: String,
    pub summary: Option<String>,
    pub body: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteGroupTranslationRequest {
    pub group_id: Uuid,
    pub locale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupAccessRequest {
    pub group_id: Uuid,
    pub action: GroupAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadGroupMembershipRequest {
    pub group_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadGroupMembershipEnforcementRequest {
    pub group_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListGroupMembershipsRequest {
    pub group_id: Uuid,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupMembershipConnection {
    pub items: Vec<GroupMembership>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}
