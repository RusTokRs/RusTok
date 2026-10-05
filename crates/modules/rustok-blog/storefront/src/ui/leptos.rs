use leptos::prelude::*;
use leptos_auth::hooks::use_token;
use leptos_ui::RichTextHtml;
use leptos_ui_routing::{read_route_query_value, use_route_query_value, use_route_query_writer};
#[cfg(target_arch = "wasm32")]
use rustok_comments_storefront_support::CommentComposer;
use rustok_ui_core::UiRouteContext;

use crate::i18n::{comment_composer_copy, t};
use crate::model::{
    BlogCommentList, BlogCommentListItem, BlogCommentsAvailability, BlogPostDetail,
    BlogPostListItem, StorefrontBlogData,
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

    let (other_posts, other_total) = if let Some(slug) = selected_post_slug.as_deref() {
        let items: Vec<_> = data
            .posts
            .items
            .into_iter()
            .filter(|item| item.slug.as_deref() != Some(slug))
            .collect();
        let total = items.len() as u64;
        (items, total)
    } else {
        let total = data.posts.total;
        (data.posts.items, total)
    };

    let related_title = t(
        locale.as_deref(),
        "blog.related.title",
        "Related articles",
    );

    let tag_query = use_route_query_value("tag");
    let category_query = use_route_query_value("category");
    let active_tag = tag_query.get();
    let active_category = category_query.get();

    let has_active_filter = active_tag.is_some() || active_category.is_some();
    let filter_bar = if has_active_filter {
        view! {
            <div class="flex flex-wrap items-center gap-2 rounded-2xl border border-border bg-muted/40 p-3 text-xs">
                <span class="font-medium text-muted-foreground">
                    {t(locale.as_deref(), "blog.filter.active", "Active filters:")}
                </span>
                {active_category.map(|cat| view! {
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
                <a href="?category=&tag=" class="ml-auto text-xs text-muted-foreground hover:text-foreground underline">
                    {t(locale.as_deref(), "blog.filter.clear", "Clear all")}
                </a>
            </div>
        }
        .into_any()
    } else {
        ().into_any()
    };

    view! {
        <div class="space-y-6">
            {filter_bar}
            <SelectedPostCard post=data.selected_post comments_page />
            {if is_post_selected {
                if !other_posts.is_empty() {
                    view! {
                        <div class="mt-8 border-t border-border pt-6">
                            <PublishedPostsList
                                items=other_posts
                                total=other_total
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
                    <PublishedPostsList items=other_posts total=other_total />
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
            {match content {
                Some(content) => view! {
                    <RichTextHtml
                        view=content
                        content_locale=effective_locale.clone()
                        class="mt-4 text-sm leading-7 text-foreground"
                    />
                }
                .into_any(),
                None => view! {
                    <p class="mt-4 whitespace-pre-line text-sm leading-7 text-muted-foreground">
                        {selected_post_content.body}
                    </p>
                }
                .into_any(),
            }}
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
fn PublicCommentsList(comments: BlogCommentList, comments_page: u64) -> impl IntoView {
    if comments.availability == BlogCommentsAvailability::Disabled {
        return ().into_any();
    }

    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let query_writer = use_route_query_writer();
    let title = t(locale.as_deref(), "blog.comments.title", "Comments");

    let degraded_message = match comments.availability {
        BlogCommentsAvailability::Available => None,
        BlogCommentsAvailability::Disabled => None,
        BlogCommentsAvailability::ReadOnly => Some(t(
            locale.as_deref(),
            "blog.comments.readOnly",
            "Comments are closed for new replies.",
        )),
        BlogCommentsAvailability::Unavailable if comments.cached_snapshot => Some(t(
            locale.as_deref(),
            "blog.comments.unavailableCached",
            "Comments are temporarily unavailable. Showing a recent cached snapshot.",
        )),
        BlogCommentsAvailability::Unavailable => Some(t(
            locale.as_deref(),
            "blog.comments.unavailable",
            "Comments are temporarily unavailable. The article is still available.",
        )),
        BlogCommentsAvailability::Timeout if comments.cached_snapshot => Some(t(
            locale.as_deref(),
            "blog.comments.timeoutCached",
            "Comments took too long to load. Showing a recent cached snapshot.",
        )),
        BlogCommentsAvailability::Timeout => Some(t(
            locale.as_deref(),
            "blog.comments.timeout",
            "Comments took too long to load. The article is still available.",
        )),
    };

    if matches!(
        comments.availability,
        BlogCommentsAvailability::Unavailable | BlogCommentsAvailability::Timeout
    ) && !comments.cached_snapshot
    {
        let message = degraded_message.unwrap_or_else(|| {
            t(
                locale.as_deref(),
                "blog.comments.unavailable",
                "Comments are temporarily unavailable.",
            )
        });
        return view! {
            <section class="mt-8 border-t border-border pt-6">
                <h4 class="text-lg font-semibold text-foreground">{title}</h4>
                <p class="mt-3 rounded-xl border border-border bg-muted/40 p-4 text-sm text-muted-foreground">
                    {message}
                </p>
            </section>
        }
        .into_any();
    }

    let total_label = core::count_label(
        comments.total,
        &t(locale.as_deref(), "blog.comments.total", "total"),
    );

    if comments.total == 0 {
        let degraded_message = degraded_message.clone();
        return view! {
            <section class="mt-8 border-t border-border pt-6">
                <div class="flex items-center justify-between gap-3">
                    <h4 class="text-lg font-semibold text-foreground">{title}</h4>
                    <span class="text-xs text-muted-foreground">{total_label}</span>
                </div>
                {degraded_message.map(|message| view! {
                    <p class="mt-3 rounded-xl border border-border bg-muted/40 p-4 text-sm text-muted-foreground">
                        {message}
                    </p>
                })}
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
            {degraded_message.map(|message| view! {
                <p class="mt-3 rounded-xl border border-border bg-muted/40 p-4 text-sm text-muted-foreground">
                    {message}
                </p>
            })}
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
fn PublishedPostsList(
    items: Vec<BlogPostListItem>,
    total: u64,
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
    let header_view = core::published_posts_header_typed_view(
        header_title,
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

#[component]
fn BlogStatusBadge(status: String, unknown_label: String) -> impl IntoView {
    let badge_view = core::status_badge_typed_view(status, unknown_label.as_str());
    view! {
        <span class=badge_view.badge_css>
            {badge_view.label}
        </span>
    }
}

#[component]
pub fn BlogShareButtons(title: String, slug: String) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let is_ru = locale.as_deref() == Some("ru");
    let share_label = if is_ru { "Поделиться" } else { "Share" };
    let telegram_label = "Telegram";
    let vk_label = "VK";
    let twitter_label = "X (Twitter)";

    let article_url = format!("/blog?slug={}", core::percent_encode(&slug));
    let tg_url = core::telegram_share_url(&article_url, &title);
    let vk_url = core::vk_share_url(&article_url, &title);
    let tw_url = core::twitter_share_url(&article_url, &title);

    view! {
        <div class="mt-6 flex flex-wrap items-center gap-2 rounded-2xl border border-border bg-card p-4">
            <span class="mr-1 text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                {share_label}
            </span>
            <a
                href=tg_url
                target="_blank"
                rel="noopener noreferrer"
                class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground hover:bg-muted transition"
            >
                {telegram_label}
            </a>
            <a
                href=vk_url
                target="_blank"
                rel="noopener noreferrer"
                class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground hover:bg-muted transition"
            >
                {vk_label}
            </a>
            <a
                href=tw_url
                target="_blank"
                rel="noopener noreferrer"
                class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground hover:bg-muted transition"
            >
                {twitter_label}
            </a>
        </div>
    }
}
