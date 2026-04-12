mod api;
mod i18n;
mod model;

use leptos::prelude::*;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{
    ForumCategoryListItem, ForumReplyDetail, ForumTopicDetail, ForumTopicListItem,
    StorefrontForumData,
};

#[component]
pub fn ForumView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_category_id = read_route_query_value(&route_context, "category");
    let selected_topic_id = read_route_query_value(&route_context, "topic");
    let locale = route_context.locale.clone();
    let badge_label = t(locale.as_deref(), "forum.badge", "forum");
    let title_label = t(
        locale.as_deref(),
        "forum.title",
        "Community threads from the module package",
    );
    let subtitle_label = t(
        locale.as_deref(),
        "forum.subtitle",
        "A NodeBB-inspired storefront surface that reads categories, topic feed, and thread replies through the forum module's public GraphQL contract.",
    );
    let load_error_label = t(
        locale.as_deref(),
        "forum.error.loadStorefront",
        "Failed to load forum storefront data",
    );

    let forum_resource = Resource::new_blocking(
        move || {
            (
                selected_category_id.clone(),
                selected_topic_id.clone(),
                locale.clone(),
            )
        },
        move |(category_id, topic_id, locale)| async move {
            api::fetch_storefront_forum(category_id, topic_id, locale).await
        },
    );

    view! {
        <section class="overflow-hidden rounded-[2rem] border border-border bg-gradient-to-br from-card via-card to-muted/35 p-8 shadow-sm">
            <div class="max-w-4xl space-y-3">
                <span class="inline-flex items-center gap-2 rounded-full border border-border bg-background/80 px-3 py-1 text-xs font-medium uppercase tracking-[0.22em] text-muted-foreground">
                    <span class="h-2 w-2 rounded-full bg-amber-500"></span>
                    {badge_label}
                </span>
                <h2 class="text-3xl font-semibold text-card-foreground">
                    {title_label}
                </h2>
                <p class="text-sm leading-6 text-muted-foreground">
                    {subtitle_label}
                </p>
            </div>

            <div class="mt-8">
                <Suspense fallback=|| view! {
                    <div class="grid gap-4 xl:grid-cols-[16rem_minmax(0,1fr)_24rem]">
                        <div class="h-80 animate-pulse rounded-[1.5rem] bg-muted"></div>
                        <div class="h-[32rem] animate-pulse rounded-[1.5rem] bg-muted"></div>
                        <div class="h-[32rem] animate-pulse rounded-[1.5rem] bg-muted"></div>
                    </div>
                }>
                    {move || {
                        let forum_resource = forum_resource.clone();
                        let load_error_label = load_error_label.clone();
                        Suspend::new(async move {
                            match forum_resource.await {
                                Ok(data) => view! { <ForumShowcase data /> }.into_any(),
                                Err(err) => view! {
                                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {format!("{}: {err}", load_error_label)}
                                    </div>
                                }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </div>
        </section>
    }
}

#[component]
fn ForumShowcase(data: StorefrontForumData) -> impl IntoView {
    let StorefrontForumData {
        categories,
        topics,
        selected_category_id,
        selected_topic_id,
        selected_topic,
        replies,
    } = data;

    view! {
        <div class="grid gap-6 xl:grid-cols-[16rem_minmax(0,1fr)_24rem]">
            <ForumCategoryRail
                items=categories.items
                total=categories.total
                selected_category_id=selected_category_id.clone()
            />
            <ForumTopicFeed
                items=topics.items
                total=topics.total
                selected_category_id=selected_category_id.clone()
                selected_topic_id=selected_topic_id
            />
            <ForumThreadPanel
                topic=selected_topic
                replies=replies.items
                replies_total=replies.total
            />
        </div>
    }
}

#[component]
fn ForumCategoryRail(
    items: Vec<ForumCategoryListItem>,
    total: u64,
    selected_category_id: Option<String>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let route_segment = route_context
        .route_segment
        .as_ref()
        .cloned()
        .unwrap_or_else(|| "forum".to_string());
    let module_route_base = route_context.module_route_base(route_segment.as_str());
    let categories_label = t(locale.as_deref(), "forum.categories.label", "Categories");
    let categories_title = t(locale.as_deref(), "forum.categories.title", "Community map");
    let categories_total_template = t(
        locale.as_deref(),
        "forum.categories.total",
        "{count} sections published from the forum module.",
    );
    let no_description_label = t(
        locale.as_deref(),
        "forum.categories.noDescription",
        "No description yet.",
    );

    view! {
        <aside class="space-y-4 rounded-[1.75rem] border border-border bg-card p-5 shadow-sm xl:sticky xl:top-6 xl:self-start">
            <div>
                <p class="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                    {categories_label}
                </p>
                <h3 class="mt-2 text-xl font-semibold text-card-foreground">{categories_title}</h3>
                <p class="mt-2 text-sm leading-6 text-muted-foreground">
                    {categories_total_template.replace("{count}", total.to_string().as_str())}
                </p>
            </div>

            <div class="space-y-2">
                {items.into_iter().map(|item| {
                    let href = category_href(module_route_base.as_str(), item.id.as_str());
                    let is_active = selected_category_id.as_deref() == Some(item.id.as_str());
                    let accent_style = item.color.as_deref()
                        .filter(|value| !value.trim().is_empty())
                        .map(|value| format!("background:{};", value))
                        .unwrap_or_else(|| "background:linear-gradient(180deg,#0ea5e9 0%,#f59e0b 100%);".to_string());
                    view! {
                        <a
                            class=move || format!(
                                "relative block overflow-hidden rounded-[1.35rem] border p-4 transition {}",
                                if is_active {
                                    "border-primary/40 bg-primary/5 shadow-sm"
                                } else {
                                    "border-border bg-background hover:border-primary/20 hover:bg-muted/40"
                                }
                            )
                            href=href
                        >
                            <span class="absolute inset-y-0 left-0 w-1.5" style=accent_style.clone()></span>
                            <div class="pl-3">
                                <div class="flex items-start justify-between gap-3">
                                    <div>
                                        <h4 class="text-sm font-semibold text-foreground">{item.name}</h4>
                                        <p class="mt-1 text-xs text-muted-foreground">{format!("#{}", item.slug)}</p>
                                    </div>
                                    <span class="rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                                        {item.topic_count}
                                    </span>
                                </div>
                                <p class="mt-3 line-clamp-3 text-sm text-muted-foreground">
                                    {item.description.unwrap_or_else(|| no_description_label.clone())}
                                </p>
                            </div>
                        </a>
                    }
                }).collect_view()}
            </div>
        </aside>
    }
}

#[component]
fn ForumTopicFeed(
    items: Vec<ForumTopicListItem>,
    total: u64,
    selected_category_id: Option<String>,
    selected_topic_id: Option<String>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let route_segment = route_context
        .route_segment
        .as_ref()
        .cloned()
        .unwrap_or_else(|| "forum".to_string());
    let module_route_base = route_context.module_route_base(route_segment.as_str());
    let empty_title = t(locale.as_deref(), "forum.feed.emptyTitle", "No topics yet");
    let empty_body = t(
        locale.as_deref(),
        "forum.feed.emptyBody",
        "Publish a topic from the forum admin package to light up this storefront feed.",
    );
    let feed_label = t(locale.as_deref(), "forum.feed.label", "Topic feed");
    let feed_title = t(locale.as_deref(), "forum.feed.title", "Latest discussions");
    let threads_template = t(locale.as_deref(), "forum.feed.threads", "{count} threads");
    let pinned_label = t(locale.as_deref(), "forum.topic.pinned", "Pinned");
    let locked_label = t(locale.as_deref(), "forum.topic.locked", "Locked");
    let slug_template = t(locale.as_deref(), "forum.topic.slug", "thread slug: {slug}");
    let replies_label = t(locale.as_deref(), "forum.topic.replies", "Replies");

    if items.is_empty() {
        return view! {
            <section class="rounded-[1.75rem] border border-dashed border-border p-8 text-center">
                <h3 class="text-lg font-semibold text-card-foreground">{empty_title}</h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {empty_body}
                </p>
            </section>
        }
        .into_any();
    }

    view! {
        <section class="space-y-4 rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
            <div class="flex flex-wrap items-center justify-between gap-3">
                <div>
                    <p class="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                        {feed_label}
                    </p>
                    <h3 class="mt-2 text-2xl font-semibold text-card-foreground">{feed_title}</h3>
                </div>
                <span class="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                    {threads_template.replace("{count}", total.to_string().as_str())}
                </span>
            </div>

            <div class="space-y-3">
                {items.into_iter().map(|item| {
                    let href = topic_href(
                        module_route_base.as_str(),
                        selected_category_id.as_deref(),
                        item.id.as_str(),
                    );
                    let is_active = selected_topic_id.as_deref() == Some(item.id.as_str());
                    let status_class = topic_status_class(item.status.as_str());
                    view! {
                        <a
                            class=move || format!(
                                "block rounded-[1.5rem] border p-5 transition {}",
                                if is_active {
                                    "border-primary/40 bg-primary/5 shadow-sm"
                                } else {
                                    "border-border bg-background hover:border-primary/25 hover:shadow-sm"
                                }
                            )
                            href=href
                        >
                            <div class="flex flex-wrap items-start justify-between gap-4">
                                <div class="space-y-3">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <span class=status_badge_class(status_class)>{item.status.clone()}</span>
                                        <span class="rounded-full border border-border px-2.5 py-1 text-[11px] font-medium text-muted-foreground">
                                            {item.effective_locale.clone()}
                                        </span>
                                        {item.is_pinned.then(|| view! {
                                            <span class="rounded-full bg-amber-500/15 px-2.5 py-1 text-[11px] font-medium text-amber-700 dark:text-amber-300">
                                                {pinned_label.clone()}
                                            </span>
                                        })}
                                        {item.is_locked.then(|| view! {
                                            <span class="rounded-full bg-destructive/10 px-2.5 py-1 text-[11px] font-medium text-destructive">
                                                {locked_label.clone()}
                                            </span>
                                        })}
                                    </div>
                                    <div>
                                        <h4 class="text-lg font-semibold text-foreground">{item.title}</h4>
                                        <p class="mt-1 text-sm text-muted-foreground">{slug_template.replace("{slug}", item.slug.as_str())}</p>
                                    </div>
                                </div>
                                <div class="text-right">
                                    <p class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                                        {replies_label.clone()}
                                    </p>
                                    <p class="mt-1 text-2xl font-semibold text-foreground">{item.reply_count}</p>
                                </div>
                            </div>
                        </a>
                    }
                }).collect_view()}
            </div>
        </section>
    }.into_any()
}

#[component]
fn ForumThreadPanel(
    topic: Option<ForumTopicDetail>,
    replies: Vec<ForumReplyDetail>,
    replies_total: u64,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let open_thread_title = t(locale.as_deref(), "forum.thread.openTitle", "Open a thread");
    let open_thread_body = t(
        locale.as_deref(),
        "forum.thread.openBody",
        "Pick a topic from the feed to read the opening post and latest replies.",
    );
    let Some(topic) = topic else {
        return view! {
            <aside class="rounded-[1.75rem] border border-dashed border-border p-8 text-center xl:sticky xl:top-6 xl:self-start">
                <h3 class="text-lg font-semibold text-card-foreground">{open_thread_title}</h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {open_thread_body}
                </p>
            </aside>
        }.into_any();
    };

    let status_class = topic_status_class(topic.status.as_str());
    let body = summarize_rich_content(
        topic.body.as_str(),
        topic.body_format.as_str(),
        locale.as_deref(),
    );
    let pinned_label = t(locale.as_deref(), "forum.topic.pinned", "Pinned");
    let locked_label = t(locale.as_deref(), "forum.topic.locked", "Locked");
    let slug_template = t(locale.as_deref(), "forum.thread.slug", "slug: {slug}");
    let replies_title = t(locale.as_deref(), "forum.thread.repliesTitle", "Replies");
    let replies_total_template = t(
        locale.as_deref(),
        "forum.thread.repliesTotal",
        "{count} total",
    );
    let no_replies_label = t(
        locale.as_deref(),
        "forum.thread.noReplies",
        "No replies yet.",
    );

    view! {
        <aside class="space-y-4 rounded-[1.75rem] border border-border bg-card p-6 shadow-sm xl:sticky xl:top-6 xl:self-start">
            <div class="space-y-3">
                <div class="flex flex-wrap items-center gap-2">
                    <span class=status_badge_class(status_class)>{topic.status.clone()}</span>
                    <span class="rounded-full border border-border px-2.5 py-1 text-[11px] font-medium text-muted-foreground">
                        {topic.effective_locale.clone()}
                    </span>
                    {topic.is_pinned.then(|| view! {
                        <span class="rounded-full bg-amber-500/15 px-2.5 py-1 text-[11px] font-medium text-amber-700 dark:text-amber-300">
                            {pinned_label}
                        </span>
                    })}
                    {topic.is_locked.then(|| view! {
                        <span class="rounded-full bg-destructive/10 px-2.5 py-1 text-[11px] font-medium text-destructive">
                            {locked_label}
                        </span>
                    })}
                </div>
                <div>
                    <h3 class="text-2xl font-semibold text-card-foreground">{topic.title}</h3>
                    <p class="mt-2 text-sm text-muted-foreground">{slug_template.replace("{slug}", topic.slug.as_str())}</p>
                </div>
                <p class="whitespace-pre-line text-sm leading-7 text-muted-foreground">{body}</p>
            </div>

            {if topic.tags.is_empty() {
                view! { <></> }.into_any()
            } else {
                view! {
                    <div class="flex flex-wrap gap-2">
                        {topic.tags.into_iter().map(|tag| view! {
                            <span class="rounded-full border border-border px-3 py-1 text-xs text-muted-foreground">
                                {tag}
                            </span>
                        }).collect_view()}
                    </div>
                }.into_any()
            }}

            <div class="rounded-[1.35rem] border border-border bg-background p-4">
                <div class="flex items-center justify-between gap-3">
                    <p class="text-sm font-semibold text-foreground">{replies_title}</p>
                    <span class="text-xs text-muted-foreground">{replies_total_template.replace("{count}", replies_total.to_string().as_str())}</span>
                </div>
                {if replies.is_empty() {
                    view! {
                        <p class="mt-3 text-sm text-muted-foreground">
                            {no_replies_label}
                        </p>
                    }.into_any()
                } else {
                    view! {
                        <div class="mt-4 space-y-3">
                            {replies.into_iter().map(|reply| view! { <ReplyCard reply /> }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </div>
        </aside>
    }.into_any()
}

#[component]
fn ReplyCard(reply: ForumReplyDetail) -> impl IntoView {
    let status_class = topic_status_class(reply.status.as_str());
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let content = summarize_rich_content(
        reply.content.as_str(),
        reply.content_format.as_str(),
        locale.as_deref(),
    );

    view! {
        <article class="rounded-[1.15rem] border border-border bg-card p-4">
            <div class="flex items-center justify-between gap-3">
                <span class=status_badge_class(status_class)>{reply.status}</span>
                <span class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                    {reply.effective_locale}
                </span>
            </div>
            <p class="mt-3 whitespace-pre-line text-sm leading-6 text-muted-foreground">{content}</p>
        </article>
    }
}

fn category_href(module_route_base: &str, category_id: &str) -> String {
    format!("{module_route_base}?category={category_id}")
}

fn topic_href(module_route_base: &str, category_id: Option<&str>, topic_id: &str) -> String {
    match category_id {
        Some(category_id) if !category_id.is_empty() => {
            format!("{module_route_base}?category={category_id}&topic={topic_id}")
        }
        _ => format!("{module_route_base}?topic={topic_id}"),
    }
}

fn summarize_rich_content(content: &str, format: &str, locale: Option<&str>) -> String {
    if format.eq_ignore_ascii_case("markdown") {
        return content.trim().to_string();
    }

    let template = t(
        locale,
        "forum.richContent.summary",
        "Stored in `{format}` format. Raw content length: {count} characters.",
    );
    template
        .replace("{format}", format)
        .replace("{count}", content.chars().count().to_string().as_str())
}

fn topic_status_class(status: &str) -> &'static str {
    match status.to_ascii_lowercase().as_str() {
        "published" | "active" | "open" | "approved" => "success",
        "draft" | "pending" => "warning",
        "archived" | "closed" | "hidden" => "muted",
        _ => "default",
    }
}

fn status_badge_class(status_class: &'static str) -> &'static str {
    match status_class {
        "success" => {
            "rounded-full bg-emerald-500/15 px-2.5 py-1 text-[11px] font-medium text-emerald-700 dark:text-emerald-300"
        }
        "warning" => {
            "rounded-full bg-amber-500/15 px-2.5 py-1 text-[11px] font-medium text-amber-700 dark:text-amber-300"
        }
        "muted" => {
            "rounded-full bg-muted px-2.5 py-1 text-[11px] font-medium text-muted-foreground"
        }
        _ => {
            "rounded-full border border-border px-2.5 py-1 text-[11px] font-medium text-muted-foreground"
        }
    }
}
