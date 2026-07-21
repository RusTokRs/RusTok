use std::str::FromStr;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{normalize_locale_tag, PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use uuid::Uuid;

use crate::domain::{GroupMembershipStatus, GroupRole};
use crate::dto::{
    DeleteGroupTranslationRequest, DeleteGroupTranslationResult, GroupTranslation,
    GroupTranslationMutationResult, ListGroupTranslationsRequest, UpsertGroupTranslationRequest,
};
use crate::entities::{group, membership, translation};
use crate::error::{GroupsError, GroupsResult};
use crate::ports::{GroupLocalizationCommandPort, GroupLocalizationReadPort};

#[derive(Clone)]
pub struct GroupLocalizationService {
    db: DatabaseConnection,
}

impl GroupLocalizationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn list_group_translations_owned(
        &self,
        context: &PortContext,
        request: ListGroupTranslationsRequest,
    ) -> GroupsResult<Vec<GroupTranslation>> {
        require_read(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        self.require_manage_direct(context, tenant_id, request.group_id, actor_user_id)
            .await?;

        translation::Entity::find()
            .filter(translation::Column::TenantId.eq(tenant_id))
            .filter(translation::Column::GroupId.eq(request.group_id))
            .order_by_asc(translation::Column::Locale)
            .all(&self.db)
            .await?
            .into_iter()
            .map(map_translation)
            .collect()
    }

    async fn upsert_group_translation_owned(
        &self,
        context: &PortContext,
        request: UpsertGroupTranslationRequest,
    ) -> GroupsResult<GroupTranslationMutationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let (locale, title, summary, body) = normalize_translation_input(
            &request.locale,
            &request.title,
            request.summary,
            request.body,
        )?;

        let transaction = self.db.begin().await?;
        let group_model = self
            .require_manage_in_transaction(
                &transaction,
                context,
                tenant_id,
                request.group_id,
                actor_user_id,
            )
            .await?;
        let now = Utc::now().fixed_offset();
        let existing = translation::Entity::find()
            .filter(translation::Column::TenantId.eq(tenant_id))
            .filter(translation::Column::GroupId.eq(request.group_id))
            .filter(translation::Column::Locale.eq(locale.clone()))
            .one(&transaction)
            .await?;
        let created = existing.is_none();
        let translation_model = if let Some(existing) = existing {
            let mut active: translation::ActiveModel = existing.into();
            active.title = Set(title);
            active.summary = Set(summary);
            active.body = Set(body);
            active.updated_at = Set(now);
            active.update(&transaction).await?
        } else {
            translation::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(request.group_id),
                locale: Set(locale),
                title: Set(title),
                summary: Set(summary),
                body: Set(body),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(&transaction)
            .await?
        };

        let next_version = group_model.version.saturating_add(1);
        let mut active_group: group::ActiveModel = group_model.into();
        active_group.version = Set(next_version);
        active_group.updated_at = Set(now);
        active_group.update(&transaction).await?;
        transaction.commit().await?;

        Ok(GroupTranslationMutationResult {
            translation: map_translation(translation_model)?,
            group_version: next_version.max(0) as u64,
            created,
        })
    }

    async fn delete_group_translation_owned(
        &self,
        context: &PortContext,
        request: DeleteGroupTranslationRequest,
    ) -> GroupsResult<DeleteGroupTranslationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let locale = normalize_locale_tag(&request.locale)
            .ok_or_else(|| GroupsError::Validation("invalid group locale".to_string()))?;

        let transaction = self.db.begin().await?;
        let group_model = self
            .require_manage_in_transaction(
                &transaction,
                context,
                tenant_id,
                request.group_id,
                actor_user_id,
            )
            .await?;
        let translation_model = translation::Entity::find()
            .filter(translation::Column::TenantId.eq(tenant_id))
            .filter(translation::Column::GroupId.eq(request.group_id))
            .filter(translation::Column::Locale.eq(locale.clone()))
            .one(&transaction)
            .await?
            .ok_or(GroupsError::NotFound)?;
        let translation_count = translation::Entity::find()
            .filter(translation::Column::TenantId.eq(tenant_id))
            .filter(translation::Column::GroupId.eq(request.group_id))
            .count(&transaction)
            .await?;
        if translation_count <= 1 {
            return Err(GroupsError::Conflict(
                "the last group translation cannot be deleted".to_string(),
            ));
        }

        translation::Entity::delete_by_id(translation_model.id)
            .exec(&transaction)
            .await?;
        let now = Utc::now().fixed_offset();
        let next_version = group_model.version.saturating_add(1);
        let mut active_group: group::ActiveModel = group_model.into();
        active_group.version = Set(next_version);
        active_group.updated_at = Set(now);
        active_group.update(&transaction).await?;
        transaction.commit().await?;

        Ok(DeleteGroupTranslationResult {
            group_id: request.group_id,
            locale,
            group_version: next_version.max(0) as u64,
        })
    }

    async fn require_manage_direct(
        &self,
        context: &PortContext,
        tenant_id: Uuid,
        group_id: Uuid,
        actor_user_id: Uuid,
    ) -> GroupsResult<()> {
        group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Id.eq(group_id))
            .one(&self.db)
            .await?
            .ok_or(GroupsError::NotFound)?;
        if has_platform_manage(context) {
            return Ok(());
        }
        let membership = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(group_id))
            .filter(membership::Column::UserId.eq(actor_user_id))
            .one(&self.db)
            .await?;
        require_local_manager(membership.as_ref())
    }

    async fn require_manage_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        context: &PortContext,
        tenant_id: Uuid,
        group_id: Uuid,
        actor_user_id: Uuid,
    ) -> GroupsResult<group::Model> {
        let group_model = group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Id.eq(group_id))
            .one(transaction)
            .await?
            .ok_or(GroupsError::NotFound)?;
        if has_platform_manage(context) {
            return Ok(group_model);
        }
        let membership = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(group_id))
            .filter(membership::Column::UserId.eq(actor_user_id))
            .one(transaction)
            .await?;
        require_local_manager(membership.as_ref())?;
        Ok(group_model)
    }
}

#[async_trait]
impl GroupLocalizationReadPort for GroupLocalizationService {
    async fn list_group_translations(
        &self,
        context: PortContext,
        request: ListGroupTranslationsRequest,
    ) -> Result<Vec<GroupTranslation>, PortError> {
        self.list_group_translations_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

#[async_trait]
impl GroupLocalizationCommandPort for GroupLocalizationService {
    async fn upsert_group_translation(
        &self,
        context: PortContext,
        request: UpsertGroupTranslationRequest,
    ) -> Result<GroupTranslationMutationResult, PortError> {
        self.upsert_group_translation_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn delete_group_translation(
        &self,
        context: PortContext,
        request: DeleteGroupTranslationRequest,
    ) -> Result<DeleteGroupTranslationResult, PortError> {
        self.delete_group_translation_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

fn require_local_manager(model: Option<&membership::Model>) -> GroupsResult<()> {
    let allowed = model
        .filter(|row| row.status == GroupMembershipStatus::Active.as_str())
        .and_then(|row| GroupRole::from_str(&row.role).ok())
        .is_some_and(GroupRole::can_manage_settings);
    if allowed {
        Ok(())
    } else {
        Err(GroupsError::Forbidden(
            "group owner or administrator role is required".to_string(),
        ))
    }
}

fn normalize_translation_input(
    locale: &str,
    title: &str,
    summary: Option<String>,
    body: Option<String>,
) -> GroupsResult<(String, String, Option<String>, Option<String>)> {
    let locale = normalize_locale_tag(locale)
        .ok_or_else(|| GroupsError::Validation("invalid group locale".to_string()))?;
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 240 {
        return Err(GroupsError::Validation(
            "group title must contain between 1 and 240 characters".to_string(),
        ));
    }
    let summary = normalize_optional_text(summary);
    if summary
        .as_deref()
        .is_some_and(|value| value.chars().count() > 500)
    {
        return Err(GroupsError::Validation(
            "group summary must not exceed 500 characters".to_string(),
        ));
    }
    Ok((
        locale,
        title.to_string(),
        summary,
        normalize_optional_text(body),
    ))
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn map_translation(model: translation::Model) -> GroupsResult<GroupTranslation> {
    let locale = normalize_locale_tag(&model.locale).ok_or_else(|| {
        GroupsError::Invariant("group translation locale is not normalized".to_string())
    })?;
    Ok(GroupTranslation {
        id: model.id,
        group_id: model.group_id,
        locale,
        title: model.title,
        summary: model.summary,
        body: model.body,
    })
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
    Uuid::parse_str(&context.tenant_id)
        .map_err(|_| GroupsError::Validation("tenant_id must be a UUID".to_string()))
}

fn actor_user_id(context: &PortContext) -> GroupsResult<Uuid> {
    if context.actor.kind != PortActorKind::User {
        return Err(GroupsError::Forbidden(
            "a user actor is required".to_string(),
        ));
    }
    Uuid::parse_str(&context.actor.id)
        .map_err(|_| GroupsError::Validation("actor.id must be a UUID".to_string()))
}

fn has_platform_manage(context: &PortContext) -> bool {
    context
        .claims
        .iter()
        .any(|claim| matches!(claim.as_str(), "groups:manage" | "groups:*" | "*:*") )
}
