use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use chrono::{DateTime, Utc};
use rustok_api::{Permission, graphql::GraphQLError, has_effective_permission};
use rustok_outbox::entity::{Column as EventCol, Entity as EventEntity};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};
use uuid::Uuid;

use crate::context::{AuthContext, TenantContext};
use crate::services::server_runtime_context::ServerRuntimeContext;

use crate::models::_entities::sessions::{Column as SessionCol, Entity as SessionEntity};

// ── Output types ──────────────────────────────────────────────────────────────

#[derive(SimpleObject, Clone, Debug)]
pub struct ComponentHealth {
    pub name: String,
    pub status: String,
    pub message: Option<String>,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct SystemHealthSummary {
    pub overall: String,
    pub components: Vec<ComponentHealth>,
    pub checked_at: DateTime<Utc>,
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
    pub configured_transport: String,
    pub iggy_mode: String,
    pub relay_interval_ms: u64,
    pub dlq_enabled: bool,
    pub max_attempts: i32,
    pub pending_events: i64,
    pub dlq_events: i64,
    pub available_transports: Vec<String>,
}

fn require_permission<'a>(
    ctx: &'a Context<'_>,
    permission: &Permission,
    message: &str,
) -> Result<&'a AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    if !has_effective_permission(&auth.permissions, permission) {
        return Err(<FieldError as GraphQLError>::permission_denied(message));
    }
    Ok(auth)
}

fn require_logs_read<'a>(ctx: &'a Context<'_>) -> Result<&'a AuthContext> {
    require_permission(
        ctx,
        &Permission::LOGS_READ,
        "logs:read required to inspect system diagnostics",
    )
}

// ── Query ─────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SystemQuery;

#[Object]
impl SystemQuery {
    /// Detailed system health is an administrative diagnostic surface. Public
    /// liveness/readiness checks must use the dedicated HTTP health endpoints.
    async fn system_health(&self, ctx: &Context<'_>) -> Result<SystemHealthSummary> {
        require_logs_read(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let mut components = Vec::new();
        let mut overall = "ok";

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

        #[cfg(feature = "mod-media")]
        {
            use rustok_storage::StorageRuntime;
            match ctx.data_opt::<StorageRuntime>() {
                Some(storage) => {
                    let health = probe_storage(storage).await;
                    rustok_telemetry::metrics::update_storage_health(
                        storage.kind.as_str(),
                        health.is_ok(),
                    );
                    components.push(ComponentHealth {
                        name: "storage".into(),
                        status: if health.is_ok() { "ok" } else { "degraded" }.into(),
                        message: health.err().map(|error| error.to_string()),
                    });
                    if overall == "ok"
                        && components.last().map(|component| component.status.as_str())
                            == Some("degraded")
                    {
                        overall = "degraded";
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

    /// Cache topology and backend failures are administrative diagnostics.
    async fn cache_health(&self, ctx: &Context<'_>) -> Result<CacheHealthPayload> {
        use rustok_cache::CacheService;

        require_logs_read(ctx)?;
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;

        let Some(cache) = runtime_ctx.shared_get::<CacheService>() else {
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

    /// Event transport topology and queue counts require operational log access.
    async fn events_status(&self, ctx: &Context<'_>) -> Result<EventsStatusPayload> {
        use crate::common::settings::EventTransportKind;
        use rustok_iggy::config::IggyMode;

        require_logs_read(ctx)?;
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let db = runtime_ctx.db();
        let ev = &runtime_ctx.settings().events;

        let configured_transport = match ev.transport {
            EventTransportKind::Memory => "memory".to_string(),
            EventTransportKind::Outbox => "outbox".to_string(),
            EventTransportKind::Iggy => match ev.iggy.mode {
                IggyMode::Embedded => "iggy_embedded".to_string(),
                IggyMode::Remote => "iggy_external".to_string(),
            },
        };
        let iggy_mode = ev.iggy.mode.to_string();

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

        Ok(EventsStatusPayload {
            configured_transport,
            iggy_mode,
            relay_interval_ms: ev.relay_interval_ms,
            dlq_enabled: ev.dlq.enabled,
            max_attempts: ev.relay_retry_policy.max_attempts,
            pending_events,
            dlq_events,
            available_transports: vec![
                "memory".to_string(),
                "outbox".to_string(),
                "iggy_embedded".to_string(),
                "iggy_external".to_string(),
            ],
        })
    }

    /// Active session count is tenant-bound and requires user read authority.
    async fn session_stats(&self, ctx: &Context<'_>, tenant_id: Uuid) -> Result<SessionStats> {
        let auth = require_permission(
            ctx,
            &Permission::USERS_READ,
            "users:read required to inspect session statistics",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        if auth.tenant_id != tenant.id || tenant_id != tenant.id {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "session statistics are restricted to the current tenant",
            ));
        }

        let db = ctx.data::<DatabaseConnection>()?;
        let now = Utc::now().fixed_offset();
        let active_sessions = SessionEntity::find()
            .filter(SessionCol::TenantId.eq(tenant.id))
            .filter(SessionCol::RevokedAt.is_null())
            .filter(SessionCol::ExpiresAt.gt(now))
            .count(db)
            .await
            .map_err(|error| <FieldError as GraphQLError>::internal_error(&error.to_string()))?
            as i64;

        Ok(SessionStats {
            tenant_id: tenant.id,
            active_sessions,
        })
    }
}

#[cfg(feature = "mod-media")]
async fn probe_storage(storage: &rustok_storage::StorageRuntime) -> object_store::Result<()> {
    use object_store::ObjectStoreExt;
    let probe_path = rustok_storage::ObjectKey::chronological(
        "platform-health",
        rustok_storage::ObjectZone::Staging,
        rustok_storage::ObjectScope::Platform,
        chrono::Utc::now(),
        uuid::Uuid::nil(),
        "probe",
    )
    .expect("platform health key constants are valid")
    .into_path();
    let data = bytes::Bytes::from_static(b"ok");
    storage
        .objects
        .put_opts(&probe_path, data.into(), storage.put_options("text/plain"))
        .await?;
    storage.objects.delete(&probe_path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rustok_api::{Action, Permission, Resource, has_effective_permission};

    #[test]
    fn manage_permissions_satisfy_diagnostic_read_requirements() {
        assert!(has_effective_permission(
            &[Permission::new(Resource::Logs, Action::Manage)],
            &Permission::LOGS_READ,
        ));
        assert!(has_effective_permission(
            &[Permission::USERS_MANAGE],
            &Permission::USERS_READ,
        ));
    }
}
