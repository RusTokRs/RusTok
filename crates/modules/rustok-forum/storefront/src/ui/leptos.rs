use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui::RichTextHtml;
use leptos_ui_routing::read_route_query_value;
use rustok_api::normalize_locale_tag;
use rustok_ui_core::UiRouteContext;

use super::member_card::{ForumAuthorBadge, member_card_context};
use crate::core::{
    ForumStorefrontCategoryRailLabels, forum_storefront_category_card_view_model,
    forum_storefront_count_label, forum_storefront_status_badge_class,
    forum_storefront_topic_card_view_model, topic_status_class,
};
use crate::i18n::t;
use crate::model::{
    ForumCategoryListItem, ForumReplyDetail, ForumTopicDetail, ForumTopicListItem,
    StorefrontForumData,
};
use crate::transport;

fn forum_storefront_content_lang(locale: &str) -> String {
    normalize_locale_tag(locale).unwrap_or_else(|| "und".to_string())
}

#[component]
pub fn ForumView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_category_id = read_route_query_value(&route_context, "category");
    let selected_topic_id = read_route_query_value(&route_context, "topic");
    let locale = route_context.locale.clone();
    let mutation_locale = route_context.locale.clone();
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
    let mutation_error_label = t(
        locale.as_deref(),
        "forum.error.updateReadState",
        "Failed to update forum read state",
    );

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (mutation_busy, set_mutation_busy) = signal(false);
    let (mutation_error, set_mutation_error) = signal(Option::<transport::TransportError>::None);

    let forum_resource = Resource::new_blocking(
        move || {
            (
                selected_category_id.clone(),
                selected_topic_id.clone(),
                locale.clone(),
                refresh_nonce.get(),
            )
        },
        move |(category_id, topic_id, locale, _)| async move {
            transport::fetch_storefront_forum(category_id, topic_id, locale).await
        },
    );

    let on_mark_topic_read = Callback::new(move |topic_id: String| {
        let locale = mutation_locale.clone();
        set_mutation_busy.set(true);
        set_mutation_error.set(None);
        spawn_local(async move {
            match transport::mark_storefront_topic_read(topic_id, locale).await {
                Ok(()) => set_refresh_nonce.update(|value| *value += 1),
                Err(error) => set_mutation_error.set(Some(error)),
            }
            set_mutation_busy.set(false);
        });
    });

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

            <div class="mt-8 space-y-4">
                {move || mutation_error.get().map(|error| view! {
                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                        {format!("{}: {error}", mutation_error_label)}
                    </div>
                })}
                <Suspense fallback=|| view! {
                    <div class="grid gap-4 xl:grid-cols-[16rem_minmax(0,1fr)_24rem]">
                        <div class="h-80 animate-pulse rounded-[1.5rem] bg-muted"></div>
                        <div class="h-[32rem] animate-pulse rounded-[1.5rem] bg-muted"></div>
                        <div class="h-[32rem] animate-pulse rounded-[1.5rem] bg-muted"></div>
                    </div>
                }>
                    {move || {
                        let forum_resource = forum_resource;
                        let load_error_label = load_error_label.clone();
                        let on_mark_topic_read = on_mark_topic_read;
                        Suspend::new(async move {
                            match forum_resource.await {
                                Ok(data) => view! {
                                    <ForumShowcase
                                        data
                                        on_mark_topic_read
                                        mutation_busy
                                    />
                                }.into_any(),
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
fn ForumShowcase(
    data: StorefrontForumData,
    on_mark_topic_read: Callback<String>,
    mutation_busy: ReadSignal<bool>,
) -> impl IntoView {
    let StorefrontForumData {
        categories,
        topics,
        selected_category_id,
        selected_topic_id,
        selected_topic,
        replies,
        member_cards,
        read_state_available,
    } = data;
    provide_context(member_card_context(member_cards));

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
                read_state_available
                on_mark_topic_read
                mutation_busy
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
    let ui_content_lang = forum_storefront_content_lang(locale.as_deref().unwrap_or_default());

    view! {
        <aside class="space-y-4 rounded-[1.75rem] border border-border bg-card p-5 shadow-sm xl:sticky xl:top-6 xl:self-start">
            <div>
                <p class="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                    {categories_label}
                </p>
                <h3 class="mt-2 text-xl font-semibold text-card-foreground">{categories_title}</h3>
                <p class="mt-2 text-sm leading-6 text-muted-foreground">
                    {forum_storefront_count_label(categories_total_template.as_str(), total)}
                </p>
            </div>

            <div class="space-y-2">
                {items.into_iter().map(|item| {
                    let labels = ForumStorefrontCategoryRailLabels {
                        no_description: no_description_label.clone(),
                        total_template: categories_total_template.clone(),
                    };
                    let content_lang = forum_storefront_content_lang(item.effective_locale.as_str());
                    let description_lang = if item
                        .description
                        .as_deref()
                        .map(str::trim)
                        .is_some_and(|value| !value.is_empty())
                    {
                        content_lang.clone()
                    } else {
                        ui_content_lang.clone()
                    };
                    let card = forum_storefront_category_card_view_model(
                        module_route_base.as_str(),
                        &item,
                        selected_category_id.as_deref(),
                        &labels,
                    );
                    view! {
                        <a
                            class=format!(
                                "relative block overflow-hidden rounded-[1.35rem] border p-4 transition {}",
                                card.container_class
                            )
                            href=card.href
                        >
                            <span class=format!("absolute inset-y-0 left-0 w-1.5 {}", card.accent_class)></span>
                            <div class="pl-3">
                                <div class="flex items-start justify-between gap-3">
                                    <div>
                                        <h4
                                            data-forum-target-localized=""
                                            lang=content_lang
                                            dir="auto"
                                            class="text-sm font-semibold text-foreground"
                                        >{card.name}</h4>
                                        <p
                                            data-forum-route-identifier=""
                                            dir="ltr"
                                            class="mt-1 text-xs text-muted-foreground"
                                        >{card.slug_badge}</p>
                                    </div>
                                    <span class="rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                                        {card.topic_count}
                                    </span>
                                </div>
                                <p
                                    data-forum-target-localized=""
                                    lang=description_lang
                                    dir="auto"
                                    class="mt-3 line-clamp-3 text-sm text-muted-foreground"
                                >
                                    {card.description}
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
    let unread_template = t(
        locale.as_deref(),
        "forum.topic.unreadCount",
        "{count} unread",
    );
    let updated_unread_label = t(locale.as_deref(), "forum.topic.updatedUnread", "Updated");
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
                    {forum_storefront_count_label(threads_template.as_str(), total)}
                </span>
            </div>

            <div class="space-y-3">
                {items.into_iter().map(|item| {
                    let author_id = item.author_id.clone();
                    let card = forum_storefront_topic_card_view_model(
                        module_route_base.as_str(),
                        &item,
                        selected_category_id.as_deref(),
                        selected_topic_id.as_deref(),
                        slug_template.as_str(),
                    );
                    let content_lang = forum_storefront_content_lang(card.effective_locale.as_str());
                    let unread_label = if card.unread_count > 0 {
                        forum_storefront_count_label(unread_template.as_str(), card.unread_count)
                    } else {
                        updated_unread_label.clone()
                    };
                    view! {
                        <a
                            class=format!(
                                "block rounded-[1.5rem] border p-5 transition {}",
                                card.container_class
                            )
                            href=card.href
                        >
                            <div class="flex flex-wrap items-start justify-between gap-4">
                                <div class="space-y-3">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <span class=card.status_badge_class>{card.status.clone()}</span>
                                        <span dir="ltr" class="rounded-full border border-border px-2.5 py-1 text-[11px] font-medium text-muted-foreground">
                                            {card.effective_locale.clone()}
                                        </span>
                                        {card.is_unread.then(|| view! {
                                            <span class=card.unread_badge_class>{unread_label}</span>
                                        })}
                                        {card.is_pinned.then(|| view! {
                                            <span class="rounded-full bg-amber-500/15 px-2.5 py-1 text-[11px] font-medium text-amber-700 dark:text-amber-300">
                                                {pinned_label.clone()}
                                            </span>
                                        })}
                                        {card.is_locked.then(|| view! {
                                            <span class="rounded-full bg-destructive/10 px-2.5 py-1 text-[11px] font-medium text-destructive">
                                                {locked_label.clone()}
                                            </span>
                                        })}
                                    </div>
                                    <div>
                                        <h4
                                            data-forum-target-localized=""
                                            lang=content_lang
                                            dir="auto"
                                            class="text-lg font-semibold text-foreground"
                                        >{card.title}</h4>
                                        <p
                                            data-forum-route-identifier=""
                                            dir="ltr"
                                            class="mt-1 text-sm text-muted-foreground"
                                        >{card.slug_label}</p>
                                    </div>
                                    <ForumAuthorBadge author_id />
                                </div>
                                <div class="text-right">
                                    <p class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                                        {replies_label.clone()}
                                    </p>
                                    <p class="mt-1 text-2xl font-semibold text-foreground">{card.reply_count}</p>
                                </div>
                            </div>
                        </a>
                    }
                    .into_any()
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
    read_state_available: bool,
    on_mark_topic_read: Callback<String>,
    mutation_busy: ReadSignal<bool>,
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

    let topic_id = topic.id.clone();
    let author_id = topic.author_id.clone();
    let status_class = topic_status_class(topic.status.as_str());
    let body = topic.body.clone();
    let body_locale = topic.effective_locale.clone();
    let content_lang = forum_storefront_content_lang(topic.effective_locale.as_str());
    let pinned_label = t(locale.as_deref(), "forum.topic.pinned", "Pinned");
    let locked_label = t(locale.as_deref(), "forum.topic.locked", "Locked");
    let mark_read_label = t(
        locale.as_deref(),
        "forum.thread.markRead",
        "Mark topic read",
    );
    let marking_read_label = t(
        locale.as_deref(),
        "forum.thread.markingRead",
        "Marking read…",
    );
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
                    <span class=forum_storefront_status_badge_class(status_class)>{topic.status.clone()}</span>
                    <span dir="ltr" class="rounded-full border border-border px-2.5 py-1 text-[11px] font-medium text-muted-foreground">
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
                    <h3
                        data-forum-target-localized=""
                        lang=content_lang.clone()
                        dir="auto"
                        class="text-2xl font-semibold text-card-foreground"
                    >{topic.title}</h3>
                    <p
                        data-forum-route-identifier=""
                        dir="ltr"
                        class="mt-2 text-sm text-muted-foreground"
                    >{crate::core::forum_storefront_slug_label(slug_template.as_str(), topic.slug.as_str())}</p>
                </div>
                <ForumAuthorBadge author_id />
                <RichTextHtml
                    view=body
                    content_locale=body_locale
                    class="richtext text-sm leading-7 text-muted-foreground"
                />
                {read_state_available.then(|| {
                    let topic_id = topic_id.clone();
                    view! {
                        <button
                            type="button"
                            class="inline-flex items-center justify-center rounded-xl border border-primary/30 bg-primary/5 px-4 py-2 text-sm font-semibold text-primary transition hover:bg-primary/10 disabled:cursor-not-allowed disabled:opacity-60"
                            disabled=move || mutation_busy.get()
                            on:click=move |_| on_mark_topic_read.run(topic_id.clone())
                        >
                            {move || if mutation_busy.get() {
                                marking_read_label.clone()
                            } else {
                                mark_read_label.clone()
                            }}
                        </button>
                    }
                })}
            </div>

            {if topic.tags.is_empty() {
                view! { <span class="hidden"></span> }.into_any()
            } else {
                view! {
                    <div class="flex flex-wrap gap-2">
                        {topic.tags.into_iter().map(|tag| view! {
                            <span
                                data-forum-target-localized=""
                                lang=content_lang.clone()
                                dir="auto"
                                class="rounded-full border border-border px-3 py-1 text-xs text-muted-foreground"
                            >
                                {tag}
                            </span>
                        }).collect_view()}
                    </div>
                }.into_any()
            }}

            <div class="rounded-[1.35rem] border border-border bg-background p-4">
                <div class="flex items-center justify-between gap-3">
                    <p class="text-sm font-semibold text-foreground">{replies_title}</p>
                    <span class="text-xs text-muted-foreground">{forum_storefront_count_label(replies_total_template.as_str(), replies_total)}</span>
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
    let author_id = reply.author_id.clone();
    let status_class = topic_status_class(reply.status.as_str());
    let content = reply.content.clone();
    let content_locale = reply.effective_locale.clone();

    view! {
        <article class="rounded-[1.15rem] border border-border bg-card p-4">
            <div class="flex items-center justify-between gap-3">
                <span class=forum_storefront_status_badge_class(status_class)>{reply.status}</span>
                <span dir="ltr" class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                    {reply.effective_locale}
                </span>
            </div>
            <div class="mt-3">
                <ForumAuthorBadge author_id />
            </div>
            <RichTextHtml
                view=content
                content_locale=content_locale
                class="richtext mt-3 text-sm leading-6 text-muted-foreground"
            />
        </article>
    }
}
