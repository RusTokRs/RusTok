use std::{env, sync::Arc, time::Duration};

use rustok_iggy::{ExternalConfig, IggyConfig, IggyMode, IggyTransport};
use rustok_module_build_transport::GrpcModuleBuildWorker;
use rustok_modules::ModuleControlPlane;
use rustok_worker_transport::MutualTlsClientConfig;
use sea_orm::{ConnectOptions, Database};
use tonic::transport::Endpoint;
use tracing::{error, info};

use crate::{IggyModuleBuildDeliverySource, ModuleBuildDeliveryConsumer};

/// Deployment-owned configuration for the module-build delivery host.
///
/// The host intentionally reads only the credentials and endpoints it needs:
/// one database connection, one external Iggy broker connection, and one mTLS
/// worker endpoint. It has no source/CAS/runtime-build configuration.
pub struct ModuleBuildDispatcherConfig {
    pub database_url: String,
    pub worker_endpoint: String,
    pub iggy: IggyConfig,
    pub idle_poll_delay: Duration,
}

impl ModuleBuildDispatcherConfig {
    const DEFAULT_IDLE_POLL_DELAY_MS: u64 = 1_000;
    const MAX_IDLE_POLL_DELAY_MS: u64 = 60_000;

    /// Loads the independent dispatcher process configuration.
    ///
    /// Required variables are `RUSTOK_MODULE_BUILD_DISPATCHER_DATABASE_URL`,
    /// `RUSTOK_MODULE_BUILD_DISPATCHER_WORKER_ENDPOINT`,
    /// `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_ADDRESSES`,
    /// `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_USERNAME`, and
    /// `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_PASSWORD`. Worker mTLS material is
    /// loaded separately from the `RUSTOK_MODULE_BUILD_*` client variables.
    pub fn from_env() -> Result<Self, String> {
        let database_url = required_env("RUSTOK_MODULE_BUILD_DISPATCHER_DATABASE_URL")?;
        let worker_endpoint = required_https_endpoint(
            "RUSTOK_MODULE_BUILD_DISPATCHER_WORKER_ENDPOINT",
            required_env("RUSTOK_MODULE_BUILD_DISPATCHER_WORKER_ENDPOINT")?,
        )?;
        let addresses = required_env("RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_ADDRESSES")?
            .split(',')
            .map(str::trim)
            .filter(|address| !address.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if addresses.is_empty() {
            return Err(
                "RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_ADDRESSES must contain at least one address"
                    .to_string(),
            );
        }
        let idle_poll_delay_ms = optional_u64(
            "RUSTOK_MODULE_BUILD_DISPATCHER_IDLE_POLL_DELAY_MS",
            Self::DEFAULT_IDLE_POLL_DELAY_MS,
        )?;
        if idle_poll_delay_ms == 0 || idle_poll_delay_ms > Self::MAX_IDLE_POLL_DELAY_MS {
            return Err(format!(
                "RUSTOK_MODULE_BUILD_DISPATCHER_IDLE_POLL_DELAY_MS must be between 1 and {}",
                Self::MAX_IDLE_POLL_DELAY_MS
            ));
        }

        Ok(Self {
            database_url,
            worker_endpoint,
            iggy: IggyConfig {
                mode: IggyMode::External,
                external: ExternalConfig {
                    addresses,
                    protocol: optional_env("RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_PROTOCOL", "tcp"),
                    username: required_env("RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_USERNAME")?,
                    password: required_env("RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_PASSWORD")?,
                    tls_enabled: required_true("RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_TLS_ENABLED")?,
                },
                ..IggyConfig::default()
            },
            idle_poll_delay: Duration::from_millis(idle_poll_delay_ms),
        })
    }
}

/// Runs the independent result-first delivery host until it receives a process
/// shutdown signal. A delivery-processing or broker-receive failure deliberately
/// terminates this process without committing its offset. The deployment
/// supervisor must restart it, allowing the persistent cursor to redeliver the
/// outstanding message instead of keeping an unacknowledgeable delivery in
/// process memory.
pub async fn run_dispatcher(config: ModuleBuildDispatcherConfig) -> Result<(), String> {
    let mut options = ConnectOptions::new(config.database_url);
    options.sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .map_err(|error| format!("module-build dispatcher database connection failed: {error}"))?;
    let transport =
        Arc::new(IggyTransport::new(config.iggy).await.map_err(|error| {
            format!("module-build dispatcher broker connection failed: {error}")
        })?);
    let source = IggyModuleBuildDeliverySource::open(Arc::clone(&transport)).await?;
    let tls = MutualTlsClientConfig::from_env_prefix("RUSTOK_MODULE_BUILD")?;
    let endpoint = Endpoint::from_shared(config.worker_endpoint.clone())
        .map_err(|error| format!("module-build worker endpoint is invalid: {error}"))?;
    let worker = GrpcModuleBuildWorker::connect_with_tls(endpoint, tls.tls_config()).await?;
    worker
        .check_readiness()
        .await
        .map_err(|error| format!("module-build worker is not ready: {error}"))?;
    let service = ModuleControlPlane::new(db).build();

    info!(
        worker_endpoint = %config.worker_endpoint,
        consumer_group = crate::MODULE_BUILD_CONSUMER_GROUP,
        "Module build dispatcher started"
    );

    loop {
        tokio::select! {
            shutdown = tokio::signal::ctrl_c() => {
                shutdown.map_err(|error| format!("module-build dispatcher shutdown listener failed: {error}"))?;
                info!("Module build dispatcher stopping after shutdown signal");
                transport.shutdown().await.map_err(|error| error.to_string())?;
                return Ok(());
            }
            received = source.receive() => {
                match received {
                    Ok(Some(delivery)) => {
                        let consumer = ModuleBuildDeliveryConsumer::new(&service, &worker);
                        if let Err(error) = consumer.process_and_acknowledge(&source, delivery).await {
                            error!(error = %error, "Module build delivery failed; broker offset remains uncommitted");
                            return Err(format!(
                                "module-build delivery failed before acknowledgement; restart the dispatcher to redeliver: {error}"
                            ));
                        }
                    }
                    Ok(None) => tokio::time::sleep(config.idle_poll_delay).await,
                    Err(error) => {
                        error!(error = %error, "Module build broker receive failed; terminating without acknowledgement");
                        return Err(format!(
                            "module-build broker receive failed; restart the dispatcher to recover its persistent cursor: {error}"
                        ));
                    }
                }
            }
        }
    }
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("{name} must be configured"))
}

fn optional_env(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

fn optional_u64(name: &str, default: u64) -> Result<u64, String> {
    env::var(name).map_or(Ok(default), |value| {
        value
            .parse()
            .map_err(|error| format!("{name} is invalid: {error}"))
    })
}

fn required_true(name: &str) -> Result<bool, String> {
    match required_env(name)?.parse::<bool>() {
        Ok(true) => Ok(true),
        Ok(false) => Err(format!("{name} must be true for the external broker")),
        Err(error) => Err(format!("{name} is invalid: {error}")),
    }
}

fn required_https_endpoint(name: &str, endpoint: String) -> Result<String, String> {
    if endpoint.starts_with("https://") {
        Ok(endpoint)
    } else {
        Err(format!("{name} must use an https:// endpoint for mTLS"))
    }
}
