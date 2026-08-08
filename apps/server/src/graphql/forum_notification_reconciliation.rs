use std::{sync::Arc, time::Instant};

use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    has_effective_permission,
};
use rustok_core::ModuleRuntimeExtensions;
use rustok_notifications::{
    NotificationError, NotificationInboxReconcileRequest, NotificationInboxReconcileService,
    NotificationRecipientPolicyRuntime,
};
use rustok_notifications::api::NotificationSourceRegistry;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

const FORUM_MODULE_SLUG: &str = "forum";
const NOTIFICATIONS_MODULE_SLUG: &str = "notifications";
const OPERATION: &str = "forum.notification_reconciliation_status";

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlForumNotificationReconciliationStatus {
    pub recipient_id: Uuid,
    pub scanned: u64,
    pub unavailable: u64,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub clean: bool,
}

#[derive(Default)]
pub struct ForumNotificationReconciliationQuery;

#[Object]
impl ForumNotificationReconciliationQuery {
    /// Dry-run FORUM-33 diagnostic over the Notifications-owned exact-recipient reconciliation
    /// boundary. It evaluates current privacy/source authorization and returns counts only; it
    /// never archives an inbox row or mutates delivery state.
    async fn forum_notification_reconciliation_status(
        &self,
        ctx: &Context<'_>,
        recipient_id: Uuid,
        cursor: Option<String>,
        limit: Option<i32>,
    ) -> Result<GqlForumNotificationReconciliationStatus> {
        require_module_enabled(ctx, FORUM_MODULE_SLUG).await?;
        require_module_enabled(ctx, NOTIFICATIONS_MODULE_SLUG).await?;

        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        if auth.tenant_id != tenant.id {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Forum notification reconciliation access is denied",
            ));
        }
        require_operator_permissions(auth)?;
        if recipient_id.is_nil() {
            return Err(FieldError::new("notification recipient id is invalid"));
        }

        let requested_limit = parse_limit(limit)?;
        let db = ctx.data::<DatabaseConnection>()?.clone();
        let extensions = ctx.data::<Arc<ModuleRuntimeExtensions>>()?;
        let registry = extensions
            .get::<Arc<NotificationSourceRegistry>>()
            .cloned()
            .ok_or_else(capability_unavailable)?;
        let policy = extensions
            .get::<NotificationRecipientPolicyRuntime>()
            .cloned()
            .ok_or_else(capability_unavailable)?;

        rustok_telemetry::metrics::record_module_entrypoint_call(
            "forum",
            "notification_reconciliation_status",
            "graphql",
        );
        let started_at = Instant::now();
        let result = NotificationInboxReconcileService::new(db, registry, policy.policy_arc())
            .inspect_page(NotificationInboxReconcileRequest {
                tenant_id: tenant.id,
                recipient_id,
                cursor,
                limit: requested_limit,
            })
            .await;
        rustok_telemetry::metrics::record_span_duration(
            OPERATION,
            started_at.elapsed().as_secs_f64(),
        );
        let page = match result {
            Ok(page) => page,
            Err(error) => {
                rustok_telemetry::metrics::record_span_error(OPERATION, "owner_status");
                rustok_telemetry::metrics::record_module_error(
                    "forum",
                    "notification_reconciliation_status",
                    "error",
                );
                return Err(map_notification_error(error));
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
}

fn require_operator_permissions(auth: &AuthContext) -> Result<()> {
    let settings_read = has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ);
    let categories_manage = has_effective_permission(
        &auth.permissions,
        &Permission::FORUM_CATEGORIES_MANAGE,
    );
    let topics_manage =
        has_effective_permission(&auth.permissions, &Permission::FORUM_TOPICS_MANAGE);
    if settings_read && categories_manage && topics_manage {
        Ok(())
    } else {
        Err(<FieldError as GraphQLError>::permission_denied(
            "settings:read, forum_categories:manage and forum_topics:manage required",
        ))
    }
}

fn parse_limit(limit: Option<i32>) -> Result<u16> {
    match limit {
        None => Ok(0),
        Some(limit) if limit >= 0 => u16::try_from(limit)
            .map_err(|_| FieldError::new("notification reconciliation limit is invalid")),
        Some(_) => Err(FieldError::new("notification reconciliation limit is invalid")),
    }
}

fn capability_unavailable() -> FieldError {
    FieldError::new("Forum notification reconciliation capability is unavailable")
}

fn map_notification_error(error: NotificationError) -> FieldError {
    match error {
        NotificationError::Validation(_) => {
            FieldError::new("notification reconciliation request is invalid")
        }
        _ => <FieldError as GraphQLError>::internal_error(
            "Forum notification reconciliation capability is unavailable",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_limit;

    #[test]
    fn limit_preserves_owner_default_and_rejects_invalid_graphql_width() {
        assert_eq!(parse_limit(None).unwrap(), 0);
        assert_eq!(parse_limit(Some(64)).unwrap(), 64);
        assert!(parse_limit(Some(-1)).is_err());
        assert!(parse_limit(Some(i32::MAX)).is_err());
    }
}
