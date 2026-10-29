//! # rustok-media-cli
//!
//! External operational command adapters for `rustok-media`.
//!
//! This crate maps platform CLI requests to owner-owned media services. It
//! deliberately contains no domain policy, terminal output, or exit handling.

use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_runtime::{RuntimeComposition, db_clone};
use rustok_storage::{StorageConfig, StorageRuntime};
use uuid::Uuid;

const RECONCILIATION_DEFAULT_LIMIT: u64 = 1_000;
const RECONCILIATION_MAX_LIMIT: u64 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconcileOptions {
    pub tenant_id: Option<Uuid>,
    pub limit: u64,
}

pub struct MediaCommandProvider {
    runtime: RuntimeComposition,
}

impl MediaCommandProvider {
    fn storage_config(&self) -> CliCoreResult<StorageConfig> {
        let mut config: StorageConfig = self
            .runtime
            .settings()
            .get("storage")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| CliCoreError::InvalidInput {
                message: format!("invalid storage settings for media CLI: {error}"),
            })
            .map(|config| config.unwrap_or_default())?;
        let storage_root =
            self.runtime
                .instance_path("storage")
                .map_err(|error| CliCoreError::InvalidInput {
                    message: error.to_string(),
                })?;
        config.bind_local_base_dir(storage_root);
        Ok(config)
    }

    async fn reconcile(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        let options = parse_reconcile_options(&request)?;
        let host = self
            .runtime
            .require_host()
            .map_err(|error| CliCoreError::CommandFailed {
                message: format!("media reconciliation requires a database runtime: {error}"),
            })?;
        let storage = StorageRuntime::from_config(&self.storage_config()?)
            .await
            .map_err(|error| CliCoreError::CommandFailed {
                message: format!("failed to initialize media storage: {error}"),
            })?;
        let service = rustok_media::MediaService::new(db_clone(host), storage);
        let report = match options.tenant_id {
            Some(tenant_id) => service.reconcile_storage(tenant_id, options.limit).await,
            None => service.reconcile_storage_all_tenants(options.limit).await,
        }
        .map_err(|error| CliCoreError::CommandFailed {
            message: format!("media reconciliation failed: {error}"),
        })?;

        let message = match options.tenant_id {
            Some(tenant_id) => format!("Media reconciliation complete for tenant {tenant_id}"),
            None => "Media reconciliation complete across all tenants".to_string(),
        };

        Ok(
            CommandOutcome::success(message).with_data(serde_json::json!({
                "tenant_id": options.tenant_id.map(|id| id.to_string()),
                "inspected": report.inspected,
                "healthy": report.healthy,
                "missing_marked": report.missing_marked,
                "deletions_completed": report.deletions_completed,
                "retry_later": report.retry_later,
            })),
        )
    }
}

#[async_trait::async_trait]
impl CommandProvider for MediaCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor::new(
            "media",
            "reconcile",
            "Reconcile Media-owned blob lifecycle with object storage",
        )]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("media", "reconcile") => self.reconcile(request).await,
            _ => Err(CliCoreError::UnknownCommand {
                namespace: request.namespace,
                name: request.name,
            }),
        }
    }
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(MediaCommandProvider {
        runtime: runtime.clone(),
    })
}

pub fn parse_reconcile_options(request: &CommandRequest) -> CliCoreResult<ReconcileOptions> {
    if request.dry_run {
        return Err(CliCoreError::DryRunNotSupported {
            namespace: request.namespace.clone(),
            name: request.name.clone(),
        });
    }

    let Some(options) = request
        .args
        .get("options")
        .and_then(serde_json::Value::as_object)
    else {
        return Err(CliCoreError::InvalidInput {
            message: "media reconcile expects normalized command options".to_string(),
        });
    };

    let tenant_id = parse_optional_tenant_id(options)?;
    let limit = parse_reconcile_limit(options)?;

    Ok(ReconcileOptions { tenant_id, limit })
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

fn parse_reconcile_limit(
    options: &serde_json::Map<String, serde_json::Value>,
) -> CliCoreResult<u64> {
    let Some(value) = options.get("limit") else {
        return Ok(RECONCILIATION_DEFAULT_LIMIT);
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
                    message: "--limit must be a positive integer".to_string(),
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

    if limit > RECONCILIATION_MAX_LIMIT {
        return Err(CliCoreError::InvalidInput {
            message: format!("--limit cannot exceed {RECONCILIATION_MAX_LIMIT}"),
        });
    }

    Ok(limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_cli_core::{CliCoreError, CommandRequest};
    use rustok_runtime::RuntimeComposition;
    use uuid::Uuid;

    #[test]
    fn provider_describes_media_reconciliation() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let commands = provider.commands();

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].namespace, "media");
        assert_eq!(commands[0].name, "reconcile");
        assert!(!commands[0].supports_dry_run);
    }

    #[test]
    fn parse_reconcile_limit_handles_defaults_strings_and_numbers() {
        let empty = serde_json::Map::new();
        assert_eq!(
            parse_reconcile_limit(&empty).unwrap(),
            RECONCILIATION_DEFAULT_LIMIT
        );

        let mut string_opt = serde_json::Map::new();
        string_opt.insert("limit".to_string(), serde_json::json!("25"));
        assert_eq!(parse_reconcile_limit(&string_opt).unwrap(), 25);

        let mut ws_opt = serde_json::Map::new();
        ws_opt.insert("limit".to_string(), serde_json::json!("  50  \n"));
        assert_eq!(parse_reconcile_limit(&ws_opt).unwrap(), 50);

        let mut num_opt = serde_json::Map::new();
        num_opt.insert("limit".to_string(), serde_json::json!(100));
        assert_eq!(parse_reconcile_limit(&num_opt).unwrap(), 100);
    }

    #[test]
    fn parse_reconcile_limit_rejects_invalid_values() {
        let mut zero_str = serde_json::Map::new();
        zero_str.insert("limit".to_string(), serde_json::json!("0"));
        assert!(parse_reconcile_limit(&zero_str).is_err());

        let mut zero_num = serde_json::Map::new();
        zero_num.insert("limit".to_string(), serde_json::json!(0));
        assert!(parse_reconcile_limit(&zero_num).is_err());

        let mut neg_num = serde_json::Map::new();
        neg_num.insert("limit".to_string(), serde_json::json!(-5));
        assert!(parse_reconcile_limit(&neg_num).is_err());

        let mut non_num = serde_json::Map::new();
        non_num.insert("limit".to_string(), serde_json::json!("invalid"));
        assert!(parse_reconcile_limit(&non_num).is_err());

        let mut bool_val = serde_json::Map::new();
        bool_val.insert("limit".to_string(), serde_json::json!(true));
        assert!(parse_reconcile_limit(&bool_val).is_err());

        let mut exceeded = serde_json::Map::new();
        exceeded.insert("limit".to_string(), serde_json::json!(2_000));
        let err = parse_reconcile_limit(&exceeded).unwrap_err();
        assert!(err.to_string().contains("cannot exceed 1000"));
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
            serde_json::json!("  11111111-2222-3333-4444-555555555555  "),
        );
        assert_eq!(
            parse_optional_tenant_id(&kebab_opt).unwrap(),
            Some(Uuid::parse_str(valid_uuid).unwrap())
        );

        let mut empty_opt = serde_json::Map::new();
        empty_opt.insert("tenant-id".to_string(), serde_json::json!("   "));
        assert!(parse_optional_tenant_id(&empty_opt).is_err());

        let mut invalid_opt = serde_json::Map::new();
        invalid_opt.insert("tenant-id".to_string(), serde_json::json!("not-a-uuid"));
        assert!(parse_optional_tenant_id(&invalid_opt).is_err());

        let mut nil_opt = serde_json::Map::new();
        nil_opt.insert(
            "tenant-id".to_string(),
            serde_json::json!("00000000-0000-0000-0000-000000000000"),
        );
        let err = parse_optional_tenant_id(&nil_opt).unwrap_err();
        assert!(err.to_string().contains("cannot be nil UUID"));

        let mut num_opt = serde_json::Map::new();
        num_opt.insert("tenant-id".to_string(), serde_json::json!(12345));
        assert!(parse_optional_tenant_id(&num_opt).is_err());
    }

    #[test]
    fn parse_reconcile_options_rejects_dry_run() {
        let request = CommandRequest {
            namespace: "media".to_string(),
            name: "reconcile".to_string(),
            args: serde_json::json!({ "options": {} }),
            dry_run: true,
        };
        let err = parse_reconcile_options(&request).unwrap_err();
        match err {
            CliCoreError::DryRunNotSupported { namespace, name } => {
                assert_eq!(namespace, "media");
                assert_eq!(name, "reconcile");
            }
            other => panic!("expected DryRunNotSupported, got {other:?}"),
        }
    }

    #[test]
    fn parse_reconcile_options_requires_options_object() {
        let request = CommandRequest {
            namespace: "media".to_string(),
            name: "reconcile".to_string(),
            args: serde_json::json!({}),
            dry_run: false,
        };
        assert!(parse_reconcile_options(&request).is_err());
    }

    #[tokio::test]
    async fn reconciliation_requires_a_database_runtime() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let error = provider
            .execute(CommandRequest {
                namespace: "media".to_string(),
                name: "reconcile".to_string(),
                args: serde_json::json!({ "options": {} }),
                dry_run: false,
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("requires a database runtime"));
    }

    #[tokio::test]
    async fn unknown_command_returns_unknown_command_error() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let error = provider
            .execute(CommandRequest {
                namespace: "media".to_string(),
                name: "optimize".to_string(),
                args: serde_json::json!({ "options": {} }),
                dry_run: false,
            })
            .await
            .unwrap_err();

        match error {
            CliCoreError::UnknownCommand { namespace, name } => {
                assert_eq!(namespace, "media");
                assert_eq!(name, "optimize");
            }
            other => panic!("expected UnknownCommand, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod contract_tests;
