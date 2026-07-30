//! Owner-local operational command adapters for `rustok-social-graph`.
//!
//! This crate delegates bounded receipt cleanup and relation-event replay to
//! Social Graph maintenance ports. It does not schedule work, read owner-private
//! tables directly, or construct outbox transport outside the owner crate.

use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use rustok_api::{PortActor, PortContext};
use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_runtime::{RuntimeComposition, db_clone};
use rustok_social_graph::{
    MAX_SOCIAL_GRAPH_RECEIPT_CLEANUP_BATCH, MAX_SOCIAL_GRAPH_RELATION_EVENT_REPLAY_BATCH,
    SocialGraphReceiptCleanupCommand, SocialGraphReceiptMaintenancePort,
    SocialGraphReceiptMaintenanceService, SocialGraphRelationEventMaintenancePort,
    SocialGraphRelationEventMaintenanceService, SocialGraphRelationEventReplayCommand,
};
use uuid::Uuid;

const DEFAULT_CLEANUP_LIMIT: u32 = 100;
const DEFAULT_REPLAY_LIMIT: u32 = 100;
const CLEANUP_DEADLINE: Duration = Duration::from_secs(30);
const REPLAY_DEADLINE: Duration = Duration::from_secs(30);

pub struct SocialGraphCommandProvider {
    runtime: RuntimeComposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReceiptCleanupOptions {
    tenant_id: Uuid,
    retention_days: u32,
    limit: u32,
    dry_run: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationEventReplayOptions {
    tenant_id: Uuid,
    after_relation_id: Option<Uuid>,
    limit: u32,
    dry_run: bool,
}

#[async_trait::async_trait]
impl CommandProvider for SocialGraphCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![
            CommandDescriptor::new(
                "social_graph",
                "receipt-cleanup",
                "Delete completed Social Graph command receipts outside an explicit retention window",
            )
            .with_dry_run(),
            CommandDescriptor::new(
                "social_graph",
                "relation-event-replay",
                "Replay bounded tenant-scoped authoritative relation state facts through the transactional outbox",
            )
            .with_dry_run(),
        ]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("social_graph", "receipt-cleanup") => self.cleanup_receipts(request).await,
            ("social_graph", "relation-event-replay") => {
                self.replay_relation_events(request).await
            }
            _ => Err(CliCoreError::UnknownCommand {
                namespace: request.namespace,
                name: request.name,
            }),
        }
    }
}

impl SocialGraphCommandProvider {
    async fn cleanup_receipts(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        let options = receipt_cleanup_options(&request)?;
        let retention = retention_duration(options.retention_days)?;
        let completed_before = Utc::now()
            .checked_sub_signed(retention)
            .ok_or_else(|| invalid_retention("produces an invalid cleanup cutoff"))?;
        let completed_before_unix_seconds = completed_before.timestamp();
        let host = self
            .runtime
            .require_host()
            .map_err(|error| CliCoreError::CommandFailed {
                message: format!(
                    "Social Graph receipt cleanup requires a database runtime: {error}"
                ),
            })?;
        let service = SocialGraphReceiptMaintenanceService::new(db_clone(host));
        let context = PortContext::new(
            options.tenant_id.to_string(),
            PortActor::system(),
            "en",
            format!("social-graph-receipt-cleanup-{}", Uuid::new_v4()),
        )
        .with_deadline(CLEANUP_DEADLINE)
        .with_idempotency_key(format!(
            "social-graph-receipt-cleanup:{}:{}:{}:{}",
            options.tenant_id, completed_before_unix_seconds, options.limit, options.dry_run
        ));
        let result = SocialGraphReceiptMaintenancePort::cleanup_completed_receipts(
            &service,
            context,
            SocialGraphReceiptCleanupCommand {
                completed_before_unix_seconds,
                limit: options.limit,
                dry_run: options.dry_run,
            },
        )
        .await
        .map_err(|error| CliCoreError::CommandFailed {
            message: error.message,
        })?;

        Ok(CommandOutcome::success("Social Graph receipt cleanup complete").with_data(
            serde_json::json!({
                "generated_at": Utc::now().to_rfc3339(),
                "tenant_id": options.tenant_id,
                "retention_days": options.retention_days,
                "completed_before_unix_seconds": completed_before_unix_seconds,
                "dry_run": options.dry_run,
                "limit": options.limit,
                "matched_receipts": result.matched_receipts,
                "deleted_receipts": result.deleted_receipts,
                "oldest_retained_completed_at_unix_seconds": result.oldest_retained_completed_at_unix_seconds,
            }),
        ))
    }

    async fn replay_relation_events(
        &self,
        request: CommandRequest,
    ) -> CliCoreResult<CommandOutcome> {
        let options = relation_event_replay_options(&request)?;
        let host = self
            .runtime
            .require_host()
            .map_err(|error| CliCoreError::CommandFailed {
                message: format!(
                    "Social Graph relation-event replay requires a database runtime: {error}"
                ),
            })?;
        let service = SocialGraphRelationEventMaintenanceService::with_outbox(db_clone(host));
        let cursor_key = options.after_relation_id.map_or_else(
            || "start".to_string(),
            |relation_id| relation_id.to_string(),
        );
        let context = PortContext::new(
            options.tenant_id.to_string(),
            PortActor::system(),
            "en",
            format!("social-graph-relation-event-replay-{}", Uuid::new_v4()),
        )
        .with_deadline(REPLAY_DEADLINE)
        .with_idempotency_key(format!(
            "social-graph-relation-event-replay:{}:{}:{}:{}",
            options.tenant_id, cursor_key, options.limit, options.dry_run
        ));
        let result = SocialGraphRelationEventMaintenancePort::replay_relation_state_events(
            &service,
            context,
            SocialGraphRelationEventReplayCommand {
                after_relation_id: options.after_relation_id,
                limit: options.limit,
                dry_run: options.dry_run,
            },
        )
        .await
        .map_err(|error| CliCoreError::CommandFailed {
            message: error.message,
        })?;

        Ok(CommandOutcome::success("Social Graph relation-event replay complete").with_data(
            serde_json::json!({
                "generated_at": Utc::now().to_rfc3339(),
                "tenant_id": options.tenant_id,
                "after_relation_id": options.after_relation_id,
                "dry_run": options.dry_run,
                "limit": options.limit,
                "selected_relations": result.selected_relations,
                "published_events": result.published_events,
                "next_after_relation_id": result.next_after_relation_id,
            }),
        ))
    }
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(SocialGraphCommandProvider {
        runtime: runtime.clone(),
    })
}

fn normalized_options<'a>(
    request: &'a CommandRequest,
    command: &str,
) -> CliCoreResult<&'a serde_json::Map<String, serde_json::Value>> {
    request
        .args
        .get("options")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| CliCoreError::InvalidInput {
            message: format!("social_graph {command} expects normalized command options"),
        })
}

fn receipt_cleanup_options(request: &CommandRequest) -> CliCoreResult<ReceiptCleanupOptions> {
    let options = normalized_options(request, "receipt-cleanup")?;
    let tenant_id = required_uuid(options, "tenant_id")?;
    let retention_days = required_u32(options, "retention_days")?;
    if retention_days == 0 {
        return Err(invalid_retention("must be a positive integer"));
    }
    retention_duration(retention_days)?;
    let limit = optional_u32(options, "limit")?.unwrap_or(DEFAULT_CLEANUP_LIMIT);
    if limit == 0 || limit > MAX_SOCIAL_GRAPH_RECEIPT_CLEANUP_BATCH {
        return Err(CliCoreError::InvalidInput {
            message: format!(
                "--limit must be between 1 and {MAX_SOCIAL_GRAPH_RECEIPT_CLEANUP_BATCH}"
            ),
        });
    }

    Ok(ReceiptCleanupOptions {
        tenant_id,
        retention_days,
        limit,
        dry_run: request.dry_run || flag(options, "dry_run"),
    })
}

fn relation_event_replay_options(
    request: &CommandRequest,
) -> CliCoreResult<RelationEventReplayOptions> {
    let options = normalized_options(request, "relation-event-replay")?;
    let tenant_id = required_uuid(options, "tenant_id")?;
    let after_relation_id = optional_uuid(options, "after_relation_id")?;
    let limit = optional_u32(options, "limit")?.unwrap_or(DEFAULT_REPLAY_LIMIT);
    if limit == 0 || limit > MAX_SOCIAL_GRAPH_RELATION_EVENT_REPLAY_BATCH {
        return Err(CliCoreError::InvalidInput {
            message: format!(
                "--limit must be between 1 and {MAX_SOCIAL_GRAPH_RELATION_EVENT_REPLAY_BATCH}"
            ),
        });
    }

    Ok(RelationEventReplayOptions {
        tenant_id,
        after_relation_id,
        limit,
        dry_run: request.dry_run || flag(options, "dry_run"),
    })
}

fn retention_duration(retention_days: u32) -> CliCoreResult<ChronoDuration> {
    let retention = ChronoDuration::try_days(i64::from(retention_days))
        .ok_or_else(|| invalid_retention("is outside the supported timestamp range"))?;
    Utc::now()
        .checked_sub_signed(retention)
        .ok_or_else(|| invalid_retention("is outside the supported timestamp range"))?;
    Ok(retention)
}

fn invalid_retention(reason: &str) -> CliCoreError {
    CliCoreError::InvalidInput {
        message: format!("--retention-days {reason}"),
    }
}

fn required_uuid(
    options: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> CliCoreResult<Uuid> {
    options
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CliCoreError::InvalidInput {
            message: format!("--{key} is required"),
        })
        .and_then(|raw| {
            Uuid::parse_str(raw).map_err(|_| CliCoreError::InvalidInput {
                message: format!("--{key} must be a UUID"),
            })
        })
}

fn optional_uuid(
    options: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> CliCoreResult<Option<Uuid>> {
    options
        .get(key)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| CliCoreError::InvalidInput {
                    message: format!("--{key} must be a UUID"),
                })
                .and_then(|raw| {
                    Uuid::parse_str(raw).map_err(|_| CliCoreError::InvalidInput {
                        message: format!("--{key} must be a UUID"),
                    })
                })
        })
        .transpose()
}

fn required_u32(
    options: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> CliCoreResult<u32> {
    options
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CliCoreError::InvalidInput {
            message: format!("--{key} is required"),
        })
        .and_then(|raw| {
            raw.parse::<u32>().map_err(|_| CliCoreError::InvalidInput {
                message: format!("--{key} must be a positive integer"),
            })
        })
}

fn optional_u32(
    options: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> CliCoreResult<Option<u32>> {
    options
        .get(key)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| CliCoreError::InvalidInput {
                    message: format!("--{key} must be a positive integer"),
                })
                .and_then(|raw| {
                    raw.parse::<u32>().map_err(|_| CliCoreError::InvalidInput {
                        message: format!("--{key} must be a positive integer"),
                    })
                })
        })
        .transpose()
}

fn flag(options: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    options
        .get(key)
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| matches!(value, "1" | "true" | "yes" | "on"))
}

#[cfg(test)]
mod tests {
    use super::{
        command_provider, receipt_cleanup_options, relation_event_replay_options,
        retention_duration,
    };
    use rustok_cli_core::CommandRequest;
    use rustok_runtime::RuntimeComposition;
    use uuid::Uuid;

    fn request(name: &str, options: serde_json::Value, dry_run: bool) -> CommandRequest {
        CommandRequest {
            namespace: "social_graph".to_string(),
            name: name.to_string(),
            args: serde_json::json!({ "options": options }),
            dry_run,
        }
    }

    #[test]
    fn provider_describes_owner_maintenance_commands() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let commands = provider.commands();
        assert_eq!(commands.len(), 2);
        assert!(commands.iter().any(|command| {
            command.namespace == "social_graph"
                && command.name == "receipt-cleanup"
                && command.supports_dry_run
        }));
        assert!(commands.iter().any(|command| {
            command.namespace == "social_graph"
                && command.name == "relation-event-replay"
                && command.supports_dry_run
        }));
    }

    #[test]
    fn cleanup_requires_explicit_retention_and_bounded_limit() {
        let tenant_id = Uuid::new_v4();
        assert!(
            receipt_cleanup_options(&request(
                "receipt-cleanup",
                serde_json::json!({ "tenant_id": tenant_id.to_string() }),
                true,
            ))
            .is_err()
        );
        assert!(
            receipt_cleanup_options(&request(
                "receipt-cleanup",
                serde_json::json!({
                    "tenant_id": tenant_id.to_string(),
                    "retention_days": "0"
                }),
                true,
            ))
            .is_err()
        );
        assert!(
            receipt_cleanup_options(&request(
                "receipt-cleanup",
                serde_json::json!({
                    "tenant_id": tenant_id.to_string(),
                    "retention_days": "30",
                    "limit": "1001"
                }),
                true,
            ))
            .is_err()
        );

        let options = receipt_cleanup_options(&request(
            "receipt-cleanup",
            serde_json::json!({
                "tenant_id": tenant_id.to_string(),
                "retention_days": "30"
            }),
            true,
        ))
        .unwrap();
        assert_eq!(options.retention_days, 30);
        assert_eq!(options.limit, 100);
        assert!(options.dry_run);
    }

    #[test]
    fn replay_requires_tenant_and_bounds_cursor_and_limit() {
        let tenant_id = Uuid::new_v4();
        let cursor = Uuid::new_v4();
        assert!(
            relation_event_replay_options(&request(
                "relation-event-replay",
                serde_json::json!({ "tenant_id": "not-a-uuid" }),
                true,
            ))
            .is_err()
        );
        assert!(
            relation_event_replay_options(&request(
                "relation-event-replay",
                serde_json::json!({
                    "tenant_id": tenant_id.to_string(),
                    "after_relation_id": "not-a-uuid"
                }),
                true,
            ))
            .is_err()
        );
        assert!(
            relation_event_replay_options(&request(
                "relation-event-replay",
                serde_json::json!({
                    "tenant_id": tenant_id.to_string(),
                    "limit": "1001"
                }),
                true,
            ))
            .is_err()
        );

        let options = relation_event_replay_options(&request(
            "relation-event-replay",
            serde_json::json!({
                "tenant_id": tenant_id.to_string(),
                "after_relation_id": cursor.to_string(),
                "limit": "25"
            }),
            true,
        ))
        .unwrap();
        assert_eq!(options.tenant_id, tenant_id);
        assert_eq!(options.after_relation_id, Some(cursor));
        assert_eq!(options.limit, 25);
        assert!(options.dry_run);
    }

    #[test]
    fn retention_duration_rejects_timestamp_overflow_without_panicking() {
        assert!(retention_duration(u32::MAX).is_err());
        assert!(retention_duration(30).is_ok());
    }

    #[tokio::test]
    async fn cleanup_requires_database_runtime_after_input_validation() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let error = provider
            .execute(request(
                "receipt-cleanup",
                serde_json::json!({
                    "tenant_id": Uuid::new_v4().to_string(),
                    "retention_days": "30"
                }),
                true,
            ))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("requires a database runtime"));
    }

    #[tokio::test]
    async fn replay_requires_database_runtime_after_input_validation() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let error = provider
            .execute(request(
                "relation-event-replay",
                serde_json::json!({
                    "tenant_id": Uuid::new_v4().to_string(),
                    "limit": "100"
                }),
                true,
            ))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("requires a database runtime"));
    }
}
