use std::{env, path::Path, sync::Arc, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use rustok_iggy::{IggyConfig, IggyMode, IggyTransport};
use rustok_outbox::{OutboxRelay, RelayConfig, SysEvents, entity};
use rustok_telemetry::{LogFormat, TelemetryConfig};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, EntityTrait, QuerySelect};
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

    let relay_config = relay_config_from_env()?;
    let database = open_database(&env::var(DATABASE_ENV).context(DATABASE_ENV)?).await?;
    let config_path = env::var_os(IGGY_CONFIG_ENV).context(IGGY_CONFIG_ENV)?;
    let config = read_broker_config(Path::new(&config_path)).await?;
    let broker = Arc::new(
        tokio::time::timeout(CONNECT_TIMEOUT, IggyTransport::new(config))
            .await
            .map_err(|_| anyhow!("Outbox Iggy connection timed out"))?
            .map_err(|error| {
                // The outward error stays free of broker internals; the
                // root cause belongs in the operator's log, not the void.
                tracing::error!(%error, "Outbox Iggy transport initialization failed");
                anyhow!("Outbox could not initialize the configured Iggy transport")
            })?,
    );
    let relay = OutboxRelay::new(database, broker.clone()).with_config(relay_config);
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
    let db = Database::connect(options).await.map_err(|error| {
        tracing::error!(%error, "Outbox PostgreSQL connection failed");
        anyhow!("Outbox PostgreSQL connection failed")
    })?;
    // Probe schema readability without loading a full event payload.
    SysEvents::find()
        .select_only()
        .column(entity::Column::Id)
        .limit(1)
        .into_tuple::<uuid::Uuid>()
        .all(&db)
        .await
        .map_err(|_| anyhow!("Outbox schema preflight failed for sys_events"))?;
    Ok(db)
}

/// Relay tuning for the standalone deployment. The embedded server resolves
/// the same policy from `rustok.events.*` settings; here every knob is an
/// optional environment variable over the `RelayConfig` defaults, so the
/// two deployments cannot silently drift into different retry policies
/// unless an operator sets them apart on purpose.
fn relay_config_from_env() -> Result<RelayConfig> {
    let defaults = RelayConfig::default();
    let config = RelayConfig {
        batch_size: env_u64("RUSTOK_OUTBOX_RELAY_BATCH_SIZE", defaults.batch_size)?,
        max_attempts: i32::try_from(env_u64(
            "RUSTOK_OUTBOX_RELAY_MAX_ATTEMPTS",
            u64::try_from(defaults.max_attempts).unwrap_or(5),
        )?)
        .map_err(|_| anyhow!("RUSTOK_OUTBOX_RELAY_MAX_ATTEMPTS is out of range"))?,
        backoff_base: env_duration_ms(
            "RUSTOK_OUTBOX_RELAY_BACKOFF_BASE_MS",
            defaults.backoff_base,
        )?,
        backoff_max: env_duration_ms("RUSTOK_OUTBOX_RELAY_BACKOFF_MAX_MS", defaults.backoff_max)?,
        max_concurrency: usize::try_from(env_u64(
            "RUSTOK_OUTBOX_RELAY_MAX_CONCURRENCY",
            defaults.max_concurrency as u64,
        )?)
        .map_err(|_| anyhow!("RUSTOK_OUTBOX_RELAY_MAX_CONCURRENCY is out of range"))?,
        claim_ttl: env_duration_ms("RUSTOK_OUTBOX_RELAY_CLAIM_TTL_MS", defaults.claim_ttl)?,
        publish_timeout: env_duration_ms(
            "RUSTOK_OUTBOX_RELAY_PUBLISH_TIMEOUT_MS",
            defaults.publish_timeout,
        )?,
        worker_id: env::var("RUSTOK_OUTBOX_RELAY_WORKER_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(defaults.worker_id),
    };
    config.validate().map_err(|error| anyhow!("Invalid Outbox relay configuration: {error}"))?;
    Ok(config)
}

fn env_u64(name: &str, default: u64) -> Result<u64> {
    match env::var(name) {
        Ok(value) => value
            .trim()
            .parse::<u64>()
            .map_err(|_| anyhow!("{name} must be a non-negative integer")),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(anyhow!("{name} is not valid Unicode: {error}")),
    }
}

fn env_duration_ms(name: &str, default: Duration) -> Result<Duration> {
    Ok(Duration::from_millis(env_u64(
        name,
        u64::try_from(default.as_millis()).unwrap_or(u64::MAX),
    )?))
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
