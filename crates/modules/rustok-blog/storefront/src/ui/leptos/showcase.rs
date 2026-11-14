use super::published_posts_list::PublishedPostsList;
use super::selected_post_card::SelectedPostCard;
use super::*;
use leptos::prelude::*;
use rustok_ui_leptos::{Button, ButtonVariant, Input, Size};

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
    let is_post_selected = data.selected_post.is_some();
    let (selected_post_slug, categories_map, other_posts) =
        core::showcase_posts(&data, search_total.is_some());

    let related_title = t(locale.as_deref(), "blog.related.title", "Related articles");
    let next_page_label = t(locale.as_deref(), "blog.posts.nextPage", "Next page");
    let previous_page_label = t(locale.as_deref(), "blog.posts.previousPage", "Previous");

    let search_query = use_route_query_value("q");
    let tag_query = use_route_query_value("tag");
    let category_query = use_route_query_value("category");
    let active_search = search_query.get().filter(|value| !value.trim().is_empty());
    let active_tag = tag_query.get().filter(|value| !value.trim().is_empty());
    let active_category = category_query
        .get()
        .filter(|value| !value.trim().is_empty());

    let selected_slug = selected_post_slug.as_deref();
    let remove_category_href = core::filter_query_href(
        selected_slug,
        active_tag.as_deref(),
        None,
        active_search.as_deref(),
    );
    let remove_tag_href = core::filter_query_href(
        selected_slug,
        None,
        active_category.as_deref(),
        active_search.as_deref(),
    );
    let remove_search_href = core::filter_query_href(
        selected_slug,
        active_tag.as_deref(),
        active_category.as_deref(),
        None,
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
                        <a href=remove_category_href.clone() class="ml-1 font-bold text-muted-foreground hover:text-foreground">"Г—"</a>
                    </span>
                })}
                {active_tag.map(|tag| view! {
                    <span class="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2.5 py-1 text-xs font-medium text-primary">
                        {format!("#{tag}")}
                        <a href=remove_tag_href class="ml-1 font-bold text-muted-foreground hover:text-foreground">"Г—"</a>
                    </span>
                })}
                {active_search.as_ref().map(|q| view! {
                    <span class="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2.5 py-1 text-xs font-medium text-primary">
                        {format!("Search: \"{q}\"")}
                        <a href=remove_search_href class="ml-1 font-bold text-muted-foreground hover:text-foreground">"Г—"</a>
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

    let search_input = RwSignal::new(active_search.clone().unwrap_or_default());
    view! {
        <div class="space-y-6" id="top">
            <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
                <form method="GET" class="relative max-w-md w-full">
                    {slug_for_form.map(|slug| view! { <input type="hidden" name="slug" value=slug /> })}
                    {tag_for_form.map(|tag| view! { <input type="hidden" name="tag" value=tag /> })}
                    {category_for_form.map(|category| view! { <input type="hidden" name="category" value=category /> })}
                    <Input r#type="search" name="q" value=search_input.read_only() set_value=search_input.write_only()
                        placeholder=t(locale.as_deref(), "blog.search.placeholder", "Search articles and tags...") />
                </form>
                <a
                    href=core::feed_href(locale.as_deref())
                    target="_blank"
                    rel="noopener noreferrer"
                    class="inline-flex items-center gap-1.5 self-start sm:self-auto rounded-full border border-border bg-background px-3.5 py-2 text-xs font-medium text-muted-foreground hover:text-foreground hover:border-primary/50 transition-colors shadow-sm"
                >

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
            {move || (search_total.is_none()).then_some(())
                .and_then(|_| posts_cursor_query.get().filter(|value| !value.trim().is_empty())).map(|_| {
                let writer = first_page_writer.clone();
                let first_page_label = first_page_label.clone();
                view! {
                    <div class="mt-6 flex justify-center">
                        <Button
                            size=Size::Sm variant=ButtonVariant::Outline
                            class="rounded-full border border-border bg-background px-4 py-2 text-sm font-medium text-foreground hover:border-primary/50"
                            on:click=move |_| {
                                writer.apply_query_intent(UiRouteQueryIntent::clear(
                                    core::POSTS_CURSOR_QUERY_KEY,
                                ));
                            }
                        >
                            {first_page_label.clone()}
                        </Button>
                    </div>
                }
            })}
            {posts_next_cursor.filter(|_| search_total.is_none()).map(|cursor| {
                let writer = next_page_writer.clone();
                let next_page_label = next_page_label.clone();
                view! {
                    <div class="mt-6 flex justify-center">
                        <Button
                            size=Size::Sm variant=ButtonVariant::Outline
                            class="rounded-full border border-border bg-background px-4 py-2 text-sm font-medium text-foreground hover:border-primary/50"
                            on:click=move |_| {
                                writer.apply_query_intent(UiRouteQueryIntent::replace(
                                    core::POSTS_CURSOR_QUERY_KEY,
                                    cursor.clone(),
                                ));
                            }
                        >
                            {next_page_label}
                        </Button>
                    </div>
                }
            })}
            {search_total.map(|total| {
                let page_view = core::search_page_view(search_page, total);
                let previous_writer = search_previous_writer.clone();
                let next_writer = search_next_writer.clone();
                view! {
                    <nav aria-label=t(locale.as_deref(), "blog.search.pages", "Search pages") class="mt-6 flex items-center justify-center gap-4">
                        {page_view.previous.map(|intent| {
                            let writer = previous_writer.clone();
                            view! {
                                <Button size=Size::Sm variant=ButtonVariant::Outline class="rounded-full border px-4 py-2 text-sm"
                                    on:click=move |_| {
                                        writer.apply_query_intent(intent.clone());
                                    }>
                                    {previous_page_label}
                                </Button>
                            }
                        })}
                        <span class="text-sm text-muted-foreground">
                            {crate::i18n::search_results_message(locale.as_deref(), search_page, total)}
                        </span>
                        {page_view.next.map(|intent| {
                            let writer = next_writer.clone();
                            view! {
                                <Button size=Size::Sm variant=ButtonVariant::Outline class="rounded-full border px-4 py-2 text-sm"
                                    on:click=move |_| {
                                        writer.apply_query_intent(intent.clone());
                                    }>
                                    {next_page_label}
                                </Button>
                            }
                        })}
                    </nav>
                }
            })}
        </div>
    }
}
