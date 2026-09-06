use std::sync::Arc;

use rustok_artifact_node_agent::{
    ArtifactNodeAgent, ArtifactNodeAgentConfig, StorageArtifactNodeMaterializer,
};
use rustok_artifact_node_transport::GrpcArtifactNodeAgent;
use rustok_modules::StorageArtifactBlobStore;
use rustok_runtime::resolve_instance_layout_from_environment;
use rustok_sandbox_transport::GrpcRhaiExecutor;
use rustok_storage::StorageRuntime;
use rustok_worker_transport::MutualTlsClientConfig;

const CONTROLLER_TLS_PREFIX: &str = "RUSTOK_ARTIFACT_NODE_AGENT_CONTROLLER";
const SANDBOX_TLS_PREFIX: &str = "RUSTOK_ARTIFACT_NODE_AGENT_SANDBOX";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ArtifactNodeAgentConfig::from_environment()?;
    let layout = resolve_instance_layout_from_environment()?;
    let mut storage_config = config.storage.clone();
    storage_config.bind_local_base_dir(layout.storage());
    let storage = StorageRuntime::from_config(&storage_config).await?;
    let controller_tls = MutualTlsClientConfig::from_env_prefix(CONTROLLER_TLS_PREFIX)?;
    let controller = Arc::new(
        GrpcArtifactNodeAgent::connect_with_tls(
            config.controller_endpoint,
            controller_tls.tls_config(),
        )
        .await?,
    );
    let sandbox_tls = MutualTlsClientConfig::from_env_prefix(SANDBOX_TLS_PREFIX)?;
    let rhai_worker =
        GrpcRhaiExecutor::connect_with_tls(config.sandbox_endpoint, sandbox_tls.tls_config())
            .await?;
    let materializer = Arc::new(StorageArtifactNodeMaterializer::new(
        StorageArtifactBlobStore::new(storage),
        layout,
        rhai_worker,
    ));
    let agent = ArtifactNodeAgent::new(
        controller,
        materializer,
        config.identity,
        config.heartbeat_interval,
    )?;
    agent.run_until_shutdown(config.poll_interval).await;
    Ok(())
}
