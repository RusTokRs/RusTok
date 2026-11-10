use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application_entities::membership_policy_revision;
use crate::effective_membership_guard::{
    GroupManagerCapability, actor_user_id, require_effective_manager_direct_owned, tenant_id,
};
use crate::{GroupApplicationQuestion, GroupApplicationRule};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupApplicationPolicyRevision {
    pub group_id: Uuid,
    pub policy_id: Uuid,
    pub revision: u64,
    pub locale: String,
    pub enabled: bool,
    pub questions: Vec<GroupApplicationQuestion>,
    pub rules: Vec<GroupApplicationRule>,
    pub created_by_user_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListGroupApplicationPolicyRevisionsRequest {
    pub group_id: Uuid,
    pub page: u64,
    pub per_page: u64,
}

impl ListGroupApplicationPolicyRevisionsRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.group_id.is_nil() {
            return Err(PortError::validation(
                "groups.invalid_group_id",
                "group_id cannot be nil UUID",
            ));
        }
        if self.page == 0 {
            return Err(PortError::validation(
                "groups.invalid_page",
                "page must be greater than zero",
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupApplicationPolicyRevisionConnection {
    pub items: Vec<GroupApplicationPolicyRevision>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[async_trait]
pub trait GroupApplicationPolicyHistoryReadPort: Send + Sync {
    async fn list_group_application_policy_revisions(
        &self,
        context: PortContext,
        request: ListGroupApplicationPolicyRevisionsRequest,
    ) -> Result<GroupApplicationPolicyRevisionConnection, PortError>;
}

#[derive(Clone)]
pub struct GroupApplicationPolicyHistoryService {
    db: DatabaseConnection,
}

impl GroupApplicationPolicyHistoryService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl GroupApplicationPolicyHistoryReadPort for GroupApplicationPolicyHistoryService {
    async fn list_group_application_policy_revisions(
        &self,
        context: PortContext,
        request: ListGroupApplicationPolicyRevisionsRequest,
    ) -> Result<GroupApplicationPolicyRevisionConnection, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        request.validate()?;
        let tenant_id = tenant_id(&context)?;
        let actor_user_id = actor_user_id(&context)?;
        // Authorization must not fetch or decode unrelated candidate application payloads.
        // Use the same owner-clock capability as the effective application review surface.
        require_effective_manager_direct_owned(
            &self.db,
            &context,
            tenant_id,
            request.group_id,
            actor_user_id,
            GroupManagerCapability::Moderate,
        )
        .await
        .map_err(PortError::from)?;

        let page = request.page.max(1);
        let per_page = request.per_page.clamp(1, 100);
        let paginator = membership_policy_revision::Entity::find()
            .filter(membership_policy_revision::Column::TenantId.eq(tenant_id))
            .filter(membership_policy_revision::Column::GroupId.eq(request.group_id))
            .order_by_desc(membership_policy_revision::Column::Revision)
            .order_by_asc(membership_policy_revision::Column::Locale)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await.map_err(persistence_error)?;
        let items = paginator
            .fetch_page(page.saturating_sub(1))
            .await
            .map_err(persistence_error)?
            .into_iter()
            .map(map_revision)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(GroupApplicationPolicyRevisionConnection {
            items,
            total,
            page,
            per_page,
        })
    }
}

fn map_revision(
    model: membership_policy_revision::Model,
) -> Result<GroupApplicationPolicyRevision, PortError> {
    let questions = serde_json::from_value(model.questions).map_err(|error| {
        PortError::invariant_violation(
            "groups.policy_revision_questions_invalid",
            format!("stored application policy revision questions are invalid: {error}"),
        )
    })?;
    let rules = serde_json::from_value(model.rules).map_err(|error| {
        PortError::invariant_violation(
            "groups.policy_revision_rules_invalid",
            format!("stored application policy revision rules are invalid: {error}"),
        )
    })?;
    Ok(GroupApplicationPolicyRevision {
        group_id: model.group_id,
        policy_id: model.policy_id,
        revision: crate::domain::stored_revision(model.revision)?,
        locale: model.locale,
        enabled: model.enabled,
        questions,
        rules,
        created_by_user_id: model.created_by_user_id,
        created_at: model.created_at.with_timezone(&Utc),
    })
}

fn persistence_error(error: sea_orm::DbErr) -> PortError {
    PortError::unavailable(
        "groups.policy_revision_persistence_unavailable",
        error.to_string(),
    )
}
