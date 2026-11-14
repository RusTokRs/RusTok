use std::collections::HashMap;
use std::str::FromStr;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError, normalize_locale_tag};
#[cfg(test)]
use sea_orm::Database;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, QueryTrait, Set, TransactionTrait,
};
use serde_json::{Value, json};
use uuid::Uuid;

use std::sync::Arc;

use crate::domain::{
    GroupAction, GroupFeatureStatus, GroupJoinPolicy, GroupMembershipStatus, GroupRole,
    GroupStatus, GroupVisibility, normalize_group_handle,
};
use crate::dto::*;
use crate::entities::{feature_binding, group, membership, translation};
use crate::error::{GroupsError, GroupsResult};
use crate::ports::taxonomy::{GroupTaxonomyPort, NoopGroupTaxonomyPort};
use crate::ports::{
    GroupAccessReadPort, GroupCommandPort, GroupMembershipReadPort, GroupSummaryReadPort,
};

#[derive(Clone)]
pub struct GroupsService {
    db: DatabaseConnection,
    taxonomy_port: Arc<dyn GroupTaxonomyPort>,
}

impl GroupsService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            taxonomy_port: Arc::new(NoopGroupTaxonomyPort),
        }
    }

    pub fn with_taxonomy_port(mut self, port: Arc<dyn GroupTaxonomyPort>) -> Self {
        self.taxonomy_port = port;
        self
    }

    pub async fn lock_membership_enforcement_target_by_id(
        &self,
        tenant_id: Uuid,
        membership_id: Uuid,
    ) -> GroupsResult<Option<GroupMembershipEffectiveState>> {
        if tenant_id.is_nil() {
            return Err(GroupsError::Validation(
                "tenant_id cannot be nil UUID".to_string(),
            ));
        }
        if membership_id.is_nil() {
            return Err(GroupsError::Validation(
                "membership_id cannot be nil UUID".to_string(),
            ));
        }
        let transaction = self.db.begin().await?;
        let target = crate::membership_enforcement_transaction::lock_membership_enforcement_target_by_id_for_update(
            &transaction,
            tenant_id,
            membership_id,
        )
        .await?;
        let result = if let Some(target) = target {
            let _enforcement = target.enforcement();
            Some(
                crate::membership_enforcement::resolve_group_membership_enforcement(
                    &transaction,
                    tenant_id,
                    target.group().id,
                    target.membership().user_id,
                    Utc::now(),
                )
                .await?,
            )
        } else {
            None
        };
        transaction.commit().await?;
        Ok(result)
    }

    async fn group_model(&self, tenant_id: Uuid, group_id: Uuid) -> GroupsResult<group::Model> {
        if tenant_id.is_nil() {
            return Err(GroupsError::Validation(
                "tenant_id cannot be nil UUID".to_string(),
            ));
        }
        if group_id.is_nil() {
            return Err(GroupsError::Validation(
                "group_id cannot be nil UUID".to_string(),
            ));
        }
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
        if tenant_id.is_nil() {
            return Err(GroupsError::Validation(
                "tenant_id cannot be nil UUID".to_string(),
            ));
        }
        if group_id.is_nil() {
            return Err(GroupsError::Validation(
                "group_id cannot be nil UUID".to_string(),
            ));
        }
        if user_id.is_nil() {
            return Err(GroupsError::Validation(
                "user_id cannot be nil UUID".to_string(),
            ));
        }
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
        if tenant_id.is_nil() {
            return Err(GroupsError::Validation(
                "tenant_id cannot be nil UUID".to_string(),
            ));
        }
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
        input.validate()?;
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
        let command_input = input.clone();
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
        if let Some(category_id) = input.category_id {
            if category_id.is_nil() {
                return Err(GroupsError::Validation(
                    "category_id cannot be a nil UUID".to_string(),
                ));
            }
        }

        let transaction = self.db.begin().await?;
        let command = crate::commands::Command::admit(
            &transaction,
            context,
            tenant_id,
            owner_user_id,
            "groups.create.v1",
            &command_input,
        )
        .await?;
        if let Some(result) = command.replay::<GroupDetails>(&transaction).await? {
            transaction.commit().await?;
            return Ok(result);
        }
        let duplicate = group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Handle.eq(handle.clone()))
            .one(&transaction)
            .await?;
        if duplicate.is_some() {
            return Err(GroupsError::HandleConflict);
        }

        if let Some(category_id) = input.category_id {
            let valid = self
                .taxonomy_port
                .validate_category_exists((&transaction).into(), tenant_id, category_id)
                .await?;
            if !valid {
                return Err(GroupsError::Validation(format!(
                    "Group category {category_id} does not exist"
                )));
            }
        }

        let now = Utc::now().fixed_offset();
        let group_id = Uuid::new_v4();
        group::ActiveModel {
            id: Set(group_id),
            tenant_id: Set(tenant_id),
            owner_user_id: Set(owner_user_id),
            handle: Set(handle.clone()),
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
        .await
        .map_err(|error| {
            if matches!(
                error.sql_err(),
                Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
            ) {
                GroupsError::HandleConflict
            } else {
                GroupsError::from(error)
            }
        })?;

        translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            group_id: Set(group_id),
            locale: Set(locale.clone()),
            title: Set(title.to_string()),
            summary: Set(summary.clone()),
            body: Set(body.clone()),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&transaction)
        .await?;

        let owner_membership = membership::ActiveModel {
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

        let category = if let Some(cat_id) = input.category_id {
            self.taxonomy_port
                .load_categories(
                    (&transaction).into(),
                    tenant_id,
                    &[cat_id],
                    &locale,
                    Some(rustok_api::PLATFORM_FALLBACK_LOCALE),
                )
                .await
                .ok()
                .and_then(|mut m| m.remove(&cat_id))
                .map(|snap| GroupCategorySummary {
                    id: snap.id,
                    name: snap.name,
                    slug: snap.slug,
                    icon_key: snap.icon_key,
                    color: snap.color,
                })
        } else {
            None
        };

        let result = GroupDetails {
            summary: GroupSummary {
                id: group_id,
                tenant_id,
                owner_user_id,
                handle,
                visibility: input.visibility,
                join_policy: input.join_policy,
                status: GroupStatus::Active,
                title: title.to_string(),
                summary,
                category_id: input.category_id,
                category,
                avatar_media_id: input.avatar_media_id,
                cover_media_id: input.cover_media_id,
                member_count: 1,
                requested_locale: locale.clone(),
                effective_locale: locale.clone(),
                available_locales: vec![locale],
            },
            body,
            viewer_membership: Some(map_membership(owner_membership)?),
            features: Vec::new(),
        };
        command
            .finish(&transaction, context, group_id, &result)
            .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn read_group_owned(
        &self,
        context: &PortContext,
        request: ReadGroupRequest,
    ) -> GroupsResult<GroupDetails> {
        require_read(context)?;
        request.validate()?;
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
        request.validate()?;
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
        let localized_groups = localized_query
            .select_only()
            .column(translation::Column::GroupId)
            .into_query();

        let mut query = group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Status.eq(GroupStatus::Active.as_str()))
            .filter(group::Column::Id.in_subquery(localized_groups));
        if !include_non_public {
            query = query.filter(group::Column::Visibility.is_in([
                GroupVisibility::Public.as_str(),
                GroupVisibility::Closed.as_str(),
            ]));
        }
        if let Some(category_id) = request.category_id {
            query = query.filter(group::Column::CategoryId.eq(category_id));
        }

        let paginator = query
            .order_by_desc(group::Column::UpdatedAt)
            .order_by_desc(group::Column::Id)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let models = paginator.fetch_page(page.saturating_sub(1)).await?;
        let translations = self
            .load_translations(
                tenant_id,
                models.iter().map(|model| model.id).collect::<Vec<_>>(),
            )
            .await?;
        let category_ids = models
            .iter()
            .filter_map(|model| model.category_id)
            .collect::<Vec<_>>();
        let categories = if category_ids.is_empty() {
            HashMap::new()
        } else {
            self.taxonomy_port
                .load_categories(
                    (&self.db).into(),
                    tenant_id,
                    &category_ids,
                    &effective_locale,
                    Some(rustok_api::PLATFORM_FALLBACK_LOCALE),
                )
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|(id, snap)| {
                    (
                        id,
                        GroupCategorySummary {
                            id: snap.id,
                            name: snap.name,
                            slug: snap.slug,
                            icon_key: snap.icon_key,
                            color: snap.color,
                        },
                    )
                })
                .collect::<HashMap<_, _>>()
        };
        let items = models
            .into_iter()
            .map(|model| {
                let category = model.category_id.and_then(|id| categories.get(&id).cloned());
                self.map_summary(
                    context,
                    model,
                    &translations,
                    &context.locale,
                    &effective_locale,
                    category,
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

    async fn decide_access_owned(
        &self,
        context: &PortContext,
        group_id: Uuid,
        action: GroupAction,
    ) -> GroupsResult<GroupAccessDecision> {
        GroupAccessReadPort::decide_group_access(
            &crate::GroupsService::new(self.db.clone()),
            context.clone(),
            GroupAccessRequest { group_id, action },
        )
        .await
        .map_err(Into::into)
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
        let category = if let Some(cat_id) = model.category_id {
            self.taxonomy_port
                .load_categories(
                    (&self.db).into(),
                    tenant_id,
                    &[cat_id],
                    effective_locale,
                    Some(rustok_api::PLATFORM_FALLBACK_LOCALE),
                )
                .await
                .ok()
                .and_then(|mut m| m.remove(&cat_id))
                .map(|snap| GroupCategorySummary {
                    id: snap.id,
                    name: snap.name,
                    slug: snap.slug,
                    icon_key: snap.icon_key,
                    color: snap.color,
                })
        } else {
            None
        };
        let summary = self.map_summary(
            context,
            model,
            &translations,
            requested_locale,
            effective_locale,
            category,
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
        category: Option<GroupCategorySummary>,
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
            category_id: model.category_id,
            category,
            avatar_media_id: model.avatar_media_id,
            cover_media_id: model.cover_media_id,
            member_count: u64::try_from(model.member_count).map_err(|_| {
                GroupsError::Invariant("stored member count cannot be negative".into())
            })?,
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
        request.validate()?;
        self.read_group_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn list_groups(
        &self,
        context: PortContext,
        request: ListGroupsRequest,
    ) -> Result<GroupConnection, PortError> {
        request.validate()?;
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
        request.validate()?;
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
        request.validate()?;
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
        let manager = GroupAccessReadPort::decide_group_access(
            &crate::GroupsService::new(self.db.clone()),
            context.clone(),
            GroupAccessRequest {
                group_id: request.group_id,
                action: GroupAction::Moderate,
            },
        )
        .await?
        .allowed;
        let mut query = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(request.group_id));
        if manager {
            query =
                query.filter(membership::Column::Status.ne(GroupMembershipStatus::Left.as_str()));
        } else {
            query =
                query.filter(membership::Column::Status.eq(GroupMembershipStatus::Active.as_str()));
        }
        let paginator = query
            .order_by_asc(membership::Column::CreatedAt)
            .order_by_asc(membership::Column::Id)
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
impl GroupCommandPort for GroupsService {
    async fn create_group(
        &self,
        context: PortContext,
        input: CreateGroupInput,
    ) -> Result<GroupDetails, PortError> {
        input.validate()?;
        self.create_group_owned(&context, input)
            .await
            .map_err(Into::into)
    }

    async fn join_group(
        &self,
        context: PortContext,
        request: JoinGroupRequest,
    ) -> Result<GroupMembership, PortError> {
        request.validate()?;
        GroupCommandPort::join_group(
            &crate::GroupsService::new(self.db.clone()),
            context,
            request,
        )
        .await
    }

    async fn leave_group(
        &self,
        context: PortContext,
        request: LeaveGroupRequest,
    ) -> Result<GroupMembership, PortError> {
        request.validate()?;
        GroupCommandPort::leave_group(
            &crate::GroupsService::new(self.db.clone()),
            context,
            request,
        )
        .await
    }

    async fn set_group_feature(
        &self,
        context: PortContext,
        request: SetGroupFeatureRequest,
    ) -> Result<GroupFeatureBinding, PortError> {
        request.validate()?;
        GroupCommandPort::set_group_feature(
            &crate::GroupsService::new(self.db.clone()),
            context,
            request,
        )
        .await
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
    if context.actor.kind != PortActorKind::User {
        return None;
    }
    let trimmed = context.actor.id.trim();
    if trimmed.is_empty() {
        return None;
    }
    match Uuid::parse_str(trimmed) {
        Ok(parsed) if !parsed.is_nil() => Some(parsed),
        _ => None,
    }
}

fn parse_uuid(value: &str, field: &str) -> GroupsResult<Uuid> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GroupsError::Validation(format!("{field} cannot be empty")));
    }
    let parsed = Uuid::parse_str(trimmed)
        .map_err(|_| GroupsError::Validation(format!("{field} must be a UUID")))?;
    if parsed.is_nil() {
        return Err(GroupsError::Validation(format!(
            "{field} cannot be nil UUID"
        )));
    }
    Ok(parsed)
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

    #[tokio::test]
    async fn lock_membership_enforcement_target_by_id_returns_none_or_err() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory database should connect");
        let service = GroupsService::new(db);
        let result = service
            .lock_membership_enforcement_target_by_id(Uuid::new_v4(), Uuid::new_v4())
            .await;
        assert!(result.is_err());
    }
}
