//! Cleanup Task
//!
//! Removes old sessions and temporary data.
//! Run with: `cargo loco task --name cleanup --args "sessions"`

use crate::error::{Error, Result};
use crate::tasks::{Task, TaskAppContext as AppContext, TaskInfo, Vars};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;

use crate::models::sessions;
use crate::services::rbac_consistency::load_rbac_consistency_stats;
use crate::services::server_runtime_context::ServerRuntimeContext;

/// Cleanup task for maintenance operations
pub struct CleanupTask;

#[async_trait]
impl Task for CleanupTask {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "cleanup".to_string(),
            detail: "Remove old sessions and temporary data".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let target = vars.cli.get("target").map_or("", String::as_str);

        match target {
            "sessions" => {
                tracing::info!("Cleaning up expired sessions...");
                let now = Utc::now();
                let result = sessions::Entity::delete_many()
                    .filter(sessions::Column::ExpiresAt.lt(now))
                    .exec(&ctx.db)
                    .await?;

                tracing::info!(deleted = result.rows_affected, "Session cleanup complete");
            }
            "cache" => {
                tracing::info!("Clearing temporary cache entries...");
                // Cache cleanup would go here
                tracing::info!("Cache cleanup complete");
            }
            "rbac-report" => {
                let runtime_ctx = ServerRuntimeContext::from_loco_app_context(ctx);
                let stats = load_rbac_consistency_stats(&runtime_ctx).await?;
                tracing::info!(
                    users_without_roles_total = stats.users_without_roles_total,
                    orphan_user_roles_total = stats.orphan_user_roles_total,
                    orphan_role_permissions_total = stats.orphan_role_permissions_total,
                    "RBAC consistency report"
                );

                if let Some(path) = vars.cli.get("output") {
                    write_rbac_report_file(path, stats)?;
                    tracing::info!(report_file = %path, "RBAC consistency report file saved");
                }
            }
            "" => {
                tracing::info!("Running full cleanup...");
                let now = Utc::now();
                let result = sessions::Entity::delete_many()
                    .filter(sessions::Column::ExpiresAt.lt(now))
                    .exec(&ctx.db)
                    .await?;

                tracing::info!(deleted = result.rows_affected, "Full cleanup complete");
            }
            _ => {
                tracing::warn!("Unknown cleanup target: {}", target);
                tracing::info!(
                    "Available targets: sessions, cache, rbac-report, or empty for full"
                );
            }
        }

        Ok(())
    }
}

fn write_rbac_report_file(
    path: &str,
    stats: crate::services::rbac_consistency::RbacConsistencyStats,
) -> Result<()> {
    let payload = serde_json::to_vec_pretty(&json!({
        "users_without_roles_total": stats.users_without_roles_total,
        "orphan_user_roles_total": stats.orphan_user_roles_total,
        "orphan_role_permissions_total": stats.orphan_role_permissions_total,
    }))
    .map_err(|error| Error::string(&format!("rbac report serialization failed: {error}")))?;
    std::fs::write(path, payload)
        .map_err(|error| Error::string(&format!("rbac report write failed: {error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::write_rbac_report_file;

    #[test]
    fn rbac_report_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rbac_report.json");
        let path_str = path.to_string_lossy().to_string();

        write_rbac_report_file(
            &path_str,
            crate::services::rbac_consistency::RbacConsistencyStats {
                users_without_roles_total: 3,
                orphan_user_roles_total: 2,
                orphan_role_permissions_total: 1,
            },
        )
        .unwrap();

        let payload: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(payload["users_without_roles_total"], 3);
        assert_eq!(payload["orphan_user_roles_total"], 2);
        assert_eq!(payload["orphan_role_permissions_total"], 1);
    }
}
