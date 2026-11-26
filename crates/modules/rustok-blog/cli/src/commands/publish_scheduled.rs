use rustok_blog::PostService;
use rustok_cli_core::{CliCoreError, CliCoreResult, CommandOutcome, CommandRequest};
use rustok_runtime::RuntimeComposition;
use uuid::Uuid;

use super::options::{parse_limit, parse_optional_tenant_id};
use crate::runtime::owner_resources;

const SCHEDULED_PUBLISH_DEFAULT_LIMIT: u32 = 100;
const SCHEDULED_PUBLISH_MAX_LIMIT: u32 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublishScheduledOptions {
    pub tenant_id: Option<Uuid>,
    pub limit: u32,
}

pub fn parse_publish_scheduled_options(
    request: &CommandRequest,
) -> CliCoreResult<PublishScheduledOptions> {
    let Some(options) = request
        .args
        .get("options")
        .and_then(serde_json::Value::as_object)
    else {
        return Err(CliCoreError::InvalidInput {
            message: "blog publish-scheduled expects normalized command options".to_string(),
        });
    };

    let tenant_id = parse_optional_tenant_id(options)?;
    let limit = parse_limit(
        options,
        SCHEDULED_PUBLISH_DEFAULT_LIMIT,
        SCHEDULED_PUBLISH_MAX_LIMIT,
    )?;

    Ok(PublishScheduledOptions { tenant_id, limit })
}

pub(crate) async fn execute(
    runtime: &RuntimeComposition,
    request: CommandRequest,
) -> CliCoreResult<CommandOutcome> {
    let options = parse_publish_scheduled_options(&request)?;
    let (db, events) = owner_resources(runtime)?;
    let service = PostService::new(db, events);

    if request.dry_run {
        let due_count =
            match options.tenant_id {
                Some(tenant_id) => {
                    service
                        .count_due_scheduled_posts(tenant_id)
                        .await
                        .map_err(|error| CliCoreError::CommandFailed {
                            message: format!("failed to count due scheduled posts: {error}"),
                        })?
                }
                None => {
                    let tenants = service
                        .list_tenants_with_due_scheduled_posts()
                        .await
                        .map_err(|error| CliCoreError::CommandFailed {
                            message: format!("failed to discover tenants with due posts: {error}"),
                        })?;
                    let mut total = 0u64;
                    for tenant_id in tenants {
                        total += service.count_due_scheduled_posts(tenant_id).await.map_err(
                            |error| CliCoreError::CommandFailed {
                                message: format!("failed to count due scheduled posts: {error}"),
                            },
                        )?;
                    }
                    total
                }
            };

        let message = match options.tenant_id {
            Some(tenant_id) => {
                format!(
                    "DRY RUN: {due_count} scheduled posts due for publishing for tenant {tenant_id}"
                )
            }
            None => format!(
                "DRY RUN: {due_count} scheduled posts due for publishing across all tenants"
            ),
        };

        return Ok(
            CommandOutcome::success(message).with_data(serde_json::json!({
                "dry_run": true,
                "due_count": due_count,
                "tenant_id": options.tenant_id.map(|id| id.to_string()),
            })),
        );
    }

    let (published_ids, processed_tenants) = match options.tenant_id {
        Some(tenant_id) => {
            let ids = service
                .publish_scheduled_posts(tenant_id, options.limit)
                .await
                .map_err(|error| CliCoreError::CommandFailed {
                    message: format!("scheduled publish failed: {error}"),
                })?;
            (ids, 1)
        }
        None => {
            let tenants = service
                .list_tenants_with_due_scheduled_posts()
                .await
                .map_err(|error| CliCoreError::CommandFailed {
                    message: format!("failed to discover tenants with due posts: {error}"),
                })?;
            let mut all_ids = Vec::new();
            let mut remaining = options.limit;
            let tenant_count = tenants.len();
            for tenant_id in tenants {
                if remaining == 0 {
                    break;
                }
                let ids = service
                    .publish_scheduled_posts(tenant_id, remaining)
                    .await
                    .map_err(|error| CliCoreError::CommandFailed {
                        message: format!(
                            "scheduled publish failed for tenant {tenant_id}: {error}"
                        ),
                    })?;
                remaining = remaining.saturating_sub(ids.len() as u32);
                all_ids.extend(ids);
            }
            (all_ids, tenant_count)
        }
    };

    let message = match options.tenant_id {
        Some(tenant_id) => {
            format!(
                "Published {} scheduled posts for tenant {tenant_id}",
                published_ids.len()
            )
        }
        None => {
            format!(
                "Published {} scheduled posts across {processed_tenants} tenants",
                published_ids.len()
            )
        }
    };

    Ok(CommandOutcome::success(message).with_data(serde_json::json!({
        "dry_run": false,
        "published_count": published_ids.len(),
        "published_post_ids": published_ids.into_iter().map(|id| id.to_string()).collect::<Vec<_>>(),
        "tenant_id": options.tenant_id.map(|id| id.to_string()),
    })))
}
