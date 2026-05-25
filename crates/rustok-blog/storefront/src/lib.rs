mod api;
mod core;
mod i18n;
mod model;
mod transport;

use leptos::prelude::*;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{BlogPostDetail, BlogPostListItem, StorefrontBlogData};

#[component]
pub fn BlogView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_slug =
        core::selected_slug_or_default(read_route_query_value(&route_context, "slug"), "latest");
    let selected_locale = route_context.locale.clone();
    let badge = t(selected_locale.as_deref(), "blog.badge", "blog");
    let title = t(
        selected_locale.as_deref(),
        "blog.title",
        "Stories published from the module package",
    );
    let subtitle = t(
        selected_locale.as_deref(),
        "blog.subtitle",
        "This storefront surface reads blog data through GraphQL with no host-specific blog wiring.",
    );
    let load_error = t(
        selected_locale.as_deref(),
        "blog.error.load",
        "Failed to load blog storefront data",
    );

    let posts_resource = Resource::new_blocking(
        move || (selected_slug.clone(), selected_locale.clone()),
        move |(post_slug, locale)| async move { transport::fetch_blog(post_slug, locale).await },
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
                        Suspend::new(async move {
                            match posts_resource.await {
                                Ok(data) => view! { <BlogShowcase data /> }.into_any(),
                                Err(err) => view! {
                                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {core::error_with_context(load_error.as_str(), &err.to_string())}
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
fn BlogShowcase(data: StorefrontBlogData) -> impl IntoView {
    view! {
        <div class="space-y-6">
            <SelectedPostCard post=data.selected_post />
            <PublishedPostsList items=data.posts.items total=data.posts.total />
        </div>
    }
}

#[component]
fn SelectedPostCard(post: Option<BlogPostDetail>) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let Some(post) = post else {
        let empty_state = core::selected_post_empty_state_typed_view(
            t(locale.as_deref(), "blog.selected.emptyTitle", "Pick a published post"),
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
    let body_format = post.body_format;
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
    let body = core::summarized_body_or_fallback(
        post.body,
        body_format.as_str(),
        &t(
            locale.as_deref(),
            "blog.selected.noBody",
            "No body content yet.",
        ),
        &t(
            locale.as_deref(),
            "blog.body.rawFormat",
            "Stored in `{format}` format. Raw body length: {count} characters.",
        ),
    );
    let selected_post_content = core::selected_post_content_view(excerpt, body);
    let selected_post_header =
        core::selected_post_header_view(post.title, selected_post_meta, selected_post_status);

    view! {
        <article class="rounded-2xl border border-border bg-background p-6">
            <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.22em] text-muted-foreground">
                <span>{selected_post_header.meta.slug_meta}</span>
                <span>{selected_post_header.meta.separator}</span>
                <span>{selected_post_header.meta.locale_meta}</span>
                <span>{selected_post_header.meta.separator}</span>
                <span>{selected_post_header.meta.published_meta}</span>
            </div>
            <h3 class="mt-3 text-2xl font-semibold text-foreground">{selected_post_header.title}</h3>
            <div class="mt-3">
                <BlogStatusBadge
                    status=selected_post_header.status.status
                    unknown_label=selected_post_header.status.unknown_label
                />
            </div>
            <p class="mt-3 text-sm text-muted-foreground">{selected_post_content.excerpt}</p>
            <p class="mt-4 whitespace-pre-line text-sm leading-7 text-muted-foreground">{selected_post_content.body}</p>
            {if let Some(tags_view) = core::selected_post_tags_view(tags) {
                view! {
                    <div class="mt-5 flex flex-wrap gap-2">
                        {tags_view
                            .items
                            .into_iter()
                            .map(|tag| {
                                view! {
                                    <span class="inline-flex rounded-full border border-border px-3 py-1 text-xs text-muted-foreground">
                                        {tag}
                                    </span>
                                }
                            })
                            .collect_view()}
                    </div>
                }
                .into_any()
            } else {
                ().into_any()
            }}
        </article>
    }
    .into_any()
}

#[component]
fn PublishedPostsList(items: Vec<BlogPostListItem>, total: u64) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let route_segment =
        core::route_segment_or_default(route_context.route_segment.as_ref().cloned(), "blog");
    let module_route_base = route_context.module_route_base(route_segment.as_str());
    let unknown_status_label = t(locale.as_deref(), "blog.list.unknownStatus", "unknown");
    let header_view = core::published_posts_header_typed_view(
        t(locale.as_deref(), "blog.list.title", "Published posts"),
        total,
        &t(locale.as_deref(), "blog.list.total", "total"),
    );

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
                    {header_view.title}
                </h3>
                <span class="text-sm text-muted-foreground">
                    {header_view.total_label}
                </span>
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
                        let post_card_view = core::published_post_card_view(
                            post.slug,
                            missing_slug_fallback.as_str(),
                            post.excerpt,
                            &t(locale.as_deref(), "blog.list.noExcerpt", "No excerpt yet."),
                            module_route_base.as_str(),
                            open_label.as_str(),
                            locale_label.as_str(),
                            post.effective_locale.as_str(),
                            post.status,
                        );
                        view! {
                            <article class="rounded-2xl border border-border bg-background p-5">
                                <BlogStatusBadge
                                    status=post_card_view.status
                                    unknown_label=unknown_status_label.clone()
                                />
                                <h4 class="mt-2 text-base font-semibold text-foreground">{post.title}</h4>
                                <p class="mt-2 text-sm text-muted-foreground">
                                    {post_card_view.excerpt}
                                </p>
                                <a class="mt-3 inline-flex text-sm text-primary hover:underline" href=post_card_view.href>
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

#[component]
fn BlogStatusBadge(status: String, unknown_label: String) -> impl IntoView {
    let badge_view = core::status_badge_typed_view(status, unknown_label.as_str());
    view! {
        <span class=badge_view.badge_css>
            {badge_view.label}
        </span>
    }
}
