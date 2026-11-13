use async_graphql::{Context, FieldError, Result, Subscription};
use futures_util::stream;
use std::fmt::Display;

use rustok_api::HostAuthorityContext;

use crate::graphql::types::BuildProgressEvent;
use crate::services::build_event_hub::BuildEventHub;
use rustok_api::graphql::GraphQLError;
use rustok_core::EventConsumerRuntime;

#[derive(Default)]
pub struct BuildSubscription;

fn graphql_internal_error(message: &'static str, error: impl Display) -> FieldError {
    tracing::error!(%error, message, "GraphQL build subscription failed");
    <FieldError as GraphQLError>::internal_error(message)
}

async fn ensure_platform_build_read_permission(ctx: &Context<'_>) -> Result<()> {
    crate::graphql::queries::ensure_platform_build_read_permission(ctx).await
}

#[Subscription]
impl BuildSubscription {
    async fn build_progress(
        &self,
        ctx: &Context<'_>,
        build_id: Option<String>,
    ) -> Result<impl futures_util::Stream<Item = BuildProgressEvent>> {
        ensure_platform_build_read_permission(ctx).await?;

        let hub = ctx.data::<std::sync::Arc<BuildEventHub>>()?;
        let receiver = hub.subscribe();
        let build_filter = build_id.filter(|value| !value.trim().is_empty());
        let host_authority = ctx
            .data_opt::<HostAuthorityContext>()
            .copied()
            .or_else(crate::host_authority::current_host_authority);
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
                            if !passes_filter {
                                continue;
                            }
                            if !crate::services::platform_build_access::is_platform_build_operator(
                                host_authority,
                            ) {
                                return None;
                            }
                            return Some((payload, (receiver, build_filter)));
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
