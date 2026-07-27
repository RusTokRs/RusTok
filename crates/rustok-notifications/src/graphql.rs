use std::{sync::Arc, time::Duration};

use async_graphql::{Context, Enum, ErrorExtensions, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, PortActor, PortContext, PortError, PortErrorKind, TenantContext,
    graphql::require_module_enabled, request::RequestContext,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    NotificationError, NotificationInboxGroupSummaryPage, NotificationInboxItem,
    NotificationInboxStorefrontGroupItemsRequest, NotificationInboxStorefrontGroupSummaryRequest,
    NotificationInboxStorefrontPort, NotificationInboxUnreadCountRequest,
    NotificationInboxUnreadCountService, in_process_notification_inbox_storefront_port,
};

const MODULE_SLUG: &str = "notifications";
const PUBLIC_UNAVAILABLE_MESSAGE: &str = "notification inbox capability is unavailable";
const GRAPHQL_READ_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct NotificationsQuery;

#[derive(Clone, Default)]
pub struct NotificationsGraphqlRuntimeData {
    storefront: Option<Arc<dyn NotificationInboxStorefrontPort>>,
}

#[cfg(feature = "server")]
pub fn attach_schema_data(
    inputs: &rustok_api::graphql::GraphqlRuntimeInputs,
) -> std::result::Result<NotificationsGraphqlRuntimeData, String> {
    use rustok_notifications_api::NotificationSourceRegistry;

    use crate::NotificationRecipientPolicyRuntime;

    let registry = inputs.shared_get::<Arc<NotificationSourceRegistry>>();
    let policy = inputs.shared_get::<NotificationRecipientPolicyRuntime>();
    let storefront = registry.zip(policy).map(|(registry, policy)| {
        in_process_notification_inbox_storefront_port(
            inputs.db_clone(),
            registry,
            policy.policy_arc(),
        )
    });
    Ok(NotificationsGraphqlRuntimeData { storefront })
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlNotificationInboxUnreadCount {
    pub unread_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "NotificationInboxItemState")]
pub enum GqlNotificationInboxItemState {
    Unread,
    Seen,
    Read,
    Archived,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "NotificationInboxPriority")]
pub enum GqlNotificationInboxPriority {
    Low,
    Normal,
    High,
    Urgent,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlNotificationTemplateField {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlNotificationInboxItem {
    pub id: String,
    pub source: String,
    pub notification_type: String,
    pub template_key: String,
    pub actor_id: Option<String>,
    pub priority: GqlNotificationInboxPriority,
    pub state: GqlNotificationInboxItemState,
    pub template_data: Vec<GqlNotificationTemplateField>,
    pub seen_at: Option<String>,
    pub read_at: Option<String>,
    pub archived_at: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlNotificationInboxGroupSummary {
    pub group_key: String,
    pub item_count: u64,
    pub unread_count: u64,
    pub latest_item: GqlNotificationInboxItem,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlNotificationInboxGroupSummaryPage {
    pub groups: Vec<GqlNotificationInboxGroupSummary>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlNotificationInboxGroupItemsPage {
    pub items: Vec<GqlNotificationInboxItem>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[Object]
impl NotificationsQuery {
    async fn notification_inbox_unread_count(
        &self,
        ctx: &Context<'_>,
    ) -> Result<GqlNotificationInboxUnreadCount> {
        let scope = authenticated_scope(ctx)?;
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx
            .data_opt::<DatabaseConnection>()
            .cloned()
            .ok_or_else(capability_unavailable)?;
        let count = NotificationInboxUnreadCountService::new(db)
            .count_unread(NotificationInboxUnreadCountRequest {
                tenant_id: scope.tenant_id,
                recipient_id: scope.recipient_id,
            })
            .await
            .map_err(map_notification_error)?;

        Ok(GqlNotificationInboxUnreadCount {
            unread_count: count.unread_count,
        })
    }

    async fn notification_inbox_group_summaries(
        &self,
        ctx: &Context<'_>,
        cursor: Option<String>,
        limit: Option<i32>,
    ) -> Result<GqlNotificationInboxGroupSummaryPage> {
        let scope = authenticated_scope(ctx)?;
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let page = grouped_storefront_port(ctx)?
            .list_group_summaries(
                scope.port_context("group-summaries"),
                NotificationInboxStorefrontGroupSummaryRequest {
                    cursor,
                    limit: parse_limit(limit)?,
                },
            )
            .await
            .map_err(map_port_error)?;
        Ok(map_group_summary_page(page))
    }

    async fn notification_inbox_group_items(
        &self,
        ctx: &Context<'_>,
        group_key: String,
        state: Option<GqlNotificationInboxItemState>,
        cursor: Option<String>,
        limit: Option<i32>,
    ) -> Result<GqlNotificationInboxGroupItemsPage> {
        let scope = authenticated_scope(ctx)?;
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let page = grouped_storefront_port(ctx)?
            .list_group_items(
                scope.port_context("group-items"),
                NotificationInboxStorefrontGroupItemsRequest {
                    group_key,
                    state: state.map(map_state_to_owner),
                    cursor,
                    limit: parse_limit(limit)?,
                },
            )
            .await
            .map_err(map_port_error)?;
        Ok(GqlNotificationInboxGroupItemsPage {
            items: page.items.into_iter().map(map_item).collect(),
            next_cursor: page.next_cursor,
            has_more: page.has_more,
        })
    }
}

#[derive(Clone)]
struct AuthenticatedInboxScope {
    tenant_id: Uuid,
    recipient_id: Uuid,
    actor: PortActor,
    claims: Vec<String>,
    locale: String,
}

impl AuthenticatedInboxScope {
    fn port_context(&self, operation: &'static str) -> PortContext {
        let mut context = PortContext::new(
            self.tenant_id.to_string(),
            self.actor.clone(),
            self.locale.clone(),
            format!("notifications-graphql-{operation}-{}", Uuid::new_v4()),
        )
        .with_deadline(GRAPHQL_READ_DEADLINE)
        .with_channel("storefront");
        for claim in &self.claims {
            context = context.with_claim(claim.clone());
        }
        context
    }
}

fn authenticated_scope(ctx: &Context<'_>) -> Result<AuthenticatedInboxScope> {
    let auth = ctx.data_opt::<AuthContext>().ok_or_else(|| {
        public_error(
            "NOTIFICATION_INBOX_USER_REQUIRED",
            "notification inbox access requires an authenticated user",
            false,
        )
    })?;
    if !auth.is_human_user_principal() {
        return Err(public_error(
            "NOTIFICATION_INBOX_USER_REQUIRED",
            "notification inbox access requires an authenticated user",
            false,
        ));
    }
    let tenant = ctx.data_opt::<TenantContext>().ok_or_else(capability_unavailable)?;
    if auth.tenant_id != tenant.id {
        return Err(public_error(
            "NOTIFICATION_INBOX_TENANT_MISMATCH",
            "notification inbox tenant context is invalid",
            false,
        ));
    }
    let locale = ctx
        .data_opt::<RequestContext>()
        .map(|request| request.locale.clone())
        .unwrap_or_else(|| tenant.default_locale.clone());

    Ok(AuthenticatedInboxScope {
        tenant_id: tenant.id,
        recipient_id: auth.user_id,
        actor: auth.port_actor(),
        claims: auth
            .permissions
            .iter()
            .map(ToString::to_string)
            .collect(),
        locale,
    })
}

fn grouped_storefront_port(
    ctx: &Context<'_>,
) -> Result<Arc<dyn NotificationInboxStorefrontPort>> {
    ctx.data_opt::<NotificationsGraphqlRuntimeData>()
        .and_then(|runtime| runtime.storefront.clone())
        .ok_or_else(capability_unavailable)
}

fn parse_limit(limit: Option<i32>) -> Result<u16> {
    let limit = limit.unwrap_or_default();
    u16::try_from(limit).map_err(|_| {
        public_error(
            "NOTIFICATION_VALIDATION_ERROR",
            "notification inbox page limit is invalid",
            false,
        )
    })
}

fn map_group_summary_page(
    page: NotificationInboxGroupSummaryPage,
) -> GqlNotificationInboxGroupSummaryPage {
    GqlNotificationInboxGroupSummaryPage {
        groups: page
            .groups
            .into_iter()
            .map(|group| GqlNotificationInboxGroupSummary {
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

fn map_item(item: NotificationInboxItem) -> GqlNotificationInboxItem {
    GqlNotificationInboxItem {
        id: item.id.to_string(),
        source: item.source.into_string(),
        notification_type: item.notification_type.into_string(),
        template_key: item.template_key.into_string(),
        actor_id: item.actor_id.map(|id| id.to_string()),
        priority: match item.priority {
            crate::api::NotificationPriority::Low => GqlNotificationInboxPriority::Low,
            crate::api::NotificationPriority::Normal => GqlNotificationInboxPriority::Normal,
            crate::api::NotificationPriority::High => GqlNotificationInboxPriority::High,
            crate::api::NotificationPriority::Urgent => GqlNotificationInboxPriority::Urgent,
        },
        state: match item.state {
            crate::model::NotificationState::Unread => GqlNotificationInboxItemState::Unread,
            crate::model::NotificationState::Seen => GqlNotificationInboxItemState::Seen,
            crate::model::NotificationState::Read => GqlNotificationInboxItemState::Read,
            crate::model::NotificationState::Archived => GqlNotificationInboxItemState::Archived,
        },
        template_data: item
            .template_data
            .into_inner()
            .into_iter()
            .map(|(key, value)| GqlNotificationTemplateField { key, value })
            .collect(),
        seen_at: item.seen_at.map(|value| value.to_rfc3339()),
        read_at: item.read_at.map(|value| value.to_rfc3339()),
        archived_at: item.archived_at.map(|value| value.to_rfc3339()),
        created_at: item.created_at.to_rfc3339(),
    }
}

fn map_state_to_owner(
    state: GqlNotificationInboxItemState,
) -> crate::model::NotificationState {
    match state {
        GqlNotificationInboxItemState::Unread => crate::model::NotificationState::Unread,
        GqlNotificationInboxItemState::Seen => crate::model::NotificationState::Seen,
        GqlNotificationInboxItemState::Read => crate::model::NotificationState::Read,
        GqlNotificationInboxItemState::Archived => crate::model::NotificationState::Archived,
    }
}

fn map_port_error(error: PortError) -> async_graphql::Error {
    match error.kind {
        PortErrorKind::Validation | PortErrorKind::Forbidden => {
            public_error(error.code, error.message, error.retryable)
        }
        PortErrorKind::NotFound
        | PortErrorKind::Conflict
        | PortErrorKind::Unavailable
        | PortErrorKind::Timeout
        | PortErrorKind::InvariantViolation => public_error(
            "NOTIFICATION_INBOX_UNAVAILABLE",
            PUBLIC_UNAVAILABLE_MESSAGE,
            error.retryable,
        ),
    }
}

fn map_notification_error(error: NotificationError) -> async_graphql::Error {
    match error {
        NotificationError::Validation(_) => public_error(
            "NOTIFICATION_VALIDATION_ERROR",
            "notification inbox identity is invalid",
            false,
        ),
        other => public_error(
            "NOTIFICATION_INBOX_UNAVAILABLE",
            PUBLIC_UNAVAILABLE_MESSAGE,
            other.is_retryable(),
        ),
    }
}

fn capability_unavailable() -> async_graphql::Error {
    public_error(
        "NOTIFICATION_INBOX_UNAVAILABLE",
        PUBLIC_UNAVAILABLE_MESSAGE,
        true,
    )
}

fn public_error(
    code: impl Into<String>,
    message: impl Into<String>,
    retryable: bool,
) -> async_graphql::Error {
    let code = code.into();
    async_graphql::Error::new(message.into()).extend_with(move |_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DbErr;

    fn extension_json(error: &async_graphql::Error, key: &str) -> Option<serde_json::Value> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get(key))
            .cloned()
            .and_then(|value| value.into_json().ok())
    }

    #[test]
    fn database_errors_map_to_generic_retryable_graphql_envelope() {
        let error = map_notification_error(NotificationError::Database(DbErr::Custom(
            "secret database detail".to_string(),
        )));

        assert_eq!(error.message, PUBLIC_UNAVAILABLE_MESSAGE);
        assert_eq!(
            extension_json(&error, "code").and_then(|value| value.as_str().map(ToOwned::to_owned)),
            Some("NOTIFICATION_INBOX_UNAVAILABLE".to_string())
        );
        assert_eq!(
            extension_json(&error, "retryable").and_then(|value| value.as_bool()),
            Some(true)
        );
        assert!(!error.message.contains("secret database detail"));
    }

    #[test]
    fn validation_errors_keep_stable_safe_code() {
        let error = map_notification_error(NotificationError::Validation(
            "internal validation detail".to_string(),
        ));

        assert_eq!(error.message, "notification inbox identity is invalid");
        assert_eq!(
            extension_json(&error, "code").and_then(|value| value.as_str().map(ToOwned::to_owned)),
            Some("NOTIFICATION_VALIDATION_ERROR".to_string())
        );
        assert_eq!(
            extension_json(&error, "retryable").and_then(|value| value.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn grouped_graphql_limit_rejects_negative_values() {
        let error = parse_limit(Some(-1)).expect_err("negative limits must be rejected");
        assert_eq!(error.message, "notification inbox page limit is invalid");
        assert_eq!(
            extension_json(&error, "code").and_then(|value| value.as_str().map(ToOwned::to_owned)),
            Some("NOTIFICATION_VALIDATION_ERROR".to_string())
        );
    }

    #[test]
    fn unavailable_port_errors_never_expose_internal_messages() {
        let error = map_port_error(PortError::unavailable(
            "notification.internal",
            "secret provider failure",
        ));
        assert_eq!(error.message, PUBLIC_UNAVAILABLE_MESSAGE);
        assert!(!error.message.contains("secret provider failure"));
    }
}
