use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use rustok_api::{PortError, normalize_locale_tag};

use crate::domain::{
    GroupAction, GroupFeatureStatus, GroupJoinPolicy, GroupMembershipEffectiveStatus,
    GroupMembershipEnforcementSourceKind, GroupMembershipEnforcementState, GroupMembershipStatus,
    GroupRole, GroupStatus, GroupVisibility, normalize_feature_key, normalize_group_handle,
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

impl CreateGroupInput {
    pub fn validate(&self) -> Result<(), PortError> {
        normalize_group_handle(&self.handle)
            .map_err(|msg| PortError::validation("groups.invalid_handle", msg))?;
        normalize_locale_tag(&self.locale)
            .ok_or_else(|| PortError::validation("groups.invalid_locale", "invalid group locale"))?;
        let title = self.title.trim();
        if title.is_empty() || title.chars().count() > 240 {
            return Err(PortError::validation(
                "groups.invalid_title",
                "group title must contain between 1 and 240 characters",
            ));
        }
        if self.summary.as_ref().is_some_and(|summary| summary.trim().chars().count() > 500) {
            return Err(PortError::validation(
                "groups.invalid_summary",
                "group summary must not exceed 500 characters",
            ));
        }
        if self.category_id.is_some_and(|id| id.is_nil()) {
            return Err(PortError::validation(
                "groups.invalid_category_id",
                "category_id cannot be a nil UUID",
            ));
        }
        if self.avatar_media_id.is_some_and(|id| id.is_nil()) {
            return Err(PortError::validation(
                "groups.invalid_avatar_media_id",
                "avatar_media_id cannot be a nil UUID",
            ));
        }
        if self.cover_media_id.is_some_and(|id| id.is_nil()) {
            return Err(PortError::validation(
                "groups.invalid_cover_media_id",
                "cover_media_id cannot be a nil UUID",
            ));
        }
        Ok(())
    }
}

impl ReadGroupRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.group_id.is_none() && self.handle.is_none() {
            return Err(PortError::validation(
                "groups.invalid_request",
                "either group_id or handle must be provided",
            ));
        }
        if self.group_id.is_some_and(|id| id.is_nil()) {
            return Err(PortError::validation(
                "groups.invalid_group_id",
                "group_id cannot be a nil UUID",
            ));
        }
        if let Some(handle) = &self.handle {
            normalize_group_handle(handle)
                .map_err(|msg| PortError::validation("groups.invalid_handle", msg))?;
        }
        Ok(())
    }
}

impl ListGroupsRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.page == 0 {
            return Err(PortError::validation(
                "groups.invalid_page",
                "page must be greater than or equal to 1",
            ));
        }
        if self.per_page == 0 || self.per_page > 100 {
            return Err(PortError::validation(
                "groups.invalid_per_page",
                "per_page must be between 1 and 100",
            ));
        }
        if self.search.as_ref().is_some_and(|search| search.trim().chars().count() > 200) {
            return Err(PortError::validation(
                "groups.invalid_search",
                "search query must not exceed 200 characters",
            ));
        }
        Ok(())
    }
}

impl JoinGroupRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.group_id.is_nil() {
            return Err(PortError::validation(
                "groups.invalid_group_id",
                "group_id cannot be a nil UUID",
            ));
        }
        Ok(())
    }
}

impl LeaveGroupRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.group_id.is_nil() {
            return Err(PortError::validation(
                "groups.invalid_group_id",
                "group_id cannot be a nil UUID",
            ));
        }
        Ok(())
    }
}

impl SetGroupFeatureRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.group_id.is_nil() {
            return Err(PortError::validation(
                "groups.invalid_group_id",
                "group_id cannot be a nil UUID",
            ));
        }
        normalize_feature_key(&self.feature_key)
            .map_err(|msg| PortError::validation("groups.invalid_feature_key", msg))?;
        let version = self.contract_version.trim();
        if version.is_empty() || version.len() > 64 {
            return Err(PortError::validation(
                "groups.invalid_contract_version",
                "contract_version must contain between 1 and 64 characters",
            ));
        }
        Ok(())
    }
}

impl ListGroupTranslationsRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.group_id.is_nil() {
            return Err(PortError::validation(
                "groups.invalid_group_id",
                "group_id cannot be a nil UUID",
            ));
        }
        Ok(())
    }
}

impl UpsertGroupTranslationRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.group_id.is_nil() {
            return Err(PortError::validation(
                "groups.invalid_group_id",
                "group_id cannot be a nil UUID",
            ));
        }
        normalize_locale_tag(&self.locale).ok_or_else(|| {
            PortError::validation("groups.invalid_locale", "invalid translation locale")
        })?;
        let title = self.title.trim();
        if title.is_empty() || title.chars().count() > 240 {
            return Err(PortError::validation(
                "groups.invalid_title",
                "translation title must contain between 1 and 240 characters",
            ));
        }
        if self.summary.as_ref().is_some_and(|summary| summary.trim().chars().count() > 500) {
            return Err(PortError::validation(
                "groups.invalid_summary",
                "translation summary must not exceed 500 characters",
            ));
        }
        Ok(())
    }
}

impl DeleteGroupTranslationRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.group_id.is_nil() {
            return Err(PortError::validation(
                "groups.invalid_group_id",
                "group_id cannot be a nil UUID",
            ));
        }
        normalize_locale_tag(&self.locale).ok_or_else(|| {
            PortError::validation("groups.invalid_locale", "invalid translation locale")
        })?;
        Ok(())
    }
}

impl GroupAccessRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.group_id.is_nil() {
            return Err(PortError::validation(
                "groups.invalid_group_id",
                "group_id cannot be a nil UUID",
            ));
        }
        Ok(())
    }
}

impl ReadGroupMembershipRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.group_id.is_nil() {
            return Err(PortError::validation(
                "groups.invalid_group_id",
                "group_id cannot be a nil UUID",
            ));
        }
        if self.user_id.is_nil() {
            return Err(PortError::validation(
                "groups.invalid_user_id",
                "user_id cannot be a nil UUID",
            ));
        }
        Ok(())
    }
}

impl ReadGroupMembershipEnforcementRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.group_id.is_nil() {
            return Err(PortError::validation(
                "groups.invalid_group_id",
                "group_id cannot be a nil UUID",
            ));
        }
        if self.user_id.is_nil() {
            return Err(PortError::validation(
                "groups.invalid_user_id",
                "user_id cannot be a nil UUID",
            ));
        }
        Ok(())
    }
}

impl ListGroupMembershipsRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.group_id.is_nil() {
            return Err(PortError::validation(
                "groups.invalid_group_id",
                "group_id cannot be a nil UUID",
            ));
        }
        if self.page == 0 {
            return Err(PortError::validation(
                "groups.invalid_page",
                "page must be greater than or equal to 1",
            ));
        }
        if self.per_page == 0 || self.per_page > 100 {
            return Err(PortError::validation(
                "groups.invalid_per_page",
                "per_page must be between 1 and 100",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_group_input_validates_bounds_and_nil_uuids() {
        let mut input = CreateGroupInput {
            handle: "valid-group".to_string(),
            locale: "en".to_string(),
            title: "Valid Group Title".to_string(),
            summary: Some("Short summary".to_string()),
            body: None,
            visibility: GroupVisibility::Public,
            join_policy: GroupJoinPolicy::Open,
            category_id: Some(Uuid::new_v4()),
            avatar_media_id: None,
            cover_media_id: None,
            metadata: serde_json::json!({}),
        };
        assert!(input.validate().is_ok());

        // Nil category ID
        input.category_id = Some(Uuid::nil());
        assert!(input.validate().is_err());
        input.category_id = None;

        // Empty title
        input.title = "   ".to_string();
        assert!(input.validate().is_err());
        input.title = "Group".to_string();

        // Invalid locale
        input.locale = "invalid locale tag!".to_string();
        assert!(input.validate().is_err());
        input.locale = "en".to_string();

        // Invalid handle
        input.handle = "a".to_string();
        assert!(input.validate().is_err());
    }

    #[test]
    fn requests_reject_nil_uuids() {
        assert!(JoinGroupRequest { group_id: Uuid::nil() }.validate().is_err());
        assert!(LeaveGroupRequest { group_id: Uuid::nil() }.validate().is_err());
        assert!(GroupAccessRequest { group_id: Uuid::nil(), action: GroupAction::View }.validate().is_err());
        assert!(ReadGroupMembershipRequest { group_id: Uuid::nil(), user_id: Uuid::new_v4() }.validate().is_err());
        assert!(ReadGroupMembershipRequest { group_id: Uuid::new_v4(), user_id: Uuid::nil() }.validate().is_err());
        assert!(ReadGroupMembershipEnforcementRequest { group_id: Uuid::nil(), user_id: Uuid::new_v4() }.validate().is_err());
        assert!(ReadGroupMembershipEnforcementRequest { group_id: Uuid::new_v4(), user_id: Uuid::nil() }.validate().is_err());
        assert!(ListGroupMembershipsRequest { group_id: Uuid::nil(), page: 1, per_page: 20 }.validate().is_err());
        assert!(ListGroupMembershipsRequest { group_id: Uuid::new_v4(), page: 0, per_page: 20 }.validate().is_err());
        assert!(ListGroupMembershipsRequest { group_id: Uuid::new_v4(), page: 1, per_page: 101 }.validate().is_err());
    }
}
