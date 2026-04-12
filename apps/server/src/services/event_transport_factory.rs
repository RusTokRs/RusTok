use std::sync::Arc;
use std::time::Duration;

use crate::error::{Error, Result};
use loco_rs::app::AppContext;
use rustok_core::events::{EventTransport, MemoryTransport};
use rustok_iggy::{IggyConfig, IggyTransport};
use rustok_outbox::{OutboxRelay, OutboxTransport, RelayConfig};
use tokio::task::JoinHandle;

use crate::common::settings::{EventTransportKind, RelayTargetKind, RustokSettings};

#[derive(Clone)]
pub struct EventRuntime {
    pub transport: Arc<dyn EventTransport>,
    pub relay_config: Option<RelayRuntimeConfig>,
    pub channel_capacity: usize,
    pub relay_fallback_active: bool,
}

#[derive(Clone)]
pub struct RelayRuntimeConfig {
    pub interval: Duration,
    pub relay: OutboxRelay,
}

pub async fn build_event_runtime(ctx: &AppContext) -> Result<EventRuntime> {
    let settings = RustokSettings::from_settings(&ctx.config.settings)
        .map_err(|error| Error::BadRequest(format!("Invalid rustok settings: {error}")))?;

    match settings.events.transport {
        EventTransportKind::Memory => Ok(EventRuntime {
            transport: Arc::new(MemoryTransport::new()),
            relay_config: None,
            channel_capacity: settings.events.channel_capacity,
            relay_fallback_active: false,
        }),
        EventTransportKind::Outbox => {
            let outbox_transport = Arc::new(OutboxTransport::new(ctx.db.clone()));
            let (relay_target, relay_fallback_active) = resolve_relay_target(&settings).await?;
            let relay_policy = &settings.events.relay_retry_policy;
            let max_attempts = if settings.events.dlq.enabled {
                settings.events.dlq.max_attempts
            } else {
                relay_policy.max_attempts
            };
            let relay_config = RelayRuntimeConfig {
                interval: Duration::from_millis(settings.events.relay_interval_ms),
                relay: OutboxRelay::new(ctx.db.clone(), relay_target).with_config(RelayConfig {
                    max_attempts,
                    backoff_base: Duration::from_millis(relay_policy.base_backoff_ms),
                    backoff_max: Duration::from_millis(relay_policy.max_backoff_ms),
                    ..RelayConfig::default()
                }),
            };

            Ok(EventRuntime {
                transport: outbox_transport,
                relay_config: Some(relay_config),
                channel_capacity: settings.events.channel_capacity,
                relay_fallback_active,
            })
        }
        EventTransportKind::Iggy => {
            let transport = IggyTransport::new(resolve_iggy_config(&settings))
                .await
                .map_err(|error| {
                    Error::BadRequest(format!("Failed to initialize iggy transport: {error}"))
                })?;
            Ok(EventRuntime {
                transport: Arc::new(transport),
                relay_config: None,
                channel_capacity: settings.events.channel_capacity,
                relay_fallback_active: false,
            })
        }
    }
}

pub fn spawn_outbox_relay_worker(
    config: RelayRuntimeConfig,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            // Check for shutdown before spawning the inner worker.
            if *stop_rx.borrow() {
                tracing::info!("Outbox relay supervisor received shutdown signal, exiting");
                return;
            }

            let relay = config.relay.clone();
            let interval = config.interval;

            // The inner worker is aborted explicitly when the supervisor receives
            // the stop signal, so it does not need its own stop_rx.
            let mut inner_handle = tokio::spawn(async move {
                loop {
                    if let Err(error) = relay.process_pending_once().await {
                        tracing::error!("Outbox relay iteration failed: {error}");
                    }
                    tokio::time::sleep(interval).await;
                }
            });

            tokio::select! {
                result = &mut inner_handle => {
                    if *stop_rx.borrow() {
                        tracing::info!("Outbox relay supervisor received shutdown signal, exiting");
                        return;
                    }
                    if let Err(panic) = result {
                        tracing::error!(
                            "Outbox relay worker panicked: {:?}. Restarting in 5s.",
                            panic
                        );
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                            _ = stop_rx.changed() => {
                                tracing::info!(
                                    "Outbox relay supervisor received shutdown signal during restart delay, exiting"
                                );
                                return;
                            }
                        }
                    }
                    // Inner task completed normally (shouldn't happen); loop back.
                }
                _ = stop_rx.changed() => {
                    tracing::info!("Outbox relay supervisor received shutdown signal, exiting");
                    inner_handle.abort();
                    return;
                }
            }
        }
    })
}

fn resolve_iggy_config(settings: &RustokSettings) -> IggyConfig {
    settings.events.iggy.clone()
}

async fn resolve_relay_target(
    settings: &RustokSettings,
) -> Result<(Arc<dyn EventTransport>, bool)> {
    match settings.events.relay_target {
        RelayTargetKind::Memory => Ok((Arc::new(MemoryTransport::new()), false)),
        RelayTargetKind::Iggy => match IggyTransport::new(resolve_iggy_config(settings)).await {
            Ok(transport) => Ok((Arc::new(transport), false)),
            Err(error) => {
                if settings.events.allow_relay_target_fallback {
                    tracing::warn!(
                        error = %error,
                        "Failed to initialize relay_target=iggy, fallback to memory due to explicit opt-in"
                    );
                    Ok((Arc::new(MemoryTransport::new()), true))
                } else {
                    Err(Error::BadRequest(format!(
                        "Failed to initialize relay_target=iggy and fallback is disabled: {error}"
                    )))
                }
            }
        },
    }
}
