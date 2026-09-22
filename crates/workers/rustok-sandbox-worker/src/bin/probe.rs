//! Exact mTLS readiness probe for the isolated Rhai worker.

use std::time::Duration;

use rustok_sandbox_transport::GrpcRhaiExecutor;
use rustok_worker_transport::MutualTlsClientConfig;

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::var("RUSTOK_SANDBOX_PROBE_ENDPOINT")
        .map_err(|_| "RUSTOK_SANDBOX_PROBE_ENDPOINT must be configured")?;
    let endpoint = tonic::transport::Endpoint::from_shared(endpoint)?
        .connect_timeout(PROBE_TIMEOUT)
        .timeout(PROBE_TIMEOUT);
    let tls = MutualTlsClientConfig::from_env_prefix("RUSTOK_SANDBOX_PROBE")?;
    let executor = tokio::time::timeout(
        PROBE_TIMEOUT,
        GrpcRhaiExecutor::connect_with_tls(endpoint, tls.tls_config()),
    )
    .await
    .map_err(|_| "sandbox worker probe connection timed out")??;
    tokio::time::timeout(PROBE_TIMEOUT, executor.check_readiness())
        .await
        .map_err(|_| "sandbox worker readiness probe timed out")??;
    Ok(())
}
