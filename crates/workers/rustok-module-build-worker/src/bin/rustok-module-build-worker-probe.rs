use rustok_module_build_transport::GrpcModuleBuildWorker;
use rustok_worker_transport::MutualTlsClientConfig;
use tonic::transport::Endpoint;

/// Authenticated Kubernetes probe for the module build worker. A listening TCP
/// socket is insufficient: readiness must revalidate the worker's hardened OCI
/// launcher, runtime, image, and isolation-attestation boundary.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::var("RUSTOK_MODULE_BUILD_PROBE_ENDPOINT")?;
    let tls = MutualTlsClientConfig::from_env_prefix("RUSTOK_MODULE_BUILD_PROBE")?;
    let endpoint = Endpoint::from_shared(endpoint)?;
    let worker = GrpcModuleBuildWorker::connect_with_tls(endpoint, tls.tls_config()).await?;
    worker.check_readiness().await?;
    Ok(())
}
