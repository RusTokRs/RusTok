//! External operational command adapters for `rustok-rbac`.

use std::fs;

use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_rbac::{
    load_consistency_stats, repair_system_roles, RbacSystemRoleRepairOptions,
};
use rustok_runtime::{db_clone, RuntimeComposition};
use uuid::Uuid;

pub struct RbacCommandProvider {
    runtime: RuntimeComposition,
}

#[async_trait::async_trait]
impl CommandProvider for RbacCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![
            CommandDescriptor::new(
                "rbac",
                "consistency-report",
                "Report structural and semantic RBAC persistence corruption",
            ),
            CommandDescriptor::new(
                "rbac",
                "repair-system-roles",
                "Plan or apply canonical built-in role permission repair",
            ),
        ]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("rbac", "consistency-report") => self.consistency_report(request.args).await,
            ("rbac", "repair-system-roles") => self.repair_system_roles(request.args).await,
            _ => Err(CliCoreError::UnknownCommand {
                namespace: request.namespace,
                name: request.name,
            }),
        }
    }
}

impl RbacCommandProvider {
    async fn consistency_report(&self, args: serde_json::Value) -> CliCoreResult<CommandOutcome> {
        let db = db_clone(self.runtime.require_host().map_err(command_failed)?);
        let stats = load_consistency_stats(&db).await.map_err(command_failed)?;
        let report = serde_json::json!({
            "users_without_roles_total": stats.users_without_roles_total,
            "orphan_user_roles_total": stats.orphan_user_roles_total,
            "orphan_role_permissions_total": stats.orphan_role_permissions_total,
            "cross_tenant_user_roles_total": stats.cross_tenant_user_roles_total,
            "cross_tenant_role_permissions_total": stats.cross_tenant_role_permissions_total,
            "reserved_role_slug_collisions_total": stats.reserved_role_slug_collisions_total,
            "system_roles_with_permission_drift_total": stats.system_roles_with_permission_drift_total,
            "missing_system_role_permissions_total": stats.missing_system_role_permissions_total,
            "extra_system_role_permissions_total": stats.extra_system_role_permissions_total,
        });
        write_output_if_requested(&args, &report)?;
        Ok(CommandOutcome::success("RBAC consistency report collected").with_data(report))
    }

    async fn repair_system_roles(&self, args: serde_json::Value) -> CliCoreResult<CommandOutcome> {
        let options = command_options(&args);
        let tenant_id = option_string(options, "tenant_id")
            .map(|raw| {
                Uuid::parse_str(raw).map_err(|_| CliCoreError::InvalidInput {
                    message: "--tenant-id must be a UUID".to_string(),
                })
            })
            .transpose()?;
        let apply = option_bool(options, "apply")?;
        let all_tenants = option_bool(options, "all_tenants")?;
        if tenant_id.is_some() && all_tenants {
            return Err(CliCoreError::InvalidInput {
                message: "--tenant-id and --all-tenants are mutually exclusive".to_string(),
            });
        }
        if apply && tenant_id.is_none() && !all_tenants {
            return Err(CliCoreError::InvalidInput {
                message: "--apply requires --tenant-id or explicit --all-tenants".to_string(),
            });
        }
        let db = db_clone(self.runtime.require_host().map_err(command_failed)?);
        let report = repair_system_roles(&db, RbacSystemRoleRepairOptions { tenant_id, apply })
            .await
            .map_err(command_failed)?;
        let changes_total = report.changes_total();
        let mut data = serde_json::to_value(&report).map_err(command_failed)?;
        let Some(object) = data.as_object_mut() else {
            return Err(command_failed("RBAC repair report did not serialize as an object"));
        };
        object.insert("changes_total".to_string(), changes_total.into());
        object.insert(
            "mode".to_string(),
            if apply { "apply" } else { "dry_run" }.into(),
        );
        object.insert(
            "runtime_restart_required_if_applied".to_string(),
            (!report.affected_users.is_empty()).into(),
        );
        object.insert(
            "scope".to_string(),
            if let Some(tenant_id) = tenant_id {
                serde_json::json!({ "tenant_id": tenant_id })
            } else {
                serde_json::json!({ "all_tenants": true })
            },
        );
        write_output_if_requested(&args, &data)?;

        let message = if apply {
            "RBAC system role repair applied"
        } else {
            "RBAC system role repair plan collected"
        };
        Ok(CommandOutcome::success(message).with_data(data))
    }
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(RbacCommandProvider {
        runtime: runtime.clone(),
    })
}

fn command_options(args: &serde_json::Value) -> Option<&serde_json::Map<String, serde_json::Value>> {
    args.get("options").and_then(serde_json::Value::as_object)
}

fn option_string<'a>(
    options: Option<&'a serde_json::Map<String, serde_json::Value>>,
    key: &str,
) -> Option<&'a str> {
    options?
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn option_bool(
    options: Option<&serde_json::Map<String, serde_json::Value>>,
    key: &str,
) -> CliCoreResult<bool> {
    let Some(value) = options.and_then(|options| options.get(key)) else {
        return Ok(false);
    };
    if let Some(value) = value.as_bool() {
        return Ok(value);
    }
    if let Some(value) = value.as_str() {
        return match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(true),
            "false" | "0" | "no" | "off" | "" => Ok(false),
            _ => Err(CliCoreError::InvalidInput {
                message: format!("--{} must be a boolean", key.replace('_', "-")),
            }),
        };
    }
    Err(CliCoreError::InvalidInput {
        message: format!("--{} must be a boolean", key.replace('_', "-")),
    })
}

fn write_output_if_requested(
    args: &serde_json::Value,
    value: &serde_json::Value,
) -> CliCoreResult<()> {
    if let Some(path) = command_options(args).and_then(|options| option_string(Some(options), "output"))
    {
        fs::write(
            path,
            serde_json::to_vec_pretty(value).map_err(command_failed)?,
        )
        .map_err(command_failed)?;
    }
    Ok(())
}

fn command_failed(error: impl std::fmt::Display) -> CliCoreError {
    CliCoreError::CommandFailed {
        message: error.to_string(),
    }
}
