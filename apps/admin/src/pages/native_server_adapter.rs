use leptos::prelude::*;

#[cfg(feature = "ssr")]
use super::dashboard::DashboardStats;
#[cfg(feature = "ssr")]
use super::dashboard::{
    calculate_percent_change, load_order_stats_snapshot, load_period_count_snapshot,
    load_recent_activity, server_error as dashboard_server_error,
};
use super::dashboard::{DashboardStatsResponse, RecentActivityResponse};
use super::roles::GraphqlRolesResponse;
#[cfg(feature = "ssr")]
use super::roles::RoleInfo;

#[cfg(feature = "ssr")]
fn platform_settings_server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

#[server(prefix = "/api/fn", endpoint = "admin/list-roles")]
pub(super) async fn list_roles_native() -> Result<GraphqlRolesResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos_axum::extract;
        use rustok_api::Permission;
        use rustok_api::{has_effective_permission, AuthContext};
        use rustok_core::{Rbac, UserRole};

        let auth = extract::<AuthContext>().await.map_err(ServerFnError::new)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new("settings:read required to list roles"));
        }

        let roles = [
            UserRole::SuperAdmin,
            UserRole::Admin,
            UserRole::Manager,
            UserRole::Customer,
        ]
        .into_iter()
        .map(|role| {
            let mut permissions = Rbac::permissions_for_role(&role)
                .iter()
                .map(|permission| permission.to_string())
                .collect::<Vec<_>>();
            permissions.sort();

            let display_name = match role {
                UserRole::SuperAdmin => "Super Admin",
                UserRole::Admin => "Admin",
                UserRole::Manager => "Manager",
                UserRole::Customer => "Customer",
            };

            RoleInfo {
                slug: role.to_string(),
                display_name: display_name.to_string(),
                permissions,
            }
        })
        .collect();

        Ok(GraphqlRolesResponse { roles })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/list-roles requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/dashboard-stats")]
pub(super) async fn dashboard_stats_native() -> Result<DashboardStatsResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use chrono::{Duration, Utc};
        use sea_orm::{ConnectionTrait, DbBackend};

        let _auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(|err| dashboard_server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|err| dashboard_server_error(err.to_string()))?;
        let app_ctx = expect_context::<loco_rs::app::AppContext>();

        let now = Utc::now();
        let current_period_start = now - Duration::days(30);
        let previous_period_start = current_period_start - Duration::days(30);

        let user_stats = load_period_count_snapshot(
            &app_ctx.db,
            "users",
            tenant.id,
            current_period_start,
            previous_period_start,
            None,
            None,
        )
        .await
        .map_err(|err| dashboard_server_error(err.to_string()))?;

        let post_stats = load_period_count_snapshot(
            &app_ctx.db,
            "nodes",
            tenant.id,
            current_period_start,
            previous_period_start,
            Some(match app_ctx.db.get_database_backend() {
                DbBackend::Sqlite => " AND kind = ?4",
                _ => " AND kind = $4",
            }),
            Some("post"),
        )
        .await
        .map_err(|err| dashboard_server_error(err.to_string()))?;

        let order_stats = load_order_stats_snapshot(
            &app_ctx.db,
            tenant.id,
            current_period_start,
            previous_period_start,
        )
        .await
        .map_err(|err| dashboard_server_error(err.to_string()))?;

        Ok(DashboardStatsResponse {
            dashboard_stats: Some(DashboardStats {
                total_users: user_stats.total_count,
                total_posts: post_stats.total_count,
                total_orders: order_stats.total_orders,
                total_revenue: order_stats.total_revenue,
                users_change: calculate_percent_change(
                    user_stats.current_count,
                    user_stats.previous_count,
                ),
                posts_change: calculate_percent_change(
                    post_stats.current_count,
                    post_stats.previous_count,
                ),
                orders_change: calculate_percent_change(
                    order_stats.current_orders,
                    order_stats.previous_orders,
                ),
                revenue_change: calculate_percent_change(
                    order_stats.current_revenue,
                    order_stats.previous_revenue,
                ),
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
        let _auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(|err| dashboard_server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|err| dashboard_server_error(err.to_string()))?;
        let app_ctx = expect_context::<loco_rs::app::AppContext>();

        Ok(RecentActivityResponse {
            recent_activity: load_recent_activity(&app_ctx.db, tenant.id, limit.clamp(1, 50))
                .await
                .map_err(|err| dashboard_server_error(err.to_string()))?,
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
