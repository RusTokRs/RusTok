use std::fmt::{Display, Formatter};

use leptos::prelude::*;

use crate::core::{
    NotificationStorefrontGroupItemsPage, NotificationStorefrontGroupItemsRequest,
    NotificationStorefrontGroupStateAction, NotificationStorefrontGroupStateCommand,
    NotificationStorefrontGroupStatePage, NotificationStorefrontGroupSummary,
    NotificationStorefrontGroupSummaryPage, NotificationStorefrontGroupSummaryRequest,
    NotificationStorefrontItem, NotificationStorefrontItemState,
    NotificationStorefrontOpenDecision, NotificationStorefrontOpenRequest,
    NotificationStorefrontPriority, NotificationStorefrontUnreadCount,
};

use serde::{Deserialize, Serialize};

const PUBLIC_CAPABILITY_UNAVAILABLE: &str = "notification inbox capability is unavailable";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeNotificationStorefrontError(pub String);

impl Display for NativeNotificationStorefrontError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeNotificationStorefrontError {}

impl From<ServerFnError> for NativeNotificationStorefrontError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_notification_unread_count()
-> Result<NotificationStorefrontUnreadCount, NativeNotificationStorefrontError> {
    notification_storefront_unread_count_native()
        .await
        .map_err(Into::into)
}

pub async fn load_notification_group_summaries(
    request: NotificationStorefrontGroupSummaryRequest,
) -> Result<NotificationStorefrontGroupSummaryPage, NativeNotificationStorefrontError> {
    notification_storefront_group_summaries_native(request)
        .await
        .map_err(Into::into)
}

pub async fn load_notification_group_items(
    request: NotificationStorefrontGroupItemsRequest,
) -> Result<NotificationStorefrontGroupItemsPage, NativeNotificationStorefrontError> {
    notification_storefront_group_items_native(request)
        .await
        .map_err(Into::into)
}

pub async fn authorize_notification_open(
    request: NotificationStorefrontOpenRequest,
) -> Result<NotificationStorefrontOpenDecision, NativeNotificationStorefrontError> {
    notification_storefront_open_native(request)
        .await
        .map_err(Into::into)
}

pub async fn apply_notification_group_state(
    command: NotificationStorefrontGroupStateCommand,
) -> Result<NotificationStorefrontGroupStatePage, NativeNotificationStorefrontError> {
    notification_storefront_group_state_native(command)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "notifications/storefront/unread-count")]
async fn notification_storefront_unread_count_native()
-> Result<NotificationStorefrontUnreadCount, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_notifications::NotificationInboxStorefrontPort;

        let (runtime, context) = authenticated_context("unread-count", None).await?;
        let result = storefront_service(&runtime)?
            .unread_count(context)
            .await
            .map_err(map_port_error)?;
        Ok(NotificationStorefrontUnreadCount {
            unread_count: result.unread_count,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "notification storefront native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "notifications/storefront/group-summaries"
)]
async fn notification_storefront_group_summaries_native(
    request: NotificationStorefrontGroupSummaryRequest,
) -> Result<NotificationStorefrontGroupSummaryPage, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_notifications::{
            NotificationInboxStorefrontGroupSummaryRequest, NotificationInboxStorefrontPort,
        };

        let (runtime, context) = authenticated_context("group-summaries", None).await?;
        let page = storefront_service(&runtime)?
            .list_group_summaries(
                context,
                NotificationInboxStorefrontGroupSummaryRequest {
                    cursor: request.cursor,
                    limit: request.limit,
                },
            )
            .await
            .map_err(map_port_error)?;
        Ok(map_group_summary_page(page))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "notification storefront native transport requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "notifications/storefront/group-items")]
async fn notification_storefront_group_items_native(
    request: NotificationStorefrontGroupItemsRequest,
) -> Result<NotificationStorefrontGroupItemsPage, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_notifications::{
            NotificationInboxStorefrontGroupItemsRequest, NotificationInboxStorefrontPort,
        };

        let (runtime, context) = authenticated_context("group-items", None).await?;
        let page = storefront_service(&runtime)?
            .list_group_items(
                context,
                NotificationInboxStorefrontGroupItemsRequest {
                    group_key: request.group_key,
                    state: request.state.map(map_item_state_to_owner),
                    cursor: request.cursor,
                    limit: request.limit,
                },
            )
            .await
            .map_err(map_port_error)?;
        Ok(NotificationStorefrontGroupItemsPage {
            items: page.items.into_iter().map(map_item).collect(),
            next_cursor: page.next_cursor,
            has_more: page.has_more,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "notification storefront native transport requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "notifications/storefront/open")]
async fn notification_storefront_open_native(
    request: NotificationStorefrontOpenRequest,
) -> Result<NotificationStorefrontOpenDecision, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_notifications::{
            NotificationInboxStorefrontOpenDecision, NotificationInboxStorefrontOpenRequest,
            NotificationInboxStorefrontPort,
        };
        use uuid::Uuid;

        let (runtime, context) = authenticated_context("open", None).await?;
        let notification_id = Uuid::parse_str(request.notification_id.as_str())
            .map_err(|_| ServerFnError::new("notification_id must be a UUID"))?;
        let decision = storefront_service(&runtime)?
            .authorize_open(
                context,
                NotificationInboxStorefrontOpenRequest { notification_id },
            )
            .await
            .map_err(map_port_error)?;
        Ok(match decision {
            NotificationInboxStorefrontOpenDecision::Allowed { route } => {
                NotificationStorefrontOpenDecision::Allowed {
                    route: route.as_str().to_string(),
                }
            }
            NotificationInboxStorefrontOpenDecision::Unavailable => {
                NotificationStorefrontOpenDecision::Unavailable
            }
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "notification storefront native transport requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "notifications/storefront/group-state")]
async fn notification_storefront_group_state_native(
    command: NotificationStorefrontGroupStateCommand,
) -> Result<NotificationStorefrontGroupStatePage, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_notifications::{
            NotificationInboxStorefrontGroupStateRequest, NotificationInboxStorefrontPort,
        };

        let NotificationStorefrontGroupStateCommand {
            group_key,
            action,
            cursor,
            limit,
            idempotency_key,
        } = command;
        let (runtime, context) =
            authenticated_context("group-state", Some(idempotency_key)).await?;
        let page = storefront_service(&runtime)?
            .apply_group_state(
                context,
                NotificationInboxStorefrontGroupStateRequest {
                    group_key,
                    action: map_group_action_to_owner(action),
                    cursor,
                    limit,
                },
            )
            .await
            .map_err(map_port_error)?;
        Ok(NotificationStorefrontGroupStatePage {
            scanned: page.scanned,
            changed: page.changed,
            next_cursor: page.next_cursor,
            has_more: page.has_more,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "notification storefront native transport requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
async fn authenticated_context(
    operation: &'static str,
    idempotency_key: Option<String>,
) -> Result<(rustok_api::HostRuntimeContext, rustok_api::PortContext), ServerFnError> {
    use std::time::Duration;

    use leptos::prelude::expect_context;
    use rustok_api::{
        AuthContext, HostRuntimeContext, PortContext, TenantContext, request::RequestContext,
    };
    use uuid::Uuid;

    let runtime = expect_context::<HostRuntimeContext>();
    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(ServerFnError::new)?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(ServerFnError::new)?;
    let request = leptos_axum::extract::<RequestContext>()
        .await
        .map_err(ServerFnError::new)?;
    if auth.tenant_id != tenant.id {
        return Err(ServerFnError::new(
            "notification storefront tenant mismatch",
        ));
    }
    if !auth.is_human_user_principal() {
        return Err(ServerFnError::new(
            "notification inbox access requires an authenticated user",
        ));
    }

    let actor = auth.port_actor();
    let mut context = PortContext::new(
        tenant.id.to_string(),
        actor,
        request.locale,
        format!("notifications-storefront-{operation}-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(5))
    .with_channel("storefront");
    for permission in auth.permissions {
        context = context.with_claim(permission.to_string());
    }
    if let Some(idempotency_key) = idempotency_key {
        context = context.with_idempotency_key(idempotency_key);
    }
    Ok((runtime, context))
}

#[cfg(feature = "ssr")]
fn storefront_service(
    runtime: &rustok_api::HostRuntimeContext,
) -> Result<rustok_notifications::NotificationInboxStorefrontService, ServerFnError> {
    use std::sync::Arc;

    use rustok_notifications::api::NotificationSourceRegistry;
    use rustok_notifications::{
        NotificationInboxStorefrontService, NotificationRecipientPolicyRuntime,
    };

    let registry = runtime
        .shared_get::<Arc<NotificationSourceRegistry>>()
        .ok_or_else(capability_unavailable)?;
    let policy = runtime
        .shared_get::<NotificationRecipientPolicyRuntime>()
        .ok_or_else(capability_unavailable)?;
    Ok(NotificationInboxStorefrontService::new(
        runtime.db_clone(),
        registry,
        policy.policy_arc(),
    ))
}

#[cfg(feature = "ssr")]
fn capability_unavailable() -> ServerFnError {
    ServerFnError::new(PUBLIC_CAPABILITY_UNAVAILABLE)
}

#[cfg(feature = "ssr")]
fn map_port_error(error: rustok_api::PortError) -> ServerFnError {
    ServerFnError::new(error.message)
}

#[cfg(feature = "ssr")]
fn map_group_summary_page(
    page: rustok_notifications::NotificationInboxGroupSummaryPage,
) -> NotificationStorefrontGroupSummaryPage {
    NotificationStorefrontGroupSummaryPage {
        groups: page
            .groups
            .into_iter()
            .map(|group| NotificationStorefrontGroupSummary {
                group_key: group.group_key,
                item_count: group.item_count,
                unread_count: group.unread_count,
                latest_item: map_item(group.latest_item),
            })
            .collect(),
        next_cursor: page.next_cursor,
        has_more: page.has_more,
    }
}

#[cfg(feature = "ssr")]
fn map_item(item: rustok_notifications::NotificationInboxItem) -> NotificationStorefrontItem {
    NotificationStorefrontItem {
        id: item.id.to_string(),
        source: item.source.into_string(),
        notification_type: item.notification_type.into_string(),
        template_key: item.template_key.into_string(),
        actor_id: item.actor_id.map(|id| id.to_string()),
        priority: match item.priority {
            rustok_notifications::api::NotificationPriority::Low => {
                NotificationStorefrontPriority::Low
            }
            rustok_notifications::api::NotificationPriority::Normal => {
                NotificationStorefrontPriority::Normal
            }
            rustok_notifications::api::NotificationPriority::High => {
                NotificationStorefrontPriority::High
            }
            rustok_notifications::api::NotificationPriority::Urgent => {
                NotificationStorefrontPriority::Urgent
            }
        },
        state: map_item_state_from_owner(item.state),
        template_data: item.template_data.into_inner(),
        seen_at: item.seen_at.map(|value| value.to_rfc3339()),
        read_at: item.read_at.map(|value| value.to_rfc3339()),
        archived_at: item.archived_at.map(|value| value.to_rfc3339()),
        created_at: item.created_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_item_state_to_owner(
    state: NotificationStorefrontItemState,
) -> rustok_notifications::model::NotificationState {
    match state {
        NotificationStorefrontItemState::Unread => {
            rustok_notifications::model::NotificationState::Unread
        }
        NotificationStorefrontItemState::Seen => {
            rustok_notifications::model::NotificationState::Seen
        }
        NotificationStorefrontItemState::Read => {
            rustok_notifications::model::NotificationState::Read
        }
        NotificationStorefrontItemState::Archived => {
            rustok_notifications::model::NotificationState::Archived
        }
    }
}

#[cfg(feature = "ssr")]
fn map_item_state_from_owner(
    state: rustok_notifications::model::NotificationState,
) -> NotificationStorefrontItemState {
    match state {
        rustok_notifications::model::NotificationState::Unread => {
            NotificationStorefrontItemState::Unread
        }
        rustok_notifications::model::NotificationState::Seen => {
            NotificationStorefrontItemState::Seen
        }
        rustok_notifications::model::NotificationState::Read => {
            NotificationStorefrontItemState::Read
        }
        rustok_notifications::model::NotificationState::Archived => {
            NotificationStorefrontItemState::Archived
        }
    }
}

#[cfg(feature = "ssr")]
fn map_group_action_to_owner(
    action: NotificationStorefrontGroupStateAction,
) -> rustok_notifications::NotificationInboxGroupStateAction {
    match action {
        NotificationStorefrontGroupStateAction::MarkRead => {
            rustok_notifications::NotificationInboxGroupStateAction::MarkRead
        }
        NotificationStorefrontGroupStateAction::MarkUnread => {
            rustok_notifications::NotificationInboxGroupStateAction::MarkUnread
        }
        NotificationStorefrontGroupStateAction::Archive => {
            rustok_notifications::NotificationInboxGroupStateAction::Archive
        }
    }
}
