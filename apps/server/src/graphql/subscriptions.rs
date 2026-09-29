use async_graphql::{Context, FieldError, Result, Subscription};
use std::fmt::Display;
use futures_util::stream;
use sea_orm::DatabaseConnection;

use crate::context::{AuthContext, TenantContext};
use crate::graphql::types::BuildProgressEvent;
use crate::services::build_event_hub::BuildEventHub;
use crate::services::rbac_service::RbacService;
use rustok_api::Permission;
use rustok_api::graphql::GraphQLError;
use rustok_core::EventConsumerRuntime;

#[derive(Default)]
pub struct BuildSubscription;

fn graphql_internal_error(message: &'static str, error: impl Display) -> FieldError {
    tracing::error!(%error, message, "GraphQL build subscription failed");
    <FieldError as GraphQLError>::internal_error(message)
}

async fn ensure_modules_read_permission(ctx: &Context<'_>) -> Result<()> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let db = ctx.data::<DatabaseConnection>()?;
    let tenant = ctx.data::<TenantContext>()?;

    let can_read_modules = RbacService::has_any_permission(
        db,
        &tenant.id,
        &auth.user_id,
        &[
            Permission::MODULES_READ,
            Permission::MODULES_LIST,
            Permission::MODULES_MANAGE,
        ],
    )
    .await
    .map_err(|err| graphql_internal_error("Unable to resolve build subscription permission", err))?;

    if !can_read_modules {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: modules:read required",
        ));
    }

    Ok(())
}

#[Subscription]
impl BuildSubscription {
    async fn build_progress(
        &self,
        ctx: &Context<'_>,
        build_id: Option<String>,
    ) -> Result<impl futures_util::Stream<Item = BuildProgressEvent>> {
        ensure_modules_read_permission(ctx).await?;

        let hub = ctx.data::<std::sync::Arc<BuildEventHub>>()?;
        let receiver = hub.subscribe();
        let build_filter = build_id.filter(|value| !value.trim().is_empty());
        let consumer_runtime = EventConsumerRuntime::new("graphql_build_progress");

        Ok(stream::unfold(
            (receiver, build_filter),
            move |(mut receiver, build_filter)| async move {
                loop {
                    match receiver.recv().await {
                        Ok(event) => {
                            let payload = BuildProgressEvent::from_event(event);
                            let passes_filter = match build_filter.as_ref() {
                                Some(build_id) => payload.build_id == *build_id,
                                None => true,
                            };
                            if passes_filter {
                                return Some((payload, (receiver, build_filter)));
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            consumer_runtime.lagged(skipped);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            consumer_runtime.closed();
                            return None;
                        }
                    }
                }
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::graphql_internal_error;

    #[test]
    fn graphql_subscription_internal_error_redacts_backend_diagnostics() {
        let error = graphql_internal_error(
            "Unable to resolve build subscription permission",
            "database password=secret table=permissions",
        );

        assert_eq!(
            error.message,
            "Unable to resolve build subscription permission"
        );
        assert!(!error.message.contains("database password=secret"));
    }
}
