use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Error, Result,
};
use uuid::Uuid;

use crate::services::build_executor::BuildExecutionService;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub struct RebuildTask;

#[async_trait]
impl Task for RebuildTask {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "rebuild".to_string(),
            detail: "Execute the queued manifest-derived build plan for rustok-server".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let dry_run = is_flag_enabled(vars, "dry_run");
        let runtime_ctx = ServerRuntimeContext::from_loco_app_context(ctx);
        let executor = BuildExecutionService::new(&runtime_ctx);

        let report = if let Some(build_id) = vars.cli.get("build_id") {
            executor
                .execute_build(parse_build_id(build_id)?, dry_run)
                .await
                .map_err(|error| Error::Message(format!("Build execution failed: {error}")))?
        } else {
            match executor
                .execute_next_queued_build(dry_run)
                .await
                .map_err(|error| Error::Message(format!("Build execution failed: {error}")))?
            {
                Some(report) => report,
                None => {
                    tracing::info!("No queued builds available");
                    return Ok(());
                }
            }
        };

        let payload = serde_json::to_string_pretty(&report).map_err(|error| {
            Error::Message(format!("Failed to serialize rebuild report: {error}"))
        })?;
        println!("{payload}");
        Ok(())
    }
}

fn parse_build_id(raw: &str) -> Result<Uuid> {
    Uuid::parse_str(raw)
        .map_err(|error| Error::Message(format!("Invalid build_id '{raw}': {error}")))
}

fn is_flag_enabled(vars: &Vars, key: &str) -> bool {
    vars.cli
        .get(key)
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}
