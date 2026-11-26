use leptos::prelude::*;
use rustok_api::normalize_locale_tag;
use rustok_ui_core::UiRouteContext;

use super::composer::ComposerSignal;
use super::member_card::ForumAuthorBadge;
use crate::core::forum_storefront_topic_card_view_model;
use crate::i18n::{count_message, slug_message, t};
use crate::model::{ForumTopicFeedOrder, ForumTopicPage, ForumTopicSelection};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TopicFilterMode {
    Latest,
    Top,
    Unread,
    Solved,
}

fn forum_storefront_content_lang(locale: &str) -> String {
    normalize_locale_tag(locale).unwrap_or_else(|| "en".to_string())
}

fn selection_mode(selection: &ForumTopicSelection) -> TopicFilterMode {
    if selection.unread_only {
        TopicFilterMode::Unread
    } else if selection.solved_only {
        TopicFilterMode::Solved
    } else if selection.order == ForumTopicFeedOrder::Top {
        TopicFilterMode::Top
    } else {
        TopicFilterMode::Latest
    }
}

fn apply_mode(current: ForumTopicSelection, mode: TopicFilterMode) -> ForumTopicSelection {
    match mode {
        TopicFilterMode::Latest => ForumTopicSelection {
            search: current.search,
            order: ForumTopicFeedOrder::Latest,
            solved_only: false,
            unread_only: false,
        },
        TopicFilterMode::Top => ForumTopicSelection {
            search: current.search,
            order: ForumTopicFeedOrder::Top,
            solved_only: false,
            unread_only: false,
        },
        TopicFilterMode::Unread => ForumTopicSelection {
            search: current.search,
            order: ForumTopicFeedOrder::Latest,
            solved_only: false,
            unread_only: true,
        },
        TopicFilterMode::Solved => ForumTopicSelection {
            search: current.search,
            order: ForumTopicFeedOrder::Latest,
            solved_only: true,
            unread_only: false,
        },
    }
}

fn commit_search(selection: RwSignal<ForumTopicSelection>, raw: String) {
    let next = {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    };
    selection.update(|current| {
        if current.search != next {
            current.search = next;
        }
    });
}

#[component]
pub fn ForumTopicFeed(
    topics: RwSignal<ForumTopicPage>,
    total: u64,
    selected_category_id: Option<String>,
    selected_topic_id: Option<String>,
    paging_busy: ReadSignal<bool>,
    paging_error: ReadSignal<Option<String>>,
    on_load_more: Callback<()>,
    selection: RwSignal<ForumTopicSelection>,
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
    let pinned_label = t(locale.as_deref(), "forum.topic.pinned", "Pinned");
    let locked_label = t(locale.as_deref(), "forum.topic.locked", "Locked");
    let updated_unread_label = t(locale.as_deref(), "forum.topic.updatedUnread", "Updated");
    let replies_label = t(locale.as_deref(), "forum.topic.replies", "Replies");
    let load_more_label = t(locale.as_deref(), "forum.feed.loadMore", "Load more topics");
    let loading_label = t(locale.as_deref(), "forum.feed.loading", "Loading\u{2026}");
    let page_error_label = t(
        locale.as_deref(),
        "forum.feed.pageFailed",
        "Could not load the next topic page",
    );
    let paging_locale = locale.clone();
    let feed_locale = locale.clone();
    let (search_draft, set_search_draft) =
        signal(selection.get_untracked().search.unwrap_or_default());

    let is_default_selection = move || selection.get() == ForumTopicSelection::default();
    if is_default_selection() && topics.get().items.is_empty() {
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

    view! {
        <section class="space-y-4 rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
            <div class="flex flex-wrap items-center justify-between gap-3">
                <div>
                    <p class="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                        {feed_label}
                    </p>
                    <h3 class="mt-2 text-2xl font-semibold text-card-foreground">{feed_title}</h3>
                </div>
                <div class="flex items-center gap-2">
                    <span class="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        {count_message(
                            locale.as_deref(),
                            "forum.feed.threads",
                            "{count} threads",
                            total,
                        )}
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

            <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <div class="flex flex-wrap items-center gap-1 rounded-2xl border border-border bg-muted/30 p-1">
                    <button
                        type="button"
                        class=move || format!(
                            "inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition {}",
                            if selection_mode(&selection.get()) == TopicFilterMode::Latest {
                                "bg-background text-foreground shadow-xs"
                            } else {
                                "text-muted-foreground hover:text-foreground"
                            }
                        )
                        on:click=move |_| selection.update(|current| *current = apply_mode(current.clone(), TopicFilterMode::Latest))
                    >
                        <span>"✨"</span>
                        <span>"Latest"</span>
                    </button>

                    <button
                        type="button"
                        class=move || format!(
                            "inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition {}",
                            if selection_mode(&selection.get()) == TopicFilterMode::Top {
                                "bg-background text-foreground shadow-xs"
                            } else {
                                "text-muted-foreground hover:text-foreground"
                            }
                        )
                        on:click=move |_| selection.update(|current| *current = apply_mode(current.clone(), TopicFilterMode::Top))
                    >
                        <span>"🔥"</span>
                        <span>"Top"</span>
                    </button>

                    <button
                        type="button"
                        class=move || format!(
                            "inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition {}",
                            if selection_mode(&selection.get()) == TopicFilterMode::Unread {
                                "bg-background text-foreground shadow-xs"
                            } else {
                                "text-muted-foreground hover:text-foreground"
                            }
                        )
                        on:click=move |_| selection.update(|current| *current = apply_mode(current.clone(), TopicFilterMode::Unread))
                    >
                        <span>"💬"</span>
                        <span>"Unread"</span>
                    </button>

                    <button
                        type="button"
                        class=move || format!(
                            "inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-semibold transition {}",
                            if selection_mode(&selection.get()) == TopicFilterMode::Solved {
                                "bg-background text-foreground shadow-xs"
                            } else {
                                "text-muted-foreground hover:text-foreground"
                            }
                        )
                        on:click=move |_| selection.update(|current| *current = apply_mode(current.clone(), TopicFilterMode::Solved))
                    >
                        <span>"✓"</span>
                        <span>"Solved"</span>
                    </button>
                </div>

                <div class="relative flex-1 sm:max-w-xs">
                    <span class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-xs text-muted-foreground">
                        "🔍"
                    </span>
                    <input
                        type="text"
                        prop:value=move || search_draft.get()
                        on:input=move |ev| set_search_draft.set(event_target_value(&ev))
                        on:blur=move |_| commit_search(selection, search_draft.get_untracked())
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" {
                                ev.prevent_default();
                                commit_search(selection, search_draft.get_untracked());
                            }
                        }
                        placeholder="Search discussions..."
                        class="w-full rounded-2xl border border-border bg-background/80 py-1.5 pl-8 pr-7 text-xs text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-hidden"
                    />
                    {move || (!search_draft.get().trim().is_empty()).then(|| view! {
                        <button
                            type="button"
                            on:click=move |_| {
                                set_search_draft.set(String::new());
                                commit_search(selection, String::new());
                            }
                            class="absolute right-2 top-1/2 -translate-y-1/2 rounded-full p-0.5 text-xs text-muted-foreground transition hover:text-foreground"
                        >
                            "✕"
                        </button>
                    })}
                </div>
            </div>

            {
                let selected_category_id = selected_category_id.clone();
                let selected_topic_id = selected_topic_id.clone();
                let module_route_base = module_route_base.clone();
                let updated_unread_label = updated_unread_label.clone();
                let pinned_label = pinned_label.clone();
                let locked_label = locked_label.clone();
                let replies_label = replies_label.clone();

                move || {
                    let list = topics.get().items;
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
                                    on:click=move |_| selection.set(ForumTopicSelection::default())
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
                                let slug_label = slug_message(
                                    feed_locale.as_deref(),
                                    "forum.topic.slug",
                                    "thread slug: {slug}",
                                    item.slug.as_str(),
                                );
                                let card = forum_storefront_topic_card_view_model(
                                    module_route_base.as_str(),
                                    &item,
                                    selected_category_id.as_deref(),
                                    selected_topic_id.as_deref(),
                                    slug_label.as_str(),
                                );
                                let content_lang = forum_storefront_content_lang(card.effective_locale.as_str());
                                let unread_label = if card.unread_count > 0 {
                                    count_message(
                                        feed_locale.as_deref(),
                                        "forum.topic.unreadCount",
                                        "{count} unread",
                                        card.unread_count,
                                    )
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
                                                    <span class=card.status_badge_class>
                                                        {crate::i18n::status_label(
                                                            feed_locale.as_deref(),
                                                            &card.status,
                                                        )}
                                                    </span>
                                                    <span dir="ltr" class="rounded-full border border-border px-2.5 py-1 text-[11px] font-medium text-muted-foreground">
                                                        {card.effective_locale.clone()}
                                                    </span>
                                                    {card.is_solved.then(|| view! {
                                                        <span class="inline-flex items-center gap-1 rounded-full bg-emerald-500/15 px-2.5 py-0.5 text-[11px] font-medium text-emerald-700 dark:text-emerald-300">
                                                            <span>"✓"</span>
                                                            <span>"Solved"</span>
                                                        </span>
                                                    })}
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

            {move || {
                let page = topics.get();
                let has_more = page.has_more();
                let loaded = u64::try_from(page.items.len()).unwrap_or(u64::MAX);
                let error = paging_error.get();
                if !has_more && error.is_none() {
                    return view! { <span class="hidden"></span> }.into_any();
                }
                let paging_locale = paging_locale.clone();
                let loaded = count_message(
                    paging_locale.as_deref(),
                    "forum.feed.loaded",
                    "{count} loaded",
                    loaded,
                );
                let load_more_label = load_more_label.clone();
                let loading_label = loading_label.clone();
                let page_error_label = page_error_label.clone();
                view! {
                    <div class="mt-4 space-y-2">
                        {error.map(|message| view! {
                            <p class="rounded-xl border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                                {format!("{}: {message}", page_error_label)}
                            </p>
                        })}
                        <div class="flex flex-wrap items-center justify-between gap-3">
                            <span class="text-xs text-muted-foreground">
                                {loaded}
                            </span>
                            {has_more.then(|| view! {
                                <button
                                    type="button"
                                    class="inline-flex items-center gap-1.5 rounded-xl border border-primary/30 bg-primary/5 px-4 py-2 text-sm font-semibold text-primary transition hover:bg-primary/10 disabled:cursor-not-allowed disabled:opacity-60"
                                    disabled=move || paging_busy.get()
                                    on:click=move |_| on_load_more.run(())
                                >
                                    {move || if paging_busy.get() {
                                        loading_label.clone()
                                    } else {
                                        load_more_label.clone()
                                    }}
                                </button>
                            })}
                        </div>
                    </div>
                }
                .into_any()
            }}
        </section>
    }.into_any()
}
