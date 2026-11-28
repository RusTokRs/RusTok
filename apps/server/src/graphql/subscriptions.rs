use std::{
    collections::{HashMap, VecDeque},
    time::Duration,
};

use async_graphql::{Context, Result, Subscription};
use futures_util::stream;
use rustok_api::HostAuthorityContext;
use rustok_core::EventConsumerRuntime;
use uuid::Uuid;

use crate::graphql::types::BuildProgressEvent;
use crate::services::build_event_hub::BuildEventHub;

const BUILD_PROGRESS_POLL_INTERVAL: Duration = Duration::from_secs(1);
const BUILD_PROGRESS_PAGE_SIZE: u64 = 100;

#[derive(Default)]
pub struct BuildSubscription;

#[cfg(test)]
fn graphql_internal_error(
    message: &'static str,
    error: impl std::fmt::Display,
) -> async_graphql::FieldError {
    use rustok_api::graphql::GraphQLError;

    tracing::error!(%error, message, "GraphQL build subscription failed");
    <async_graphql::FieldError as GraphQLError>::internal_error(message)
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

        let hub = ctx.data::<std::sync::Arc<BuildEventHub>>()?.clone();
        let _stream_permit = hub
            .try_acquire_stream()
            .ok_or_else(|| async_graphql::Error::new("Build progress stream capacity reached"))?;
        let control = hub.control();

        let build_filter = build_id
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                Uuid::parse_str(value.trim())
                    .map_err(|_| async_graphql::Error::new("buildId must be a valid UUID"))
            })
            .transpose()?;

        // Capture the cursor before subscribing. Any update committed after
        // this point is returned by the durable query, even if its local
        // wake-up races with receiver creation.
        let cursor = control.0.current_progress_sequence().await.map_err(|error| {
            tracing::error!(%error, "Unable to initialize durable build progress cursor");
            async_graphql::Error::new("Unable to initialize build progress stream")
        })?;
        let receiver = hub.subscribe();

        // A filtered subscription may target a terminal build; an unfiltered
        // subscription starts with every queued/running build.
        let initial = if let Some(build_id) = build_filter {
            control
                .0
                .build_progress(build_id)
                .await
                .map_err(|error| {
                    tracing::error!(%error, "Unable to read initial build progress snapshot");
                    async_graphql::Error::new("Unable to initialize build progress stream")
                })?
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            control.0.active_build_progress().await.map_err(|error| {
                tracing::error!(%error, "Unable to read initial active build snapshots");
                async_graphql::Error::new("Unable to initialize build progress stream")
            })?
        };

        let mut last_sequences = HashMap::new();
        let mut pending = VecDeque::new();
        for snapshot in initial {
            last_sequences.insert(snapshot.id.clone(), snapshot.sequence);
            pending.push_back(BuildProgressEvent::from_snapshot(snapshot, true));
        }

        let host_authority = ctx
            .data_opt::<HostAuthorityContext>()
            .copied()
            .or_else(crate::host_authority::current_host_authority);
        let consumer_runtime = EventConsumerRuntime::new("graphql_build_progress");
        let interval = tokio::time::interval(BUILD_PROGRESS_POLL_INTERVAL);

        Ok(stream::unfold(
            (
                receiver,
                control,
                cursor,
                last_sequences,
                pending,
                build_filter,
                host_authority,
                consumer_runtime,
                interval,
                _stream_permit,
            ),
            move |(
                mut receiver,
                control,
                mut cursor,
                mut last_sequences,
                mut pending,
                build_filter,
                host_authority,
                consumer_runtime,
                mut interval,
                stream_permit,
            )| async move {
                loop {
                    if let Some(payload) = pending.pop_front() {
                        if !crate::services::platform_build_access::is_platform_build_operator(
                            host_authority,
                        ) {
                            return None;
                        }
                        return Some((
                            payload,
                            (
                                receiver,
                                control,
                                cursor,
                                last_sequences,
                                pending,
                                build_filter,
                                host_authority,
                                consumer_runtime,
                                interval,
                                stream_permit,
                            ),
                        ));
                    }

                    tokio::select! {
                        _ = interval.tick() => {}
                        result = receiver.recv() => match result {
                            Ok(()) => {}
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                                // A lagged wake-up is not lost state. Re-read the
                                // durable cursor immediately and recover the latest row.
                                consumer_runtime.lagged(skipped);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                consumer_runtime.closed();
                                return None;
                            }
                        }
                    }

                    if !crate::services::platform_build_access::is_platform_build_operator(
                        host_authority,
                    ) {
                        return None;
                    }

                    let updates = match control
                        .0
                        .build_progress_after(cursor, BUILD_PROGRESS_PAGE_SIZE)
                        .await
                    {
                        Ok(updates) => updates,
                        Err(error) => {
                            tracing::error!(%error, "Durable GraphQL build progress read failed");
                            return None;
                        }
                    };

                    for snapshot in updates {
                        cursor = cursor.max(snapshot.sequence);
                        if last_sequences
                            .get(&snapshot.id)
                            .is_some_and(|seen| *seen >= snapshot.sequence)
                        {
                            continue;
                        }
                        last_sequences.insert(snapshot.id.clone(), snapshot.sequence);

                        if build_filter
                            .is_some_and(|build_id| snapshot.id != build_id.to_string())
                        {
                            continue;
                        }

                        pending.push_back(BuildProgressEvent::from_snapshot(snapshot, false));
                    }

                    // Once the cursor passes the initial snapshot watermarks,
                    // they are no longer needed for duplicate suppression.
                    last_sequences.retain(|_, sequence| *sequence > cursor);
                }
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::graphql_internal_error;
    use crate::graphql::types::{BuildProgressEvent, GqlBuildEventKind, GqlBuildStage, GqlBuildStatus};
    use rustok_api::{
        PlatformBuildProgressSnapshot, PlatformBuildStage, PlatformBuildStatus,
    };

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

    #[test]
    fn progress_snapshot_preserves_decimal_cursor_and_snapshot_marker() {
        let event = BuildProgressEvent::from_snapshot(
            PlatformBuildProgressSnapshot {
                sequence: 9_007_199_254_740_993,
                id: "00000000-0000-0000-0000-000000000001".to_string(),
                status: PlatformBuildStatus::Running,
                stage: PlatformBuildStage::Build,
                progress: 42,
                requested_by: "operator".to_string(),
                error_message: None,
            },
            true,
        );

        assert_eq!(event.kind, GqlBuildEventKind::Progress);
        assert_eq!(event.status, GqlBuildStatus::Running);
        assert_eq!(event.stage, GqlBuildStage::Build);
        assert_eq!(event.sequence, "9007199254740993");
        assert!(event.is_snapshot);
    }
}
