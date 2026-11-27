use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use chrono::{DateTime, Duration, Utc};
use rustok_api::{HostAuthority, Permission, graphql::GraphQLError, has_effective_permission};
use rustok_outbox::entity::{Column as EventCol, Entity as EventEntity};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
};
use std::fmt::Display;
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
    pub configured_profile: String,
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

/// Authority for the cache diagnostics surface.
///
/// Cache health is part of the tenant cache-administration surface: a tenant
/// administrator holding `settings:read` may read it, and a host operator with host
/// read authority may read it. The rule itself is owned by
/// [`rustok_api::authorizes_host_or_permission`] and is evaluated identically by the
/// native cache-admin server functions, so the GraphQL and SSR transports cannot
/// drift apart again. Cache health carries no tenant data, so no tenant scope is
/// consulted here; the settings category itself is tenant-scoped.
fn require_cache_read_authority(ctx: &Context<'_>) -> Result<()> {
    let auth = ctx.data_opt::<AuthContext>();
    if rustok_api::authorizes_host_or_permission(
        auth,
        &Permission::SETTINGS_READ,
        HostAuthority::Read,
    ) {
        return Ok(());
    }

    Err(<FieldError as GraphQLError>::permission_denied(
        "cache diagnostics require settings:read or host-global authority",
    ))
}

fn require_host_authority(_ctx: &Context<'_>, required: HostAuthority) -> Result<()> {
    if crate::host_authority::current_host_authority()
        .filter(|authority| authority.allows(required))
        .is_some()
    {
        return Ok(());
    }

    Err(<FieldError as GraphQLError>::permission_denied(
        "host-global authority required",
    ))
}

fn graphql_system_internal_error(message: &'static str, _error: impl Display) -> FieldError {
    tracing::error!(
        error_code = "graphql_system_query_failed",
        operation = message,
        "GraphQL system query failed"
    );
    <FieldError as GraphQLError>::internal_error(message)
}

/// Build the same active-session predicate used by the authentication lifecycle.
/// Legacy rows without an explicit absolute deadline are bounded by their original
/// creation time, and future-created rows are never counted as active.
fn active_sessions_query(
    tenant_id: Uuid,
    now: DateTime<Utc>,
) -> Result<sea_orm::Select<SessionEntity>> {
    let original_cutoff = now
        .checked_sub_signed(Duration::days(
            crate::models::sessions::SESSION_ABSOLUTE_LIFETIME_DAYS,
        ))
        .ok_or_else(|| {
            graphql_system_internal_error(
                "Unable to calculate active session cutoff",
                "timestamp out of range",
            )
        })?;

    Ok(SessionEntity::find()
        .filter(SessionCol::TenantId.eq(tenant_id))
        .filter(SessionCol::RevokedAt.is_null())
        .filter(SessionCol::CreatedAt.gt(original_cutoff))
        .filter(SessionCol::CreatedAt.lte(now.clone()))
        .filter(SessionCol::ExpiresAt.gt(now.clone()))
        .filter(
            Condition::any()
                .add(SessionCol::AbsoluteExpiresAt.gt(now.clone()))
                .add(SessionCol::AbsoluteExpiresAt.is_null()),
        ))
}

/// An initialized EventRuntime with no Iggy connector is explicitly inactive.
/// Only before runtime initialization do we fall back to the deployment setting.
fn resolved_iggy_mode(
    runtime_mode: Option<Option<&rustok_iggy::config::IggyMode>>,
    configured_mode: &rustok_iggy::config::IggyMode,
) -> String {
    match runtime_mode {
        Some(Some(mode)) => mode.to_string(),
        Some(None) => "inactive".to_string(),
        None => configured_mode.to_string(),
    }
}

// ── Query ─────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SystemQuery;

#[Object]
impl SystemQuery {
    /// Detailed system health is a host-operator diagnostic surface. Public
    /// liveness/readiness checks must use the dedicated HTTP health endpoints.
    async fn system_health(&self, ctx: &Context<'_>) -> Result<SystemHealthSummary> {
        require_host_authority(ctx, HostAuthority::Read)?;
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
                    let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
                    let health = crate::services::storage_health_probe::probe_storage_backend(
                        runtime_ctx,
                        storage,
                    )
                    .await;
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

    /// Cache topology and backend failures are host-operator diagnostics.
    async fn cache_health(&self, ctx: &Context<'_>) -> Result<CacheHealthPayload> {
        use rustok_cache::CacheService;

        require_cache_read_authority(ctx)?;
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

    /// Event transport topology and all-tenant queue counts require host read authority.
    async fn events_status(&self, ctx: &Context<'_>) -> Result<EventsStatusPayload> {
        require_host_authority(ctx, HostAuthority::Read)?;
        let runtime_ctx = ctx.data::<ServerRuntimeContext>()?;
        let db = runtime_ctx.db();
        let ev = &runtime_ctx.settings().events;

        let event_runtime = runtime_ctx
            .shared_get::<std::sync::Arc<crate::services::event_transport_factory::EventRuntime>>();
        let configured_profile = match event_runtime
            .as_ref()
            .map(|runtime| runtime.delivery_profile)
            .unwrap_or(ev.delivery_profile)
        {
            crate::common::settings::EventDeliveryProfile::Outbox => "outbox".to_string(),
            crate::common::settings::EventDeliveryProfile::OutboxIggy => "outbox_iggy".to_string(),
        };
        let iggy_mode = resolved_iggy_mode(
            event_runtime.as_ref().map(|runtime| runtime.iggy_mode.as_ref()),
            &ev.iggy.mode,
        );

        let pending_events = EventEntity::find()
            .filter(EventCol::Status.eq("pending"))
            .count(db)
            .await
            .map_err(|error| {
                graphql_system_internal_error("Unable to read pending event count", error)
            })? as i64;
        let dlq_events = EventEntity::find()
            .filter(EventCol::Status.eq("failed"))
            .count(db)
            .await
            .map_err(|error| {
                graphql_system_internal_error("Unable to read dead-letter event count", error)
            })? as i64;

        Ok(EventsStatusPayload {
            configured_profile,
            iggy_mode,
            relay_interval_ms: ev.relay_interval_ms,
            dlq_enabled: ev.dlq.enabled,
            max_attempts: ev.relay_retry_policy.max_attempts,
            pending_events,
            dlq_events,
            available_transports: vec!["outbox".to_string(), "outbox_iggy".to_string()],
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
        let now = rustok_auth::application_clock::verified_now(db)
            .await
            .map_err(|error| {
                graphql_system_internal_error("Unable to resolve active session clock", error)
            })?;
        let active_sessions = active_sessions_query(tenant.id, now)?
            .count(db)
            .await
            .map_err(|error| {
                graphql_system_internal_error("Unable to read active session count", error)
            })? as i64;

        Ok(SessionStats {
            tenant_id: tenant.id,
            active_sessions,
        })
    }
}

#[cfg(test)]
mod tests {
    use async_graphql::{EmptyMutation, EmptySubscription, Schema, Value};
    use rustok_api::{Permission, has_effective_permission};

    #[tokio::test]
    async fn host_global_system_queries_require_host_authority() {
        let schema = Schema::build(super::SystemQuery, EmptyMutation, EmptySubscription).finish();
        for query in [
            "{ systemHealth { overall } }",
            "{ eventsStatus { pendingEvents } }",
        ] {
            let response = schema.execute(query).await;
            assert_eq!(response.errors.len(), 1, "{query}: {response:?}");
            let error = &response.errors[0];
            assert_eq!(error.message, "host-global authority required");
            assert_eq!(
                error.extensions.as_ref().and_then(|ext| ext.get("code")),
                Some(&Value::String("PERMISSION_DENIED".into())),
            );
        }
    }

    #[test]
    fn tenant_manage_permission_still_satisfies_tenant_session_read() {
        assert!(has_effective_permission(
            &[Permission::USERS_MANAGE],
            &Permission::USERS_READ,
        ));
    }

    #[test]
    fn active_session_stats_query_uses_authentication_expiry_fences() {
        use sea_orm::{DbBackend, QueryTrait};

        let statement = super::active_sessions_query(Uuid::new_v4(), Utc::now())
            .expect("active-session query should be buildable")
            .build(DbBackend::Sqlite);
        let sql = statement.sql.to_ascii_lowercase();

        for column in [
            "tenant_id",
            "revoked_at",
            "created_at",
            "expires_at",
            "absolute_expires_at",
        ] {
            assert!(sql.contains(column), "query is missing {column}: {sql}");
        }
        assert!(
            sql.matches("created_at").count() >= 2,
            "the query must enforce both the 90-day creation cutoff and no-future-creation fence: {sql}"
        );
    }

    #[test]
    fn events_status_distinguishes_active_and_configured_iggy_modes() {
        use rustok_iggy::config::IggyMode;

        let configured = IggyMode::Bundled;
        let active = IggyMode::External;

        assert_eq!(
            super::resolved_iggy_mode(None, &configured),
            configured.to_string(),
            "before runtime initialization, report the deployment setting"
        );
        assert_eq!(
            super::resolved_iggy_mode(Some(None), &configured),
            "inactive",
            "an initialized Outbox runtime has no active Iggy connector"
        );
        assert_eq!(
            super::resolved_iggy_mode(Some(Some(&active)), &configured),
            active.to_string(),
            "once initialized, report the actual runtime connector mode"
        );
    }
}
