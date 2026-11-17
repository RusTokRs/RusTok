use std::{env, path::Path, sync::Arc, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use rustok_iggy::{IggyConfig, IggyMode, IggyTransport};
use rustok_outbox::{OutboxRelay, RelayConfig, SysEvents};
use rustok_telemetry::{LogFormat, TelemetryConfig};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, EntityTrait};
use url::Url;

const DATABASE_ENV: &str = "RUSTOK_OUTBOX_DATABASE_URL";
const IGGY_CONFIG_ENV: &str = "RUSTOK_OUTBOX_IGGY_CONFIG_PATH";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

#[tokio::main]
async fn main() -> Result<()> {
    rustok_telemetry::init(TelemetryConfig {
        service_name: "rustok-outbox-service".to_owned(),
        log_format: LogFormat::Json,
        metrics: false,
        otel: None,
    })?;

    let database = open_database(&env::var(DATABASE_ENV).context(DATABASE_ENV)?).await?;
    let config_path = env::var_os(IGGY_CONFIG_ENV).context(IGGY_CONFIG_ENV)?;
    let config = read_broker_config(Path::new(&config_path)).await?;
    let broker = Arc::new(
        tokio::time::timeout(CONNECT_TIMEOUT, IggyTransport::new(config))
            .await
            .map_err(|_| anyhow!("Outbox Iggy connection timed out"))?
            .map_err(|_| anyhow!("Outbox could not initialize the configured Iggy transport"))?,
    );
    let config = RelayConfig::default();
    let relay = OutboxRelay::new(database, broker.clone()).with_config(RelayConfig {
        // Keep each claimed row eligible for immediate dispatch by a worker.
        batch_size: config.max_concurrency as u64,
        ..config
    });
    tracing::info!("Outbox relay started");
    let result = run_until_shutdown(&relay).await;
    let shutdown = tokio::time::timeout(CONNECT_TIMEOUT, broker.shutdown())
        .await
        .map_err(|_| anyhow!("Outbox Iggy shutdown timed out"))
        .and_then(|result| result.context("Outbox Iggy shutdown failed"));
    result.and(shutdown)
}

async fn open_database(value: &str) -> Result<DatabaseConnection> {
    let url = Url::parse(value).map_err(|_| anyhow!("{DATABASE_ENV} must be a PostgreSQL URL"))?;
    if !matches!(url.scheme(), "postgres" | "postgresql") {
        bail!("{DATABASE_ENV} must be a PostgreSQL URL");
    }
    let mut options = ConnectOptions::new(value.to_owned());
    options
        .min_connections(1)
        .max_connections(10)
        .connect_timeout(CONNECT_TIMEOUT)
        .acquire_timeout(CONNECT_TIMEOUT)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .map_err(|_| anyhow!("Outbox PostgreSQL connection failed"))?;
    SysEvents::find()
        .one(&db)
        .await
        .map_err(|_| anyhow!("Outbox schema preflight failed for sys_events"))?;
    Ok(db)
}

async fn read_broker_config(path: &Path) -> Result<IggyConfig> {
    let file = tokio::fs::read(path)
        .await
        .context("Cannot read Iggy config")?;
    let invalid = || anyhow!("Invalid Outbox Iggy JSON config");
    let value: serde_json::Value = serde_json::from_slice(&file).map_err(|_| invalid())?;
    if !value
        .get("external")
        .is_some_and(serde_json::Value::is_object)
    {
        bail!("Outbox Iggy config requires explicit external broker settings");
    }
    let config: IggyConfig = serde_json::from_value(value).map_err(|_| invalid())?;
    if config.mode != IggyMode::External {
        bail!("Outbox Iggy config requires external mode");
    }
    Ok(config)
}

async fn run_until_shutdown(relay: &OutboxRelay) -> Result<()> {
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    let mut delay = Duration::ZERO;
    loop {
        tokio::select! {
            biased;
            result = &mut shutdown => return result,
            _ = tokio::time::sleep(delay) => {}
        }
        // Complete the current bounded batch before accepting shutdown.
        delay = match relay.process_pending_once(None).await {
            Ok(0) => Duration::from_millis(100),
            Ok(_) => Duration::ZERO,
            Err(error) => {
                tracing::error!(%error, "Outbox relay pass failed");
                Duration::from_secs(1)
            }
        };
    }
}

async fn shutdown_signal() -> Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result?,
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;
    Ok(())
}
