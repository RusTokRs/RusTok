use leptos::prelude::*;
use rustok_api::normalize_locale_tag;
use rustok_ui_core::UiRouteContext;

use super::composer::ComposerSignal;
use super::member_card::ForumAuthorBadge;
use crate::core::ForumTopicFeedMode;
use crate::core::forum_storefront_topic_card_view_model;
use crate::i18n::{count_message, slug_message, t};
use crate::model::{ForumTopicPage, ForumTopicSelection};

fn forum_storefront_content_lang(locale: &str) -> String {
    normalize_locale_tag(locale).unwrap_or_else(|| "en".to_string())
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

    let feed_label = t(locale.as_deref(), "forum.feed.label", "Topic feed");
    let feed_title = t(locale.as_deref(), "forum.feed.title", "Discussions");
    let empty_title = t(
        locale.as_deref(),
        "forum.feed.noMatches",
        "No matching discussions found",
    );
    let empty_body = t(
        locale.as_deref(),
        "forum.feed.noMatchesBody",
        "No topics match the selected feed or search.",
    );
    let clear_label = t(locale.as_deref(), "forum.feed.clear", "Clear filters");
    let new_topic_label = t(locale.as_deref(), "forum.feed.newTopic", "New topic");
    let pinned_label = t(locale.as_deref(), "forum.topic.pinned", "Pinned");
    let locked_label = t(locale.as_deref(), "forum.topic.locked", "Locked");
    let solved_label = t(locale.as_deref(), "forum.topic.solved", "Solved");
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
    // Card labels resolve per topic because each card interpolates its own count and slug.
    let feed_locale = locale.clone();

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
                        {new_topic_label}
                    </button>
                </div>
            </div>

            // Filtered Topics List
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
                                    {empty_title.clone()}
                                </h4>
                                <p class="mt-2 text-sm text-muted-foreground">
                                    {empty_body.clone()}
                                </p>
                                <button
                                    type="button"
                                    class="mt-4 inline-flex items-center gap-1.5 rounded-xl border border-border bg-background px-4 py-2 text-xs font-semibold text-foreground shadow-xs transition hover:bg-muted"
                                    on:click=move |_| {
                                        selection.set(ForumTopicSelection::default());
                                    }
                                >
                                    {clear_label.clone()}
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
                                                            locale.as_deref(),
                                                            &card.status,
                                                        )}
                                                    </span>
                                                    <span dir="ltr" class="rounded-full border border-border px-2.5 py-1 text-[11px] font-medium text-muted-foreground">
                                                        {card.effective_locale.clone()}
                                                    </span>

                                                    // Solved Badge
                                                    {card.is_solved.then(|| view! {
                                                        <span class="inline-flex items-center gap-1 rounded-full bg-emerald-500/15 px-2.5 py-0.5 text-[11px] font-medium text-emerald-700 dark:text-emerald-300">
                                                            <span>"✓"</span>
                                                            <span>{solved_label.clone()}</span>
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

            // Cursor paging: the owner's `nextCursor` is the only source of "more topics exist", so
            // the control disappears on the last page and a failed page is named instead of leaving
            // the list silently short. The header count stays the owner-authored category total.
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

/// Kept outside the reloading result subtree so typing never remounts the input.
#[component]
pub fn ForumFeedControls(
    selection: RwSignal<ForumTopicSelection>,
    read_state_available: ReadSignal<bool>,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let search_label = t(locale.as_deref(), "forum.feed.search", "Search discussions");
    let clear_label = t(locale.as_deref(), "forum.feed.clear", "Clear filters");
    // Mode/ranking changes must not erase text that has not been submitted.
    // Memo equality makes only an actual applied-search change reset the draft.
    let applied_search = Memo::new(move |_| selection.get().search.unwrap_or_default());
    let (search_draft, set_search_draft) = signal(applied_search.get_untracked());
    Effect::new(move |_| {
        set_search_draft.set(applied_search.get());
    });
    let modes = [
        (ForumTopicFeedMode::Latest, "forum.feed.latest", "Latest"),
        (ForumTopicFeedMode::Top, "forum.feed.top", "Top"),
        (ForumTopicFeedMode::Unread, "forum.feed.unread", "Unread"),
        (ForumTopicFeedMode::Solved, "forum.feed.solved", "Solved"),
    ];
    view! {
        <div class="flex flex-wrap items-center gap-3 rounded-2xl border border-border bg-card p-3">
            {modes.into_iter().map(|(mode, key, fallback)| {
                let label = t(locale.as_deref(), key, fallback);
                view! {
                    <button
                        type="button"
                        class=move || if selection.get().mode() == mode {
                            "rounded-xl bg-primary px-3 py-2 text-sm text-primary-foreground"
                        } else {
                            "rounded-xl px-3 py-2 text-sm text-muted-foreground disabled:opacity-50"
                        }
                        aria-pressed=move || (selection.get().mode() == mode).to_string()
                        disabled=move || mode == ForumTopicFeedMode::Unread && !read_state_available.get()
                        on:click=move |_| selection.update(|value| value.set_mode(mode))
                    >{label}</button>
                }
            }).collect_view()}
            <form
                class="flex min-w-48 flex-1 items-center gap-2"
                on:submit=move |ev| {
                    ev.prevent_default();
                    selection.update(|value| value.search = Some(search_draft.get_untracked()));
                }
            >
            <input
                type="search"
                aria-label=search_label.clone()
                placeholder=search_label.clone()
                prop:value=move || search_draft.get()
                on:input=move |ev| set_search_draft.set(event_target_value(&ev))
                class="min-w-48 flex-1 rounded-xl border border-border bg-background px-3 py-2 text-sm"
            />
            <button type="submit" class="rounded-xl border border-border px-3 py-2 text-sm">{search_label}</button>
            </form>
            <button
                type="button"
                on:click=move |_| {
                    set_search_draft.set(String::new());
                    selection.set(ForumTopicSelection::default());
                }
                class="rounded-xl border border-border px-3 py-2 text-sm"
            >{clear_label}</button>
        </div>
    }
}
