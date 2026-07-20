use crate::features::dashboard::transport;
use leptos::prelude::*;
use leptos_auth::hooks::{use_current_user, use_tenant, use_token};

use crate::app::modules::{AdminSlot, components_for_slot};
use crate::app::providers::enabled_modules::use_enabled_modules;
use crate::shared::ui::{
    Badge, BadgeVariant, Card, CardContent, CardDescription, CardHeader, CardTitle, PageHeader,
};
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

#[component]
pub fn Dashboard() -> impl IntoView {
    let i18n = use_i18n();
    let current_user = use_current_user();
    let token = use_token();
    let tenant = use_tenant();

    let dashboard_stats = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            transport::fetch_dashboard_stats(token_value, tenant_value).await
        },
    );

    let recent_activity = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            transport::fetch_recent_activity(token_value, tenant_value, 10).await
        },
    );

    let enabled_modules = use_enabled_modules();

    let module_sections = Signal::derive(move || {
        let enabled = enabled_modules.get();
        components_for_slot(AdminSlot::DashboardSection, Some(&enabled))
    });

    view! {
        <section class="flex flex-1 flex-col p-4 md:px-6">
            <PageHeader
                title=move || {
                    current_user
                        .get()
                        .and_then(|user| user.name)
                        .unwrap_or_else(|| "Dashboard".to_string())
                }
                eyebrow=move || t_string!(i18n, app.nav.dashboard).to_string()
                subtitle=move || t_string!(i18n, app.dashboard.subtitle).to_string()
            />

            <div class="flex flex-1 flex-col gap-6">
            <Suspense
                fallback=move || view! {
                    <div class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
                        {(0..4)
                            .map(|_| {
                                view! { <div class="h-36 animate-pulse rounded-xl bg-muted"></div> }
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
                                    stats.users_change >= 0.0,
                                ),
                                (
                                    t_string!(i18n, app.dashboard.stats.posts),
                                    stats.total_posts.to_string(),
                                    format!("{:+.1}%", stats.posts_change),
                                    stats.posts_change >= 0.0,
                                ),
                                (
                                    t_string!(i18n, app.dashboard.stats.orders),
                                    stats.total_orders.to_string(),
                                    format!("{:+.1}%", stats.orders_change),
                                    stats.orders_change >= 0.0,
                                ),
                                (
                                    t_string!(i18n, app.dashboard.stats.revenue),
                                    format!("${}", stats.total_revenue),
                                    format!("{:+.1}%", stats.revenue_change),
                                    stats.revenue_change >= 0.0,
                                ),
                            ]
                        })
                        .unwrap_or_else(|| {
                            vec![
                                (t_string!(i18n, app.dashboard.stats.users), "-".to_string(), "0.0%".to_string(), true),
                                (t_string!(i18n, app.dashboard.stats.posts), "-".to_string(), "0.0%".to_string(), true),
                                (t_string!(i18n, app.dashboard.stats.orders), "-".to_string(), "0.0%".to_string(), true),
                                (t_string!(i18n, app.dashboard.stats.revenue), "-".to_string(), "0.0%".to_string(), true),
                            ]
                        });

                    view! {
                        <div class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
                            {stats
                                .into_iter()
                                .map(|(title, value, hint, trend_up)| {
                                    view! {
                                        <StatsCard
                                            title=title
                                            value=value
                                            icon=view! { <span class="size-5 text-center text-base leading-5">"вЂў"</span> }.into_any()
                                            trend=hint
                                            trend_label=t_string!(i18n, app.dashboard.stats.vsLastMonth)
                                            trend_up=trend_up
                                        />
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                }}
            </Suspense>

            <div class="grid grid-cols-1 gap-4">
                <Card>
                    <CardHeader>
                        <CardTitle>{move || t_string!(i18n, app.dashboard.activity.title)}</CardTitle>
                        <CardDescription>{move || t_string!(i18n, app.dashboard.subtitle).to_string()}</CardDescription>
                    </CardHeader>
                    <CardContent>
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
                                    <div class="rounded-lg border border-dashed p-6 text-sm text-muted-foreground">
                                        {t_string!(i18n, app.dashboard.activity.empty)}
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="divide-y divide-border">
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
                                                <div class="flex items-start justify-between gap-4 py-3 first:pt-0 last:pb-0">
                                                    <div class="min-w-0">
                                                        <div class="flex items-center gap-2">
                                                            <Badge variant=BadgeVariant::Secondary>{item.r#type.clone()}</Badge>
                                                            <span class="truncate font-medium text-foreground">{item.description}</span>
                                                        </div>
                                                        <p class="mt-1 text-sm text-muted-foreground">
                                                            {format!("by {}", user_name)}
                                                        </p>
                                                    </div>
                                                    <span class="shrink-0 text-xs text-muted-foreground">
                                                        {time_ago}
                                                    </span>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                    </div>
                                }.into_any()
                            }
                        }}
                    </Suspense>
                    </CardContent>
                </Card>
            </div>

            <div class="grid gap-4 lg:grid-cols-2">
                {move || module_sections.get().into_iter().map(|module| (module.render)()).collect_view()}
            </div>
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
