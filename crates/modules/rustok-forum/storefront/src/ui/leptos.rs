use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui::RichTextHtml;
use leptos_ui_routing::read_route_query_value;
use rustok_api::normalize_locale_tag;
use rustok_ui_core::UiRouteContext;

use super::category_overview::CategoryOverview;
use super::composer::{ComposerQuote, ComposerSignal, ComposerState, ForumComposer};
use super::member_card::{ForumAuthorBadge, member_card_context};
use super::timeline::TimelineScroller;
use super::topic_feed::ForumTopicFeed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForumViewMode {
    Topics,
    Categories,
}
use crate::core::{
    ForumStorefrontCategoryRailLabels, forum_storefront_category_card_view_model,
    forum_storefront_count_label, forum_storefront_status_badge_class, topic_status_class,
};
use crate::i18n::t;
use crate::model::{
    ForumCategoryListItem, ForumReplyDetail, ForumTopicDetail, StorefrontForumData,
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
    let composer_signal: ComposerSignal = RwSignal::new(ComposerState::default());
    provide_context(composer_signal);

    let composer_categories = categories.items.clone();
    let overview_categories = categories.items.clone();
    let overview_total = categories.total;
    let (view_mode, set_view_mode) = signal(ForumViewMode::Topics);

    view! {
        <div class="space-y-6">
            // View Mode Switcher
            <div class="flex items-center justify-end">
                <div class="flex items-center rounded-2xl border border-border bg-background p-1 shadow-xs">
                    <button
                        type="button"
                        class=move || format!(
                            "flex items-center gap-2 rounded-xl px-4 py-2 text-xs font-semibold transition {}",
                            if view_mode.get() == ForumViewMode::Topics {
                                "bg-primary text-primary-foreground shadow-xs"
                            } else {
                                "text-muted-foreground hover:text-foreground"
                            }
                        )
                        on:click=move |_| set_view_mode.set(ForumViewMode::Topics)
                    >
                        <span>"💬"</span>
                        <span>"Discussions"</span>
                    </button>
                    <button
                        type="button"
                        class=move || format!(
                            "flex items-center gap-2 rounded-xl px-4 py-2 text-xs font-semibold transition {}",
                            if view_mode.get() == ForumViewMode::Categories {
                                "bg-primary text-primary-foreground shadow-xs"
                            } else {
                                "text-muted-foreground hover:text-foreground"
                            }
                        )
                        on:click=move |_| set_view_mode.set(ForumViewMode::Categories)
                    >
                        <span>"📁"</span>
                        <span>"Categories"</span>
                    </button>
                </div>
            </div>

            {move || match view_mode.get() {
                ForumViewMode::Categories => {
                    let overview_categories = overview_categories.clone();
                    view! {
                        <CategoryOverview
                            items=overview_categories
                            total=overview_total
                            on_select_category=Callback::new(move |_category_id: String| {
                                set_view_mode.set(ForumViewMode::Topics);
                            })
                        />
                    }.into_any()
                }
                ForumViewMode::Topics => {
                    view! {
                        <div class="grid gap-6 xl:grid-cols-[16rem_minmax(0,1fr)_24rem]">
                            <ForumCategoryRail
                                items=categories.items.clone()
                                total=categories.total
                                selected_category_id=selected_category_id.clone()
                                on_switch_to_overview=Callback::new(move |_| set_view_mode.set(ForumViewMode::Categories))
                            />
                            <ForumTopicFeed
                                items=topics.items.clone()
                                total=topics.total
                                selected_category_id=selected_category_id.clone()
                                selected_topic_id=selected_topic_id.clone()
                            />
                            <ForumThreadPanel
                                topic=selected_topic.clone()
                                replies=replies.items.clone()
                                replies_total=replies.total
                                read_state_available
                                on_mark_topic_read
                                mutation_busy
                            />
                        </div>
                    }.into_any()
                }
            }}
        </div>

        <ForumComposer categories=composer_categories />
    }
}

#[component]
fn ForumCategoryRail(
    items: Vec<ForumCategoryListItem>,
    total: u64,
    selected_category_id: Option<String>,
    on_switch_to_overview: Callback<()>,
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
            <div class="flex items-start justify-between gap-2">
                <div>
                    <p class="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                        {categories_label}
                    </p>
                    <h3 class="mt-2 text-xl font-semibold text-card-foreground">{categories_title}</h3>
                    <p class="mt-2 text-sm leading-6 text-muted-foreground">
                        {forum_storefront_count_label(categories_total_template.as_str(), total)}
                    </p>
                </div>
                <button
                    type="button"
                    title="Open category matrix with subcategories"
                    class="shrink-0 rounded-full border border-border bg-background p-2 text-xs font-medium text-muted-foreground transition hover:border-primary hover:text-foreground"
                    on:click=move |_| on_switch_to_overview.run(())
                >
                    "Grid ⊞"
                </button>
            </div>

            <div class="space-y-2">
                {items.into_iter().map(|item| {
                    let is_subcategory = item.parent_id.is_some();
                    let subcategory_indent = if is_subcategory { "ml-3 w-[calc(100%-0.75rem)] border-dashed" } else { "" };
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
                                "relative block overflow-hidden rounded-[1.35rem] border p-4 transition {} {}",
                                card.container_class,
                                subcategory_indent
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
                                            class="flex items-center gap-1.5 text-sm font-semibold text-foreground"
                                        >
                                            {is_subcategory.then(|| view! { <span class="text-xs text-muted-foreground">"↳"</span> })}
                                            <span>{card.name}</span>
                                        </h4>
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
    let topic_title = topic.title.clone();
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
        <div class="relative flex gap-3 xl:sticky xl:top-6 xl:self-start">
            <aside class="flex-1 space-y-4 rounded-[1.75rem] border border-border bg-card p-6 shadow-sm overflow-hidden">
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
                <div class="flex flex-wrap items-center gap-2">
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
                    {
                        let topic_id = topic_id.clone();
                        let topic_title = topic_title.clone();
                        view! {
                            <button
                                type="button"
                                class="inline-flex items-center justify-center rounded-xl bg-primary px-4 py-2 text-sm font-semibold text-primary-foreground shadow-xs transition hover:bg-primary/90"
                                on:click=move |_| {
                                    if let Some(composer) = use_context::<ComposerSignal>() {
                                        composer.update(|c| c.open_reply(topic_id.clone(), topic_title.clone(), None));
                                    }
                                }
                            >
                                "Reply"
                            </button>
                        }
                    }
                </div>
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

        <TimelineScroller current_post=1 total_posts=replies_total + 1 />
    </div>
    }.into_any()
}

#[component]
fn ReplyCard(reply: ForumReplyDetail) -> impl IntoView {
    let author_id = reply.author_id.clone();
    let status_class = topic_status_class(reply.status.as_str());
    let content = reply.content.clone();
    let content_locale = reply.effective_locale.clone();
    let reply_id = reply.id.clone();

    view! {
        <article class="rounded-[1.15rem] border border-border bg-card p-4">
            <div class="flex items-center justify-between gap-3">
                <span class=forum_storefront_status_badge_class(status_class)>{reply.status}</span>
                <div class="flex items-center gap-2">
                    <span dir="ltr" class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                        {reply.effective_locale}
                    </span>
                    <button
                        type="button"
                        class="rounded-lg border border-border px-2 py-0.5 text-xs text-muted-foreground transition hover:bg-muted hover:text-foreground"
                        title="Quote / Reply"
                        on:click={
                            let reply_id = reply_id.clone();
                            move |_| {
                                if let Some(composer) = use_context::<ComposerSignal>() {
                                    composer.update(|c| {
                                        c.append_quote(
                                            ComposerQuote {
                                                target_kind: "REPLY".to_string(),
                                                target_id: reply_id.clone(),
                                                revision_id: 1,
                                                author_handle: None,
                                                snippet: None,
                                            },
                                            None,
                                        );
                                    });
                                }
                            }
                        }
                    >
                        "Quote"
                    </button>
                </div>
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
