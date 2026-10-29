use super::*;
use leptos::prelude::*;

#[component]
pub(super) fn PublishedPostsList(
    items: Vec<BlogPostListItem>,
    #[prop(optional)] list_title: Option<String>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let route_segment = core::route_segment_or_default(
        route_context.route_segment.as_ref().cloned(),
        core::DEFAULT_ROUTE_SEGMENT,
    );
    let module_route_base = route_context.module_route_base(route_segment.as_str());
    let list_slug_filter = use_route_query_value("slug").get();
    let list_tag_filter = use_route_query_value("tag").get();
    let list_category_filter = use_route_query_value("category").get();
    let list_search_filter = use_route_query_value("q").get();
    let unknown_status_label = t(locale.as_deref(), "blog.list.unknownStatus", "unknown");
    let default_title = t(locale.as_deref(), "blog.list.title", "Published posts");
    let header_title = list_title.unwrap_or(default_title);
    // The public list is cursor-paginated and carries no total, so the header
    // shows the title only.

    let items = match core::published_posts_ready_typed_view(
        items,
        t(
            locale.as_deref(),
            "blog.list.empty",
            "No published blog posts are available for storefront rendering yet.",
        ),
    ) {
        core::PublishedPostsReadyView::Items(items) => items,
        core::PublishedPostsReadyView::Empty(empty_state) => {
            return view! {
                <article class="rounded-2xl border border-dashed border-border p-6">
                    <p class="text-sm text-muted-foreground">
                        {empty_state.message}
                    </p>
                </article>
            }
            .into_any();
        }
    };

    view! {
        <div class="space-y-3">
            <div class="flex items-center justify-between gap-3">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {header_title}
                </h3>
            </div>
            <div class="grid gap-3 md:grid-cols-2">
                {items
                    .into_iter()
                    .map(|post| {
                        let module_route_base = module_route_base.clone();
                        let locale = locale.clone();
                        let missing_slug_fallback = t(
                            locale.as_deref(),
                            "blog.selected.missingSlug",
                            "missing-slug",
                        );
                        let open_label = t(locale.as_deref(), "blog.list.open", "Open");
                        let locale_label = t(locale.as_deref(), "blog.list.localeLabel", "locale");
                        let no_excerpt_fallback =
                            t(locale.as_deref(), "blog.list.noExcerpt", "No excerpt yet.");
                        let post_card_view = core::published_post_card_view(
                            core::PublishedPostCardInput {
                                slug: post.slug,
                                missing_slug_fallback: missing_slug_fallback.as_str(),
                                excerpt: post.excerpt,
                                excerpt_fallback: no_excerpt_fallback.as_str(),
                                module_route_base: module_route_base.as_str(),
                                open_label: open_label.as_str(),
                                locale_label: locale_label.as_str(),
                                effective_locale: post.effective_locale.as_str(),
                                status: post.status,
                            },
                        );
                        let published_meta = post.published_at.as_deref().map(|published_at| {
                            core::label_value_pair(
                                &t(
                                    locale.as_deref(),
                                    "blog.list.publishedLabel",
                                    "published",
                                ),
                                published_at,
                            )
                        });
                        view! {
                            <article class="rounded-2xl border border-border bg-background p-5">
                                {post.featured_image_url.as_deref().map(|url| view! {
                                    <img
                                        src=url.to_string()
                                        alt=post.title.clone()
                                        loading="lazy"
                                        class="mb-4 aspect-video w-full rounded-lg object-cover"
                                    />
                                })}
                                <div class="flex items-center gap-2">
                                    <BlogStatusBadge
                                        status=post_card_view.status
                                        unknown_label=unknown_status_label.clone()
                                    />
                                    {post.category_name.as_deref().map(|cat| {
                                        if let Some(cat_id) = post.category_id.as_deref() {
                                            view! {
                                                <a
                                                    href=core::filter_query_href(
                                                        list_slug_filter.as_deref(),
                                                        list_tag_filter.as_deref(),
                                                        Some(cat_id),
                                                        list_search_filter.as_deref(),
                                                    )
                                                    class="rounded-full bg-secondary px-2 py-0.5 text-xs font-medium text-secondary-foreground hover:bg-secondary/80 transition-colors"
                                                >
                                                    {cat.to_string()}
                                                </a>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <span class="rounded-full bg-secondary px-2 py-0.5 text-xs font-medium text-secondary-foreground">
                                                    {cat.to_string()}
                                                </span>
                                            }.into_any()
                                        }
                                    })}
                                </div>
                                {if let Some(author) = post.author_profile.as_ref() {
                                    view! {
                                        <div class="mt-1 text-xs font-medium text-muted-foreground">
                                            {author.display_name.clone()}
                                        </div>
                                    }
                                    .into_any()
                                } else {
                                    ().into_any()
                                }}
                                <h4 class="mt-2 text-base font-semibold text-foreground">{post.title}</h4>
                                <p class="mt-2 text-sm text-muted-foreground">
                                    {post_card_view.excerpt}
                                </p>
                                {if post.tags.is_empty() {
                                    ().into_any()
                                } else {
                                    view! {
                                        <div class="mt-3 flex flex-wrap gap-1.5">
                                            {post.tags
                                                .iter()
                                                .map(|tag| {
                                                    let tag_link = core::filter_query_href(
                                                        list_slug_filter.as_deref(),
                                                        Some(tag),
                                                        list_category_filter.as_deref(),
                                                        list_search_filter.as_deref(),
                                                    );
                                                    view! {
                                                        <a
                                                            href=tag_link
                                                            class="rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary hover:bg-primary/20 transition-colors"
                                                        >
                                                            {format!("#{tag}")}
                                                        </a>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    }
                                    .into_any()
                                }}
                                {published_meta.map(|meta| view! {
                                    <div class="mt-3 text-xs text-muted-foreground">{meta}</div>
                                })}
                                <a class="mt-4 inline-flex text-sm text-primary hover:underline" href=post_card_view.href>
                                    {post_card_view.open_label}
                                </a>
                                <p class="mt-3 text-xs text-muted-foreground">
                                    {post_card_view.locale_meta}
                                </p>
                            </article>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
    .into_any()
}
