use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_notifications_api::{NotificationSourceRegistry, NotificationTargetRoute};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::candidate::NotificationRecipientPolicy;
use crate::error::NotificationError;
use crate::inbox::{NotificationInboxOpenDecision, NotificationInboxOpenRequest, NotificationInboxOpenService, NotificationInboxPage};
use crate::inbox_count::{NotificationInboxUnreadCount, NotificationInboxUnreadCountRequest, NotificationInboxUnreadCountService};
use crate::inbox_group::{NotificationInboxGroupListRequest, NotificationInboxGroupListService};
use crate::inbox_group_state::{NotificationInboxGroupStateAction, NotificationInboxGroupStatePage, NotificationInboxGroupStateRequest, NotificationInboxGroupStateService};
use crate::inbox_group_summary::{NotificationInboxGroupSummaryPage, NotificationInboxGroupSummaryRequest, NotificationInboxGroupSummaryService};
use crate::model::NotificationState;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxStorefrontGroupSummaryRequest {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxStorefrontGroupItemsRequest {
    pub group_key: String,
    #[serde(default)]
    pub state: Option<NotificationState>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxStorefrontOpenRequest {
    pub notification_id: Uuid,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum NotificationInboxStorefrontOpenDecision {
    Allowed { route: NotificationTargetRoute },
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxStorefrontGroupStateRequest {
    pub group_key: String,
    pub action: NotificationInboxGroupStateAction,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
}

/// Transport-neutral owner boundary for one authenticated user's notification inbox.
///
/// Tenant and recipient scope are derived exclusively from `PortContext`; transport requests cannot
/// select another owner identity. Reads require deadline semantics. Group-state writes require both a
/// deadline and an idempotency key before any database query. The port delegates to existing owner
/// read and command services, preserving authorization, pagination, state, and timestamp invariants.
#[async_trait]
pub trait NotificationInboxStorefrontPort: Send + Sync {
    async fn unread_count(
        &self,
        context: PortContext,
    ) -> Result<NotificationInboxUnreadCount, PortError>;

    async fn list_group_summaries(
        &self,
        context: PortContext,
        request: NotificationInboxStorefrontGroupSummaryRequest,
    ) -> Result<NotificationInboxGroupSummaryPage, PortError>;

    async fn list_group_items(
        &self,
        context: PortContext,
        request: NotificationInboxStorefrontGroupItemsRequest,
    ) -> Result<NotificationInboxPage, PortError>;

    async fn authorize_open(
        &self,
        context: PortContext,
        request: NotificationInboxStorefrontOpenRequest,
    ) -> Result<NotificationInboxStorefrontOpenDecision, PortError>;

    async fn apply_group_state(
        &self,
        context: PortContext,
        request: NotificationInboxStorefrontGroupStateRequest,
    ) -> Result<NotificationInboxGroupStatePage, PortError>;
}

#[derive(Clone)]
pub struct NotificationInboxStorefrontService {
    unread_count: NotificationInboxUnreadCountService,
    group_summaries: NotificationInboxGroupSummaryService,
    group_items: NotificationInboxGroupListService,
    open: NotificationInboxOpenService,
    group_state: NotificationInboxGroupStateService,
}

impl NotificationInboxStorefrontService {
    pub fn new(
        db: DatabaseConnection,
        registry: Arc<NotificationSourceRegistry>,
        policy: Arc<dyn NotificationRecipientPolicy>,
    ) -> Self {
        Self {
            unread_count: NotificationInboxUnreadCountService::new(db.clone()),
            group_summaries: NotificationInboxGroupSummaryService::new(
                db.clone(),
                registry.clone(),
                policy.clone(),
            ),
            group_items: NotificationInboxGroupListService::new(
                db.clone(),
                registry.clone(),
                policy.clone(),
            ),
            open: NotificationInboxOpenService::new(db.clone(), registry, policy),
            group_state: NotificationInboxGroupStateService::new(db),
        }
    }
}

pub fn in_process_notification_inbox_storefront_port(
    db: DatabaseConnection,
    registry: Arc<NotificationSourceRegistry>,
    policy: Arc<dyn NotificationRecipientPolicy>,
) -> Arc<dyn NotificationInboxStorefrontPort> {
    Arc::new(NotificationInboxStorefrontService::new(db, registry, policy))
}

#[async_trait]
impl NotificationInboxStorefrontPort for NotificationInboxStorefrontService {
    async fn unread_count(
        &self,
        context: PortContext,
    ) -> Result<NotificationInboxUnreadCount, PortError> {
        let scope = resolve_scope(&context, PortCallPolicy::read())?;
        self.unread_count
            .count(NotificationInboxUnreadCountRequest {
                tenant_id: scope.tenant_id,
                recipient_id: scope.recipient_id,
            })
            .await
            .map_err(notification_error_to_port_error)
    }

    async fn list_group_summaries(
        &self,
        context: PortContext,
        request: NotificationInboxStorefrontGroupSummaryRequest,
    ) -> Result<NotificationInboxGroupSummaryPage, PortError> {
        let scope = resolve_scope(&context, PortCallPolicy::read())?;
        self.group_summaries
            .list_page(NotificationInboxGroupSummaryRequest {
                tenant_id: scope.tenant_id,
                recipient_id: scope.recipient_id,
                cursor: request.cursor,
                limit: request.limit,
            })
            .await
            .map_err(notification_error_to_port_error)
    }

    async fn list_group_items(
        &self,
        context: PortContext,
        request: NotificationInboxStorefrontGroupItemsRequest,
    ) -> Result<NotificationInboxPage, PortError> {
        let scope = resolve_scope(&context, PortCallPolicy::read())?;
        self.group_items
            .list_page(NotificationInboxGroupListRequest {
                tenant_id: scope.tenant_id,
                recipient_id: scope.recipient_id,
                group_key: request.group_key,
                state: request.state,
                cursor: request.cursor,
                limit: request.limit,
            })
            .await
            .map_err(notification_error_to_port_error)
    }

    async fn authorize_open(
        &self,
        context: PortContext,
        request: NotificationInboxStorefrontOpenRequest,
    ) -> Result<NotificationInboxStorefrontOpenDecision, PortError> {
        let scope = resolve_scope(&context, PortCallPolicy::read())?;
        let decision = self
            .open
            .authorize_open(NotificationInboxOpenRequest {
                tenant_id: scope.tenant_id,
                recipient_id: scope.recipient_id,
                notification_id: request.notification_id,
            })
            .await
            .map_err(notification_error_to_port_error)?;
        Ok(match decision {
            NotificationInboxOpenDecision::Allowed { route } => {
                NotificationInboxStorefrontOpenDecision::Allowed { route }
            }
            NotificationInboxOpenDecision::Unavailable => {
                NotificationInboxStorefrontOpenDecision::Unavailable
            }
        })
    }

    async fn apply_group_state(
        &self,
        context: PortContext,
        request: NotificationInboxStorefrontGroupStateRequest,
    ) -> Result<NotificationInboxGroupStatePage, PortError> {
        let scope = resolve_scope(&context, PortCallPolicy::write())?;
        self.group_state
            .apply_page(NotificationInboxGroupStateRequest {
                tenant_id: scope.tenant_id,
                recipient_id: scope.recipient_id,
                group_key: request.group_key,
                action: request.action,
                cursor: request.cursor,
                limit: request.limit,
            })
            .await
            .map_err(notification_error_to_port_error)
    }
}

#[derive(Clone, Copy)]
struct NotificationInboxStorefrontScope {
    tenant_id: Uuid,
    recipient_id: Uuid,
}

fn resolve_scope(
    context: &PortContext,
    policy: PortCallPolicy,
) -> Result<NotificationInboxStorefrontScope, PortError> {
    context.require_policy(policy)?;
    if context.actor.kind != PortActorKind::User {
        return Err(PortError::forbidden(
            "notifications.storefront.user_required",
            "notification inbox access requires an authenticated user",
        ));
    }
    let tenant_id = parse_non_nil_uuid(
        context.tenant_id.as_str(),
        "notifications.storefront.tenant_invalid",
        "notification inbox tenant identity is invalid",
    )?;
    let recipient_id = parse_non_nil_uuid(
        context.actor.id.as_str(),
        "notifications.storefront.user_invalid",
        "notification inbox user identity is invalid",
    )?;
    Ok(NotificationInboxStorefrontScope {
        tenant_id,
        recipient_id,
    })
}

fn parse_non_nil_uuid(value: &str, code: &'static str, message: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|value| !value.is_nil())
        .ok_or_else(|| PortError::validation(code, message))
}

fn notification_error_to_port_error(error: NotificationError) -> PortError {
    let code = error.stable_code();
    let retryable = error.is_retryable();
    match error {
        NotificationError::Validation(_) => {
            PortError::validation(code, "notification inbox request is invalid")
        }
        NotificationError::SourceUnavailable
        | NotificationError::ProviderFailure { .. }
        | NotificationError::RecipientPolicyFailure { .. }
        | NotificationError::TenantCapabilityDisabled
        | NotificationError::TenantPolicyRevisionChanged
        | NotificationError::TenantPolicyCommitFailure { .. }
        | NotificationError::LeaseUnavailable
        | NotificationError::Database(_) => PortError::new(
            PortErrorKind::Unavailable,
            code,
            "notification inbox capability is unavailable",
            retryable,
        ),
        NotificationError::UnsupportedEvent
        | NotificationError::InvalidEvent
        | NotificationError::ProviderRejected
        | NotificationError::SourceIdentityConflict
        | NotificationError::CursorDidNotAdvance
        | NotificationError::InvalidDescriptor
        | NotificationError::Serialization(_) => PortError::invariant_violation(
            code,
            "notification inbox result is invalid",
        ),
    }
}
