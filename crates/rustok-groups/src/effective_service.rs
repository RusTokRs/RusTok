use std::str::FromStr;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use serde_json::json;
use uuid::Uuid;

use crate::domain::{
    GroupAction, GroupFeatureStatus, GroupJoinPolicy, GroupMembershipEffectiveStatus,
    GroupMembershipStatus, GroupRole, GroupStatus, GroupVisibility, normalize_feature_key,
};
use crate::dto::*;
use crate::entities::{feature_binding, group, membership};
use crate::error::{GroupsError, GroupsResult};
use crate::membership_enforcement::resolve_group_membership_enforcement;
use crate::ports::{
    GroupAccessReadPort, GroupCommandPort, GroupMembershipReadPort, GroupSummaryReadPort,
};
use crate::service::GroupsService as LegacyGroupsService;

/// Effective-membership facade for the public Groups owner boundary.
///
/// The legacy service remains available only as an implementation delegate for operations that
/// have not yet been converted. Every public core access decision, private group read, join, and
/// feature-settings authorization in this facade uses the Groups-owned enforcement resolver.
#[derive(Clone)]
pub struct GroupsService {
    db: DatabaseConnection,
    legacy: LegacyGroupsService,
}

impl GroupsService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            legacy: LegacyGroupsService::new(db.clone()),
            db,
        }
    }

    async fn group_model(&self, tenant_id: Uuid, group_id: Uuid) -> GroupsResult<group::Model> {
        group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Id.eq(group_id))
            .one(&self.db)
            .await?
            .ok_or(GroupsError::NotFound)
    }

    async fn decide_access_owned(
        &self,
        context: &PortContext,
        group_id: Uuid,
        action: GroupAction,
    ) -> GroupsResult<GroupAccessDecision> {
        require_read(context)?;
        let tenant_id = context_tenant_id(context)?;
        let group_model = self.group_model(tenant_id, group_id).await?;
        let visibility = GroupVisibility::from_str(&group_model.visibility)
            .map_err(GroupsError::Invariant)?;

        let effective = match optional_actor_user_id(context) {
            Some(user_id) => Some(
                resolve_group_membership_enforcement(
                    &self.db,
                    tenant_id,
                    group_id,
                    user_id,
                    Utc::now(),
                )
                .await?,
            ),
            None => None,
        };
        let membership_role = effective.as_ref().and_then(|state| state.role);
        let membership_status = effective.as_ref().and_then(|state| state.stored_status);
        let effective_status = effective
            .as_ref()
            .map(|state| state.effective_status)
            .unwrap_or(GroupMembershipEffectiveStatus::Missing);

        let allowed = access_allowed(
            has_platform_manage(context),
            group_model.status == GroupStatus::Active.as_str(),
            visibility,
            action,
            effective_status,
            membership_role,
        );

        Ok(GroupAccessDecision {
            group_id,
            action,
            allowed,
            reason_code: access_reason_code(allowed, effective_status).to_string(),
            membership_role,
            // Preserve the existing wire field as stored lifecycle status. Effective state is
            // authoritative for `allowed` and remains available through the focused enforcement
            // read port.
            membership_status,
        })
    }

    async fn join_group_owned(
        &self,
        context: &PortContext,
        request: JoinGroupRequest,
    ) -> GroupsResult<GroupMembership> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let user_id = actor_user_id(context)?;
        let transaction = self.db.begin().await?;
        let group_model = group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Id.eq(request.group_id))
            .one(&transaction)
            .await?
            .ok_or(GroupsError::NotFound)?;
        if group_model.status != GroupStatus::Active.as_str() {
            return Err(GroupsError::Conflict("group is not active".to_string()));
        }

        let effective = resolve_group_membership_enforcement(
            &transaction,
            tenant_id,
            request.group_id,
            user_id,
            Utc::now(),
        )
        .await?;
        if effective.denied_reentry {
            return Err(GroupsError::Forbidden(
                "group membership is suspended or banned".to_string(),
            ));
        }

        let existing = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(request.group_id))
            .filter(membership::Column::UserId.eq(user_id))
            .one(&transaction)
            .await?;
        if effective.active_member {
            return existing
                .map(map_membership)
                .transpose()?
                .ok_or_else(|| {
                    GroupsError::Invariant(
                        "effective active membership is missing its owner row".to_string(),
                    )
                });
        }

        let visibility = GroupVisibility::from_str(&group_model.visibility)
            .map_err(GroupsError::Invariant)?;
        let join_policy = GroupJoinPolicy::from_str(&group_model.join_policy)
            .map_err(GroupsError::Invariant)?;
        let target_status = match (visibility, join_policy, existing.as_ref()) {
            (_, _, Some(row)) if row.status == GroupMembershipStatus::Invited.as_str() => {
                GroupMembershipStatus::Active
            }
            (GroupVisibility::Public, GroupJoinPolicy::Open, _) => GroupMembershipStatus::Active,
            (GroupVisibility::Secret, _, _) | (_, GroupJoinPolicy::InviteOnly, _) => {
                return Err(GroupsError::Forbidden(
                    "group requires an invitation".to_string(),
                ));
            }
            _ => GroupMembershipStatus::Pending,
        };

        let now = Utc::now().fixed_offset();
        let updated_membership = if let Some(existing) = existing {
            let mut active: membership::ActiveModel = existing.into();
            active.role = Set(GroupRole::Member.as_str().to_string());
            active.status = Set(target_status.as_str().to_string());
            active.joined_at = Set((target_status == GroupMembershipStatus::Active).then_some(now));
            active.left_at = Set(None);
            active.updated_at = Set(now);
            active.update(&transaction).await?
        } else {
            membership::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(request.group_id),
                user_id: Set(user_id),
                role: Set(GroupRole::Member.as_str().to_string()),
                status: Set(target_status.as_str().to_string()),
                invited_by_user_id: Set(None),
                joined_at: Set((target_status == GroupMembershipStatus::Active).then_some(now)),
                left_at: Set(None),
                metadata: Set(json!({})),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(&transaction)
            .await?
        };

        if target_status == GroupMembershipStatus::Active {
            let next_member_count = group_model.member_count.saturating_add(1);
            let next_version = group_model.version.saturating_add(1);
            let mut active: group::ActiveModel = group_model.into();
            active.member_count = Set(next_member_count);
            active.version = Set(next_version);
            active.updated_at = Set(now);
            active.update(&transaction).await?;
        }

        transaction.commit().await?;
        map_membership(updated_membership)
    }

    async fn set_group_feature_owned(
        &self,
        context: &PortContext,
        request: SetGroupFeatureRequest,
    ) -> GroupsResult<GroupFeatureBinding> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_id = actor_user_id(context)?;
        self.group_model(tenant_id, request.group_id).await?;

        let effective = resolve_group_membership_enforcement(
            &self.db,
            tenant_id,
            request.group_id,
            actor_id,
            Utc::now(),
        )
        .await?;
        let local_allowed = effective.active_member
            && effective.role.is_some_and(GroupRole::can_manage_settings);
        if !local_allowed && !has_platform_manage(context) {
            return Err(GroupsError::Forbidden(
                "active group owner or administrator role is required".to_string(),
            ));
        }

        let feature_key =
            normalize_feature_key(&request.feature_key).map_err(GroupsError::Validation)?;
        let owner_module = feature_key
            .split_once('.')
            .map(|(owner, _)| owner.to_string())
            .ok_or_else(|| GroupsError::Invariant("feature key lost namespace".to_string()))?;
        let status = if request.enabled {
            GroupFeatureStatus::Enabled
        } else {
            GroupFeatureStatus::Disabled
        };
        let now = Utc::now().fixed_offset();
        let existing = feature_binding::Entity::find()
            .filter(feature_binding::Column::TenantId.eq(tenant_id))
            .filter(feature_binding::Column::GroupId.eq(request.group_id))
            .filter(feature_binding::Column::FeatureKey.eq(feature_key.clone()))
            .one(&self.db)
            .await?;

        let model = if let Some(existing) = existing {
            let mut active: feature_binding::ActiveModel = existing.into();
            active.contract_version = Set(request.contract_version);
            active.status = Set(status.as_str().to_string());
            active.sort_order = Set(request.sort_order);
            active.configuration = Set(request.configuration);
            active.updated_at = Set(now);
            active.update(&self.db).await?
        } else {
            feature_binding::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(request.group_id),
                feature_key: Set(feature_key),
                owner_module: Set(owner_module),
                contract_version: Set(request.contract_version),
                status: Set(status.as_str().to_string()),
                sort_order: Set(request.sort_order),
                configuration: Set(request.configuration),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(&self.db)
            .await?
        };
        map_feature(model)
    }
}

#[async_trait]
impl GroupSummaryReadPort for GroupsService {
    async fn read_group(
        &self,
        context: PortContext,
        request: ReadGroupRequest,
    ) -> Result<GroupDetails, PortError> {
        let mut details =
            GroupSummaryReadPort::read_group(&self.legacy, context.clone(), request).await?;
        let summary_decision = self
            .decide_access_owned(&context, details.summary.id, GroupAction::ViewSummary)
            .await
            .map_err(PortError::from)?;
        if !summary_decision.allowed {
            return Err(PortError::not_found(
                "groups.not_found",
                "group was not found",
            ));
        }
        let content_decision = self
            .decide_access_owned(&context, details.summary.id, GroupAction::View)
            .await
            .map_err(PortError::from)?;
        if !content_decision.allowed {
            details.body = None;
            details.features.clear();
        }
        Ok(details)
    }

    async fn list_groups(
        &self,
        context: PortContext,
        request: ListGroupsRequest,
    ) -> Result<GroupConnection, PortError> {
        GroupSummaryReadPort::list_groups(&self.legacy, context, request).await
    }
}

#[async_trait]
impl GroupMembershipReadPort for GroupsService {
    async fn read_membership(
        &self,
        context: PortContext,
        request: ReadGroupMembershipRequest,
    ) -> Result<Option<GroupMembership>, PortError> {
        GroupMembershipReadPort::read_membership(&self.legacy, context, request).await
    }

    async fn list_memberships(
        &self,
        context: PortContext,
        request: ListGroupMembershipsRequest,
    ) -> Result<GroupMembershipConnection, PortError> {
        let decision = self
            .decide_access_owned(&context, request.group_id, GroupAction::ViewMembers)
            .await
            .map_err(PortError::from)?;
        if !decision.allowed {
            return Err(PortError::forbidden(
                "groups.memberships_forbidden",
                "group memberships are not visible",
            ));
        }
        GroupMembershipReadPort::list_memberships(&self.legacy, context, request).await
    }
}

#[async_trait]
impl GroupAccessReadPort for GroupsService {
    async fn decide_group_access(
        &self,
        context: PortContext,
        request: GroupAccessRequest,
    ) -> Result<GroupAccessDecision, PortError> {
        self.decide_access_owned(&context, request.group_id, request.action)
            .await
            .map_err(Into::into)
    }

    async fn enabled_group_features(
        &self,
        context: PortContext,
        group_id: Uuid,
    ) -> Result<Vec<GroupFeatureBinding>, PortError> {
        let decision = self
            .decide_access_owned(&context, group_id, GroupAction::View)
            .await
            .map_err(PortError::from)?;
        if !decision.allowed {
            return Err(PortError::forbidden(
                "groups.features_forbidden",
                "group features are not visible",
            ));
        }

        let tenant_id = context_tenant_id(&context).map_err(PortError::from)?;
        feature_binding::Entity::find()
            .filter(feature_binding::Column::TenantId.eq(tenant_id))
            .filter(feature_binding::Column::GroupId.eq(group_id))
            .filter(feature_binding::Column::Status.eq(GroupFeatureStatus::Enabled.as_str()))
            .order_by_asc(feature_binding::Column::SortOrder)
            .all(&self.db)
            .await
            .map_err(|error| {
                PortError::unavailable("groups.features_unavailable", error.to_string())
            })?
            .into_iter()
            .map(map_feature)
            .collect::<GroupsResult<Vec<_>>>()
            .map_err(Into::into)
    }
}

#[async_trait]
impl GroupCommandPort for GroupsService {
    async fn create_group(
        &self,
        context: PortContext,
        input: CreateGroupInput,
    ) -> Result<GroupDetails, PortError> {
        GroupCommandPort::create_group(&self.legacy, context, input).await
    }

    async fn join_group(
        &self,
        context: PortContext,
        request: JoinGroupRequest,
    ) -> Result<GroupMembership, PortError> {
        self.join_group_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn leave_group(
        &self,
        context: PortContext,
        request: LeaveGroupRequest,
    ) -> Result<GroupMembership, PortError> {
        GroupCommandPort::leave_group(&self.legacy, context, request).await
    }

    async fn set_group_feature(
        &self,
        context: PortContext,
        request: SetGroupFeatureRequest,
    ) -> Result<GroupFeatureBinding, PortError> {
        self.set_group_feature_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

fn access_allowed(
    platform_manage: bool,
    group_active: bool,
    visibility: GroupVisibility,
    action: GroupAction,
    effective_status: GroupMembershipEffectiveStatus,
    membership_role: Option<GroupRole>,
) -> bool {
    if platform_manage {
        return true;
    }
    if !group_active {
        return false;
    }

    let active_member = effective_status.is_active_member();
    match action {
        GroupAction::Discover => visibility != GroupVisibility::Secret,
        GroupAction::ViewSummary => visibility != GroupVisibility::Secret || active_member,
        GroupAction::View | GroupAction::ViewMembers => {
            visibility == GroupVisibility::Public || active_member
        }
        GroupAction::Join => {
            visibility != GroupVisibility::Secret && !effective_status.denies_reentry()
        }
        GroupAction::Post | GroupAction::Comment => active_member,
        GroupAction::Invite | GroupAction::ReviewMemberships | GroupAction::Moderate => {
            active_member && membership_role.is_some_and(GroupRole::can_moderate)
        }
        GroupAction::ManageFeatures | GroupAction::ManageSettings => {
            active_member && membership_role.is_some_and(GroupRole::can_manage_settings)
        }
        GroupAction::TransferOwnership => {
            active_member && membership_role == Some(GroupRole::Owner)
        }
    }
}

fn access_reason_code(
    allowed: bool,
    effective_status: GroupMembershipEffectiveStatus,
) -> &'static str {
    if allowed {
        "groups.access.allowed"
    } else {
        match effective_status {
            GroupMembershipEffectiveStatus::Suspended => "groups.access.membership_suspended",
            GroupMembershipEffectiveStatus::LegacyBanned => "groups.access.membership_banned",
            _ => "groups.access.denied",
        }
    }
}

fn require_read(context: &PortContext) -> GroupsResult<()> {
    context
        .require_policy(PortCallPolicy::read())
        .map_err(|error| GroupsError::Validation(error.message))
}

fn require_write(context: &PortContext) -> GroupsResult<()> {
    context
        .require_policy(PortCallPolicy::write())
        .map_err(|error| GroupsError::Validation(error.message))
}

fn context_tenant_id(context: &PortContext) -> GroupsResult<Uuid> {
    parse_uuid(&context.tenant_id, "tenant_id")
}

fn actor_user_id(context: &PortContext) -> GroupsResult<Uuid> {
    if context.actor.kind != PortActorKind::User {
        return Err(GroupsError::Forbidden(
            "a user actor is required".to_string(),
        ));
    }
    parse_uuid(&context.actor.id, "actor.id")
}

fn optional_actor_user_id(context: &PortContext) -> Option<Uuid> {
    (context.actor.kind == PortActorKind::User)
        .then(|| Uuid::parse_str(&context.actor.id).ok())
        .flatten()
}

fn parse_uuid(value: &str, field: &str) -> GroupsResult<Uuid> {
    Uuid::parse_str(value).map_err(|_| GroupsError::Validation(format!("{field} must be a UUID")))
}

fn has_platform_manage(context: &PortContext) -> bool {
    context
        .claims
        .iter()
        .any(|claim| matches!(claim.as_str(), "groups:manage" | "groups:*" | "*:*"))
}

fn map_membership(model: membership::Model) -> GroupsResult<GroupMembership> {
    Ok(GroupMembership {
        id: model.id,
        group_id: model.group_id,
        user_id: model.user_id,
        role: GroupRole::from_str(&model.role).map_err(GroupsError::Invariant)?,
        status: GroupMembershipStatus::from_str(&model.status)
            .map_err(GroupsError::Invariant)?,
    })
}

fn map_feature(model: feature_binding::Model) -> GroupsResult<GroupFeatureBinding> {
    Ok(GroupFeatureBinding {
        id: model.id,
        group_id: model.group_id,
        feature_key: model.feature_key,
        owner_module: model.owner_module,
        contract_version: model.contract_version,
        status: GroupFeatureStatus::from_str(&model.status).map_err(GroupsError::Invariant)?,
        sort_order: model.sort_order,
        configuration: model.configuration,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspended_membership_is_not_private_or_manager_authority() {
        assert!(!access_allowed(
            false,
            true,
            GroupVisibility::Closed,
            GroupAction::View,
            GroupMembershipEffectiveStatus::Suspended,
            Some(GroupRole::Admin),
        ));
        assert!(!access_allowed(
            false,
            true,
            GroupVisibility::Closed,
            GroupAction::ManageSettings,
            GroupMembershipEffectiveStatus::Suspended,
            Some(GroupRole::Admin),
        ));
        assert!(!access_allowed(
            false,
            true,
            GroupVisibility::Secret,
            GroupAction::ViewSummary,
            GroupMembershipEffectiveStatus::Suspended,
            Some(GroupRole::Owner),
        ));
    }

    #[test]
    fn public_read_survives_suspension_but_mutation_does_not() {
        assert!(access_allowed(
            false,
            true,
            GroupVisibility::Public,
            GroupAction::View,
            GroupMembershipEffectiveStatus::Suspended,
            Some(GroupRole::Member),
        ));
        assert!(!access_allowed(
            false,
            true,
            GroupVisibility::Public,
            GroupAction::Post,
            GroupMembershipEffectiveStatus::Suspended,
            Some(GroupRole::Member),
        ));
        assert!(!access_allowed(
            false,
            true,
            GroupVisibility::Public,
            GroupAction::Join,
            GroupMembershipEffectiveStatus::LegacyBanned,
            Some(GroupRole::Member),
        ));
    }
}
