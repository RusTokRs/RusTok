#[cfg(feature = "ssr")]
use chrono::{Duration, Utc};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_auth, use_current_user, use_tenant, use_token};
#[cfg(feature = "ssr")]
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::app::modules::{components_for_slot, AdminSlot};
use crate::app::providers::enabled_modules::use_enabled_modules;
use crate::shared::api::queries::{DASHBOARD_STATS_QUERY, RECENT_ACTIVITY_QUERY};
use crate::shared::api::request;
use crate::shared::api::ApiError;
use crate::shared::ui::{Button, LanguageToggle, PageHeader};
use crate::widgets::stats_card::StatsCard;
use crate::{t_string, use_i18n};

fn local_resource<S, Fut, T>(
    source: impl Fn() -> S + 'static,
    fetcher: impl Fn(S) -> Fut + 'static,
) -> LocalResource<T>
where
    S: 'static,
    Fut: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    LocalResource::new(move || fetcher(source()))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DashboardStatsResponse {
    #[serde(rename = "dashboardStats")]
    dashboard_stats: Option<DashboardStats>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DashboardStats {
    #[serde(rename = "totalUsers")]
    total_users: i64,
    #[serde(rename = "totalPosts")]
    total_posts: i64,
    #[serde(rename = "totalOrders")]
    total_orders: i64,
    #[serde(rename = "totalRevenue")]
    total_revenue: i64,
    #[serde(rename = "usersChange")]
    users_change: f64,
    #[serde(rename = "postsChange")]
    posts_change: f64,
    #[serde(rename = "ordersChange")]
    orders_change: f64,
    #[serde(rename = "revenueChange")]
    revenue_change: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RecentActivityResponse {
    #[serde(rename = "recentActivity")]
    recent_activity: Vec<ActivityItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ActivityItem {
    id: String,
    #[serde(rename = "type")]
    r#type: String,
    description: String,
    timestamp: String,
    user: Option<ActivityUser>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ActivityUser {
    id: String,
    name: Option<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Copy, Default)]
struct OrderStatsSnapshot {
    total_orders: i64,
    total_revenue: i64,
    current_orders: i64,
    previous_orders: i64,
    current_revenue: i64,
    previous_revenue: i64,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Copy, Default)]
struct PeriodCountSnapshot {
    total_count: i64,
    current_count: i64,
    previous_count: i64,
}

#[cfg(feature = "ssr")]
fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

fn map_fallback_error(context: &str, native: ServerFnError, graphql: ApiError) -> String {
    format!("{context}: native path failed ({native}); GraphQL fallback failed ({graphql})")
}

async fn fetch_dashboard_stats_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<DashboardStatsResponse, ApiError> {
    request::<_, DashboardStatsResponse>(DASHBOARD_STATS_QUERY, json!({}), token, tenant_slug).await
}

async fn fetch_recent_activity_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
    limit: i64,
) -> Result<RecentActivityResponse, ApiError> {
    request::<_, RecentActivityResponse>(
        RECENT_ACTIVITY_QUERY,
        json!({ "limit": limit }),
        token,
        tenant_slug,
    )
    .await
}

async fn fetch_dashboard_stats(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<DashboardStatsResponse, String> {
    match dashboard_stats_native().await {
        Ok(response) => Ok(response),
        Err(native_error) => fetch_dashboard_stats_graphql(token, tenant_slug)
            .await
            .map_err(|graphql_error| {
                map_fallback_error(
                    "dashboard stats request failed",
                    native_error,
                    graphql_error,
                )
            }),
    }
}

async fn fetch_recent_activity(
    token: Option<String>,
    tenant_slug: Option<String>,
    limit: i64,
) -> Result<RecentActivityResponse, String> {
    match recent_activity_native(limit).await {
        Ok(response) => Ok(response),
        Err(native_error) => fetch_recent_activity_graphql(token, tenant_slug, limit)
            .await
            .map_err(|graphql_error| {
                map_fallback_error(
                    "recent activity request failed",
                    native_error,
                    graphql_error,
                )
            }),
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/dashboard-stats")]
async fn dashboard_stats_native() -> Result<DashboardStatsResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let _auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
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
        .map_err(|err| server_error(err.to_string()))?;

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
        .map_err(|err| server_error(err.to_string()))?;

        let order_stats = load_order_stats_snapshot(
            &app_ctx.db,
            tenant.id,
            current_period_start,
            previous_period_start,
        )
        .await
        .map_err(|err| server_error(err.to_string()))?;

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
async fn recent_activity_native(_limit: i64) -> Result<RecentActivityResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let _auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let app_ctx = expect_context::<loco_rs::app::AppContext>();

        Ok(RecentActivityResponse {
            recent_activity: load_recent_activity(&app_ctx.db, tenant.id, _limit.clamp(1, 50))
                .await
                .map_err(|err| server_error(err.to_string()))?,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/recent-activity requires the `ssr` feature",
        ))
    }
}

#[component]
pub fn Dashboard() -> impl IntoView {
    let i18n = use_i18n();
    let auth = use_auth();
    let current_user = use_current_user();
    let token = use_token();
    let tenant = use_tenant();

    let dashboard_stats = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            fetch_dashboard_stats(token_value, tenant_value).await
        },
    );

    let recent_activity = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            fetch_recent_activity(token_value, tenant_value, 10).await
        },
    );

    let logout = move |_| {
        let auth = auth.clone();
        spawn_local(async move {
            let _ = auth.sign_out().await;
        });
    };

    let enabled_modules = use_enabled_modules();

    let title = current_user
        .get()
        .and_then(|user| user.name)
        .unwrap_or_else(|| "Dashboard".to_string());

    let module_sections = Signal::derive(move || {
        let enabled = enabled_modules.get();
        components_for_slot(AdminSlot::DashboardSection, Some(&enabled))
    });

    view! {
        <section class="p-4 md:p-8">
            <PageHeader
                title=title
                eyebrow=t_string!(i18n, app.nav.dashboard).to_string()
                subtitle=t_string!(i18n, app.dashboard.subtitle).to_string()
                actions=view! {
                    <LanguageToggle />
                    <Button
                        on_click=logout
                        class="border border-border bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                    >
                        {move || t_string!(i18n, app.dashboard.logout)}
                    </Button>
                    <Button on_click=move |_| {}>
                        {move || t_string!(i18n, app.dashboard.createTenant)}
                    </Button>
                }
                .into_any()
            />

            <Suspense
                fallback=move || view! {
                    <div class="mb-8 grid gap-5 md:grid-cols-2 xl:grid-cols-4">
                        {(0..4)
                            .map(|_| {
                                view! { <div class="h-32 animate-pulse rounded-xl bg-muted"></div> }
                            })
                            .collect_view()}
                    </div>
                }
            >
                {move || {
                    let stats = dashboard_stats
                        .get()
                        .and_then(|res| res.ok())
                        .and_then(|res| res.dashboard_stats)
                        .map(|stats| {
                            vec![
                                (
                                    t_string!(i18n, app.dashboard.stats.users),
                                    stats.total_users.to_string(),
                                    format!("{:+.1}%", stats.users_change),
                                ),
                                (
                                    t_string!(i18n, app.dashboard.stats.posts),
                                    stats.total_posts.to_string(),
                                    format!("{:+.1}%", stats.posts_change),
                                ),
                                (
                                    t_string!(i18n, app.dashboard.stats.orders),
                                    stats.total_orders.to_string(),
                                    format!("{:+.1}%", stats.orders_change),
                                ),
                                (
                                    t_string!(i18n, app.dashboard.stats.revenue),
                                    format!("${}", stats.total_revenue),
                                    format!("{:+.1}%", stats.revenue_change),
                                ),
                            ]
                        })
                        .unwrap_or_else(|| {
                            vec![
                                (t_string!(i18n, app.dashboard.stats.users), "—".to_string(), "".to_string()),
                                (t_string!(i18n, app.dashboard.stats.posts), "—".to_string(), "".to_string()),
                                (t_string!(i18n, app.dashboard.stats.orders), "—".to_string(), "".to_string()),
                                (t_string!(i18n, app.dashboard.stats.revenue), "—".to_string(), "".to_string()),
                            ]
                        });

                    view! {
                        <div class="mb-8 grid gap-5 md:grid-cols-2 xl:grid-cols-4">
                            {stats
                                .into_iter()
                                .map(|(title, value, hint)| {
                                    view! {
                                        <StatsCard
                                            title=title
                                            value=value
                                            icon=view! { <span class="text-muted-foreground">"•"</span> }.into_any()
                                            trend=hint
                                            trend_label=t_string!(i18n, app.dashboard.stats.vsLastMonth)
                                            class="transition-all hover:scale-[1.02]"
                                        />
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                }}
            </Suspense>

            <div class="grid gap-6 lg:grid-cols-[1.4fr_1fr]">
                <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                    <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                        {move || t_string!(i18n, app.dashboard.activity.title)}
                    </h4>
                    <Suspense
                        fallback=move || view! {
                            <div class="space-y-3">
                                {(0..4)
                                    .map(|_| {
                                        view! { <div class="h-14 animate-pulse rounded-lg bg-muted"></div> }
                                    })
                                    .collect_view()}
                            </div>
                        }
                    >
                        {move || {
                            let activities = recent_activity
                                .get()
                                .and_then(|res| res.ok())
                                .map(|res| res.recent_activity)
                                .unwrap_or_default();

                            if activities.is_empty() {
                                view! {
                                    <div class="py-8 text-center text-muted-foreground">
                                        {t_string!(i18n, app.dashboard.activity.empty)}
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    {activities
                                        .into_iter()
                                        .map(|item| {
                                            let time_ago = format_time_ago(&item.timestamp);
                                            let user_name = item
                                                .user
                                                .as_ref()
                                                .and_then(|u| u.name.clone())
                                                .unwrap_or_else(|| t_string!(i18n, app.dashboard.activity.system).to_string());
                                            view! {
                                                <div class="flex items-center justify-between border-b border-border py-3 last:border-b-0">
                                                    <div class="min-w-0 flex-1">
                                                        <div class="flex items-center gap-2">
                                                            <ActivityIcon activity_type=item.r#type.clone() />
                                                            <strong class="truncate text-foreground">{item.description}</strong>
                                                        </div>
                                                        <p class="mt-1 text-sm text-muted-foreground">
                                                            {format!("by {}", user_name)}
                                                        </p>
                                                    </div>
                                                    <span class="ml-3 inline-flex shrink-0 items-center rounded-full bg-secondary px-3 py-1 text-xs font-medium text-secondary-foreground">
                                                        {time_ago}
                                                    </span>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                }.into_any()
                            }
                        }}
                    </Suspense>
                </div>
                <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                    <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                        {move || t_string!(i18n, app.dashboard.quick.title)}
                    </h4>
                    <div class="grid gap-3">
                        <a class="rounded-lg bg-secondary px-4 py-3 text-left text-sm font-semibold text-secondary-foreground transition hover:bg-secondary/80" href="/security">
                            {move || t_string!(i18n, app.dashboard.quick.security)}
                        </a>
                        <a class="rounded-lg bg-secondary px-4 py-3 text-left text-sm font-semibold text-secondary-foreground transition hover:bg-secondary/80" href="/profile">
                            {move || t_string!(i18n, app.dashboard.quick.profile)}
                        </a>
                        <a class="rounded-lg bg-secondary px-4 py-3 text-left text-sm font-semibold text-secondary-foreground transition hover:bg-secondary/80" href="/users">
                            {move || t_string!(i18n, app.dashboard.quick.users)}
                        </a>
                    </div>
                </div>
            </div>

            <div class="mt-8 grid gap-6 lg:grid-cols-2">
                {move || module_sections.get().into_iter().map(|module| (module.render)()).collect_view()}
            </div>

        </section>
    }
}

fn format_time_ago(timestamp: &str) -> String {
    use chrono::{DateTime, Utc};

    let i18n = use_i18n();

    let Ok(dt) = timestamp.parse::<DateTime<Utc>>() else {
        return timestamp.to_string();
    };

    let now = Utc::now();
    let duration = now.signed_duration_since(dt);

    let minutes = duration.num_minutes();
    let hours = duration.num_hours();
    let days = duration.num_days();

    if minutes < 1 {
        t_string!(i18n, app.time.justNow).to_string()
    } else if minutes < 60 {
        format!("{} {}", minutes, t_string!(i18n, app.time.minutesAgo))
    } else if hours < 24 {
        format!("{}{}", hours, t_string!(i18n, app.time.hoursAgo))
    } else if days < 30 {
        format!("{}{}", days, t_string!(i18n, app.time.daysAgo))
    } else {
        dt.format("%d.%m.%Y").to_string()
    }
}

#[cfg(feature = "ssr")]
fn calculate_percent_change(current: i64, previous: i64) -> f64 {
    if previous == 0 {
        if current == 0 {
            0.0
        } else {
            100.0
        }
    } else {
        ((current - previous) as f64 / previous as f64) * 100.0
    }
}

#[cfg(feature = "ssr")]
async fn load_period_count_snapshot(
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

    let Some(row) = db.query_one(statement).await? else {
        return Ok(PeriodCountSnapshot::default());
    };

    Ok(PeriodCountSnapshot {
        total_count: row.try_get("", "total_count")?,
        current_count: row.try_get("", "current_count")?,
        previous_count: row.try_get("", "previous_count")?,
    })
}

#[cfg(feature = "ssr")]
async fn load_order_stats_snapshot(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    current_period_start: chrono::DateTime<Utc>,
    previous_period_start: chrono::DateTime<Utc>,
) -> std::result::Result<OrderStatsSnapshot, sea_orm::DbErr> {
    let backend = db.get_database_backend();
    let tenant_id = tenant_id.to_string();

    let statement = match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            backend,
            r#"
            SELECT
                CAST(COUNT(*) AS INTEGER) AS total_orders,
                CAST(COALESCE(SUM(COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0)), 0) AS INTEGER) AS total_revenue,
                CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_orders,
                CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_orders,
                CAST(COALESCE(SUM(CASE
                    WHEN created_at >= ?2 THEN COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0)
                    ELSE 0
                END), 0) AS INTEGER) AS current_revenue,
                CAST(COALESCE(SUM(CASE
                    WHEN created_at >= ?3 AND created_at < ?2 THEN COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0)
                    ELSE 0
                END), 0) AS INTEGER) AS previous_revenue
            FROM sys_events
            WHERE event_type = 'order.placed'
              AND (
                  json_extract(payload, '$.tenant_id') = ?1
                  OR json_extract(payload, '$.event.tenant_id') = ?1
              )
            "#,
            vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ],
        ),
        _ => Statement::from_sql_and_values(
            backend,
            r#"
            SELECT
                COUNT(*)::bigint AS total_orders,
                COALESCE(SUM(COALESCE((payload->'event'->'data'->>'total')::bigint, 0)), 0)::bigint AS total_revenue,
                COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_orders,
                COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_orders,
                COALESCE(SUM(CASE
                    WHEN created_at >= $2 THEN COALESCE((payload->'event'->'data'->>'total')::bigint, 0)
                    ELSE 0
                END), 0)::bigint AS current_revenue,
                COALESCE(SUM(CASE
                    WHEN created_at >= $3 AND created_at < $2 THEN COALESCE((payload->'event'->'data'->>'total')::bigint, 0)
                    ELSE 0
                END), 0)::bigint AS previous_revenue
            FROM sys_events
            WHERE event_type = 'order.placed'
              AND (
                  payload->>'tenant_id' = $1
                  OR payload->'event'->>'tenant_id' = $1
              )
            "#,
            vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ],
        ),
    };

    let Some(row) = db.query_one(statement).await? else {
        return Ok(OrderStatsSnapshot::default());
    };

    Ok(OrderStatsSnapshot {
        total_orders: row.try_get("", "total_orders")?,
        total_revenue: row.try_get("", "total_revenue")?,
        current_orders: row.try_get("", "current_orders")?,
        previous_orders: row.try_get("", "previous_orders")?,
        current_revenue: row.try_get("", "current_revenue")?,
        previous_revenue: row.try_get("", "previous_revenue")?,
    })
}

#[cfg(feature = "ssr")]
async fn load_recent_activity(
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
            ORDER BY created_at DESC
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
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            vec![tenant_id.into(), limit.into()],
        ),
    };

    let rows = db.query_all(statement).await?;
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

#[component]
fn ActivityIcon(activity_type: String) -> impl IntoView {
    let (icon, color_class) = match activity_type.as_str() {
        "user.created" | "user.joined" => (
            "M16 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2M8 7a4 4 0 1 0 0-8 4 4 0 0 0 0 8z",
            "text-green-500",
        ),
        "user.updated" | "user.changed" => (
            "M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z",
            "text-blue-500",
        ),
        "user.deleted" | "user.disabled" => (
            "M18 6L6 18M6 6l12 12",
            "text-red-500",
        ),
        "system.started" | "system.initialized" => (
            "M13 2L3 14h9l-1 8 10-12h-9l1-8z",
            "text-yellow-500",
        ),
        "tenant.checked" | "tenant.verified" => (
            "M9 12l2 2 4-4m6 2a9 9 0 1 1-18 0 9 9 0 0 1 18 0z",
            "text-purple-500",
        ),
        "security.login" | "security.auth" => (
            "M12 15v2m-6 4h12a2 2 0 0 0 2-2v-6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2zm10-10V7a4 4 0 0 0-8 0v4h8z",
            "text-violet-500",
        ),
        _ => (
            "M12 8v4l3 3m6-3a9 9 0 1 1-18 0 9 9 0 0 1 18 0z",
            "text-muted-foreground",
        ),
    };

    view! {
        <svg
            class=format!("h-4 w-4 shrink-0 {}", color_class)
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <path d=icon />
        </svg>
    }
}
