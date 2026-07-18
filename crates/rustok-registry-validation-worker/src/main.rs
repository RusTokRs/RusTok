use std::{env, time::Duration};

use rustok_modules::ModuleControlPlane;
use rustok_registry_validation_worker::RegistryValidationWorker;
use rustok_storage::{StorageConfig, StorageService};
use sea_orm::{ConnectOptions, Database};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = required_env("RUSTOK_REGISTRY_VALIDATION_DATABASE_URL")?;
    let storage_config: StorageConfig = serde_json::from_str(&required_env(
        "RUSTOK_REGISTRY_VALIDATION_STORAGE_CONFIG_JSON",
    )?)?;
    let actor_id = required_env("RUSTOK_REGISTRY_VALIDATION_WORKER_ID")?;
    let poll_delay = Duration::from_millis(optional_u64(
        "RUSTOK_REGISTRY_VALIDATION_POLL_DELAY_MS",
        1_000,
    )?);
    if poll_delay.is_zero() {
        return Err("RUSTOK_REGISTRY_VALIDATION_POLL_DELAY_MS must be positive".into());
    }
    let mut options = ConnectOptions::new(database_url);
    options.sqlx_logging(false);
    let database = Database::connect(options).await?;
    let storage = StorageService::from_config(&storage_config).await?;
    let worker = RegistryValidationWorker::new(
        ModuleControlPlane::new(database).publication(),
        storage,
        actor_id,
    )?;
    loop {
        tokio::select! {
            shutdown = tokio::signal::ctrl_c() => {
                shutdown?;
                return Ok(());
            }
            result = worker.process_next() => match result {
                Ok(Some(job_id)) => tracing::info!(validation_job_id = %job_id, "Registry validation job completed"),
                Ok(None) => tokio::time::sleep(poll_delay).await,
                Err(error) => {
                    tracing::error!(error = %error, "Registry validation worker iteration failed");
                    tokio::time::sleep(poll_delay).await;
                }
            }
        }
    }
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("{name} must be configured"))
}

fn optional_u64(name: &str, default: u64) -> Result<u64, String> {
    env::var(name).map_or(Ok(default), |value| {
        value
            .parse()
            .map_err(|error| format!("{name} is invalid: {error}"))
    })
}
