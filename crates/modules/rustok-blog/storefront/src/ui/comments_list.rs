use leptos::prelude::*;
use leptos_ui_routing::use_route_query_writer;
use rustok_ui_core::UiRouteContext;

use crate::comments_pagination;
use crate::core;
use crate::i18n::t;
use crate::model::{BlogCommentList, BlogCommentListItem, BlogCommentsAvailability};

#[component]
pub fn PublicCommentsList(comments: BlogCommentList, comments_page: u64) -> impl IntoView {
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
