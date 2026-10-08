use leptos::prelude::*;
use rustok_api::normalize_locale_tag;
use rustok_ui_core::UiRouteContext;

use super::composer::ComposerSignal;
use super::member_card::ForumAuthorBadge;
use crate::core::{forum_storefront_count_label, forum_storefront_topic_card_view_model};
use crate::i18n::t;
use crate::model::ForumTopicListItem;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopicFilterMode {
    Latest,
    Top,
    Unread,
    Solved,
}

fn forum_storefront_content_lang(locale: &str) -> String {
    normalize_locale_tag(locale).unwrap_or_else(|| "en".to_string())
}

#[component]
pub fn ForumTopicFeed(
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

    let (filter_mode, set_filter_mode) = signal(TopicFilterMode::Latest);
    let (search_query, set_search_query) = signal(String::new());

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
                <p class="mt-2 text-sm text-muted-foreground">{empty_body}</p>
                <button
                    type="button"
                    class="mt-4 inline-flex items-center gap-1.5 rounded-xl border border-primary/30 bg-primary/5 px-4 py-2 text-sm font-semibold text-primary transition hover:bg-primary/10"
                    on:click={
                        let selected_category_id = selected_category_id.clone();
                        move |_| {
                            if let Some(composer) = use_context::<ComposerSignal>() {
                                composer.update(|c| c.open_topic(selected_category_id.clone()));
                            }
                        }
                    }
                >
                    "+ Start Discussion"
                </button>
            </section>
        }
        .into_any();
    }

    let all_items = items;
    let unread_total = all_items
        .iter()
        .filter(|t| t.is_unread.unwrap_or(false) || t.unread_count.unwrap_or(0) > 0)
        .count();
    let solved_total = all_items
        .iter()
        .filter(|t| t.solution_reply_id.is_some())
        .count();

    let filtered_items = {
        let all_items = all_items.clone();
        move || {
            let q = search_query.get().trim().to_lowercase();
            let mode = filter_mode.get();
            let mut list = all_items.clone();
            if !q.is_empty() {
                list.retain(|t| {
                    t.title.to_lowercase().contains(&q) || t.slug.to_lowercase().contains(&q)
                });
            }
            match mode {
                TopicFilterMode::Latest => {
                    list.sort_by(|a, b| match (a.is_pinned, b.is_pinned) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => std::cmp::Ordering::Equal,
                    });
                }
                TopicFilterMode::Top => {
                    list.sort_by(|a, b| b.reply_count.cmp(&a.reply_count));
                }
                TopicFilterMode::Unread => {
                    list.retain(|t| {
                        t.is_unread.unwrap_or(false) || t.unread_count.unwrap_or(0) > 0
                    });
                }
                TopicFilterMode::Solved => {
                    list.retain(|t| t.solution_reply_id.is_some());
                }
            }
            list
        }
    };

    view! {
        <section class="space-y-4 rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
            // Header
            <div class="flex flex-wrap items-center justify-between gap-3">
                <div>
                    <p class="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                        {feed_label}
                    </p>
                    <h3 class="mt-2 text-2xl font-semibold text-card-foreground">{feed_title}</h3>
                </div>
                <div class="flex items-center gap-2">
                    <span class="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        {forum_storefront_count_label(threads_template.as_str(), total)}
                    </span>
                    <button
                        type="button"
                        class="inline-flex items-center gap-1.5 rounded-full bg-primary px-3.5 py-1 text-xs font-semibold text-primary-foreground shadow-xs transition hover:bg-primary/90"
                        on:click={
                            let selected_category_id = selected_category_id.clone();
                            move |_| {
                                if let Some(composer) = use_context::<ComposerSignal>() {
                                    composer.update(|c| c.open_topic(selected_category_id.clone()));
                                }
                            }
                        }
                    >
                        "+ New Topic"
                    </button>
                </div>
            </div>

            // Discourse / NodeBB Filter Tabs & Real-Time Search Box
            <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <div class="flex flex-wrap items-center gap-1 rounded-2xl border border-border bg-muted/30 p-1">
                    <button
                        type="button"
                        class=move || format!(
                            "inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition {}",
                            if filter_mode.get() == TopicFilterMode::Latest {
                                "bg-background text-foreground shadow-xs"
                            } else {
                                "text-muted-foreground hover:text-foreground"
                            }
                        )
                        on:click=move |_| set_filter_mode.set(TopicFilterMode::Latest)
                    >
                        <span>"✨"</span>
                        <span>"Latest"</span>
                    </button>

                    <button
                        type="button"
                        class=move || format!(
                            "inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition {}",
                            if filter_mode.get() == TopicFilterMode::Top {
                                "bg-background text-foreground shadow-xs"
                            } else {
                                "text-muted-foreground hover:text-foreground"
                            }
                        )
                        on:click=move |_| set_filter_mode.set(TopicFilterMode::Top)
                    >
                        <span>"🔥"</span>
                        <span>"Top"</span>
                    </button>

                    <button
                        type="button"
                        class=move || format!(
                            "inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition {}",
                            if filter_mode.get() == TopicFilterMode::Unread {
                                "bg-background text-foreground shadow-xs"
                            } else {
                                "text-muted-foreground hover:text-foreground"
                            }
                        )
                        on:click=move |_| set_filter_mode.set(TopicFilterMode::Unread)
                    >
                        <span>"💬"</span>
                        <span>"Unread"</span>
                        {(unread_total > 0).then(|| view! {
                            <span class="ml-0.5 rounded-full bg-blue-500/20 px-1.5 py-0.2 text-[10px] font-bold text-blue-600 dark:text-blue-400">
                                {unread_total}
                            </span>
                        })}
                    </button>

                    <button
                        type="button"
                        class=move || format!(
                            "inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition {}",
                            if filter_mode.get() == TopicFilterMode::Solved {
                                "bg-background text-foreground shadow-xs"
                            } else {
                                "text-muted-foreground hover:text-foreground"
                            }
                        )
                        on:click=move |_| set_filter_mode.set(TopicFilterMode::Solved)
                    >
                        <span>"✓"</span>
                        <span>"Solved"</span>
                        {(solved_total > 0).then(|| view! {
                            <span class="ml-0.5 rounded-full bg-emerald-500/20 px-1.5 py-0.2 text-[10px] font-bold text-emerald-600 dark:text-emerald-400">
                                {solved_total}
                            </span>
                        })}
                    </button>
                </div>

                // Real-time Search Input
                <div class="relative flex-1 sm:max-w-xs">
                    <span class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-xs text-muted-foreground">
                        "🔍"
                    </span>
                    <input
                        type="text"
                        prop:value=move || search_query.get()
                        on:input=move |ev| set_search_query.set(event_target_value(&ev))
                        placeholder="Search discussions..."
                        class="w-full rounded-2xl border border-border bg-background/80 py-1.5 pl-8 pr-7 text-xs text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-hidden"
                    />
                    {move || (!search_query.get().is_empty()).then(|| view! {
                        <button
                            type="button"
                            on:click=move |_| set_search_query.set(String::new())
                            class="absolute right-2 top-1/2 -translate-y-1/2 rounded-full p-0.5 text-xs text-muted-foreground transition hover:text-foreground"
                        >
                            "✕"
                        </button>
                    })}
                </div>
            </div>

            // Filtered Topics List
            {
                let selected_category_id = selected_category_id.clone();
                let selected_topic_id = selected_topic_id.clone();
                let module_route_base = module_route_base.clone();
                let slug_template = slug_template.clone();
                let unread_template = unread_template.clone();
                let updated_unread_label = updated_unread_label.clone();
                let pinned_label = pinned_label.clone();
                let locked_label = locked_label.clone();
                let replies_label = replies_label.clone();

                move || {
                    let list = filtered_items();
                    if list.is_empty() {
                        return view! {
                            <div class="rounded-[1.5rem] border border-dashed border-border p-8 text-center">
                                <h4 class="text-base font-semibold text-card-foreground">
                                    "No matching discussions found"
                                </h4>
                                <p class="mt-2 text-sm text-muted-foreground">
                                    "No topics match your current filter tab or search keyword."
                                </p>
                                <button
                                    type="button"
                                    class="mt-4 inline-flex items-center gap-1.5 rounded-xl border border-border bg-background px-4 py-2 text-xs font-semibold text-foreground shadow-xs transition hover:bg-muted"
                                    on:click=move |_| {
                                        set_filter_mode.set(TopicFilterMode::Latest);
                                        set_search_query.set(String::new());
                                    }
                                >
                                    "Clear filters"
                                </button>
                            </div>
                        }.into_any();
                    }

                    view! {
                        <div class="space-y-3">
                            {list.into_iter().map(|item| {
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

                                                    // Solved Badge
                                                    {card.is_solved.then(|| view! {
                                                        <span class="inline-flex items-center gap-1 rounded-full bg-emerald-500/15 px-2.5 py-0.5 text-[11px] font-medium text-emerald-700 dark:text-emerald-300">
                                                            <span>"✓"</span>
                                                            <span>"Solved"</span>
                                                        </span>
                                                    })}

                                                    // Unread Badge
                                                    {card.is_unread.then(|| view! {
                                                        <span class=card.unread_badge_class>{unread_label}</span>
                                                    })}

                                                    // Pinned Badge
                                                    {card.is_pinned.then(|| view! {
                                                        <span class="rounded-full bg-amber-500/15 px-2.5 py-1 text-[11px] font-medium text-amber-700 dark:text-amber-300">
                                                            {pinned_label.clone()}
                                                        </span>
                                                    })}

                                                    // Locked Badge
                                                    {card.is_locked.then(|| view! {
                                                        <span class="rounded-full bg-destructive/10 px-2.5 py-1 text-[11px] font-medium text-destructive">
                                                            {locked_label.clone()}
                                                        </span>
                                                    })}

                                                    // Vote Score Badge
                                                    {(card.vote_score > 0).then(|| view! {
                                                        <span class="inline-flex items-center gap-1 rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                                                            <span>"▲"</span>
                                                            <span>{card.vote_score}</span>
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
                    }.into_any()
                }
            }
        </section>
    }.into_any()
}
