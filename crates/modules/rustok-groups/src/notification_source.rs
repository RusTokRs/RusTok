use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::HostRuntimeContext;
use rustok_notifications_api::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest, NotificationAudienceCandidate,
    NotificationAudiencePage, NotificationOpenAuthorization, NotificationPriority,
    NotificationProviderError, NotificationProviderResult, NotificationSemanticDescriptor,
    NotificationSourceEventRef, NotificationSourceProvider, NotificationSourceProviderFactory,
    NotificationSourceSlug, NotificationTargetKind, NotificationTargetRef, NotificationTargetRoute,
    NotificationTemplateData, NotificationTemplateKey, NotificationTypeKey,
    ResolveNotificationAudienceRequest,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::domain::GroupStatus;
use crate::entities::group;
use crate::group_event_entities::Entity as GroupDomainEventEntity;
use crate::group_event_entities::{Column as GroupDomainEventColumn, Model as GroupDomainEvent};
use crate::invitation_entities::invitation;

const GROUPS_SOURCE: &str = "groups";
const TARGETED_INVITATION_CREATED_TYPE: &str = "groups.invitation.targeted_created";
const GROUP_INVITATION_TARGET: &str = "groups.invitation";

#[derive(Clone, Default)]
pub(crate) struct GroupsNotificationSourceProviderFactory;

impl NotificationSourceProviderFactory for GroupsNotificationSourceProviderFactory {
    fn slug(&self) -> NotificationSourceSlug {
        groups_source_slug()
    }

    fn build(
        &self,
        host: &HostRuntimeContext,
    ) -> NotificationProviderResult<Arc<dyn NotificationSourceProvider>> {
        Ok(Arc::new(GroupsNotificationSourceProvider::new(
            host.db_clone(),
        )))
    }
}

#[derive(Clone)]
struct GroupsNotificationSourceProvider {
    db: DatabaseConnection,
}

impl GroupsNotificationSourceProvider {
    fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn load_event(
        &self,
        event: &NotificationSourceEventRef,
    ) -> NotificationProviderResult<GroupDomainEvent> {
        if event.source() != &groups_source_slug()
            || event.event_type() != &targeted_invitation_created_type()
        {
            return Err(NotificationProviderError::InvalidEvent);
        }
        let sequence_no = i64::try_from(event.source_revision())
            .map_err(|_| NotificationProviderError::InvalidEvent)?;
        GroupDomainEventEntity::find()
            .filter(GroupDomainEventColumn::TenantId.eq(event.tenant_id()))
            .filter(GroupDomainEventColumn::EventId.eq(event.event_id()))
            .filter(GroupDomainEventColumn::EventType.eq(TARGETED_INVITATION_CREATED_TYPE))
            .filter(GroupDomainEventColumn::SequenceNo.eq(sequence_no))
            .one(&self.db)
            .await
            .map_err(retryable_database_error)?
            .ok_or(NotificationProviderError::InvalidEvent)
    }

    async fn load_active_targeted_invitation(
        &self,
        tenant_id: Uuid,
        invitation_id: Uuid,
    ) -> NotificationProviderResult<Option<invitation::Model>> {
        let invitation = invitation::Entity::find()
            .filter(invitation::Column::TenantId.eq(tenant_id))
            .filter(invitation::Column::Id.eq(invitation_id))
            .one(&self.db)
            .await
            .map_err(retryable_database_error)?;
        let Some(invitation) = invitation else {
            return Ok(None);
        };
        if invitation.target_user_id.is_none()
            || invitation.max_uses != 1
            || invitation.revoked_at.is_some()
            || invitation.use_count >= invitation.max_uses
            || invitation.expires_at.with_timezone(&Utc) <= Utc::now()
        {
            return Ok(None);
        }
        let group_is_active = group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Id.eq(invitation.group_id))
            .filter(group::Column::Status.eq(GroupStatus::Active.as_str()))
            .one(&self.db)
            .await
            .map_err(retryable_database_error)?
            .is_some();
        Ok(group_is_active.then_some(invitation))
    }

    fn validate_descriptor(
        &self,
        event: &GroupDomainEvent,
        descriptor: &NotificationSemanticDescriptor,
    ) -> NotificationProviderResult<()> {
        if descriptor.notification_type != targeted_invitation_created_type()
            || descriptor.target.owner != groups_source_slug()
            || descriptor.target.kind != group_invitation_target_kind()
            || descriptor.target.id != event.aggregate_id
        {
            return Err(NotificationProviderError::Rejected);
        }
        Ok(())
    }
}

#[async_trait]
impl NotificationSourceProvider for GroupsNotificationSourceProvider {
    fn slug(&self) -> NotificationSourceSlug {
        groups_source_slug()
    }

    fn display_name(&self) -> &'static str {
        "Groups"
    }

    fn supported_types(&self) -> Vec<NotificationTypeKey> {
        vec![targeted_invitation_created_type()]
    }

    async fn describe_event(
        &self,
        request: DescribeNotificationRequest,
    ) -> NotificationProviderResult<Option<NotificationSemanticDescriptor>> {
        let event = self.load_event(&request.event).await?;
        if event.aggregate_type != "invitation" || event.schema_version != 1 {
            return Err(NotificationProviderError::InvalidEvent);
        }
        let Some(invitation) = self
            .load_active_targeted_invitation(event.tenant_id, event.aggregate_id)
            .await?
        else {
            return Ok(None);
        };
        let template_data = NotificationTemplateData::try_new(BTreeMap::from([
            ("invitation_id".to_string(), invitation.id.to_string()),
            ("group_id".to_string(), invitation.group_id.to_string()),
        ]))
        .map_err(|_| NotificationProviderError::InvalidEvent)?;
        Ok(Some(NotificationSemanticDescriptor {
            notification_type: targeted_invitation_created_type(),
            template_key: NotificationTemplateKey::new(TARGETED_INVITATION_CREATED_TYPE)
                .expect("groups targeted invitation template key must stay valid"),
            target: NotificationTargetRef {
                owner: groups_source_slug(),
                kind: group_invitation_target_kind(),
                id: invitation.id,
            },
            actor_id: event.actor_id.or(Some(invitation.invited_by_user_id)),
            priority: NotificationPriority::Normal,
            template_data,
        }))
    }

    async fn resolve_audience(
        &self,
        request: ResolveNotificationAudienceRequest,
    ) -> NotificationProviderResult<NotificationAudiencePage> {
        let event = self.load_event(&request.event).await?;
        self.validate_descriptor(&event, &request.descriptor)?;
        if request.bounded_limit() == 0 {
            return Err(NotificationProviderError::Rejected);
        }
        if request.cursor.is_some() {
            return Ok(NotificationAudiencePage::empty());
        }
        let Some(invitation) = self
            .load_active_targeted_invitation(event.tenant_id, event.aggregate_id)
            .await?
        else {
            return Ok(NotificationAudiencePage::empty());
        };
        let Some(recipient_id) = invitation.target_user_id else {
            return Ok(NotificationAudiencePage::empty());
        };
        NotificationAudiencePage::try_new(
            vec![NotificationAudienceCandidate { recipient_id }],
            None,
        )
        .map_err(|_| NotificationProviderError::Internal { retryable: false })
    }

    async fn authorize_target_open(
        &self,
        request: AuthorizeNotificationTargetRequest,
    ) -> NotificationProviderResult<NotificationOpenAuthorization> {
        if request.target.owner != groups_source_slug()
            || request.target.kind != group_invitation_target_kind()
        {
            return Ok(NotificationOpenAuthorization::Unavailable);
        }
        let Some(invitation) = self
            .load_active_targeted_invitation(request.tenant_id, request.target.id)
            .await?
        else {
            return Ok(NotificationOpenAuthorization::Unavailable);
        };
        if invitation.target_user_id != Some(request.recipient_id) {
            return Ok(NotificationOpenAuthorization::Unavailable);
        }
        let route =
            NotificationTargetRoute::new(format!("/modules/groups?invitation={}", invitation.id))
                .map_err(|_| NotificationProviderError::Internal { retryable: false })?;
        Ok(NotificationOpenAuthorization::Allowed { route })
    }
}

fn retryable_database_error(_error: sea_orm::DbErr) -> NotificationProviderError {
    NotificationProviderError::Internal { retryable: true }
}

fn groups_source_slug() -> NotificationSourceSlug {
    NotificationSourceSlug::new(GROUPS_SOURCE)
        .expect("groups notification source slug must stay valid")
}

fn targeted_invitation_created_type() -> NotificationTypeKey {
    NotificationTypeKey::new(TARGETED_INVITATION_CREATED_TYPE)
        .expect("groups targeted invitation type must stay valid")
}

fn group_invitation_target_kind() -> NotificationTargetKind {
    NotificationTargetKind::new(GROUP_INVITATION_TARGET)
        .expect("groups invitation target kind must stay valid")
}
