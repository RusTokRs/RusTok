use std::{net::SocketAddr, sync::Arc};

use rustok_verification_transport::proto::verification_service_server::VerificationServiceServer;
use rustok_verification_worker::{
    CosignTrustVerifier, VerificationGrpcService, VerificationPolicy, VerificationWorker,
};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address: SocketAddr = std::env::var("RUSTOK_VERIFICATION_LISTEN_ADDR")?.parse()?;
    let policy: VerificationPolicy =
        serde_json::from_str(&std::env::var("RUSTOK_VERIFICATION_POLICY_JSON")?)?;
    if policy.allowed_signer_identities.is_empty() || policy.allowed_oidc_issuers.is_empty() {
        return Err("verification policy must configure signer identities and OIDC issuers".into());
    }
    let worker = Arc::new(VerificationWorker::new(
        CosignTrustVerifier::new(policy.clone()),
        policy,
    ));
    Server::builder()
        .add_service(VerificationServiceServer::new(
            VerificationGrpcService::new(worker),
        ))
        .serve(address)
        .await?;
    Ok(())
}
