use std::collections::HashMap;
use std::str::FromStr;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError, normalize_locale_tag};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::domain::{
    GroupAction, GroupFeatureStatus, GroupJoinPolicy, GroupMembershipStatus, GroupRole,
    GroupStatus, GroupVisibility, normalize_feature_key, normalize_group_handle,
};
use crate::dto::*;
use crate::entities::{feature_binding, group, membership, translation};
use crate::error::{GroupsError, GroupsResult};
use crate::ports::{
    GroupAccessReadPort, GroupCommandPort, GroupMembershipReadPort, GroupSummaryReadPort,
};

#[derive(Clone)]
pub struct GroupsService {
    db: DatabaseConnection,
}

impl GroupsService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn group_model(&self, tenant_id: Uuid, group_id: Uuid) -> GroupsResult<group::Model> {
        group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Id.eq(group_id))
            .one(&self.db)
            .await?
            .ok_or(GroupsError::NotFound)
    }

    async fn membership_model(
        &self,
        tenant_id: Uuid,
        group_id: Uuid,
        user_id: Uuid,
    ) -> GroupsResult<Option<membership::Model>> {
        Ok(membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(group_id))
            .filter(membership::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?)
    }

    async fn load_translations(
        &self,
        tenant_id: Uuid,
        group_ids: Vec<Uuid>,
    ) -> GroupsResult<HashMap<Uuid, Vec<translation::Model>>> {
        if group_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut grouped = HashMap::<Uuid, Vec<translation::Model>>::new();
        for row in translation::Entity::find()
            .filter(translation::Column::TenantId.eq(tenant_id))
            .filter(translation::Column::GroupId.is_in(group_ids))
            .all(&self.db)
            .await?
        {
            grouped.entry(row.group_id).or_default().push(row);
        }
        Ok(grouped)
    }

    async fn create_group_owned(
        &self,
        context: &PortContext,
        input: CreateGroupInput,
    ) -> GroupsResult<GroupDetails> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let owner_user_id = actor_user_id(context)?;
        let handle = normalize_group_handle(&input.handle).map_err(GroupsError::Validation)?;
        let locale = normalize_locale_tag(&input.locale)
            .ok_or_else(|| GroupsError::Validation("invalid group locale".to_string()))?;
        let title = input.title.trim();
        if title.is_empty() || title.chars().count() > 240 {
            return Err(GroupsError::Validation(
                "group title must contain between 1 and 240 characters".to_string(),
            ));
        }
        let summary = normalize_optional_text(input.summary);
        if summary
            .as_deref()
            .is_some_and(|value| value.chars().count() > 500)
        {
            return Err(GroupsError::Validation(
                "group summary must not exceed 500 characters".to_string(),
            ));
        }
        let body = normalize_optional_text(input.body);
        let metadata = normalize_language_agnostic_metadata(input.metadata)?;

        let transaction = self.db.begin().await?;
        let duplicate = group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Handle.eq(handle.clone()))
            .one(&transaction)
            .await?;
        if duplicate.is_some() {
            return Err(GroupsError::HandleConflict);
        }

        let now = Utc::now().fixed_offset();
        let group_id = Uuid::new_v4();
        group::ActiveModel {
            id: Set(group_id),
            tenant_id: Set(tenant_id),
            owner_user_id: Set(owner_user_id),
            handle: Set(handle),
            visibility: Set(input.visibility.as_str().to_string()),
            join_policy: Set(input.join_policy.as_str().to_string()),
            status: Set(GroupStatus::Active.as_str().to_string()),
            category_id: Set(input.category_id),
            avatar_media_id: Set(input.avatar_media_id),
            cover_media_id: Set(input.cover_media_id),
            member_count: Set(1),
            version: Set(1),
            metadata: Set(metadata),
            created_at: Set(now),
            updated_at: Set(now),
            archived_at: Set(None),
        }
        .insert(&transaction)
        .await?;

        translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            group_id: Set(group_id),
            locale: Set(locale.clone()),
            title: Set(title.to_string()),
            summary: Set(summary),
            body: Set(body),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&transaction)
        .await?;

        membership::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            group_id: Set(group_id),
            user_id: Set(owner_user_id),
            role: Set(GroupRole::Owner.as_str().to_string()),
            status: Set(GroupMembershipStatus::Active.as_str().to_string()),
            invited_by_user_id: Set(None),
            joined_at: Set(Some(now)),
            left_at: Set(None),
            metadata: Set(json!({})),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&transaction)
        .await?;

        transaction.commit().await?;
        let model = self.group_model(tenant_id, group_id).await?;
        self.map_details(context, model, true, &locale, &locale)
            .await
    }

    async fn read_group_owned(
        &self,
        context: &PortContext,
        request: ReadGroupRequest,
    ) -> GroupsResult<GroupDetails> {
        require_read(context)?;
        self.read_group_for_locale_owned(context, request, &context.locale)
            .await
    }

    async fn read_group_for_locale_owned(
        &self,
        context: &PortContext,
        request: ReadGroupRequest,
        requested_locale: &str,
    ) -> GroupsResult<GroupDetails> {
        let tenant_id = context_tenant_id(context)?;
        let effective_locale = normalize_effective_locale(requested_locale)?;
        let mut query = group::Entity::find().filter(group::Column::TenantId.eq(tenant_id));
        query = match (request.group_id, request.handle) {
            (Some(group_id), _) => query.filter(group::Column::Id.eq(group_id)),
            (None, Some(handle)) => query.filter(
                group::Column::Handle
                    .eq(normalize_group_handle(&handle).map_err(GroupsError::Validation)?),
            ),
            (None, None) => {
                return Err(GroupsError::Validation(
                    "group id or handle is required".to_string(),
                ));
            }
        };

        let model = query.one(&self.db).await?.ok_or(GroupsError::NotFound)?;
        let summary_decision = self
            .decide_access_owned(context, model.id, GroupAction::ViewSummary)
            .await?;
        if !summary_decision.allowed {
            return Err(GroupsError::NotFound);
        }
        let content_decision = self
            .decide_access_owned(context, model.id, GroupAction::View)
            .await?;
        self.map_details(
            context,
            model,
            content_decision.allowed,
            requested_locale,
            &effective_locale,
        )
        .await
    }

    async fn list_groups_owned(
        &self,
        context: &PortContext,
        request: ListGroupsRequest,
    ) -> GroupsResult<GroupConnection> {
        require_read(context)?;
        let tenant_id = context_tenant_id(context)?;
        let effective_locale = normalize_effective_locale(&context.locale)?;
        let page = request.page.max(1);
        let per_page = request.per_page.clamp(1, 100);
        let include_non_public = request.include_non_public && has_platform_manage(context);

        let mut localized_query = translation::Entity::find()
            .filter(translation::Column::TenantId.eq(tenant_id))
            .filter(translation::Column::Locale.eq(effective_locale.clone()));
        if let Some(search) = normalize_optional_text(request.search) {
            localized_query = localized_query.filter(translation::Column::Title.contains(&search));
        }
        let localized_group_ids = localized_query
            .all(&self.db)
            .await?
            .into_iter()
            .map(|row| row.group_id)
            .collect::<Vec<_>>();
        if localized_group_ids.is_empty() {
            return Ok(GroupConnection {
                items: Vec::new(),
                total: 0,
                page,
                per_page,
            });
        }

        let mut query = group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Status.eq(GroupStatus::Active.as_str()))
            .filter(group::Column::Id.is_in(localized_group_ids));
        if !include_non_public {
            query = query.filter(group::Column::Visibility.is_in([
                GroupVisibility::Public.as_str(),
                GroupVisibility::Closed.as_str(),
            ]));
        }

        let paginator = query
            .order_by_desc(group::Column::UpdatedAt)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let models = paginator.fetch_page(page.saturating_sub(1)).await?;
        let translations = self
            .load_translations(
                tenant_id,
                models.iter().map(|model| model.id).collect::<Vec<_>>(),
            )
            .await?;
        let items = models
            .into_iter()
            .map(|model| {
                self.map_summary(
                    context,
                    model,
                    &translations,
                    &context.locale,
                    &effective_locale,
                )
            })
            .collect::<GroupsResult<Vec<_>>>()?;

        Ok(GroupConnection {
            items,
            total,
            page,
            per_page,
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

        let existing = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(request.group_id))
            .filter(membership::Column::UserId.eq(user_id))
            .one(&transaction)
            .await?;
        if existing
            .as_ref()
            .is_some_and(|row| row.status == GroupMembershipStatus::Banned.as_str())
        {
            return Err(GroupsError::Forbidden(
                "group membership is banned".to_string(),
            ));
        }
        if let Some(active) = existing
            .as_ref()
            .filter(|row| row.status == GroupMembershipStatus::Active.as_str())
        {
            return map_membership(active.clone());
        }

        let visibility = parse_visibility(&group_model.visibility)?;
        let join_policy = parse_join_policy(&group_model.join_policy)?;
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

    async fn leave_group_owned(
        &self,
        context: &PortContext,
        request: LeaveGroupRequest,
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
        let membership_model = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(request.group_id))
            .filter(membership::Column::UserId.eq(user_id))
            .one(&transaction)
            .await?
            .ok_or_else(|| GroupsError::Conflict("membership is required".to_string()))?;
        if membership_model.role == GroupRole::Owner.as_str() {
            return Err(GroupsError::Invariant(
                "group owner must transfer ownership before leaving".to_string(),
            ));
        }
        if membership_model.status == GroupMembershipStatus::Left.as_str() {
            return map_membership(membership_model);
        }

        let was_active = membership_model.status == GroupMembershipStatus::Active.as_str();
        let now = Utc::now().fixed_offset();
        let mut active: membership::ActiveModel = membership_model.into();
        active.status = Set(GroupMembershipStatus::Left.as_str().to_string());
        active.left_at = Set(Some(now));
        active.updated_at = Set(now);
        let updated_membership = active.update(&transaction).await?;

        if was_active {
            let next_member_count = group_model.member_count.saturating_sub(1);
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

        let membership = self
            .membership_model(tenant_id, request.group_id, actor_id)
            .await?;
        let local_allowed = membership
            .as_ref()
            .filter(|row| row.status == GroupMembershipStatus::Active.as_str())
            .and_then(|row| GroupRole::from_str(&row.role).ok())
            .is_some_and(GroupRole::can_manage_settings);
        if !local_allowed && !has_platform_manage(context) {
            return Err(GroupsError::Forbidden(
                "group owner or administrator role is required".to_string(),
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

    async fn decide_access_owned(
        &self,
        context: &PortContext,
        group_id: Uuid,
        action: GroupAction,
    ) -> GroupsResult<GroupAccessDecision> {
        require_read(context)?;
        let tenant_id = context_tenant_id(context)?;
        let group_model = self.group_model(tenant_id, group_id).await?;
        let visibility = parse_visibility(&group_model.visibility)?;
        let membership = match optional_actor_user_id(context) {
            Some(user_id) => self.membership_model(tenant_id, group_id, user_id).await?,
            None => None,
        };
        let membership_role = membership
            .as_ref()
            .and_then(|row| GroupRole::from_str(&row.role).ok());
        let membership_status = membership
            .as_ref()
            .and_then(|row| GroupMembershipStatus::from_str(&row.status).ok());
        let active_member = membership_status == Some(GroupMembershipStatus::Active);

        let allowed = if has_platform_manage(context) {
            true
        } else if group_model.status != GroupStatus::Active.as_str() {
            false
        } else {
            match action {
                GroupAction::Discover => visibility != GroupVisibility::Secret,
                GroupAction::ViewSummary => visibility != GroupVisibility::Secret || active_member,
                GroupAction::View | GroupAction::ViewMembers => {
                    visibility == GroupVisibility::Public || active_member
                }
                GroupAction::Join => {
                    visibility != GroupVisibility::Secret
                        && membership_status != Some(GroupMembershipStatus::Banned)
                }
                GroupAction::Post | GroupAction::Comment => active_member,
                GroupAction::Invite | GroupAction::ReviewMemberships | GroupAction::Moderate => {
                    active_member && membership_role.is_some_and(GroupRole::can_moderate)
                }
                GroupAction::ManageFeatures | GroupAction::ManageSettings => {
                    active_member && membership_role.is_some_and(GroupRole::can_manage_settings)
                }
                GroupAction::TransferOwnership => membership_role == Some(GroupRole::Owner),
            }
        };

        Ok(GroupAccessDecision {
            group_id,
            action,
            allowed,
            reason_code: if allowed {
                "groups.access.allowed"
            } else {
                "groups.access.denied"
            }
            .to_string(),
            membership_role,
            membership_status,
        })
    }

    async fn map_details(
        &self,
        context: &PortContext,
        model: group::Model,
        include_private_content: bool,
        requested_locale: &str,
        effective_locale: &str,
    ) -> GroupsResult<GroupDetails> {
        let group_id = model.id;
        let tenant_id = model.tenant_id;
        let translations = self.load_translations(tenant_id, vec![group_id]).await?;
        let selected = select_translation(&translations, group_id, effective_locale)?;
        let body = include_private_content
            .then(|| selected.body.clone())
            .flatten();
        let summary = self.map_summary(
            context,
            model,
            &translations,
            requested_locale,
            effective_locale,
        )?;
        let viewer_membership = match optional_actor_user_id(context) {
            Some(user_id) => self
                .membership_model(tenant_id, group_id, user_id)
                .await?
                .map(map_membership)
                .transpose()?,
            None => None,
        };
        let features = if include_private_content {
            feature_binding::Entity::find()
                .filter(feature_binding::Column::TenantId.eq(tenant_id))
                .filter(feature_binding::Column::GroupId.eq(group_id))
                .order_by_asc(feature_binding::Column::SortOrder)
                .all(&self.db)
                .await?
                .into_iter()
                .map(map_feature)
                .collect::<GroupsResult<Vec<_>>>()?
        } else {
            Vec::new()
        };

        Ok(GroupDetails {
            summary,
            body,
            viewer_membership,
            features,
        })
    }

    fn map_summary(
        &self,
        _context: &PortContext,
        model: group::Model,
        translations: &HashMap<Uuid, Vec<translation::Model>>,
        requested_locale: &str,
        effective_locale: &str,
    ) -> GroupsResult<GroupSummary> {
        let selected = select_translation(translations, model.id, effective_locale)?;
        let mut available_locales = translations
            .get(&model.id)
            .into_iter()
            .flatten()
            .map(|row| row.locale.clone())
            .collect::<Vec<_>>();
        available_locales.sort();
        available_locales.dedup();

        Ok(GroupSummary {
            id: model.id,
            tenant_id: model.tenant_id,
            owner_user_id: model.owner_user_id,
            handle: model.handle,
            visibility: parse_visibility(&model.visibility)?,
            join_policy: parse_join_policy(&model.join_policy)?,
            status: GroupStatus::from_str(&model.status).map_err(GroupsError::Invariant)?,
            title: selected.title.clone(),
            summary: selected.summary.clone(),
            avatar_media_id: model.avatar_media_id,
            cover_media_id: model.cover_media_id,
            member_count: model.member_count.max(0) as u64,
            requested_locale: requested_locale.to_string(),
            effective_locale: selected.locale.clone(),
            available_locales,
        })
    }
}

#[async_trait]
impl GroupSummaryReadPort for GroupsService {
    async fn read_group(
        &self,
        context: PortContext,
        request: ReadGroupRequest,
    ) -> Result<GroupDetails, PortError> {
        self.read_group_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn list_groups(
        &self,
        context: PortContext,
        request: ListGroupsRequest,
    ) -> Result<GroupConnection, PortError> {
        self.list_groups_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

#[async_trait]
impl GroupMembershipReadPort for GroupsService {
    async fn read_membership(
        &self,
        context: PortContext,
        request: ReadGroupMembershipRequest,
    ) -> Result<Option<GroupMembership>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = context_tenant_id(&context).map_err(PortError::from)?;
        self.membership_model(tenant_id, request.group_id, request.user_id)
            .await
            .map_err(PortError::from)?
            .map(map_membership)
            .transpose()
            .map_err(Into::into)
    }

    async fn list_memberships(
        &self,
        context: PortContext,
        request: ListGroupMembershipsRequest,
    ) -> Result<GroupMembershipConnection, PortError> {
        context.require_policy(PortCallPolicy::read())?;
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

        let tenant_id = context_tenant_id(&context).map_err(PortError::from)?;
        let page = request.page.max(1);
        let per_page = request.per_page.clamp(1, 100);
        let paginator = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(request.group_id))
            .filter(membership::Column::Status.ne(GroupMembershipStatus::Left.as_str()))
            .order_by_asc(membership::Column::CreatedAt)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await.map_err(|error| {
            PortError::unavailable("groups.memberships_unavailable", error.to_string())
        })?;
        let items = paginator
            .fetch_page(page.saturating_sub(1))
            .await
            .map_err(|error| {
                PortError::unavailable("groups.memberships_unavailable", error.to_string())
            })?
            .into_iter()
            .map(map_membership)
            .collect::<GroupsResult<Vec<_>>>()
            .map_err(PortError::from)?;

        Ok(GroupMembershipConnection {
            items,
            total,
            page,
            per_page,
        })
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
        self.create_group_owned(&context, input)
            .await
            .map_err(Into::into)
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
        self.leave_group_owned(&context, request)
            .await
            .map_err(Into::into)
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

fn parse_visibility(value: &str) -> GroupsResult<GroupVisibility> {
    GroupVisibility::from_str(value).map_err(GroupsError::Invariant)
}

fn parse_join_policy(value: &str) -> GroupsResult<GroupJoinPolicy> {
    GroupJoinPolicy::from_str(value).map_err(GroupsError::Invariant)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn normalize_effective_locale(value: &str) -> GroupsResult<String> {
    normalize_locale_tag(value)
        .ok_or_else(|| GroupsError::Validation("host effective locale is invalid".to_string()))
}

fn normalize_language_agnostic_metadata(value: Value) -> GroupsResult<Value> {
    const LOCALIZED_COPY_KEYS: &[&str] = &[
        "title",
        "summary",
        "body",
        "name",
        "description",
        "translations",
        "localized",
        "locales",
        "i18n",
        "seo",
    ];

    let object = value.as_object().ok_or_else(|| {
        GroupsError::Validation("group metadata must be a JSON object".to_string())
    })?;
    if let Some(key) = LOCALIZED_COPY_KEYS
        .iter()
        .find(|key| object.contains_key(**key))
    {
        return Err(GroupsError::Validation(format!(
            "group metadata must remain language-agnostic; localized presentation key `{key}` belongs in group_translations"
        )));
    }
    Ok(value)
}

fn has_platform_manage(context: &PortContext) -> bool {
    context
        .claims
        .iter()
        .any(|claim| matches!(claim.as_str(), "groups:manage" | "groups:*" | "*:*"))
}

fn select_translation<'a>(
    translations: &'a HashMap<Uuid, Vec<translation::Model>>,
    group_id: Uuid,
    effective_locale: &str,
) -> GroupsResult<&'a translation::Model> {
    let effective_locale = normalize_effective_locale(effective_locale)?;
    translations
        .get(&group_id)
        .and_then(|rows| rows.iter().find(|row| row.locale == effective_locale))
        .ok_or(GroupsError::NotFound)
}

fn map_membership(model: membership::Model) -> GroupsResult<GroupMembership> {
    Ok(GroupMembership {
        id: model.id,
        group_id: model.group_id,
        user_id: model.user_id,
        role: GroupRole::from_str(&model.role).map_err(GroupsError::Invariant)?,
        status: GroupMembershipStatus::from_str(&model.status).map_err(GroupsError::Invariant)?,
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
    fn base_metadata_rejects_localized_presentation_copy() {
        assert!(normalize_language_agnostic_metadata(json!({"flags": ["featured"]})).is_ok());
        assert!(normalize_language_agnostic_metadata(json!({"title": "Localized copy"})).is_err());
        assert!(normalize_language_agnostic_metadata(json!({"translations": {"ru": {}}})).is_err());
    }
}
