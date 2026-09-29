use std::{sync::Arc, time::Duration};

use async_graphql::{Context, Enum, ErrorExtensions, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, TenantContext,
    graphql::require_module_enabled, has_any_effective_permission, request::RequestContext,
};
use rustok_core::ModuleRuntimeExtensions;
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE, NotificationError, NotificationInboxGroupStateAction,
    NotificationInboxGroupStatePage, NotificationInboxGroupSummaryPage, NotificationInboxItem,
    NotificationInboxStorefrontGroupItemsRequest, NotificationInboxStorefrontGroupStateRequest,
    NotificationInboxStorefrontGroupSummaryRequest, NotificationInboxStorefrontOpenDecision,
    NotificationInboxStorefrontOpenRequest, NotificationInboxStorefrontPort,
    NotificationInboxUnreadCountRequest, NotificationInboxUnreadCountService,
    in_process_notification_inbox_storefront_port,
};

const MODULE_SLUG: &str = "notifications";
const FORUM_MODULE_SLUG: &str = "forum";
const FORUM_NOTIFICATION_RECONCILIATION_OPERATION: &str =
    "forum.notification_reconciliation_status";
const PUBLIC_UNAVAILABLE_MESSAGE: &str = "notification inbox capability is unavailable";
const GRAPHQL_READ_DEADLINE: Duration = Duration::from_secs(5);
const GRAPHQL_WRITE_DEADLINE: Duration = Duration::from_secs(5);
const MAX_NOTIFICATION_ID_BYTES: usize = 64;
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 128;

#[derive(Default)]
pub struct NotificationsQuery;

#[derive(Default)]
pub struct NotificationsMutation;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "NotificationInboxOpenDecision")]
pub enum GqlNotificationInboxOpenDecision {
    Allowed,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "NotificationInboxGroupStateAction")]
pub enum GqlNotificationInboxGroupStateAction {
    MarkRead,
    MarkUnread,
    Archive,
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

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlNotificationInboxOpenAuthorization {
    pub decision: GqlNotificationInboxOpenDecision,
    pub route: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlNotificationInboxGroupStatePage {
    pub scanned: u64,
    pub changed: u64,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlForumNotificationReconciliationStatus {
    pub recipient_id: Uuid,
    pub scanned: u64,
    pub unavailable: u64,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub clean: bool,
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

    /// Dry-run operator status for one recipient's Notifications reconciliation page.
    ///
    /// Notifications remains the owner of durable reconciliation state. This query evaluates the
    /// current privacy/source authorization pipeline without mutating inbox or delivery state.
    async fn forum_notification_reconciliation_status(
        &self,
        ctx: &Context<'_>,
        recipient_id: Uuid,
        cursor: Option<String>,
        limit: Option<i32>,
    ) -> Result<GqlForumNotificationReconciliationStatus> {
        require_module_enabled(ctx, FORUM_MODULE_SLUG).await?;
        require_module_enabled(ctx, MODULE_SLUG).await?;

        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <async_graphql::FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        if auth.tenant_id != tenant.id {
            return Err(<async_graphql::FieldError as GraphQLError>::permission_denied(
                "Forum notification reconciliation access is denied",
            ));
        }
        require_forum_notification_operator_permissions(auth)?;
        if recipient_id.is_nil() {
            return Err(public_error(
                "NOTIFICATION_VALIDATION_ERROR",
                "notification recipient id is invalid",
                false,
            ));
        }

        let requested_limit = parse_limit(limit)?;
        let extensions = ctx.data::<Arc<ModuleRuntimeExtensions>>()?;
        let registry = extensions
            .get::<Arc<crate::api::NotificationSourceRegistry>>()
            .cloned()
            .ok_or_else(capability_unavailable)?;
        let policy = extensions
            .get::<crate::NotificationRecipientPolicyRuntime>()
            .cloned()
            .ok_or_else(capability_unavailable)?;
        let db = ctx
            .data_opt::<DatabaseConnection>()
            .cloned()
            .ok_or_else(capability_unavailable)?;

        metrics::record_module_entrypoint_call(
            "forum",
            "notification_reconciliation_status",
            "graphql",
        );
        let started_at = std::time::Instant::now();
        let result = NotificationInboxReconcileService::new(db, registry, policy.policy_arc())
            .inspect_page(NotificationInboxReconcileRequest {
                tenant_id: tenant.id,
                recipient_id,
                cursor,
                limit: requested_limit,
            })
            .await;
        metrics::record_span_duration(
            FORUM_NOTIFICATION_RECONCILIATION_OPERATION,
            started_at.elapsed().as_secs_f64(),
        );

        let page = match result {
            Ok(page) => page,
            Err(error) => {
                metrics::record_span_error(
                    FORUM_NOTIFICATION_RECONCILIATION_OPERATION,
                    "owner_status",
                );
                metrics::record_module_error(
                    "forum",
                    "notification_reconciliation_status",
                    "error",
                );
                return Err(map_reconciliation_error(error));
            }
        };

        Ok(GqlForumNotificationReconciliationStatus {
            recipient_id,
            scanned: u64::from(page.scanned),
            unavailable: u64::from(page.unavailable),
            next_cursor: page.next_cursor,
            has_more: page.has_more,
            clean: page.unavailable == 0,
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

    async fn notification_inbox_authorize_open(
        &self,
        ctx: &Context<'_>,
        notification_id: String,
    ) -> Result<GqlNotificationInboxOpenAuthorization> {
        let scope = authenticated_scope(ctx)?;
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let notification_id = parse_notification_id(notification_id.as_str())?;
        let decision = grouped_storefront_port(ctx)?
            .authorize_open(
                scope.port_context("open"),
                NotificationInboxStorefrontOpenRequest { notification_id },
            )
            .await
            .map_err(map_port_error)?;
        Ok(map_open_decision(decision))
    }
}

#[Object]
impl NotificationsMutation {
    async fn notification_inbox_apply_group_state(
        &self,
        ctx: &Context<'_>,
        group_key: String,
        action: GqlNotificationInboxGroupStateAction,
        cursor: Option<String>,
        limit: Option<i32>,
        idempotency_key: String,
    ) -> Result<GqlNotificationInboxGroupStatePage> {
        let scope = authenticated_scope(ctx)?;
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let idempotency_key = parse_idempotency_key(idempotency_key)?;
        let limit = parse_limit(limit)?;
        let page = grouped_storefront_port(ctx)?
            .apply_group_state(
                scope.write_port_context("group-state", idempotency_key),
                NotificationInboxStorefrontGroupStateRequest {
                    group_key,
                    action: map_group_action_to_owner(action),
                    cursor,
                    limit,
                },
            )
            .await
            .map_err(map_port_error)?;
        Ok(map_group_state_page(page))
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
        self.base_port_context(operation, GRAPHQL_READ_DEADLINE)
            .with_deadline(GRAPHQL_READ_DEADLINE)
    }

    fn write_port_context(&self, operation: &'static str, idempotency_key: String) -> PortContext {
        self.base_port_context(operation, GRAPHQL_WRITE_DEADLINE)
            .with_idempotency_key(idempotency_key)
    }

    fn base_port_context(&self, operation: &'static str, deadline: Duration) -> PortContext {
        let mut context = PortContext::new(
            self.tenant_id.to_string(),
            self.actor.clone(),
            self.locale.clone(),
            format!("notifications-graphql-{operation}-{}", Uuid::new_v4()),
        )
        .with_deadline(deadline)
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
    let tenant = ctx
        .data_opt::<TenantContext>()
        .ok_or_else(capability_unavailable)?;
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
        claims: auth.permissions.iter().map(ToString::to_string).collect(),
        locale,
    })
}

fn grouped_storefront_port(ctx: &Context<'_>) -> Result<Arc<dyn NotificationInboxStorefrontPort>> {
    ctx.data_opt::<NotificationsGraphqlRuntimeData>()
        .and_then(|runtime| runtime.storefront.clone())
        .ok_or_else(capability_unavailable)
}

fn require_forum_notification_operator_permissions(auth: &AuthContext) -> Result<()> {
    let categories_manage = has_any_effective_permission(
        &auth.permissions,
        &[Permission::FORUM_CATEGORIES_MANAGE],
    );
    let topics_manage =
        has_any_effective_permission(&auth.permissions, &[Permission::FORUM_TOPICS_MANAGE]);
    if categories_manage && topics_manage {
        Ok(())
    } else {
        Err(<async_graphql::FieldError as GraphQLError>::permission_denied(
            "forum_categories:manage and forum_topics:manage required",
        ))
    }
}

fn parse_limit(limit: Option<i32>) -> Result<u16> {
    let limit = limit.unwrap_or(i32::from(DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE));
    u16::try_from(limit).map_err(|_| {
        public_error(
            "NOTIFICATION_VALIDATION_ERROR",
            "notification inbox page limit is invalid",
            false,
        )
    })
}

fn parse_notification_id(value: &str) -> Result<Uuid> {
    if value.is_empty()
        || value.len() > MAX_NOTIFICATION_ID_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(invalid_notification_id());
    }
    Uuid::parse_str(value)
        .ok()
        .filter(|notification_id| !notification_id.is_nil())
        .ok_or_else(invalid_notification_id)
}

fn invalid_notification_id() -> async_graphql::Error {
    public_error(
        "NOTIFICATION_VALIDATION_ERROR",
        "notification id is invalid",
        false,
    )
}

fn parse_idempotency_key(value: String) -> Result<String> {
    if value.is_empty()
        || value.len() > MAX_IDEMPOTENCY_KEY_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(public_error(
            "NOTIFICATION_VALIDATION_ERROR",
            "notification idempotency key is invalid",
            false,
        ));
    }
    Ok(value)
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

fn map_group_state_page(
    page: NotificationInboxGroupStatePage,
) -> GqlNotificationInboxGroupStatePage {
    GqlNotificationInboxGroupStatePage {
        scanned: u64::from(page.scanned),
        changed: u64::from(page.changed),
        next_cursor: page.next_cursor,
        has_more: page.has_more,
    }
}

fn map_group_action_to_owner(
    action: GqlNotificationInboxGroupStateAction,
) -> NotificationInboxGroupStateAction {
    match action {
        GqlNotificationInboxGroupStateAction::MarkRead => {
            NotificationInboxGroupStateAction::MarkRead
        }
        GqlNotificationInboxGroupStateAction::MarkUnread => {
            NotificationInboxGroupStateAction::MarkUnread
        }
        GqlNotificationInboxGroupStateAction::Archive => NotificationInboxGroupStateAction::Archive,
    }
}

fn map_open_decision(
    decision: NotificationInboxStorefrontOpenDecision,
) -> GqlNotificationInboxOpenAuthorization {
    match decision {
        NotificationInboxStorefrontOpenDecision::Allowed { route } => {
            GqlNotificationInboxOpenAuthorization {
                decision: GqlNotificationInboxOpenDecision::Allowed,
                route: Some(route.as_str().to_string()),
            }
        }
        NotificationInboxStorefrontOpenDecision::Unavailable => {
            GqlNotificationInboxOpenAuthorization {
                decision: GqlNotificationInboxOpenDecision::Unavailable,
                route: None,
            }
        }
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

fn map_state_to_owner(state: GqlNotificationInboxItemState) -> crate::model::NotificationState {
    match state {
        GqlNotificationInboxItemState::Unread => crate::model::NotificationState::Unread,
        GqlNotificationInboxItemState::Seen => crate::model::NotificationState::Seen,
        GqlNotificationInboxItemState::Read => crate::model::NotificationState::Read,
        GqlNotificationInboxItemState::Archived => crate::model::NotificationState::Archived,
    }
}

fn map_port_error(error: PortError) -> async_graphql::Error {
    let PortError {
        kind,
        code,
        message,
        retryable,
    } = error;
    match kind {
        PortErrorKind::Validation | PortErrorKind::Forbidden => {
            public_error(code, message, retryable)
        }
        PortErrorKind::NotFound
        | PortErrorKind::Conflict
        | PortErrorKind::Unavailable
        | PortErrorKind::Timeout
        | PortErrorKind::InvariantViolation => public_error(
            "NOTIFICATION_INBOX_UNAVAILABLE",
            PUBLIC_UNAVAILABLE_MESSAGE,
            retryable,
        ),
    }
}

fn map_reconciliation_error(error: NotificationError) -> async_graphql::Error {
    match error {
        NotificationError::Validation(_) => public_error(
            "NOTIFICATION_VALIDATION_ERROR",
            "notification reconciliation request is invalid",
            false,
        ),
        other => public_error(
            "NOTIFICATION_INBOX_UNAVAILABLE",
            PUBLIC_UNAVAILABLE_MESSAGE,
            other.is_retryable(),
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
    async_graphql::Error::new(message.into()).extend_with(|_, extensions| {
        extensions.set("code", code.clone());
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
    fn forum_reconciliation_operator_requires_only_canonical_forum_manage_permissions() {
        let mut auth = AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions: vec![
                Permission::FORUM_CATEGORIES_MANAGE,
                Permission::FORUM_TOPICS_MANAGE,
            ],
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        };
        assert!(require_forum_notification_operator_permissions(&auth).is_ok());
        auth.permissions.push(Permission::SETTINGS_READ);
        assert!(require_forum_notification_operator_permissions(&auth).is_ok());
        auth.permissions = vec![Permission::FORUM_TOPICS_MANAGE];
        assert!(require_forum_notification_operator_permissions(&auth).is_err());
    }

    #[test]
    fn grouped_graphql_limit_uses_canonical_default() {
        assert_eq!(
            parse_limit(None).expect("omitted limit should use canonical default"),
            DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE
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
    fn open_graphql_rejects_invalid_and_nil_notification_ids() {
        let nil_notification_id = Uuid::nil().to_string();
        for value in ["not-a-uuid", nil_notification_id.as_str()] {
            let error = parse_notification_id(value)
                .expect_err("invalid notification identifiers must be rejected");
            assert_eq!(error.message, "notification id is invalid");
            assert_eq!(
                extension_json(&error, "code")
                    .and_then(|value| value.as_str().map(ToOwned::to_owned)),
                Some("NOTIFICATION_VALIDATION_ERROR".to_string())
            );
        }
    }

    #[test]
    fn unavailable_open_decision_never_exposes_a_route() {
        let decision = map_open_decision(NotificationInboxStorefrontOpenDecision::Unavailable);
        assert_eq!(
            decision.decision,
            GqlNotificationInboxOpenDecision::Unavailable
        );
        assert_eq!(decision.route, None);
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
