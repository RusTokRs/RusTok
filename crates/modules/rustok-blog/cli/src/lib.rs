//! # rustok-blog-cli
//!
//! External operational command adapters for `rustok-blog`.
//!
//! This crate maps platform CLI requests to owner-owned Blog services. It
//! deliberately contains no domain policy, terminal output, or exit handling.

#[cfg(test)]
mod contract_tests;

use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_runtime::{RuntimeComposition, db_clone};
use uuid::Uuid;

const SCHEDULED_PUBLISH_DEFAULT_LIMIT: u32 = 100;
const SCHEDULED_PUBLISH_MAX_LIMIT: u32 = 200;

const CLEANUP_TOKENS_DEFAULT_LIMIT: u32 = 100;
const CLEANUP_TOKENS_MAX_LIMIT: u32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublishScheduledOptions {
    pub tenant_id: Option<Uuid>,
    pub limit: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CleanupTokensOptions {
    pub tenant_id: Option<Uuid>,
    pub limit: u32,
}

pub struct BlogCommandProvider {
    runtime: RuntimeComposition,
}

impl BlogCommandProvider {
    fn post_service(&self) -> CliCoreResult<rustok_blog::PostService> {
        let host = self
            .runtime
            .require_host()
            .map_err(|error| CliCoreError::CommandFailed {
                message: format!("blog CLI requires a database runtime: {error}"),
            })?;
        let db = db_clone(host);
        let event_bus = host
            .shared_get::<rustok_outbox::TransactionalEventBus>()
            .unwrap_or_else(|| {
                let transport = std::sync::Arc::new(rustok_outbox::OutboxTransport::new(db.clone()));
                rustok_outbox::TransactionalEventBus::new(transport)
            });
        Ok(rustok_blog::PostService::new(db, event_bus))
    }

    fn preview_token_service(&self) -> CliCoreResult<rustok_blog::PreviewTokenService> {
        let host = self
            .runtime
            .require_host()
            .map_err(|error| CliCoreError::CommandFailed {
                message: format!("blog CLI requires a database runtime: {error}"),
            })?;
        let db = db_clone(host);
        let event_bus = host
            .shared_get::<rustok_outbox::TransactionalEventBus>()
            .unwrap_or_else(|| {
                let transport = std::sync::Arc::new(rustok_outbox::OutboxTransport::new(db.clone()));
                rustok_outbox::TransactionalEventBus::new(transport)
            });
        Ok(rustok_blog::PreviewTokenService::new(db, event_bus))
    }

    async fn publish_scheduled(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        let options = parse_publish_scheduled_options(&request)?;
        let service = self.post_service()?;

        if request.dry_run {
            let due_count = match options.tenant_id {
                Some(tenant_id) => service
                    .count_due_scheduled_posts(tenant_id)
                    .await
                    .map_err(|error| CliCoreError::CommandFailed {
                        message: format!("failed to count due scheduled posts: {error}"),
                    })?,
                None => {
                    let tenants = service
                        .list_tenants_with_due_scheduled_posts()
                        .await
                        .map_err(|error| CliCoreError::CommandFailed {
                            message: format!("failed to discover tenants with due posts: {error}"),
                        })?;
                    let mut total = 0u64;
                    for tenant_id in tenants {
                        total += service
                            .count_due_scheduled_posts(tenant_id)
                            .await
                            .map_err(|error| CliCoreError::CommandFailed {
                                message: format!("failed to count due scheduled posts: {error}"),
                            })?;
                    }
                    total
                }
            };

            let message = match options.tenant_id {
                Some(tenant_id) => {
                    format!("DRY RUN: {due_count} scheduled posts due for publishing for tenant {tenant_id}")
                }
                None => format!("DRY RUN: {due_count} scheduled posts due for publishing across all tenants"),
            };

            return Ok(CommandOutcome::success(message).with_data(serde_json::json!({
                "dry_run": true,
                "due_count": due_count,
                "tenant_id": options.tenant_id.map(|id| id.to_string()),
            })));
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
                            message: format!("scheduled publish failed for tenant {tenant_id}: {error}"),
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

    async fn cleanup_tokens(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        let options = parse_cleanup_tokens_options(&request)?;
        let service = self.preview_token_service()?;

        if request.dry_run {
            let expired_count = match options.tenant_id {
                Some(tenant_id) => service
                    .count_expired(tenant_id)
                    .await
                    .map_err(|error| CliCoreError::CommandFailed {
                        message: format!("failed to count expired tokens: {error}"),
                    })?,
                None => {
                    let tenants = service
                        .list_tenants_with_expired_tokens()
                        .await
                        .map_err(|error| CliCoreError::CommandFailed {
                            message: format!("failed to list tenants with expired tokens: {error}"),
                        })?;
                    let mut total = 0u64;
                    for tenant_id in tenants {
                        total += service
                            .count_expired(tenant_id)
                            .await
                            .map_err(|error| CliCoreError::CommandFailed {
                                message: format!("failed to count expired tokens: {error}"),
                            })?;
                    }
                    total
                }
            };

            let message = match options.tenant_id {
                Some(tenant_id) => {
                    format!("DRY RUN: {expired_count} expired preview tokens found for tenant {tenant_id}")
                }
                None => format!("DRY RUN: {expired_count} expired preview tokens found across all tenants"),
            };

            return Ok(CommandOutcome::success(message).with_data(serde_json::json!({
                "dry_run": true,
                "expired_count": expired_count,
                "tenant_id": options.tenant_id.map(|id| id.to_string()),
            })));
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
                            message: format!("preview token cleanup failed for tenant {tenant_id}: {error}"),
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

        Ok(CommandOutcome::success(message).with_data(serde_json::json!({
            "dry_run": false,
            "deleted_count": deleted_count,
            "tenant_id": options.tenant_id.map(|id| id.to_string()),
        })))
    }
}

#[async_trait::async_trait]
impl CommandProvider for BlogCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![
            CommandDescriptor::new(
                "blog",
                "publish-scheduled",
                "Publish blog posts whose scheduled date has arrived",
            )
            .with_dry_run(),
            CommandDescriptor::new(
                "blog",
                "cleanup-tokens",
                "Remove expired blog preview tokens",
            )
            .with_dry_run(),
        ]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("blog", "publish-scheduled") => self.publish_scheduled(request).await,
            ("blog", "cleanup-tokens") => self.cleanup_tokens(request).await,
            _ => Err(CliCoreError::UnknownCommand {
                namespace: request.namespace,
                name: request.name,
            }),
        }
    }
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(BlogCommandProvider {
        runtime: runtime.clone(),
    })
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

pub fn parse_cleanup_tokens_options(request: &CommandRequest) -> CliCoreResult<CleanupTokensOptions> {
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

fn parse_optional_tenant_id(
    options: &serde_json::Map<String, serde_json::Value>,
) -> CliCoreResult<Option<Uuid>> {
    let value = options
        .get("tenant_id")
        .or_else(|| options.get("tenant-id"));
    let Some(val) = value else {
        return Ok(None);
    };

    let str_val = val.as_str().ok_or_else(|| CliCoreError::InvalidInput {
        message: "--tenant-id must be a UUID string".to_string(),
    })?;

    let trimmed = str_val.trim();
    if trimmed.is_empty() {
        return Err(CliCoreError::InvalidInput {
            message: "--tenant-id cannot be empty".to_string(),
        });
    }

    let parsed = Uuid::parse_str(trimmed).map_err(|_| CliCoreError::InvalidInput {
        message: format!("--tenant-id '{trimmed}' must be a valid UUID"),
    })?;

    if parsed.is_nil() {
        return Err(CliCoreError::InvalidInput {
            message: "--tenant-id cannot be nil UUID".to_string(),
        });
    }

    Ok(Some(parsed))
}

fn parse_limit(
    options: &serde_json::Map<String, serde_json::Value>,
    default_limit: u32,
    max_limit: u32,
) -> CliCoreResult<u32> {
    let Some(value) = options.get("limit") else {
        return Ok(default_limit);
    };

    let limit = match value {
        serde_json::Value::Number(n) => n.as_u64().ok_or_else(|| CliCoreError::InvalidInput {
            message: "--limit must be a positive integer".to_string(),
        })?,
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            trimmed
                .parse::<u64>()
                .map_err(|_| CliCoreError::InvalidInput {
                    message: format!("invalid integer value for --limit: '{trimmed}'"),
                })?
        }
        _ => {
            return Err(CliCoreError::InvalidInput {
                message: "--limit must be a positive integer".to_string(),
            });
        }
    };

    if limit == 0 {
        return Err(CliCoreError::InvalidInput {
            message: "--limit must be a positive integer".to_string(),
        });
    }

    if limit > u64::from(max_limit) {
        return Err(CliCoreError::InvalidInput {
            message: format!("--limit cannot exceed {max_limit}"),
        });
    }

    Ok(limit as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_runtime::RuntimeComposition;

    #[test]
    fn provider_describes_blog_commands() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let commands = provider.commands();

        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].namespace, "blog");
        assert_eq!(commands[0].name, "publish-scheduled");
        assert!(commands[0].supports_dry_run);

        assert_eq!(commands[1].namespace, "blog");
        assert_eq!(commands[1].name, "cleanup-tokens");
        assert!(commands[1].supports_dry_run);
    }

    #[test]
    fn parse_limit_handles_defaults_strings_and_numbers() {
        let empty = serde_json::Map::new();
        assert_eq!(parse_limit(&empty, 100, 200).unwrap(), 100);

        let mut string_opt = serde_json::Map::new();
        string_opt.insert("limit".to_string(), serde_json::json!("25"));
        assert_eq!(parse_limit(&string_opt, 100, 200).unwrap(), 25);

        let mut ws_opt = serde_json::Map::new();
        ws_opt.insert("limit".to_string(), serde_json::json!("  50  \n"));
        assert_eq!(parse_limit(&ws_opt, 100, 200).unwrap(), 50);

        let mut num_opt = serde_json::Map::new();
        num_opt.insert("limit".to_string(), serde_json::json!(150));
        assert_eq!(parse_limit(&num_opt, 100, 200).unwrap(), 150);
    }

    #[test]
    fn parse_limit_rejects_invalid_values() {
        let mut zero_str = serde_json::Map::new();
        zero_str.insert("limit".to_string(), serde_json::json!("0"));
        assert!(parse_limit(&zero_str, 100, 200).is_err());

        let mut zero_num = serde_json::Map::new();
        zero_num.insert("limit".to_string(), serde_json::json!(0));
        assert!(parse_limit(&zero_num, 100, 200).is_err());

        let mut neg_num = serde_json::Map::new();
        neg_num.insert("limit".to_string(), serde_json::json!(-5));
        assert!(parse_limit(&neg_num, 100, 200).is_err());

        let mut non_num = serde_json::Map::new();
        non_num.insert("limit".to_string(), serde_json::json!("not_a_number"));
        assert!(parse_limit(&non_num, 100, 200).is_err());

        let mut bool_val = serde_json::Map::new();
        bool_val.insert("limit".to_string(), serde_json::json!(true));
        assert!(parse_limit(&bool_val, 100, 200).is_err());

        let mut exceeded = serde_json::Map::new();
        exceeded.insert("limit".to_string(), serde_json::json!(500));
        let err = parse_limit(&exceeded, 100, 200).unwrap_err();
        assert!(err.to_string().contains("cannot exceed 200"));
    }

    #[test]
    fn parse_optional_tenant_id_parses_and_validates() {
        let empty = serde_json::Map::new();
        assert_eq!(parse_optional_tenant_id(&empty).unwrap(), None);

        let valid_uuid = "11111111-2222-3333-4444-555555555555";
        let mut snake_opt = serde_json::Map::new();
        snake_opt.insert("tenant_id".to_string(), serde_json::json!(valid_uuid));
        assert_eq!(
            parse_optional_tenant_id(&snake_opt).unwrap(),
            Some(Uuid::parse_str(valid_uuid).unwrap())
        );

        let mut kebab_opt = serde_json::Map::new();
        kebab_opt.insert(
            "tenant-id".to_string(),
            serde_json::json!(format!("  {valid_uuid}  ")),
        );
        assert_eq!(
            parse_optional_tenant_id(&kebab_opt).unwrap(),
            Some(Uuid::parse_str(valid_uuid).unwrap())
        );

        let mut empty_str = serde_json::Map::new();
        empty_str.insert("tenant_id".to_string(), serde_json::json!("   "));
        assert!(parse_optional_tenant_id(&empty_str).is_err());

        let mut invalid_str = serde_json::Map::new();
        invalid_str.insert("tenant_id".to_string(), serde_json::json!("not-a-uuid"));
        assert!(parse_optional_tenant_id(&invalid_str).is_err());

        let mut nil_str = serde_json::Map::new();
        nil_str.insert(
            "tenant_id".to_string(),
            serde_json::json!(Uuid::nil().to_string()),
        );
        assert!(parse_optional_tenant_id(&nil_str).is_err());
    }

    #[test]
    fn request_options_extraction() {
        let request = CommandRequest {
            namespace: "blog".to_string(),
            name: "publish-scheduled".to_string(),
            args: serde_json::json!({
                "options": {
                    "tenant_id": "11111111-2222-3333-4444-555555555555",
                    "limit": "42"
                }
            }),
            dry_run: true,
        };

        let opts = parse_publish_scheduled_options(&request).unwrap();
        assert_eq!(
            opts.tenant_id,
            Some(Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap())
        );
        assert_eq!(opts.limit, 42);
    }
}
