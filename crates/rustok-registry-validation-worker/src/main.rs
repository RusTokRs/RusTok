use std::{
    env,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use rustok_build_publication::CommandRegistryCredentialBroker;
use rustok_modules::{ModuleControlPlane, ModulePlatformPublicationEvidenceProducer};
use rustok_registry_validation_worker::{
    CredentialedOciRegistryProvider, RegistryValidationPublicationPolicy, RegistryValidationWorker,
};
use rustok_storage::{StorageConfig, StorageRuntime};
use rustok_verification_transport::GrpcTrustVerifier;
use rustok_worker_transport::MutualTlsClientConfig;
use sea_orm::{ConnectOptions, Database};
use tonic::transport::Endpoint;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = required_env("RUSTOK_REGISTRY_VALIDATION_DATABASE_URL")?;
    let mut storage_config: StorageConfig = serde_json::from_str(&required_env(
        "RUSTOK_REGISTRY_VALIDATION_STORAGE_CONFIG_JSON",
    )?)?;
    required_env("RUSTOK_INSTANCE_ROOT")?;
    let instance_root = rustok_runtime::resolve_instance_root_from_environment()?;
    storage_config.bind_local_base_dir(instance_root.join("storage"));
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
    let storage = StorageRuntime::from_config(&storage_config).await?;
    let verification_endpoint = required_https_endpoint(
        "RUSTOK_REGISTRY_VALIDATION_VERIFICATION_ENDPOINT",
        required_env("RUSTOK_REGISTRY_VALIDATION_VERIFICATION_ENDPOINT")?,
    )?;
    let verification_tls =
        MutualTlsClientConfig::from_env_prefix("RUSTOK_REGISTRY_VALIDATION_VERIFICATION")?;
    let verifier = Arc::new(
        GrpcTrustVerifier::connect_with_tls(
            Endpoint::from_shared(verification_endpoint)?,
            verification_tls.tls_config(),
        )
        .await?,
    );
    verifier.check_readiness().await?;
    let credential_broker = Arc::new(CommandRegistryCredentialBroker::new(
        required_instance_path(
            &instance_root,
            "RUSTOK_REGISTRY_VALIDATION_REGISTRY_CREDENTIAL_BROKER",
        )?,
        required_env("RUSTOK_REGISTRY_VALIDATION_REGISTRY_CREDENTIAL_BROKER_DIGEST")?,
    )?);
    let registry_provider = Arc::new(CredentialedOciRegistryProvider::new(credential_broker)?);
    let owner = ModuleControlPlane::new(database).publication();
    let publication_evidence = Arc::new(ModulePlatformPublicationEvidenceProducer::new(
        Arc::new(owner.clone()),
        registry_provider,
        verifier,
    ));
    let publication_policy = RegistryValidationPublicationPolicy {
        registry_id: required_env("RUSTOK_REGISTRY_VALIDATION_REGISTRY_ID")?,
        trust_policy_revision: required_u64("RUSTOK_REGISTRY_VALIDATION_TRUST_POLICY_REVISION")?,
        capability_policy_revision: required_u64(
            "RUSTOK_REGISTRY_VALIDATION_CAPABILITY_POLICY_REVISION",
        )?,
        build_service_issuer_identity: required_env(
            "RUSTOK_REGISTRY_VALIDATION_BUILD_SERVICE_ISSUER_IDENTITY",
        )?,
        build_service_policy_revision: required_env(
            "RUSTOK_REGISTRY_VALIDATION_BUILD_SERVICE_POLICY_REVISION",
        )?,
    };
    let worker = RegistryValidationWorker::new(
        owner,
        storage,
        actor_id,
        publication_evidence,
        publication_policy,
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

fn required_instance_path(instance_root: &Path, name: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(required_env(name)?);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(format!(
            "{name} must be a path relative to RUSTOK_INSTANCE_ROOT"
        ));
    }
    Ok(instance_root.join(path))
}

fn optional_u64(name: &str, default: u64) -> Result<u64, String> {
    env::var(name).map_or(Ok(default), |value| {
        value
            .parse()
            .map_err(|error| format!("{name} is invalid: {error}"))
    })
}

fn required_u64(name: &str) -> Result<u64, String> {
    let value = required_env(name)?;
    let value = value
        .parse::<u64>()
        .map_err(|error| format!("{name} is invalid: {error}"))?;
    if value == 0 {
        return Err(format!("{name} must be positive"));
    }
    Ok(value)
}

fn required_https_endpoint(name: &str, endpoint: String) -> Result<String, String> {
    if endpoint.starts_with("https://") {
        Ok(endpoint)
    } else {
        Err(format!("{name} must use an https:// endpoint for mTLS"))
    }
}
