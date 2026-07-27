use async_graphql::{Context, ErrorExtensions, Object, Result, SimpleObject};
use rustok_api::{AuthContext, TenantContext, graphql::require_module_enabled};
use sea_orm::DatabaseConnection;

use crate::{
    NotificationError, NotificationInboxUnreadCountRequest, NotificationInboxUnreadCountService,
};

const MODULE_SLUG: &str = "notifications";
const PUBLIC_UNAVAILABLE_MESSAGE: &str = "notification inbox capability is unavailable";

#[derive(Default)]
pub struct NotificationsQuery;

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlNotificationInboxUnreadCount {
    pub unread_count: u64,
}

#[Object]
impl NotificationsQuery {
    async fn notification_inbox_unread_count(
        &self,
        ctx: &Context<'_>,
    ) -> Result<GqlNotificationInboxUnreadCount> {
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

        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx
            .data_opt::<DatabaseConnection>()
            .cloned()
            .ok_or_else(capability_unavailable)?;
        let count = NotificationInboxUnreadCountService::new(db)
            .count_unread(NotificationInboxUnreadCountRequest {
                tenant_id: tenant.id,
                recipient_id: auth.user_id,
            })
            .await
            .map_err(map_notification_error)?;

        Ok(GqlNotificationInboxUnreadCount {
            unread_count: count.unread_count,
        })
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

fn public_error(code: &'static str, message: &'static str, retryable: bool) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, extensions| {
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
}
