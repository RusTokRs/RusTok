use async_graphql::{Context, Object, Result, SimpleObject};
use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use rustok_outbox::entity::{Column as EventCol, Entity as EventEntity};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect,
};
use uuid::Uuid;

use crate::common::settings::RustokSettings;

use crate::models::_entities::sessions::{Column as SessionCol, Entity as SessionEntity};

// ── Output types ──────────────────────────────────────────────────────────────

#[derive(SimpleObject, Clone, Debug)]
pub struct ComponentHealth {
    pub name: String,
    pub status: String, // "ok" | "degraded" | "unhealthy"
    pub message: Option<String>,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct SystemHealthSummary {
    pub overall: String,
    pub components: Vec<ComponentHealth>,
    pub checked_at: DateTime<Utc>,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct MediaUsageStats {
    pub tenant_id: Uuid,
    pub file_count: i64,
    pub total_bytes: i64,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct SessionStats {
    pub tenant_id: Uuid,
    pub active_sessions: i64,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct CacheHealthPayload {
    pub redis_configured: bool,
    pub redis_healthy: bool,
    pub redis_error: Option<String>,
    pub backend: String,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct EventsStatusPayload {
    /// Transport kind active in current process (from YAML/env config).
    pub configured_transport: String,
    /// Iggy mode when transport or relay involves Iggy.
    pub iggy_mode: String,
    /// Relay interval in milliseconds.
    pub relay_interval_ms: u64,
    /// DLQ enabled flag.
    pub dlq_enabled: bool,
    /// Max relay attempts before DLQ.
    pub max_attempts: i32,
    /// Events waiting to be dispatched.
    pub pending_events: i64,
    /// Events that exhausted retries (in DLQ / failed).
    pub dlq_events: i64,
    /// Available transport options given current build.
    pub available_transports: Vec<String>,
}

// ── Query ─────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SystemQuery;

#[Object]
impl SystemQuery {
    /// Live system health summary: DB connectivity + storage backend.
    async fn system_health(&self, ctx: &Context<'_>) -> Result<SystemHealthSummary> {
        let db = ctx.data::<DatabaseConnection>()?;
        let mut components = Vec::new();
        let mut overall = "ok";

        // DB probe
        let db_ok = sea_orm::ConnectionTrait::execute_unprepared(db, "SELECT 1")
            .await
            .is_ok();
        components.push(ComponentHealth {
            name: "database".into(),
            status: if db_ok { "ok" } else { "unhealthy" }.into(),
            message: if db_ok {
                None
            } else {
                Some("Database ping failed".into())
            },
        });
        if !db_ok {
            overall = "unhealthy";
        }

        // Storage probe (if wired)
        #[cfg(feature = "mod-media")]
        {
            use rustok_storage::StorageService;
            match ctx.data_opt::<StorageService>() {
                Some(storage) => {
                    let health = probe_storage(storage).await;
                    rustok_telemetry::metrics::update_storage_health(
                        storage.backend_name(),
                        health.is_ok(),
                    );
                    components.push(ComponentHealth {
                        name: "storage".into(),
                        status: if health.is_ok() { "ok" } else { "degraded" }.into(),
                        message: health.err().map(|e| e.to_string()),
                    });
                    if let "ok" = overall {
                        if components.last().map(|c| c.status.as_str()) == Some("degraded") {
                            overall = "degraded";
                        }
                    }
                }
                None => {
                    components.push(ComponentHealth {
                        name: "storage".into(),
                        status: "ok".into(),
                        message: Some("not configured".into()),
                    });
                }
            }
        }

        Ok(SystemHealthSummary {
            overall: overall.into(),
            components,
            checked_at: Utc::now(),
        })
    }

    /// Media usage statistics for a tenant (requires mod-media feature).
    #[cfg(feature = "mod-media")]
    async fn media_usage(&self, ctx: &Context<'_>, tenant_id: Uuid) -> Result<MediaUsageStats> {
        use rustok_media::media::{Column as MediaCol, Entity as MediaEntity};

        let db = ctx.data::<DatabaseConnection>()?;

        let file_count = MediaEntity::find()
            .filter(MediaCol::TenantId.eq(tenant_id))
            .count(db)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))? as i64;

        // SUM(size) — manual aggregation via select_only
        let total_bytes: i64 = MediaEntity::find()
            .filter(MediaCol::TenantId.eq(tenant_id))
            .select_only()
            .column_as(sea_orm::sea_query::Expr::col(MediaCol::Size).sum(), "total")
            .into_tuple::<Option<i64>>()
            .one(db)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?
            .flatten()
            .unwrap_or(0);

        Ok(MediaUsageStats {
            tenant_id,
            file_count,
            total_bytes,
        })
    }

    /// Cache backend health status. No auth required (platform infrastructure info).
    async fn cache_health(&self, ctx: &Context<'_>) -> Result<CacheHealthPayload> {
        use rustok_cache::CacheService;

        let app_ctx = ctx.data::<AppContext>()?;

        let Some(cache) = app_ctx.shared_store.get::<CacheService>() else {
            return Ok(CacheHealthPayload {
                redis_configured: false,
                redis_healthy: false,
                redis_error: None,
                backend: "none".to_string(),
            });
        };

        let report = cache.health().await;
        let backend = if report.redis_configured {
            "redis"
        } else {
            "in-memory"
        }
        .to_string();

        Ok(CacheHealthPayload {
            redis_configured: report.redis_configured,
            redis_healthy: report.redis_healthy,
            redis_error: report.redis_error,
            backend,
        })
    }

    /// Events transport runtime status: active config + outbox stats.
    async fn events_status(&self, ctx: &Context<'_>) -> Result<EventsStatusPayload> {
        use crate::common::settings::EventTransportKind;
        use rustok_iggy::config::IggyMode;

        let app_ctx = ctx.data::<AppContext>()?;
        let db = &app_ctx.db;

        let settings = RustokSettings::from_settings(&app_ctx.config.settings).unwrap_or_default();
        let ev = &settings.events;

        // Derive human-readable transport key (matches UI dropdown values).
        let configured_transport = match ev.transport {
            EventTransportKind::Memory => "memory".to_string(),
            EventTransportKind::Outbox => "outbox".to_string(),
            EventTransportKind::Iggy => match ev.iggy.mode {
                IggyMode::Embedded => "iggy_embedded".to_string(),
                IggyMode::Remote => "iggy_external".to_string(),
            },
        };

        let iggy_mode = ev.iggy.mode.to_string();

        // Outbox stats — graceful fallback if table not yet migrated.
        let pending_events = EventEntity::find()
            .filter(EventCol::Status.eq("pending"))
            .count(db)
            .await
            .unwrap_or(0) as i64;

        let dlq_events = EventEntity::find()
            .filter(EventCol::Status.eq("failed"))
            .count(db)
            .await
            .unwrap_or(0) as i64;

        // Available transports: always offer all four; UI filters by module registry.
        let available_transports = vec![
            "memory".to_string(),
            "outbox".to_string(),
            "iggy_embedded".to_string(),
            "iggy_external".to_string(),
        ];

        Ok(EventsStatusPayload {
            configured_transport,
            iggy_mode,
            relay_interval_ms: ev.relay_interval_ms,
            dlq_enabled: ev.dlq.enabled,
            max_attempts: ev.relay_retry_policy.max_attempts,
            pending_events,
            dlq_events,
            available_transports,
        })
    }

    /// Active (non-expired, non-revoked) session count for a tenant.
    async fn session_stats(&self, ctx: &Context<'_>, tenant_id: Uuid) -> Result<SessionStats> {
        let db = ctx.data::<DatabaseConnection>()?;
        let now = Utc::now().fixed_offset();

        let active_sessions = SessionEntity::find()
            .filter(SessionCol::TenantId.eq(tenant_id))
            .filter(SessionCol::RevokedAt.is_null())
            .filter(SessionCol::ExpiresAt.gt(now))
            .count(db)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?
            as i64;

        Ok(SessionStats {
            tenant_id,
            active_sessions,
        })
    }
}

// ── Storage probe ─────────────────────────────────────────────────────────────

#[cfg(feature = "mod-media")]
async fn probe_storage(
    storage: &rustok_storage::StorageService,
) -> std::result::Result<(), rustok_storage::StorageError> {
    let probe_path = ".health-probe";
    let data = bytes::Bytes::from_static(b"ok");
    storage.store(probe_path, data, "text/plain").await?;
    storage.delete(probe_path).await?;
    Ok(())
}
