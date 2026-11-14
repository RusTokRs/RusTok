use leptos::prelude::*;
use leptos_ui_routing::use_route_query_writer;
use rustok_ui_core::UiRouteContext;

use crate::comments_pagination;
use crate::i18n::t;
use crate::model::{BlogCommentList, BlogCommentListItem};
use rustok_ui_leptos::{Button, ButtonVariant, Size};

#[component]
pub fn PublicCommentsList(
    comments: BlogCommentList,
    comments_page: u64,
    #[prop(optional)] on_reply: Option<Callback<String>>,
) -> impl IntoView {
    let state = crate::core::discussion::comments_view(&comments, comments_page);
    if !state.show_section {
        return ().into_any();
    }

    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let query_writer = use_route_query_writer();
    let title = t(locale.as_deref(), "blog.comments.title", "Comments");

    let degraded_message = state
        .degraded_message
        .map(|(key, fallback)| t(locale.as_deref(), key, fallback));

    if !state.show_snapshot {
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

    let on_reply = on_reply.filter(|_| state.can_reply);
    let total_label = format!(
        "{} {}",
        comments.total,
        t(locale.as_deref(), "blog.comments.total", "total")
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

    let total_pages = state.total_pages;
    let current_page = state.page;
    let can_previous = state.can_previous;
    let can_next = state.can_next;
    let previous_writer = query_writer.clone();
    let next_writer = query_writer;
    let previous_label = t(locale.as_deref(), "blog.comments.previous", "Previous");
    let next_label = t(locale.as_deref(), "blog.comments.next", "Next");

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
            {if !comments.items.is_empty() {
                view! {
                    <div class="mt-4 space-y-3">
                        {crate::core::discussion::order_comments(comments.items)
                            .into_iter()
                            .map(|(comment, depth)| view! { <PublicCommentCard comment depth on_reply /> })
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
                <Button
                    size=Size::Sm variant=ButtonVariant::Outline
                    disabled=!can_previous
                    on_click=Box::new(move || {
                        previous_writer.apply_query_intent(
                            comments_pagination::comments_page_query_intent(
                                state.previous_page,
                            ),
                        );
                    })
                >
                    {previous_label}
                </Button>
                <Button
                    size=Size::Sm variant=ButtonVariant::Outline
                    disabled=!can_next
                    on_click=Box::new(move || {
                        next_writer.apply_query_intent(
                            comments_pagination::comments_page_query_intent(
                                state.next_page,
                            ),
                        );
                    })
                >
                    {next_label}
                </Button>
            </div>
        </section>
    }
    .into_any()
}

#[component]
fn PublicCommentCard(
    comment: BlogCommentListItem,
    depth: usize,
    on_reply: Option<Callback<String>>,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let locale_meta = format!(
        "{}: {}",
        t(locale.as_deref(), "blog.comments.localeLabel", "locale"),
        comment.effective_locale
    );
    let created_meta = format!(
        "{}: {}",
        t(locale.as_deref(), "blog.comments.createdLabel", "created"),
        comment.created_at
    );
    let comment_id = comment.id;
    let is_reply = comment.parent_comment_id.is_some();

    view! {
        <article class=format!("rounded-xl border border-border bg-card/50 p-4 {}", crate::core::discussion::comment_indent(depth))>
            <div class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                <span>{locale_meta}</span>
                <span>{"·"}</span>
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
            {on_reply.map(|reply| view! {
                <Button size=Size::Sm variant=ButtonVariant::Outline on_click=Box::new(move || reply.run(comment_id.clone()))>
                    {t(locale.as_deref(), "blog.comments.replyAction", "Reply")}
                </Button>
            })}
        </article>
    }
}
