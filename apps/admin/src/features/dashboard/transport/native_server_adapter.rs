use leptos::prelude::*;

#[cfg(feature = "ssr")]
use crate::features::dashboard::model::{ActivityItem, ActivityUser, DashboardStats};
use crate::features::dashboard::model::{DashboardStatsResponse, RecentActivityResponse};

#[cfg(feature = "ssr")]
use chrono::Utc;
#[cfg(feature = "ssr")]
use rustok_api::{
    AuthContext, HostRuntimeContext, Permission, TenantContext, has_effective_permission,
};
#[cfg(feature = "ssr")]
use sea_orm::{ConnectionTrait, DbBackend, Statement};

#[server(prefix = "/api/fn", endpoint = "admin/dashboard-stats")]
pub(super) async fn dashboard_stats_native() -> Result<DashboardStatsResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use chrono::Duration;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|_| server_error("Authentication required"))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|_| server_error("Tenant context is required"))?;
        require_dashboard_permission(&auth, tenant.id, Permission::ANALYTICS_READ)?;
        let runtime = use_context::<HostRuntimeContext>()
            .ok_or_else(|| server_error("Dashboard runtime is unavailable"))?;
        let now = Utc::now();
        let current_period_start = now - Duration::days(30);
        let previous_period_start = current_period_start - Duration::days(30);
        let user_stats = load_period_count_snapshot(
            runtime.db(),
            "users",
            tenant.id,
            current_period_start,
            previous_period_start,
            None,
            None,
        )
        .await
        .map_err(dashboard_read_error)?;
        Ok(DashboardStatsResponse {
            dashboard_stats: Some(DashboardStats {
                total_users: user_stats.total_count,
                users_change: calculate_percent_change(
                    user_stats.current_count,
                    user_stats.previous_count,
                ),
                total_posts: None,
                total_orders: None,
                total_revenue: None,
                posts_change: None,
                orders_change: None,
                revenue_change: None,
            }),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/dashboard-stats requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/recent-activity")]
pub(super) async fn recent_activity_native(
    limit: i64,
) -> Result<RecentActivityResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|_| server_error("Authentication required"))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|_| server_error("Tenant context is required"))?;
        require_dashboard_permission(&auth, tenant.id, Permission::USERS_LIST)?;
        let runtime = use_context::<HostRuntimeContext>()
            .ok_or_else(|| server_error("Dashboard runtime is unavailable"))?;
        Ok(RecentActivityResponse {
            recent_activity: load_recent_activity(runtime.db(), tenant.id, limit.clamp(1, 50))
                .await
                .map_err(dashboard_read_error)?,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = limit;
        Err(ServerFnError::new(
            "admin/recent-activity requires the `ssr` feature",
        ))
    }
}
#[cfg(feature = "ssr")]
fn require_dashboard_permission(
    auth: &AuthContext,
    tenant_id: uuid::Uuid,
    permission: Permission,
) -> Result<(), ServerFnError> {
    if auth.tenant_id != tenant_id || !has_effective_permission(&auth.permissions, &permission) {
        return Err(server_error(format!(
            "Permission denied: {permission} required"
        )));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn dashboard_read_error(_error: sea_orm::DbErr) -> ServerFnError {
    tracing::error!(
        error_code = "dashboard_read_failed",
        "Dashboard read failed"
    );
    server_error("Dashboard read failed")
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PeriodCountSnapshot {
    pub(super) total_count: i64,
    pub(super) current_count: i64,
    pub(super) previous_count: i64,
}

#[cfg(feature = "ssr")]
pub(crate) fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

#[cfg(feature = "ssr")]
pub(crate) fn calculate_percent_change(current: i64, previous: i64) -> f64 {
    if previous == 0 {
        if current == 0 { 0.0 } else { 100.0 }
    } else {
        ((current - previous) as f64 / previous as f64) * 100.0
    }
}

#[cfg(feature = "ssr")]
pub(crate) async fn load_period_count_snapshot(
    db: &sea_orm::DatabaseConnection,
    table: &str,
    tenant_id: uuid::Uuid,
    current_period_start: chrono::DateTime<Utc>,
    previous_period_start: chrono::DateTime<Utc>,
    extra_filter_sql: Option<&str>,
    extra_value: Option<&str>,
) -> std::result::Result<PeriodCountSnapshot, sea_orm::DbErr> {
    let backend = db.get_database_backend();
    let filter_sql = extra_filter_sql.unwrap_or("");

    let statement = match backend {
        DbBackend::Sqlite => {
            let sql = format!(
                r#"
                SELECT
                    CAST(COUNT(*) AS INTEGER) AS total_count,
                    CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_count,
                    CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_count
                FROM {table}
                WHERE tenant_id = ?1{filter_sql}
                "#
            );

            let mut values = vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ];
            if let Some(extra_value) = extra_value {
                values.push(extra_value.into());
            }

            Statement::from_sql_and_values(backend, sql, values)
        }
        _ => {
            let sql = format!(
                r#"
                SELECT
                    COUNT(*)::bigint AS total_count,
                    COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_count,
                    COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_count
                FROM {table}
                WHERE tenant_id = $1{filter_sql}
                "#
            );

            let mut values = vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ];
            if let Some(extra_value) = extra_value {
                values.push(extra_value.into());
            }

            Statement::from_sql_and_values(backend, sql, values)
        }
    };

    let Some(row) = db.query_one_raw(statement).await? else {
        return Ok(PeriodCountSnapshot::default());
    };

    Ok(PeriodCountSnapshot {
        total_count: row.try_get("", "total_count")?,
        current_count: row.try_get("", "current_count")?,
        previous_count: row.try_get("", "previous_count")?,
    })
}

#[cfg(feature = "ssr")]
pub(crate) async fn load_recent_activity(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    limit: i64,
) -> std::result::Result<Vec<ActivityItem>, sea_orm::DbErr> {
    let backend = db.get_database_backend();
    let statement = match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            backend,
            r#"
            SELECT id, email, name, created_at
            FROM users
            WHERE tenant_id = ?1
            ORDER BY created_at DESC, id DESC
            LIMIT ?2
            "#,
            vec![tenant_id.into(), limit.into()],
        ),
        _ => Statement::from_sql_and_values(
            backend,
            r#"
            SELECT id, email, name, created_at
            FROM users
            WHERE tenant_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
            vec![tenant_id.into(), limit.into()],
        ),
    };

    let rows = db.query_all_raw(statement).await?;
    rows.into_iter()
        .map(|row| {
            let id: uuid::Uuid = row.try_get("", "id")?;
            let email: String = row.try_get("", "email")?;
            let name: Option<String> = row.try_get("", "name")?;
            let created_at: chrono::DateTime<chrono::FixedOffset> =
                row.try_get("", "created_at")?;

            Ok(ActivityItem {
                id: id.to_string(),
                r#type: "user.created".to_string(),
                description: format!("New user {email} joined"),
                timestamp: created_at.to_rfc3339(),
                user: Some(ActivityUser {
                    id: id.to_string(),
                    name,
                }),
            })
        })
        .collect()
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn auth(tenant_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "password".to_owned(),
        }
    }

    #[test]
    fn native_dashboard_reads_require_field_specific_tenant_authority() {
        let tenant_id = Uuid::new_v4();
        let no_grants = auth(tenant_id, vec![]);
        let analyst = auth(tenant_id, vec![Permission::ANALYTICS_READ]);
        let user_reader = auth(tenant_id, vec![Permission::USERS_LIST]);
        let user_manager = auth(tenant_id, vec![Permission::USERS_MANAGE]);

        for required in [Permission::ANALYTICS_READ, Permission::USERS_LIST] {
            assert!(require_dashboard_permission(&no_grants, tenant_id, required).is_err());
            let other_tenant = auth(Uuid::new_v4(), vec![required]);
            assert!(require_dashboard_permission(&other_tenant, tenant_id, required).is_err());
        }
        assert!(
            require_dashboard_permission(&analyst, tenant_id, Permission::ANALYTICS_READ).is_ok()
        );
        assert!(require_dashboard_permission(&analyst, tenant_id, Permission::USERS_LIST).is_err());
        assert!(
            require_dashboard_permission(&user_reader, tenant_id, Permission::USERS_LIST).is_ok()
        );
        assert!(
            require_dashboard_permission(&user_reader, tenant_id, Permission::ANALYTICS_READ)
                .is_err()
        );
        assert!(
            require_dashboard_permission(&user_manager, tenant_id, Permission::USERS_LIST).is_ok()
        );
    }

    #[test]
    fn dashboard_database_errors_are_redacted() {
        let error = dashboard_read_error(sea_orm::DbErr::Custom(
            "database credentials and private query details".to_owned(),
        ));
        assert!(error.to_string().contains("Dashboard read failed"));
        assert!(!error.to_string().contains("private query details"));
    }
}
