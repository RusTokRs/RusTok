use rustok_blog::PreviewTokenService;
use rustok_cli_core::{CliCoreError, CliCoreResult, CommandOutcome, CommandRequest};
use rustok_runtime::RuntimeComposition;
use uuid::Uuid;

use super::options::{parse_limit, parse_optional_tenant_id};
use crate::runtime::owner_resources;

const CLEANUP_TOKENS_DEFAULT_LIMIT: u32 = 100;
const CLEANUP_TOKENS_MAX_LIMIT: u32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CleanupTokensOptions {
    pub tenant_id: Option<Uuid>,
    pub limit: u32,
}

pub fn parse_cleanup_tokens_options(
    request: &CommandRequest,
) -> CliCoreResult<CleanupTokensOptions> {
    let Some(options) = request
        .args
        .get("options")
        .and_then(serde_json::Value::as_object)
    else {
        return Err(CliCoreError::InvalidInput {
            message: "blog cleanup-tokens expects normalized command options".to_string(),
        });
    };

    let tenant_id = parse_optional_tenant_id(options)?;
    let limit = parse_limit(
        options,
        CLEANUP_TOKENS_DEFAULT_LIMIT,
        CLEANUP_TOKENS_MAX_LIMIT,
    )?;

    Ok(CleanupTokensOptions { tenant_id, limit })
}

pub(crate) async fn execute(
    runtime: &RuntimeComposition,
    request: CommandRequest,
) -> CliCoreResult<CommandOutcome> {
    let options = parse_cleanup_tokens_options(&request)?;
    let (db, events) = owner_resources(runtime)?;
    let service = PreviewTokenService::new(db, events);

    if request.dry_run {
        let expired_count = match options.tenant_id {
            Some(tenant_id) => service.count_expired(tenant_id).await.map_err(|error| {
                CliCoreError::CommandFailed {
                    message: format!("failed to count expired tokens: {error}"),
                }
            })?,
            None => {
                let tenants =
                    service
                        .list_tenants_with_expired_tokens()
                        .await
                        .map_err(|error| CliCoreError::CommandFailed {
                            message: format!("failed to list tenants with expired tokens: {error}"),
                        })?;
                let mut total = 0u64;
                for tenant_id in tenants {
                    total += service.count_expired(tenant_id).await.map_err(|error| {
                        CliCoreError::CommandFailed {
                            message: format!("failed to count expired tokens: {error}"),
                        }
                    })?;
                }
                total
            }
        };

        let message = match options.tenant_id {
            Some(tenant_id) => {
                format!(
                    "DRY RUN: {expired_count} expired preview tokens found for tenant {tenant_id}"
                )
            }
            None => {
                format!("DRY RUN: {expired_count} expired preview tokens found across all tenants")
            }
        };

        return Ok(
            CommandOutcome::success(message).with_data(serde_json::json!({
                "dry_run": true,
                "expired_count": expired_count,
                "tenant_id": options.tenant_id.map(|id| id.to_string()),
            })),
        );
    }

    let deleted_count = match options.tenant_id {
        Some(tenant_id) => service
            .cleanup_expired(tenant_id, options.limit)
            .await
            .map_err(|error| CliCoreError::CommandFailed {
                message: format!("preview token cleanup failed: {error}"),
            })?,
        None => {
            let tenants = service
                .list_tenants_with_expired_tokens()
                .await
                .map_err(|error| CliCoreError::CommandFailed {
                    message: format!("failed to list tenants with expired tokens: {error}"),
                })?;
            let mut total = 0u64;
            let mut remaining = options.limit;
            for tenant_id in tenants {
                if remaining == 0 {
                    break;
                }
                let deleted = service
                    .cleanup_expired(tenant_id, remaining)
                    .await
                    .map_err(|error| CliCoreError::CommandFailed {
                        message: format!(
                            "preview token cleanup failed for tenant {tenant_id}: {error}"
                        ),
                    })?;
                remaining = remaining.saturating_sub(deleted as u32);
                total += deleted;
            }
            total
        }
    };

    let message = match options.tenant_id {
        Some(tenant_id) => {
            format!("Deleted {deleted_count} expired preview tokens for tenant {tenant_id}")
        }
        None => format!("Deleted {deleted_count} expired preview tokens across all tenants"),
    };

    Ok(
        CommandOutcome::success(message).with_data(serde_json::json!({
            "dry_run": false,
            "deleted_count": deleted_count,
            "tenant_id": options.tenant_id.map(|id| id.to_string()),
        })),
    )
}
