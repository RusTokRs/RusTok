//! External operational command adapters for `rustok-rbac`.

use std::fs;

use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_rbac::load_consistency_stats;
use rustok_runtime::{db_clone, RuntimeComposition};

pub struct RbacCommandProvider {
    runtime: RuntimeComposition,
}

#[async_trait::async_trait]
impl CommandProvider for RbacCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor::new(
            "rbac",
            "consistency-report",
            "Report structural and semantic RBAC persistence corruption",
        )]
    }
    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("rbac", "consistency-report") => self.consistency_report(request.args).await,
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
        if let Some(path) = args
            .get("options")
            .and_then(serde_json::Value::as_object)
            .and_then(|options| options.get("output"))
            .and_then(serde_json::Value::as_str)
        {
            fs::write(
                path,
                serde_json::to_vec_pretty(&report).map_err(command_failed)?,
            )
            .map_err(command_failed)?;
        }
        Ok(CommandOutcome::success("RBAC consistency report collected").with_data(report))
    }
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(RbacCommandProvider {
        runtime: runtime.clone(),
    })
}
fn command_failed(error: impl std::fmt::Display) -> CliCoreError {
    CliCoreError::CommandFailed {
        message: error.to_string(),
    }
}
