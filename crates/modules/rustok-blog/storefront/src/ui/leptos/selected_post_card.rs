use super::*;
use leptos::prelude::*;

#[component]
pub(super) fn SelectedPostCard(post: Option<BlogPostDetail>, comments_page: u64) -> impl IntoView {
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
    let selected_tag_filter = use_route_query_value("tag").get();
    let selected_category_filter = use_route_query_value("category").get();
    let selected_search_filter = use_route_query_value("q").get();
    let selected_slug_filter = slug.clone();
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
                    let category_label = if let Some(cat_id) = category_id {
                        view! {
                            <a
                                href=core::filter_query_href(
                                    Some(selected_slug_filter.as_str()),
                                    selected_tag_filter.as_deref(),
                                    Some(&cat_id),
                                    selected_search_filter.as_deref(),
                                )
                                class="rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground text-xs normal-case tracking-normal hover:bg-secondary/80 transition-colors"
                            >
                                {name}
                            </a>
                        }.into_any()
                    } else {
                        // A display name is not a UUID accepted by the Blog owner.
                        view! {
                            <span class="rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground text-xs normal-case tracking-normal">
                                {name}
                            </span>
                        }.into_any()
                    };
                    view! {
                        {category_label}
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
                                let tag_link = core::filter_query_href(
                                    Some(slug.as_str()),
                                    Some(tag.as_str()),
                                    selected_category_filter.as_deref(),
                                    selected_search_filter.as_deref(),
                                );
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
