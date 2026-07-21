use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::use_route_query_value;
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

use crate::i18n::t;
use crate::model::{BlogModerationComment, BlogModerationCommentList, BlogModerationStatus};
use crate::transport;

#[component]
pub(crate) fn BlogModerationPanel() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let resource_locale = ui_locale.clone();
    let action_locale = ui_locale.clone();
    let panel_locale = ui_locale;
    let selected_post_query = use_route_query_value(AdminQueryKey::PostId.as_str());
    let token = use_token();
    let tenant = use_tenant();
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (busy_comment_id, set_busy_comment_id) = signal(Option::<String>::None);
    let (action_error, set_action_error) = signal(Option::<String>::None);

    let comments_resource = LocalResource::new(move || {
        let post_id = selected_post_query.get();
        let token = token.get();
        let tenant = tenant.get();
        let locale = resource_locale.clone();
        let _ = refresh_nonce.get();

        async move {
            let Some(post_id) = post_id
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
            else {
                return Ok::<Option<BlogModerationCommentList>, transport::ApiError>(None);
            };

            transport::fetch_moderation_comments(token, tenant, post_id, locale)
                .await
                .map(Some)
        }
    });

    let moderate_comment = Callback::new(
        move |(comment_id, status): (String, BlogModerationStatus)| {
            let token = token.get_untracked();
            let tenant = tenant.get_untracked();
            let locale = action_locale.clone();
            set_action_error.set(None);
            set_busy_comment_id.set(Some(comment_id.clone()));

            spawn_local(async move {
                match transport::moderate_comment(
                    token,
                    tenant,
                    comment_id,
                    status,
                    locale,
                )
                .await
                {
                    Ok(true) => set_refresh_nonce.update(|value| *value += 1),
                    Ok(false) => set_action_error.set(Some(
                        "Comment moderation returned false".to_string(),
                    )),
                    Err(error) => set_action_error.set(Some(error.to_string())),
                }
                set_busy_comment_id.set(None);
            });
        },
    );

    view! {
        <section class="mx-auto mt-6 max-w-7xl rounded-3xl border border-border bg-card p-6 shadow-sm">
            <div class="flex flex-wrap items-start justify-between gap-3">
                <div>
                    <h2 class="text-xl font-semibold text-card-foreground">
                        {t(panel_locale.as_deref(), "blog.moderation.title", "Comment moderation")}
                    </h2>
                    <p class="mt-1 text-sm text-muted-foreground">
                        {t(
                            panel_locale.as_deref(),
                            "blog.moderation.subtitle",
                            "Review the selected post's non-deleted comment queue through the Comments owner boundary.",
                        )}
                    </p>
                </div>
                <span class="rounded-full border border-border px-3 py-1 text-xs text-muted-foreground">
                    {t(panel_locale.as_deref(), "blog.moderation.graphql", "GraphQL managed")}
                </span>
            </div>

            <Show when=move || action_error.get().is_some()>
                <div class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
                    {move || action_error.get().unwrap_or_default()}
                </div>
            </Show>

            <div class="mt-5">
                <Suspense fallback=move || view! {
                    <div class="space-y-3">
                        <div class="h-20 animate-pulse rounded-xl bg-muted"></div>
                        <div class="h-20 animate-pulse rounded-xl bg-muted"></div>
                    </div>
                }>
                    {move || {
                        comments_resource.get().map(|result| {
                            match result {
                                Ok(None) => view! {
                                    <ModerationEmptyState
                                        message=t(
                                            panel_locale.as_deref(),
                                            "blog.moderation.selectPost",
                                            "Open a post to review its comments.",
                                        )
                                    />
                                }.into_any(),
                                Ok(Some(comments)) => view! {
                                    <ModerationQueue
                                        comments
                                        busy_comment_id=busy_comment_id.get()
                                        on_moderate=moderate_comment
                                    />
                                }.into_any(),
                                Err(error) if transport::is_moderation_contract_unavailable(&error) => view! {
                                    <ModerationEmptyState
                                        message=t(
                                            panel_locale.as_deref(),
                                            "blog.moderation.unavailable",
                                            "Comment moderation is unavailable in this reduced server build.",
                                        )
                                    />
                                }.into_any(),
                                Err(error) => view! {
                                    <div class="rounded-xl border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
                                        {format!(
                                            "{}: {}",
                                            t(
                                                panel_locale.as_deref(),
                                                "blog.moderation.loadError",
                                                "Failed to load comment moderation queue",
                                            ),
                                            error,
                                        )}
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
fn ModerationQueue(
    comments: BlogModerationCommentList,
    busy_comment_id: Option<String>,
    on_moderate: Callback<(String, BlogModerationStatus)>,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    if comments.items.is_empty() {
        return view! {
            <ModerationEmptyState
                message=t(
                    locale.as_deref(),
                    "blog.moderation.empty",
                    "This post has no comments to moderate.",
                )
            />
        }
        .into_any();
    }

    view! {
        <div class="space-y-3">
            <div class="text-sm text-muted-foreground">
                {t(locale.as_deref(), "blog.moderation.total", "Total")}
                {": "}
                {comments.total}
            </div>
            {comments
                .items
                .into_iter()
                .map(|comment| {
                    let is_busy = busy_comment_id.as_deref() == Some(comment.id.as_str());
                    view! {
                        <ModerationCommentCard comment is_busy on_moderate />
                    }
                })
                .collect_view()}
        </div>
    }
    .into_any()
}

#[component]
fn ModerationCommentCard(
    comment: BlogModerationComment,
    is_busy: bool,
    on_moderate: Callback<(String, BlogModerationStatus)>,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let approve_id = comment.id.clone();
    let spam_id = comment.id.clone();
    let trash_id = comment.id.clone();
    let reply_label = comment.parent_comment_id.is_some().then(|| {
        t(
            locale.as_deref(),
            "blog.moderation.reply",
            "reply",
        )
    });

    view! {
        <article class="rounded-xl border border-border bg-background p-4">
            <div class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                <span class="rounded-full border border-border px-2 py-0.5 uppercase tracking-wide">
                    {comment.status.clone()}
                </span>
                <span>{comment.effective_locale.clone()}</span>
                <span>{"·"}</span>
                <span>{comment.created_at.clone()}</span>
                {reply_label.map(|label| view! {
                    <span class="rounded-full border border-border px-2 py-0.5">{label}</span>
                })}
            </div>
            <p class="mt-3 whitespace-pre-line text-sm leading-6 text-card-foreground">
                {comment.content_preview}
            </p>
            <div class="mt-4 flex flex-wrap gap-2">
                <button
                    type="button"
                    class="rounded-lg border border-emerald-500/40 px-3 py-1.5 text-xs font-medium text-emerald-700 disabled:opacity-50 dark:text-emerald-300"
                    disabled=is_busy
                    on:click=move |_| on_moderate.run((approve_id.clone(), BlogModerationStatus::Approved))
                >
                    {t(locale.as_deref(), "blog.moderation.approve", "Approve")}
                </button>
                <button
                    type="button"
                    class="rounded-lg border border-amber-500/40 px-3 py-1.5 text-xs font-medium text-amber-700 disabled:opacity-50 dark:text-amber-300"
                    disabled=is_busy
                    on:click=move |_| on_moderate.run((spam_id.clone(), BlogModerationStatus::Spam))
                >
                    {t(locale.as_deref(), "blog.moderation.spam", "Mark spam")}
                </button>
                <button
                    type="button"
                    class="rounded-lg border border-destructive/40 px-3 py-1.5 text-xs font-medium text-destructive disabled:opacity-50"
                    disabled=is_busy
                    on:click=move |_| on_moderate.run((trash_id.clone(), BlogModerationStatus::Trash))
                >
                    {t(locale.as_deref(), "blog.moderation.trash", "Move to trash")}
                </button>
            </div>
        </article>
    }
}

#[component]
fn ModerationEmptyState(message: String) -> impl IntoView {
    view! {
        <div class="rounded-xl border border-dashed border-border p-5 text-sm text-muted-foreground">
            {message}
        </div>
    }
}
