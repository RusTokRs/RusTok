use leptos::prelude::*;
use leptos_ui_routing::{
    read_route_query_value, use_route_query_value, use_route_query_writer,
};
use rustok_ui_core::UiRouteContext;

use crate::i18n::t;
use crate::model::{
    BlogCommentList, BlogCommentListItem, BlogPostDetail, BlogPostListItem, StorefrontBlogData,
};
use crate::{comments_pagination, core, transport};

#[component]
pub fn BlogView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_locale = route_context.locale.clone();
    let comments_page_query =
        use_route_query_value(comments_pagination::COMMENTS_PAGE_QUERY_KEY);
    let route_state = core::build_storefront_route_state(
        read_route_query_value(&route_context, core::SELECTED_POST_QUERY_KEY),
        route_context.route_segment.as_ref().cloned(),
    );
    let fetch_request = core::build_storefront_fetch_request(&route_state, selected_locale.clone());
    let shell_view = core::build_storefront_shell_view_model(selected_locale.as_deref());
    let badge = shell_view.badge;
    let title = shell_view.title;
    let subtitle = shell_view.subtitle;
    let load_error = shell_view.load_error;

    let posts_resource = Resource::new_blocking(
        move || {
            (
                fetch_request.clone(),
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
    view! {
        <div class="space-y-6">
            <SelectedPostCard post=data.selected_post comments_page />
            <PublishedPostsList items=data.posts.items total=data.posts.total />
        </div>
    }
}

#[component]
fn SelectedPostCard(post: Option<BlogPostDetail>, comments_page: u64) -> impl IntoView {
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
    let public_comments = post.public_comments;
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
            <PublicCommentsList comments=public_comments comments_page />
        </article>
    }
    .into_any()
}

#[component]
fn PublicCommentsList(comments: BlogCommentList, comments_page: u64) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let query_writer = use_route_query_writer();
    let title = t(locale.as_deref(), "blog.comments.title", "Comments");
    let total_label = core::count_label(
        comments.total,
        &t(locale.as_deref(), "blog.comments.total", "total"),
    );

    if comments.total == 0 {
        return view! {
            <section class="mt-8 border-t border-border pt-6">
                <div class="flex items-center justify-between gap-3">
                    <h4 class="text-lg font-semibold text-foreground">{title}</h4>
                    <span class="text-xs text-muted-foreground">{total_label}</span>
                </div>
                <p class="mt-3 rounded-xl border border-dashed border-border p-4 text-sm text-muted-foreground">
                    {t(
                        locale.as_deref(),
                        "blog.comments.empty",
                        "No approved comments have been published yet.",
                    )}
                </p>
            </section>
        }
        .into_any();
    }

    let total_pages = comments_pagination::comments_total_pages(comments.total);
    let current_page = comments_pagination::bounded_comments_page(comments_page, comments.total);
    let can_previous = current_page > 1;
    let can_next = current_page < total_pages;
    let previous_writer = query_writer.clone();
    let next_writer = query_writer;

    view! {
        <section class="mt-8 border-t border-border pt-6">
            <div class="flex flex-wrap items-center justify-between gap-3">
                <h4 class="text-lg font-semibold text-foreground">{title}</h4>
                <div class="flex items-center gap-3 text-xs text-muted-foreground">
                    <span>{total_label}</span>
                    <span>
                        {t(locale.as_deref(), "blog.comments.page", "Page")}
                        {" "}
                        {current_page}
                        {" / "}
                        {total_pages}
                    </span>
                </div>
            </div>
            {if core::has_items(comments.items.as_slice()) {
                view! {
                    <div class="mt-4 space-y-3">
                        {comments
                            .items
                            .into_iter()
                            .map(|comment| view! { <PublicCommentCard comment /> })
                            .collect_view()}
                    </div>
                }
                .into_any()
            } else {
                view! {
                    <p class="mt-3 rounded-xl border border-dashed border-border p-4 text-sm text-muted-foreground">
                        {t(
                            locale.as_deref(),
                            "blog.comments.emptyPage",
                            "No approved comments are available on this page.",
                        )}
                    </p>
                }
                .into_any()
            }}
            <div class="mt-4 flex items-center justify-end gap-2">
                <button
                    type="button"
                    class="rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-foreground disabled:opacity-40"
                    disabled=!can_previous
                    on:click=move |_| {
                        previous_writer.apply_query_intent(
                            comments_pagination::comments_page_query_intent(
                                current_page.saturating_sub(1).max(1),
                            ),
                        );
                    }
                >
                    {t(locale.as_deref(), "blog.comments.previous", "Previous")}
                </button>
                <button
                    type="button"
                    class="rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-foreground disabled:opacity-40"
                    disabled=!can_next
                    on:click=move |_| {
                        next_writer.apply_query_intent(
                            comments_pagination::comments_page_query_intent(
                                current_page.saturating_add(1).min(total_pages),
                            ),
                        );
                    }
                >
                    {t(locale.as_deref(), "blog.comments.next", "Next")}
                </button>
            </div>
        </section>
    }
    .into_any()
}

#[component]
fn PublicCommentCard(comment: BlogCommentListItem) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let locale_meta = core::label_value_pair(
        &t(locale.as_deref(), "blog.comments.localeLabel", "locale"),
        comment.effective_locale.as_str(),
    );
    let created_meta = core::label_value_pair(
        &t(locale.as_deref(), "blog.comments.createdLabel", "created"),
        comment.created_at.as_str(),
    );
    let is_reply = comment.parent_comment_id.is_some();

    view! {
        <article class="rounded-xl border border-border bg-card/50 p-4">
            <div class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                <span>{locale_meta}</span>
                <span>{core::meta_separator()}</span>
                <span>{created_meta}</span>
                {is_reply.then(|| view! {
                    <span class="rounded-full border border-border px-2 py-0.5">
                        {t(locale.as_deref(), "blog.comments.reply", "reply")}
                    </span>
                })}
            </div>
            <p class="mt-2 whitespace-pre-line text-sm leading-6 text-foreground">
                {comment.content_preview}
            </p>
        </article>
    }
}

#[component]
fn PublishedPostsList(items: Vec<BlogPostListItem>, total: u64) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let route_segment = core::route_segment_or_default(
        route_context.route_segment.as_ref().cloned(),
        core::DEFAULT_ROUTE_SEGMENT,
    );
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
