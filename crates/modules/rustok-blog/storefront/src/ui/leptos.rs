use leptos::prelude::*;
use leptos_auth::hooks::use_token;
use leptos_ui::{RichTextHtml, TableOfContents};
use leptos_ui_routing::{read_route_query_value, use_route_query_value};
#[cfg(target_arch = "wasm32")]
use rustok_comments_storefront_support::CommentComposer;
use rustok_ui_core::UiRouteContext;

use super::comments_list::PublicCommentsList;
use super::share::{BlogShareButtons, BlogStatusBadge};

use crate::i18n::{comment_composer_copy, t};
use crate::model::{
    BlogCommentsAvailability, BlogPostDetail, BlogPostListItem, StorefrontBlogData,
};
use crate::{comments_pagination, core, transport};

#[component]
pub fn BlogView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_locale = route_context.locale.clone();
    let comments_page_query = use_route_query_value(comments_pagination::COMMENTS_PAGE_QUERY_KEY);
    let tag_query = use_route_query_value("tag");
    let category_query = use_route_query_value("category");
    let shell_view = core::build_storefront_shell_view_model(selected_locale.as_deref());
    let badge = shell_view.badge;
    let title = shell_view.title;
    let subtitle = shell_view.subtitle;
    let load_error = shell_view.load_error;

    let route_context_for_fetch = route_context.clone();
    let selected_locale_for_fetch = selected_locale.clone();
    let posts_resource = Resource::new_blocking(
        move || {
            let route_state = core::build_storefront_route_state(
                read_route_query_value(&route_context_for_fetch, core::SELECTED_POST_QUERY_KEY),
                route_context_for_fetch.route_segment.as_ref().cloned(),
            )
            .with_filters(tag_query.get(), category_query.get());
            let fetch_request = core::build_storefront_fetch_request(
                &route_state,
                selected_locale_for_fetch.clone(),
            );
            (
                fetch_request,
                comments_pagination::comments_page_from_query(comments_page_query.get()),
            )
        },
        move |(request, comments_page)| async move {
            transport::fetch_blog(request, comments_page).await
        },
    );

    view! {
        <section class="rounded-3xl border border-border bg-card p-8 shadow-sm">
            <div class="max-w-3xl space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                    {badge}
                </span>
                <h2 class="text-3xl font-semibold text-card-foreground">
                    {title}
                </h2>
                <p class="text-sm text-muted-foreground">
                    {subtitle}
                </p>
            </div>

            <div class="mt-8">
                <Suspense fallback=|| view! {
                    <div class="space-y-4">
                        <div class="h-40 animate-pulse rounded-2xl bg-muted"></div>
                        <div class="grid gap-3 md:grid-cols-2">
                            <div class="h-28 animate-pulse rounded-2xl bg-muted"></div>
                            <div class="h-28 animate-pulse rounded-2xl bg-muted"></div>
                        </div>
                    </div>
                }>
                    {move || {
                        let posts_resource = posts_resource;
                        let load_error = load_error.clone();
                        let comments_page = comments_pagination::comments_page_from_query(
                            comments_page_query.get(),
                        );
                        Suspend::new(async move {
                            match posts_resource.await {
                                Ok(data) => view! {
                                    <BlogShowcase data comments_page />
                                }
                                .into_any(),
                                Err(err) => view! {
                                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {core::error_with_context(load_error.as_str(), &err.to_string())}
                                    </div>
                                }
                                .into_any(),
                            }
                        })
                    }}
                </Suspense>
            </div>
        </section>
    }
}

#[component]
fn BlogShowcase(data: StorefrontBlogData, comments_page: u64) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
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
    if let Some(ref sel) = data.selected_post {
        if let (Some(id), Some(name)) = (&sel.category_id, &sel.category_name) {
            categories_map
                .entry(id.clone())
                .or_insert_with(|| name.clone());
        }
    }

    let other_posts = if let Some(slug) = selected_post_slug.as_deref() {
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
    let active_search = search_query.get();
    let active_tag = tag_query.get();
    let active_category = category_query.get();

    let other_posts = if let Some(q) = active_search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let q_lower = q.to_lowercase();
        other_posts
            .into_iter()
            .filter(|p| {
                p.title.to_lowercase().contains(&q_lower)
                    || p.excerpt
                        .as_deref()
                        .map(|e| e.to_lowercase().contains(&q_lower))
                        .unwrap_or(false)
                    || p.tags.iter().any(|t| t.to_lowercase().contains(&q_lower))
                    || p.category_name
                        .as_deref()
                        .map(|c| c.to_lowercase().contains(&q_lower))
                        .unwrap_or(false)
            })
            .collect()
    } else {
        other_posts
    };

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
                        <a href="?category=" class="ml-1 font-bold text-muted-foreground hover:text-foreground">"×"</a>
                    </span>
                })}
                {active_tag.map(|tag| view! {
                    <span class="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2.5 py-1 text-xs font-medium text-primary">
                        {format!("#{tag}")}
                        <a href="?tag=" class="ml-1 font-bold text-muted-foreground hover:text-foreground">"×"</a>
                    </span>
                })}
                {active_search.as_ref().map(|q| view! {
                    <span class="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2.5 py-1 text-xs font-medium text-primary">
                        {format!("Search: \"{q}\"")}
                        <a href="?q=" class="ml-1 font-bold text-muted-foreground hover:text-foreground">"×"</a>
                    </span>
                })}
                <a href="?category=&tag=&q=" class="ml-auto text-xs text-muted-foreground hover:text-foreground underline">
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
                    href="?category="
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
                        let cat_link = format!("?category={}", cat_id);
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
                    <input
                        type="search"
                        name="q"
                        value=active_search.unwrap_or_default()
                        placeholder=t(locale.as_deref(), "blog.search.placeholder", "Search articles and tags...")
                        class="w-full rounded-full border border-border bg-background px-4 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary/50 shadow-sm"
                    />
                </form>
                <a
                    href="/blog/feed.xml"
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
        </div>
    }
}

#[component]
fn SelectedPostCard(post: Option<BlogPostDetail>, comments_page: u64) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let token = use_token();
    let Some(post) = post else {
        let empty_state = core::selected_post_empty_state_typed_view(
            t(
                locale.as_deref(),
                "blog.selected.emptyTitle",
                "Pick a published post",
            ),
            t(
                locale.as_deref(),
                "blog.selected.emptyBody",
                "Open a post from the list below with `?slug=` or publish one from the blog admin package.",
            ),
        );
        return view! {
            <article class="rounded-2xl border border-dashed border-border p-6">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {empty_state.title}
                </h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {empty_state.body}
                </p>
            </article>
        }
        .into_any();
    };

    let post_id = post.id;
    let title_str = post.title.clone();
    let effective_locale = post.effective_locale;
    let status = post.status;
    let (slug, excerpt, published_at) = core::selected_post_fallback_fields(
        post.slug,
        &t(
            locale.as_deref(),
            "blog.selected.missingSlug",
            "missing-slug",
        ),
        post.excerpt,
        &t(
            locale.as_deref(),
            "blog.selected.noExcerpt",
            "No excerpt yet.",
        ),
        post.published_at,
        &t(
            locale.as_deref(),
            "blog.selected.unscheduled",
            "Unscheduled",
        ),
    );
    let tags = post.tags;
    let public_comments = post.public_comments;
    let content = post.content;
    let content_plain_text = core::fallback_text(
        post.content_plain_text,
        &t(
            locale.as_deref(),
            "blog.selected.noBody",
            "No body content yet.",
        ),
    );
    let selected_post_status = core::selected_post_status_view(
        status,
        t(locale.as_deref(), "blog.selected.unknownStatus", "unknown"),
    );
    let selected_post_meta = core::selected_post_meta_view(
        &t(locale.as_deref(), "blog.selected.slugLabel", "slug"),
        slug.as_str(),
        &t(locale.as_deref(), "blog.selected.localeLabel", "locale"),
        effective_locale.as_str(),
        &t(
            locale.as_deref(),
            "blog.selected.publishedLabel",
            "published",
        ),
        published_at.as_str(),
    );
    let author_profile = post.author_profile;
    let category_name = post.category_name;
    let category_id = post.category_id;
    let featured_image_url = post.featured_image_url;
    let share_slug = slug.clone();
    let reading_minutes = core::calculate_reading_time(&content_plain_text);
    let reading_time_label = core::format_reading_time(reading_minutes, locale.as_deref());
    let selected_post_content = core::selected_post_content_view(excerpt, content_plain_text);
    let selected_post_header =
        core::selected_post_header_view(post.title, selected_post_meta, selected_post_status);
    let comment_form_available = matches!(
        &public_comments.availability,
        BlogCommentsAvailability::Available
    );
    let comment_post_id = post_id.clone();
    let comment_locale = effective_locale.clone();
    let submit_comment = Action::new_local(move |content: &rustok_api::RichTextDocument| {
        let request = crate::model::BlogCommentCreateRequest::for_post(
            comment_post_id.clone(),
            comment_locale.clone(),
            content.clone(),
        );
        let token = token.get_untracked();
        async move {
            transport::create_comment(token, request)
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
    });
    let comment_composer_copy = comment_composer_copy(locale.as_deref());
    #[cfg(target_arch = "wasm32")]
    let comment_composer = if comment_form_available {
        view! {
            <CommentComposer content_locale=effective_locale.clone() submit_action=submit_comment copy=comment_composer_copy />
        }
        .into_any()
    } else {
        ().into_any()
    };
    #[cfg(not(target_arch = "wasm32"))]
    let comment_composer = if comment_form_available {
        let _ = (submit_comment, comment_composer_copy);
        view! {
            <section
                class="mt-6 rounded-2xl border border-border bg-card p-5"
                data-blog-comment-island="true"
                data-post-id=post_id
                data-content-locale=effective_locale.clone()
            >
                <h4 class="text-base font-semibold text-foreground">
                    {t(locale.as_deref(), "blog.comments.composer.title", "Join the discussion")}
                </h4>
                <p class="mt-3 rounded-xl border border-border bg-muted/40 p-4 text-sm text-muted-foreground">
                    {t(locale.as_deref(), "blog.comments.composer.signInRequired", "Sign in to join the discussion.")}
                </p>
            </section>
        }.into_any()
    } else {
        let _ = (submit_comment, comment_composer_copy);
        ().into_any()
    };

    view! {
        <article class="rounded-2xl border border-border bg-background p-6">
            <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.22em] text-muted-foreground">
                {category_name.map(|name| {
                    let cat_link = if let Some(cat_id) = category_id {
                        format!("?category={}", cat_id)
                    } else {
                        format!("?category={}", core::percent_encode(&name))
                    };
                    view! {
                        <a
                            href=cat_link
                            class="rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground text-xs normal-case tracking-normal hover:bg-secondary/80 transition-colors"
                        >
                            {name}
                        </a>
                        <span>{selected_post_header.meta.separator}</span>
                    }
                })}
                {if let Some(author) = author_profile.as_ref() {
                    view! {
                        <span class="font-semibold text-foreground">{author.display_name.clone()}</span>
                        <span>{selected_post_header.meta.separator}</span>
                    }
                    .into_any()
                } else {
                    ().into_any()
                }}
                <span>{selected_post_header.meta.slug_meta}</span>
                <span>{selected_post_header.meta.separator}</span>
                <span>{selected_post_header.meta.locale_meta}</span>
                <span>{selected_post_header.meta.separator}</span>
                <span>{selected_post_header.meta.published_meta}</span>
                <span>{selected_post_header.meta.separator}</span>
                <span>{reading_time_label}</span>
            </div>
            <h3 class="mt-3 text-2xl font-semibold text-foreground">{selected_post_header.title}</h3>
            <div class="mt-3">
                <BlogStatusBadge
                    status=selected_post_header.status.status
                    unknown_label=selected_post_header.status.unknown_label
                />
            </div>
            <p class="mt-3 text-sm text-muted-foreground">{selected_post_content.excerpt}</p>
            {featured_image_url.map(|url| view! {
                <div class="mt-4 aspect-video w-full overflow-hidden rounded-2xl border border-border shadow-sm">
                    <img
                        src=url
                        alt=title_str.clone()
                        loading="lazy"
                        class="h-full w-full object-cover"
                    />
                </div>
            })}
            {
                let content_html = content.as_ref().map(|c| c.html.clone());
                view! {
                    <div class="mt-4 lg:grid lg:grid-cols-12 lg:gap-8 items-start">
                        <div class="lg:col-span-8 min-w-0">
                            {match content {
                                Some(content) => view! {
                                    <RichTextHtml
                                        view=content
                                        content_locale=effective_locale.clone()
                                        class="text-sm leading-7 text-foreground"
                                    />
                                }
                                .into_any(),
                                None => view! {
                                    <p class="whitespace-pre-line text-sm leading-7 text-muted-foreground">
                                        {selected_post_content.body}
                                    </p>
                                }
                                .into_any(),
                            }}
                        </div>
                        <aside class="hidden lg:block lg:col-span-4 min-w-0">
                            <TableOfContents
                                html=content_html.unwrap_or_default()
                                locale=effective_locale.clone()
                            />
                        </aside>
                    </div>
                }
            }
            {if let Some(tags_view) = core::selected_post_tags_view(tags) {
                view! {
                    <div class="mt-5 flex flex-wrap gap-2">
                        {tags_view
                            .items
                            .into_iter()
                            .map(|tag| {
                                let tag_link = format!("?tag={}", core::percent_encode(&tag));
                                view! {
                                    <a
                                        href=tag_link
                                        class="inline-flex rounded-full border border-border px-3 py-1 text-xs text-muted-foreground hover:border-primary/50 hover:text-foreground transition-colors"
                                    >
                                        {format!("#{tag}")}
                                    </a>
                                }
                            })
                            .collect_view()}
                    </div>
                }
                .into_any()
            } else {
                ().into_any()
            }}
            <BlogShareButtons title=title_str slug=share_slug />
            {if let Some(author) = author_profile {
                let initial = author
                    .display_name
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                view! {
                    <div class="mt-6 flex items-center gap-3 rounded-2xl border border-border bg-card p-4">
                        <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-primary text-sm font-bold text-primary-foreground">
                            {initial}
                        </div>
                        <div class="min-w-0 flex-1">
                            <div class="text-sm font-semibold text-foreground">
                                {author.display_name}
                            </div>
                            <div class="text-xs text-muted-foreground">
                                {format!("@{}", author.handle)}
                            </div>
                        </div>
                    </div>
                }
                .into_any()
            } else {
                ().into_any()
            }}
            {comment_composer}
            <PublicCommentsList comments=public_comments comments_page />
        </article>
    }
    .into_any()
}

#[component]
fn PublishedPostsList(
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
                                        let cat_link = if let Some(cat_id) = post.category_id {
                                            format!("?category={}", cat_id)
                                        } else {
                                            format!("?category={}", core::percent_encode(cat))
                                        };
                                        view! {
                                            <a
                                                href=cat_link
                                                class="rounded-full bg-secondary px-2 py-0.5 text-xs font-medium text-secondary-foreground hover:bg-secondary/80 transition-colors"
                                            >
                                                {cat.to_string()}
                                            </a>
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
                                                .cloned()
                                                .map(|tag| {
                                                    let tag_link = format!("?tag={}", core::percent_encode(&tag));
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
