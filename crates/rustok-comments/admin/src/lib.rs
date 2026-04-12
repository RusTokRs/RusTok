mod api;
mod i18n;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_api::{AdminQueryKey, UiRouteContext};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_comments::{CommentStatus, CommentThreadStatus};

use crate::i18n::t;

#[component]
pub fn CommentsAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let selected_thread_query = use_route_query_value(AdminQueryKey::ThreadId.as_str());
    let requested_locale_query = use_route_query_value(AdminQueryKey::Locale.as_str());
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();
    let badge_text = t(ui_locale.as_deref(), "comments.badge", "comments");
    let title_text = t(
        ui_locale.as_deref(),
        "comments.title",
        "Comments Moderation",
    );
    let subtitle_text = t(
        ui_locale.as_deref(),
        "comments.subtitle",
        "Module-owned moderation surface for generic non-forum comments. This UI is native-first and intentionally does not invent a new GraphQL or REST transport.",
    );
    let target_type_placeholder = t(
        ui_locale.as_deref(),
        "comments.filters.targetTypePlaceholder",
        "Target type",
    );
    let all_thread_statuses = t(
        ui_locale.as_deref(),
        "comments.filters.allThreadStatuses",
        "All thread statuses",
    );
    let open_label = t(ui_locale.as_deref(), "comments.thread.open", "Open");
    let closed_label = t(ui_locale.as_deref(), "comments.thread.closed", "Closed");
    let all_comment_statuses = t(
        ui_locale.as_deref(),
        "comments.filters.allCommentStatuses",
        "All comment statuses",
    );
    let pending_label = t(ui_locale.as_deref(), "comments.comment.pending", "Pending");
    let approved_label = t(ui_locale.as_deref(), "comments.comment.approve", "Approve");
    let spam_label = t(ui_locale.as_deref(), "comments.comment.spam", "Spam");
    let trash_label = t(ui_locale.as_deref(), "comments.comment.trash", "Trash");
    let locale_placeholder = t(
        ui_locale.as_deref(),
        "comments.filters.localePlaceholder",
        "Locale",
    );
    let threads_title = t(ui_locale.as_deref(), "comments.threads.title", "Threads");
    let prev_label = t(ui_locale.as_deref(), "comments.pagination.prev", "Prev");
    let next_label = t(ui_locale.as_deref(), "comments.pagination.next", "Next");
    let load_threads_error = t(
        ui_locale.as_deref(),
        "comments.error.loadThreads",
        "Failed to load threads",
    );
    let thread_detail_title = t(
        ui_locale.as_deref(),
        "comments.detail.title",
        "Thread Detail",
    );
    let thread_label = t(ui_locale.as_deref(), "comments.detail.thread", "Thread");

    let (page, set_page) = signal(1_u64);
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (selected_thread_id, set_selected_thread_id) =
        signal(selected_thread_query.get_untracked());
    let (target_type_filter, set_target_type_filter) = signal(String::new());
    let (thread_status_filter, set_thread_status_filter) = signal("all".to_string());
    let (comment_status_filter, set_comment_status_filter) = signal("all".to_string());
    let (locale, set_locale) = signal(
        requested_locale_query
            .get_untracked()
            .or_else(|| ui_locale.clone())
            .unwrap_or_default(),
    );
    let (mutation_error, set_mutation_error) = signal(Option::<String>::None);
    let (_busy_key, set_busy_key) = signal(Option::<String>::None);
    let locale_query_writer = query_writer.clone();
    let thread_query_writer = query_writer.clone();
    let select_thread_query_writer = thread_query_writer.clone();
    let detail_error_locale = ui_locale.clone();
    let route_effect_locale = ui_locale.clone();

    let threads = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                page.get(),
                refresh_nonce.get(),
                target_type_filter.get(),
                thread_status_filter.get(),
                comment_status_filter.get(),
            )
        },
        move |(_, _, page_value, _, target_type, thread_status, comment_status)| async move {
            api::fetch_threads(
                page_value,
                20,
                target_type,
                parse_thread_status(&thread_status),
                parse_comment_status(&comment_status),
            )
            .await
        },
    );

    let detail = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                selected_thread_id.get(),
                refresh_nonce.get(),
                locale.get(),
            )
        },
        move |(_, _, thread_id, _, locale_value)| {
            let detail_error_locale = detail_error_locale.clone();
            async move {
            match thread_id {
                Some(thread_id) => api::fetch_thread_detail(thread_id, locale_value, 1, 100).await,
                None => Err(api::ApiError::ServerFn(t(
                    detail_error_locale.as_deref(),
                    "comments.error.selectThread",
                    "Select a thread first",
                ))),
            }
        }},
    );

    Effect::new(move |_| {
        set_selected_thread_id.set(selected_thread_query.get());
        set_locale.set(
            requested_locale_query
                .get()
                .or_else(|| route_effect_locale.clone())
                .unwrap_or_default(),
        );
    });

    let update_thread_status = move |status: CommentThreadStatus| {
        set_mutation_error.set(None);
        let Some(thread_id) = selected_thread_id.get_untracked() else {
            return;
        };
        set_busy_key.set(Some(format!("thread:{thread_id}")));
        spawn_local(async move {
            match api::set_thread_status(thread_id, status).await {
                Ok(_) => set_refresh_nonce.update(|value| *value += 1),
                Err(err) => set_mutation_error.set(Some(format!(
                    "{}: {err}",
                    t(
                        locale.get_untracked().as_str().into(),
                        "comments.error.updateThread",
                        "Failed to update thread",
                    )
                ))),
            }
            set_busy_key.set(None);
        });
    };

    let update_comment_status = move |comment_id: String, status: CommentStatus| {
        let locale_value = locale.get_untracked();
        set_busy_key.set(Some(format!("comment:{comment_id}")));
        spawn_local(async move {
            match api::set_comment_status(comment_id, status, locale_value.clone()).await {
                Ok(_) => set_refresh_nonce.update(|value| *value += 1),
                Err(err) => set_mutation_error.set(Some(format!(
                    "{}: {err}",
                    t(
                        Some(locale_value.as_str()),
                        "comments.error.updateComment",
                        "Failed to update comment",
                    )
                ))),
            }
            set_busy_key.set(None);
        });
    };

    view! {
        <div class="space-y-6">
            <header class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-2">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        {badge_text}
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">{title_text}</h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">{subtitle_text}</p>
                </div>
            </header>

            {move || mutation_error.get().map(|error| view! {
                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                    {error}
                </div>
            })}

            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="grid gap-3 md:grid-cols-4">
                    <input
                        class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                        placeholder=target_type_placeholder
                        prop:value=target_type_filter
                        on:input=move |ev| set_target_type_filter.set(event_target_value(&ev))
                    />
                    <select
                        class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                        on:change=move |ev| set_thread_status_filter.set(event_target_value(&ev))
                    >
                        <option value="all">{all_thread_statuses}</option>
                        <option value="open">{open_label.clone()}</option>
                        <option value="closed">{closed_label.clone()}</option>
                    </select>
                    <select
                        class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                        on:change=move |ev| set_comment_status_filter.set(event_target_value(&ev))
                    >
                        <option value="all">{all_comment_statuses}</option>
                        <option value="pending">{pending_label.clone()}</option>
                        <option value="approved">{approved_label.clone()}</option>
                        <option value="spam">{spam_label.clone()}</option>
                        <option value="trash">{trash_label.clone()}</option>
                    </select>
                            <input
                                class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                                placeholder=locale_placeholder
                                prop:value=locale
                                on:input=move |ev| {
                                    let next_value = event_target_value(&ev);
                                    set_locale.set(next_value.clone());
                                    if next_value.trim().is_empty() {
                                        locale_query_writer.clear_key(AdminQueryKey::Locale.as_str());
                                    } else {
                                        locale_query_writer.replace_value(AdminQueryKey::Locale.as_str(), next_value);
                                    }
                                }
                            />
                </div>
            </section>

            <div class="grid gap-6 xl:grid-cols-[1.1fr_1.3fr]">
                <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="mb-4 flex items-center justify-between gap-4">
                        <h2 class="text-lg font-semibold text-card-foreground">{threads_title}</h2>
                        <div class="flex items-center gap-2">
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-2 text-sm disabled:opacity-60"
                                disabled=move || page.get() <= 1
                                on:click=move |_| set_page.update(|value| *value = value.saturating_sub(1).max(1))
                            >
                                {prev_label.clone()}
                            </button>
                            <span class="text-sm text-muted-foreground">{move || {
                                t(
                                    locale.get().as_str().into(),
                                    "comments.pagination.page",
                                    "Page {count}",
                                )
                                .replace("{count}", &page.get().to_string())
                            }}</span>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-2 text-sm"
                                on:click=move |_| set_page.update(|value| *value += 1)
                            >
                                {next_label.clone()}
                            </button>
                        </div>
                    </div>
                    <Suspense fallback=move || view! { <div class="h-64 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || {
                            threads.get().map(|result| match result {
                                Ok(payload) => view! {
                                    <div class="space-y-3">
                                        <div class="text-sm text-muted-foreground">
                                            {t(
                                                locale.get().as_str().into(),
                                                "comments.threads.total",
                                                "{count} matching threads",
                                            ).replace("{count}", &payload.total.to_string())}
                                        </div>
                                        <div class="space-y-2">
                                            {payload.items.into_iter().map(|thread| {
                                                let thread_id = thread.id.to_string();
                                                let status = format!("{:?}", thread.status).to_lowercase();
                                                let thread_query_writer = select_thread_query_writer.clone();
                                                view! {
                                                    <button
                                                        type="button"
                                                        class="w-full rounded-xl border border-border px-4 py-3 text-left transition hover:border-primary/50 hover:bg-accent/40"
                                                        on:click=move |_| thread_query_writer.push_value(AdminQueryKey::ThreadId.as_str(), thread_id.clone())
                                                    >
                                                        <div class="space-y-1">
                                                            <div class="flex items-center justify-between gap-3">
                                                                <span class="text-sm font-semibold text-card-foreground">
                                                                    {format!("{}:{}", thread.target_type, thread.target_id)}
                                                                </span>
                                                                <span class="rounded-full border border-border px-2 py-1 text-[11px] text-muted-foreground">
                                                                    {status}
                                                                </span>
                                                            </div>
                                                            <div class="text-xs text-muted-foreground">
                                                                {t(
                                                                    locale.get().as_str().into(),
                                                                    "comments.threads.count",
                                                                    "{count} comments",
                                                                ).replace("{count}", &thread.comment_count.to_string())}
                                                            </div>
                                                        </div>
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                }.into_any(),
                                Err(err) => view! {
                                    <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {format!("{}: {err}", load_threads_error.clone())}
                                    </div>
                                }.into_any(),
                            })
                        }}
                    </Suspense>
                </section>

                <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="mb-4 flex items-center justify-between gap-3">
                        <h2 class="text-lg font-semibold text-card-foreground">{thread_detail_title}</h2>
                        <div class="flex items-center gap-2">
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-2 text-sm"
                                on:click=move |_| update_thread_status(CommentThreadStatus::Open)
                            >
                                {open_label}
                            </button>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-2 text-sm"
                                on:click=move |_| update_thread_status(CommentThreadStatus::Closed)
                            >
                                {closed_label}
                            </button>
                        </div>
                    </div>
                    <Suspense fallback=move || view! { <div class="h-72 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || {
                            detail.get().map(|result| match result {
                                Ok(detail) => view! {
                                    <div class="space-y-4">
                                        <div class="rounded-xl border border-border bg-background/60 p-4 text-sm">
                                            <div class="text-xs uppercase tracking-wide text-muted-foreground">{thread_label.clone()}</div>
                                            <div class="mt-2 font-medium text-card-foreground">
                                                {format!("{}:{}", detail.thread.target_type, detail.thread.target_id)}
                                            </div>
                                            <div class="mt-2 text-xs text-muted-foreground">
                                                {t(
                                                    locale.get().as_str().into(),
                                                    "comments.detail.statusLine",
                                                    "{count} comments, status {status}",
                                                )
                                                .replace("{count}", &detail.thread.comment_count.to_string())
                                                .replace("{status}", &format!("{:?}", detail.thread.status))}
                                            </div>
                                        </div>
                                        <div class="space-y-3">
                                            {detail.comments.into_iter().map(|comment| {
                                                let comment_id = comment.id.to_string();
                                                view! {
                                                    <div class="rounded-xl border border-border p-4">
                                                        <div class="flex flex-wrap items-center justify-between gap-3">
                                                            <div class="text-xs text-muted-foreground">
                                                                {t(
                                                                    locale.get().as_str().into(),
                                                                    "comments.detail.authorLine",
                                                                    "author {author} · {created_at}",
                                                                )
                                                                .replace("{author}", &comment.author_id.to_string())
                                                                .replace("{created_at}", comment.created_at.as_str())}
                                                            </div>
                                                            <div class="flex flex-wrap gap-2">
                                                                <StatusButton
                                                                    label=pending_label.clone()
                                                                    on_click=Callback::new({
                                                                        let comment_id = comment_id.clone();
                                                                        move |_| update_comment_status(comment_id.clone(), CommentStatus::Pending)
                                                                    })
                                                                />
                                                                <StatusButton
                                                                    label=approved_label.clone()
                                                                    on_click=Callback::new({
                                                                        let comment_id = comment_id.clone();
                                                                        move |_| update_comment_status(comment_id.clone(), CommentStatus::Approved)
                                                                    })
                                                                />
                                                                <StatusButton
                                                                    label=spam_label.clone()
                                                                    on_click=Callback::new({
                                                                        let comment_id = comment_id.clone();
                                                                        move |_| update_comment_status(comment_id.clone(), CommentStatus::Spam)
                                                                    })
                                                                />
                                                                <StatusButton
                                                                    label=trash_label.clone()
                                                                    on_click=Callback::new(move |_| update_comment_status(comment_id.clone(), CommentStatus::Trash))
                                                                />
                                                            </div>
                                                        </div>
                                                        <div class="mt-3 rounded-lg bg-muted/40 px-3 py-2 text-sm text-card-foreground">
                                                            {comment.body}
                                                        </div>
                                                        <div class="mt-2 text-xs text-muted-foreground">
                                                            {t(
                                                                locale.get().as_str().into(),
                                                                "comments.detail.localeLine",
                                                                "locale {requested} -> {effective}",
                                                            )
                                                            .replace("{requested}", comment.requested_locale.as_str())
                                                            .replace("{effective}", comment.effective_locale.as_str())}
                                                        </div>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                }.into_any(),
                                Err(err) => view! {
                                    <div class="rounded-xl border border-dashed border-border px-4 py-8 text-sm text-muted-foreground">
                                        {format!("{err}")}
                                    </div>
                                }.into_any(),
                            })
                        }}
                    </Suspense>
                </section>
            </div>
        </div>
    }
}

#[component]
fn StatusButton(label: String, on_click: Callback<()>) -> impl IntoView {
    view! {
        <button
            type="button"
            class="rounded-full border border-border px-3 py-1 text-[11px] text-muted-foreground"
            on:click=move |_| on_click.run(())
        >
            {label}
        </button>
    }
}

fn parse_thread_status(value: &str) -> Option<CommentThreadStatus> {
    match value {
        "open" => Some(CommentThreadStatus::Open),
        "closed" => Some(CommentThreadStatus::Closed),
        _ => None,
    }
}

fn parse_comment_status(value: &str) -> Option<CommentStatus> {
    match value {
        "pending" => Some(CommentStatus::Pending),
        "approved" => Some(CommentStatus::Approved),
        "spam" => Some(CommentStatus::Spam),
        "trash" => Some(CommentStatus::Trash),
        _ => None,
    }
}
