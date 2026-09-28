use async_graphql::{Context, FieldError, Result, Subscription};
use futures_util::stream;
use sea_orm::DatabaseConnection;

use rustok_api::{HostAuthority, HostAuthorityContext};
use crate::graphql::types::BuildProgressEvent;
use crate::services::build_event_hub::BuildEventHub;
use rustok_api::graphql::GraphQLError;
use rustok_core::EventConsumerRuntime;

#[derive(Default)]
pub struct BuildSubscription;

async fn ensure_host_build_read_authority(ctx: &Context<'_>) -> Result<()> {
    let authority = ctx
        .data::<HostAuthorityContext>()
        .map_err(|_| <FieldError as GraphQLError>::permission_denied(
            "Host-global authority required",
        ))?;

    if !authority.allows(HostAuthority::Read) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Host-global read authority required",
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
        ensure_host_build_read_authority(ctx).await?;

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
