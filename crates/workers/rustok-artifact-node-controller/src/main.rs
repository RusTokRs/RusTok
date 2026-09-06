use std::{sync::Arc, time::Duration};

use rustok_artifact_node_controller::ArtifactNodeControllerConfig;
use rustok_artifact_node_transport::ArtifactNodeGrpcService;
use rustok_modules::ModuleControlPlane;
use rustok_worker_transport::{MutualTlsListenerConfig, WorkerAdmission, shutdown_signal};
use sea_orm::{ConnectOptions, Database};

const LISTENER_PREFIX: &str = "RUSTOK_ARTIFACT_NODE_CONTROLLER";
const DATABASE_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ArtifactNodeControllerConfig::from_environment()?;
    let listener = MutualTlsListenerConfig::from_env_prefix(
        LISTENER_PREFIX,
        MutualTlsListenerConfig::STANDARD_MESSAGE_SIZE_CEILING,
    )?;
    let mut database_options = ConnectOptions::new(config.database_url);
    database_options.sqlx_logging(false);
    database_options.connect_timeout(DATABASE_CONNECT_TIMEOUT);
    let database = Database::connect(database_options).await?;
    let owner = Arc::new(ModuleControlPlane::new(database).artifact_node_agent());
    let admission = WorkerAdmission::from_listener(&listener)?;
    let service = ArtifactNodeGrpcService::new(owner, config.agent_authenticator, admission)
        .into_tonic_service();

    listener
        .server()?
        .concurrency_limit_per_connection(listener.concurrency_limit)
        .timeout(listener.request_timeout)
        .add_service(service)
        .serve_with_shutdown(listener.address, shutdown_signal())
        .await?;
    Ok(())
}
