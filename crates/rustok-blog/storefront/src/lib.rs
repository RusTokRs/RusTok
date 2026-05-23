mod api;
mod core;
mod i18n;
mod model;

use leptos::prelude::*;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{BlogPostDetail, BlogPostListItem, StorefrontBlogData};

#[component]
pub fn BlogView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_slug = core::selected_slug_or_default(
        read_route_query_value(&route_context, "slug"),
        "latest",
    );
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
        move |(post_slug, locale)| async move { api::fetch_storefront_blog(post_slug, locale).await },
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
        return view! {
            <article class="rounded-2xl border border-dashed border-border p-6">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {t(locale.as_deref(), "blog.selected.emptyTitle", "Pick a published post")}
                </h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {t(locale.as_deref(), "blog.selected.emptyBody", "Open a post from the list below with `?slug=` or publish one from the blog admin package.")}
                </p>
            </article>
        }
        .into_any();
    };

    let title = post.title;
    let effective_locale = post.effective_locale;
    let slug = core::fallback_text(
        post.slug,
        &t(locale.as_deref(), "blog.selected.missingSlug", "missing-slug"),
    );
    let excerpt = core::fallback_text(
        post.excerpt,
        &t(locale.as_deref(), "blog.selected.noExcerpt", "No excerpt yet."),
    );
    let published_at = core::fallback_text(
        post.published_at,
        &t(locale.as_deref(), "blog.selected.unscheduled", "Unscheduled"),
    );
    let tags = post.tags;
    let body_format = post.body_format;
    let body = core::body_or_fallback(
        post.body.map(|body| {
            core::summarize_content(
                body.as_str(),
                body_format.as_str(),
                &t(
                    locale.as_deref(),
                    "blog.body.rawFormat",
                    "Stored in `{format}` format. Raw body length: {count} characters.",
                ),
            )
        }),
        &t(locale.as_deref(), "blog.selected.noBody", "No body content yet."),
    );

    view! {
        <article class="rounded-2xl border border-border bg-background p-6">
            <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.22em] text-muted-foreground">
                <span>{core::label_value_pair(&t(locale.as_deref(), "blog.selected.slugLabel", "slug"), slug.as_str())}</span>
                <span>"В·"</span>
                <span>{core::label_value_pair(&t(locale.as_deref(), "blog.selected.localeLabel", "locale"), effective_locale.as_str())}</span>
                <span>"В·"</span>
                <span>{core::label_value_pair(&t(locale.as_deref(), "blog.selected.publishedLabel", "published"), published_at.as_str())}</span>
            </div>
            <h3 class="mt-3 text-2xl font-semibold text-foreground">{title}</h3>
            <p class="mt-3 text-sm text-muted-foreground">{excerpt}</p>
            <p class="mt-4 whitespace-pre-line text-sm leading-7 text-muted-foreground">{body}</p>
            {if tags.is_empty() {
                ().into_any()
            } else {
                view! {
                    <div class="mt-5 flex flex-wrap gap-2">
                        {tags
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

    if items.is_empty() {
        return view! {
            <article class="rounded-2xl border border-dashed border-border p-6">
                <p class="text-sm text-muted-foreground">
                    {t(locale.as_deref(), "blog.list.empty", "No published blog posts are available for storefront rendering yet.")}
                </p>
            </article>
        }
        .into_any();
    }

    view! {
        <div class="space-y-3">
            <div class="flex items-center justify-between gap-3">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {t(locale.as_deref(), "blog.list.title", "Published posts")}
                </h3>
                <span class="text-sm text-muted-foreground">
                    {core::count_label(total, &t(locale.as_deref(), "blog.list.total", "total"))}
                </span>
            </div>
            <div class="grid gap-3 md:grid-cols-2">
                {items
                    .into_iter()
                    .map(|post| {
                        let module_route_base = module_route_base.clone();
                        let locale = locale.clone();
                        let slug = core::fallback_slug(
                            post.slug,
                            &t(locale.as_deref(), "blog.selected.missingSlug", "missing-slug"),
                        );
                        let href = core::module_href(module_route_base.as_str(), slug.as_str());
                        view! {
                            <article class="rounded-2xl border border-border bg-background p-5">
                                <div class="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">
                                    {post.status}
                                </div>
                                <h4 class="mt-2 text-base font-semibold text-foreground">{post.title}</h4>
                                <p class="mt-2 text-sm text-muted-foreground">
                                    {core::fallback_excerpt(
                                        post.excerpt,
                                        &t(locale.as_deref(), "blog.list.noExcerpt", "No excerpt yet."),
                                    )}
                                </p>
                                <a class="mt-3 inline-flex text-sm text-primary hover:underline" href=href>
                                    {core::open_link_label(&t(locale.as_deref(), "blog.list.open", "Open"), slug.as_str())}
                                </a>
                                <p class="mt-3 text-xs text-muted-foreground">
                                    {core::label_value_pair(&t(locale.as_deref(), "blog.list.localeLabel", "locale"), post.effective_locale.as_str())}
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
