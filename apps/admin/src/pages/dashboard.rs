use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_auth, use_current_user, use_tenant, use_token};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::app::modules::{components_for_slot, AdminSlot};
use crate::shared::api::queries::{DASHBOARD_STATS_QUERY, RECENT_ACTIVITY_QUERY};
use crate::shared::api::request;
use crate::shared::ui::{Button, LanguageToggle, PageHeader};
use crate::widgets::stats_card::StatsCard;
use crate::{t_string, use_i18n};

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

#[component]
pub fn Dashboard() -> impl IntoView {
    let i18n = use_i18n();
    let auth = use_auth();
    let current_user = use_current_user();
    let token = use_token();
    let tenant = use_tenant();

    let dashboard_stats = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            request::<_, DashboardStatsResponse>(
                DASHBOARD_STATS_QUERY,
                json!({}),
                token_value.clone(),
                tenant_value.clone(),
            )
            .await
        },
    );

    let recent_activity = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            request::<_, RecentActivityResponse>(
                RECENT_ACTIVITY_QUERY,
                json!({"limit": 10}),
                token_value,
                tenant_value,
            )
            .await
        },
    );

    let logout = move |_| {
        let auth = auth.clone();
        spawn_local(async move {
            let _ = auth.sign_out().await;
        });
    };

    let title = current_user
        .get()
        .and_then(|user| user.name)
        .unwrap_or_else(|| "Dashboard".to_string());

    let module_sections = components_for_slot(AdminSlot::DashboardSection);

    view! {
        <section class="px-10 py-8">
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
                {module_sections
                    .into_iter()
                    .map(|module| (module.render)())
                    .collect_view()}
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
