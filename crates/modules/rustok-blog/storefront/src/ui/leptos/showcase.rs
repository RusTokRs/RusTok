use leptos::prelude::*;
use super::*;
use super::published_posts_list::PublishedPostsList;
use super::selected_post_card::SelectedPostCard;

#[component]
pub(super) fn BlogShowcase(
    data: StorefrontBlogData,
    comments_page: u64,
    search_total: Option<u64>,
    search_page: usize,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let posts_query_writer = use_route_query_writer();
    let first_page_writer = posts_query_writer.clone();
    let next_page_writer = posts_query_writer.clone();
    let search_previous_writer = posts_query_writer.clone();
    let search_next_writer = posts_query_writer;
    let posts_next_cursor = data.posts.next_cursor.clone();
    let posts_cursor_query = use_route_query_value(core::POSTS_CURSOR_QUERY_KEY);
    let first_page_label = t(
        locale.as_deref(),
        "blog.posts.firstPage",
        "Back to the first page",
    );
    let selected_post_slug = data.selected_post.as_ref().and_then(|p| p.slug.clone());
    let is_post_selected = data.selected_post.is_some();

    let mut categories_map = std::collections::BTreeMap::new();
    for post in &data.posts.items {
        if let (Some(id), Some(name)) = (&post.category_id, &post.category_name) {
            categories_map
                .entry(id.clone())
                .or_insert_with(|| name.clone());
        }
    }
    if let Some(ref sel) = data.selected_post
        && let (Some(id), Some(name)) = (&sel.category_id, &sel.category_name)
    {
        categories_map
            .entry(id.clone())
            .or_insert_with(|| name.clone());
    }

    let other_posts = if let Some(slug) = selected_post_slug.as_deref().filter(|_| search_total.is_none()) {
        data.posts
            .items
            .into_iter()
            .filter(|item| item.slug.as_deref() != Some(slug))
            .collect::<Vec<_>>()
    } else {
        data.posts.items
    };

    let related_title = t(locale.as_deref(), "blog.related.title", "Related articles");

    let search_query = use_route_query_value("q");
    let tag_query = use_route_query_value("tag");
    let category_query = use_route_query_value("category");
    let active_search = search_query.get().filter(|value| !value.trim().is_empty());
    let active_tag = tag_query.get().filter(|value| !value.trim().is_empty());
    let active_category = category_query.get().filter(|value| !value.trim().is_empty());

    let selected_slug = selected_post_slug.as_deref();
    let remove_category_href = core::filter_query_href(
        selected_slug, active_tag.as_deref(), None, active_search.as_deref(),
    );
    let remove_tag_href = core::filter_query_href(
        selected_slug, None, active_category.as_deref(), active_search.as_deref(),
    );
    let remove_search_href = core::filter_query_href(
        selected_slug, active_tag.as_deref(), active_category.as_deref(), None,
    );
    let clear_filters_href = core::filter_query_href(selected_slug, None, None, None);
    let all_categories_href = remove_category_href.clone();
    let tag_for_links = active_tag.clone();
    let search_for_links = active_search.clone();
    let slug_for_links = selected_post_slug.clone();
    let tag_for_form = active_tag.clone();
    let category_for_form = active_category.clone();
    let slug_for_form = selected_post_slug.clone();

    // Query results already passed full-text ranking, tenant/channel and
    // exact category/tag filters *before* Search applied the page offset.
    // Never locally filter the returned page again.

    let has_active_filter =
        active_tag.is_some() || active_category.is_some() || active_search.is_some();
    let filter_bar = if has_active_filter {
        view! {
            <div class="flex flex-wrap items-center gap-2 rounded-2xl border border-border bg-muted/40 p-3 text-xs">
                <span class="font-medium text-muted-foreground">
                    {t(locale.as_deref(), "blog.filter.active", "Active filters:")}
                </span>
                {active_category.as_ref().map(|cat| view! {
                    <span class="inline-flex items-center gap-1 rounded-full bg-secondary px-2.5 py-1 text-xs font-medium text-secondary-foreground">
                        {format!("Category: {cat}")}
                        <a href=remove_category_href.clone() class="ml-1 font-bold text-muted-foreground hover:text-foreground">"×"</a>
                    </span>
                })}
                {active_tag.map(|tag| view! {
                    <span class="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2.5 py-1 text-xs font-medium text-primary">
                        {format!("#{tag}")}
                        <a href=remove_tag_href class="ml-1 font-bold text-muted-foreground hover:text-foreground">"×"</a>
                    </span>
                })}
                {active_search.as_ref().map(|q| view! {
                    <span class="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2.5 py-1 text-xs font-medium text-primary">
                        {format!("Search: \"{q}\"")}
                        <a href=remove_search_href class="ml-1 font-bold text-muted-foreground hover:text-foreground">"×"</a>
                    </span>
                })}
                <a href=clear_filters_href class="ml-auto text-xs text-muted-foreground hover:text-foreground underline">
                    {t(locale.as_deref(), "blog.filter.clear", "Clear all")}
                </a>
            </div>
        }
        .into_any()
    } else {
        ().into_any()
    };

    let category_pills = if !categories_map.is_empty() {
        let is_all_active = active_category.is_none();
        view! {
            <div class="flex flex-wrap items-center gap-2 pt-1">
                <a
                    href=all_categories_href
                    class=if is_all_active {
                        "rounded-full bg-primary px-3 py-1.5 text-xs font-semibold text-primary-foreground shadow-sm"
                    } else {
                        "rounded-full bg-muted/60 px-3 py-1.5 text-xs font-medium text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                    }
                >
                    {t(locale.as_deref(), "blog.category.all", "All")}
                </a>
                {categories_map
                    .into_iter()
                    .map(|(cat_id, cat_name)| {
                        let is_active = active_category.as_deref() == Some(&cat_id)
                            || active_category.as_deref() == Some(&cat_name);
                        let cat_link = core::filter_query_href(
                            slug_for_links.as_deref(),
                            tag_for_links.as_deref(),
                            Some(cat_id.as_str()),
                            search_for_links.as_deref(),
                        );
                        view! {
                            <a
                                href=cat_link
                                class=if is_active {
                                    "rounded-full bg-primary px-3 py-1.5 text-xs font-semibold text-primary-foreground shadow-sm"
                                } else {
                                    "rounded-full bg-muted/60 px-3 py-1.5 text-xs font-medium text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                                }
                            >
                                {cat_name}
                            </a>
                        }
                    })
                    .collect_view()}
            </div>
        }
        .into_any()
    } else {
        ().into_any()
    };

    view! {
        <div class="space-y-6" id="top">
            <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
                <form method="GET" class="relative max-w-md w-full">
                    {slug_for_form.map(|slug| view! { <input type="hidden" name="slug" value=slug /> })}
                    {tag_for_form.map(|tag| view! { <input type="hidden" name="tag" value=tag /> })}
                    {category_for_form.map(|category| view! { <input type="hidden" name="category" value=category /> })}
                    <input
                        type="search"
                        name="q"
                        value=active_search.unwrap_or_default()
                        placeholder=t(locale.as_deref(), "blog.search.placeholder", "Search articles and tags...")
                        class="w-full rounded-full border border-border bg-background px-4 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary/50 shadow-sm"
                    />
                </form>
                <a
                    href=core::feed_href(locale.as_deref())
                    target="_blank"
                    rel="noopener noreferrer"
                    class="inline-flex items-center gap-1.5 self-start sm:self-auto rounded-full border border-border bg-background px-3.5 py-2 text-xs font-medium text-muted-foreground hover:text-foreground hover:border-primary/50 transition-colors shadow-sm"
                >
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-orange-500">
                        <path d="M4 11a9 9 0 0 1 9 9"></path>
                        <path d="M4 4a16 16 0 0 1 16 16"></path>
                        <circle cx="5" cy="19" r="1"></circle>
                    </svg>
                    <span>"RSS"</span>
                </a>
            </div>
            {category_pills}
            {filter_bar}
            <SelectedPostCard post=data.selected_post comments_page />
            {if is_post_selected {
                view! {
                    <a
                        href="#top"
                        class="fixed bottom-6 right-6 z-40 flex h-10 w-10 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-lg hover:bg-primary/90 hover:scale-105 active:scale-95 transition-all duration-200"
                        aria-label=t(locale.as_deref(), "blog.scrollTop", "Scroll to top")
                        title=t(locale.as_deref(), "blog.scrollTop", "Scroll to top")
                    >
                        <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <line x1="12" y1="19" x2="12" y2="5"></line>
                            <polyline points="5 12 12 5 19 12"></polyline>
                        </svg>
                    </a>
                }.into_any()
            } else {
                ().into_any()
            }}

            {if is_post_selected {
                if !other_posts.is_empty() {
                    view! {
                        <div class="mt-8 border-t border-border pt-6">
                            <PublishedPostsList
                                items=other_posts
                                list_title=related_title
                            />
                        </div>
                    }
                    .into_any()
                } else {
                    ().into_any()
                }
            } else {
                view! {
                    <PublishedPostsList items=other_posts />
                }
                .into_any()
            }}
            {move || (search_total.is_none()).then(|| ())
                .and_then(|_| posts_cursor_query.get().filter(|value| !value.trim().is_empty())).map(|_| {
                let writer = first_page_writer.clone();
                view! {
                    <div class="mt-6 flex justify-center">
                        <button
                            type="button"
                            class="rounded-full border border-border bg-background px-4 py-2 text-sm font-medium text-foreground hover:border-primary/50"
                            on:click=move |_| {
                                writer.apply_query_intent(UiRouteQueryIntent::clear(
                                    core::POSTS_CURSOR_QUERY_KEY,
                                ));
                            }
                        >
                            {first_page_label.clone()}
                        </button>
                    </div>
                }
            })}
            {posts_next_cursor.filter(|_| search_total.is_none()).map(|cursor| {
                let writer = next_page_writer.clone();
                view! {
                    <div class="mt-6 flex justify-center">
                        <button
                            type="button"
                            class="rounded-full border border-border bg-background px-4 py-2 text-sm font-medium text-foreground hover:border-primary/50"
                            on:click=move |_| {
                                writer.apply_query_intent(UiRouteQueryIntent::replace(
                                    core::POSTS_CURSOR_QUERY_KEY,
                                    cursor.clone(),
                                ));
                            }
                        >
                            {t(locale.as_deref(), "blog.posts.nextPage", "Next page")}
                        </button>
                    </div>
                }
            })}
            {search_total.map(|total| {
                let has_previous = search_page > 1;
                let has_next = (search_page as u64) * (BLOG_SEARCH_PAGE_SIZE as u64) < total;
                let previous_writer = search_previous_writer.clone();
                let next_writer = search_next_writer.clone();
                view! {
                    <nav aria-label="Blog search pages" class="mt-6 flex items-center justify-center gap-4">
                        {has_previous.then(|| {
                            let writer = previous_writer.clone();
                            view! {
                                <button type="button" class="rounded-full border px-4 py-2 text-sm"
                                    on:click=move |_| {
                                        if search_page <= 2 {
                                            writer.apply_query_intent(UiRouteQueryIntent::clear("page"));
                                        } else {
                                            writer.apply_query_intent(UiRouteQueryIntent::replace(
                                                "page", (search_page - 1).to_string(),
                                            ));
                                        }
                                    }>
                                    {t(locale.as_deref(), "blog.posts.previousPage", "Previous")}
                                </button>
                            }
                        })}
                        <span class="text-sm text-muted-foreground">
                            {format!("Page {search_page} · {total} results")}
                        </span>
                        {has_next.then(|| {
                            let writer = next_writer.clone();
                            view! {
                                <button type="button" class="rounded-full border px-4 py-2 text-sm"
                                    on:click=move |_| {
                                        writer.apply_query_intent(UiRouteQueryIntent::replace(
                                            "page", (search_page + 1).to_string(),
                                        ));
                                    }>
                                    {t(locale.as_deref(), "blog.posts.nextPage", "Next page")}
                                </button>
                            }
                        })}
                    </nav>
                }
            })}
        </div>
    }
}
