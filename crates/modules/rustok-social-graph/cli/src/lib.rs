//! Owner-local operational command adapters for `rustok-social-graph`.
//!
//! This crate derives an explicit receipt cutoff from reviewed retention input
//! and delegates the bounded operation to the Social Graph maintenance port. It
//! does not schedule cleanup or read owner-private receipt rows directly.

use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use rustok_api::{PortActor, PortContext};
use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_runtime::{RuntimeComposition, db_clone};
use rustok_social_graph::{
    MAX_SOCIAL_GRAPH_RECEIPT_CLEANUP_BATCH, SocialGraphReceiptCleanupCommand,
    SocialGraphReceiptMaintenancePort, SocialGraphReceiptMaintenanceService,
};
use uuid::Uuid;

const DEFAULT_CLEANUP_LIMIT: u32 = 100;
const CLEANUP_DEADLINE: Duration = Duration::from_secs(30);

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

#[async_trait::async_trait]
impl CommandProvider for SocialGraphCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor::new(
            "social_graph",
            "receipt-cleanup",
            "Delete completed Social Graph command receipts outside an explicit retention window",
        )
        .with_dry_run()]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("social_graph", "receipt-cleanup") => self.cleanup_receipts(request).await,
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
            message: format!("{}: {}", error.code, error.message),
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
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(SocialGraphCommandProvider {
        runtime: runtime.clone(),
    })
}

fn receipt_cleanup_options(request: &CommandRequest) -> CliCoreResult<ReceiptCleanupOptions> {
    let options = request
        .args
        .get("options")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| CliCoreError::InvalidInput {
            message: "social_graph receipt-cleanup expects normalized command options".to_string(),
        })?;
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

fn retention_duration(retention_days: u32) -> CliCoreResult<ChronoDuration> {
    let retention = ChronoDuration::try_days(i64::from(retention_days))
        .ok_or_else(|| invalid_retention("is outside the supported timestamp range"))?;
    let cutoff = Utc::now()
        .checked_sub_signed(retention)
        .ok_or_else(|| invalid_retention("is outside the supported timestamp range"))?;
    if cutoff.timestamp() <= 0 {
        return Err(invalid_retention(
            "must produce a timestamp after the Unix epoch",
        ));
    }
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
    let raw = match options.get(key) {
        None => {
            return Err(CliCoreError::InvalidInput {
                message: format!("--{key} is required"),
            });
        }
        Some(serde_json::Value::String(s)) => s.trim(),
        Some(_) => {
            return Err(CliCoreError::InvalidInput {
                message: format!("--{key} must be a UUID"),
            });
        }
    };
    if raw.is_empty() {
        return Err(CliCoreError::InvalidInput {
            message: format!("--{key} is required"),
        });
    }
    let parsed = Uuid::parse_str(raw).map_err(|_| CliCoreError::InvalidInput {
        message: format!("--{key} must be a UUID"),
    })?;
    if parsed.is_nil() {
        return Err(CliCoreError::InvalidInput {
            message: format!("--{key} cannot be a nil UUID"),
        });
    }
    Ok(parsed)
}

fn required_u32(
    options: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> CliCoreResult<u32> {
    match options.get(key) {
        None => Err(CliCoreError::InvalidInput {
            message: format!("--{key} is required"),
        }),
        Some(serde_json::Value::Number(n)) => {
            let val = n.as_u64().ok_or_else(|| CliCoreError::InvalidInput {
                message: format!("--{key} must be a positive integer"),
            })?;
            u32::try_from(val).map_err(|_| CliCoreError::InvalidInput {
                message: format!("--{key} must be a positive integer"),
            })
        }
        Some(serde_json::Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Err(CliCoreError::InvalidInput {
                    message: format!("--{key} is required"),
                });
            }
            trimmed.parse::<u32>().map_err(|_| CliCoreError::InvalidInput {
                message: format!("--{key} must be a positive integer"),
            })
        }
        Some(_) => Err(CliCoreError::InvalidInput {
            message: format!("--{key} must be a positive integer"),
        }),
    }
}

fn optional_u32(
    options: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> CliCoreResult<Option<u32>> {
    match options.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => {
            let val = n.as_u64().ok_or_else(|| CliCoreError::InvalidInput {
                message: format!("--{key} must be a positive integer"),
            })?;
            let val_u32 = u32::try_from(val).map_err(|_| CliCoreError::InvalidInput {
                message: format!("--{key} must be a positive integer"),
            })?;
            Ok(Some(val_u32))
        }
        Some(serde_json::Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            let parsed = trimmed.parse::<u32>().map_err(|_| CliCoreError::InvalidInput {
                message: format!("--{key} must be a positive integer"),
            })?;
            Ok(Some(parsed))
        }
        Some(_) => Err(CliCoreError::InvalidInput {
            message: format!("--{key} must be a positive integer"),
        }),
    }
}

fn flag(options: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    match options.get(key) {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::Number(n)) => n.as_i64() == Some(1),
        Some(serde_json::Value::String(s)) => {
            matches!(
                s.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{command_provider, receipt_cleanup_options, retention_duration};
    use rustok_cli_core::CommandRequest;
    use rustok_runtime::RuntimeComposition;
    use uuid::Uuid;

    fn request(options: serde_json::Value, dry_run: bool) -> CommandRequest {
        CommandRequest {
            namespace: "social_graph".to_string(),
            name: "receipt-cleanup".to_string(),
            args: serde_json::json!({ "options": options }),
            dry_run,
        }
    }

    #[test]
    fn provider_describes_owner_receipt_cleanup() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let commands = provider.commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].namespace, "social_graph");
        assert_eq!(commands[0].name, "receipt-cleanup");
        assert!(commands[0].supports_dry_run);
    }

    #[test]
    fn cleanup_requires_explicit_retention_and_bounded_limit() {
        let tenant_id = Uuid::new_v4();
        // Missing retention_days
        assert!(
            receipt_cleanup_options(&request(
                serde_json::json!({ "tenant_id": tenant_id.to_string() }),
                true,
            ))
            .is_err()
        );
        // Zero retention_days
        assert!(
            receipt_cleanup_options(&request(
                serde_json::json!({
                    "tenant_id": tenant_id.to_string(),
                    "retention_days": "0"
                }),
                true,
            ))
            .is_err()
        );
        // Zero retention_days as JSON number
        assert!(
            receipt_cleanup_options(&request(
                serde_json::json!({
                    "tenant_id": tenant_id.to_string(),
                    "retention_days": 0
                }),
                true,
            ))
            .is_err()
        );
        // Limit exceeding maximum batch
        assert!(
            receipt_cleanup_options(&request(
                serde_json::json!({
                    "tenant_id": tenant_id.to_string(),
                    "retention_days": "30",
                    "limit": "1001"
                }),
                true,
            ))
            .is_err()
        );
        // Zero limit
        assert!(
            receipt_cleanup_options(&request(
                serde_json::json!({
                    "tenant_id": tenant_id.to_string(),
                    "retention_days": "30",
                    "limit": 0
                }),
                true,
            ))
            .is_err()
        );

        // Valid with strings
        let options = receipt_cleanup_options(&request(
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

        // Valid with native JSON numbers and boolean flag
        let options_num = receipt_cleanup_options(&request(
            serde_json::json!({
                "tenant_id": format!("  {}  ", tenant_id),
                "retention_days": 45,
                "limit": 250,
                "dry_run": true
            }),
            false,
        ))
        .unwrap();
        assert_eq!(options_num.tenant_id, tenant_id);
        assert_eq!(options_num.retention_days, 45);
        assert_eq!(options_num.limit, 250);
        assert!(options_num.dry_run);
    }

    #[test]
    fn cleanup_rejects_nil_and_invalid_tenant_uuid() {
        // Nil UUID
        let err = receipt_cleanup_options(&request(
            serde_json::json!({
                "tenant_id": Uuid::nil().to_string(),
                "retention_days": 30
            }),
            true,
        ))
        .unwrap_err();
        assert!(err.to_string().contains("cannot be a nil UUID"));

        // Malformed UUID
        let err = receipt_cleanup_options(&request(
            serde_json::json!({
                "tenant_id": "not-a-valid-uuid",
                "retention_days": 30
            }),
            true,
        ))
        .unwrap_err();
        assert!(err.to_string().contains("must be a UUID"));

        // Empty string
        let err = receipt_cleanup_options(&request(
            serde_json::json!({
                "tenant_id": "   ",
                "retention_days": 30
            }),
            true,
        ))
        .unwrap_err();
        assert!(err.to_string().contains("--tenant_id is required"));
    }

    #[test]
    fn retention_duration_rejects_timestamp_overflow_without_panicking() {
        assert!(retention_duration(u32::MAX).is_err());
        assert!(retention_duration(30).is_ok());
        // Extremely large retention producing negative unix timestamp (before 1970)
        assert!(retention_duration(50_000_000).is_err());
    }

    #[tokio::test]
    async fn cleanup_requires_database_runtime_after_input_validation() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let error = provider
            .execute(request(
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
    async fn provider_rejects_unknown_command() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let error = provider
            .execute(CommandRequest {
                namespace: "social_graph".to_string(),
                name: "non-existent-action".to_string(),
                args: serde_json::json!({}),
                dry_run: false,
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("non-existent-action"));
    }
}
