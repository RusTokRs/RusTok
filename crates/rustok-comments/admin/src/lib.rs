mod api;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_comments::{CommentStatus, CommentThreadStatus};

#[component]
pub fn CommentsAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();

    let (page, set_page) = signal(1_u64);
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (selected_thread_id, set_selected_thread_id) = signal(Option::<String>::None);
    let (target_type_filter, set_target_type_filter) = signal(String::new());
    let (thread_status_filter, set_thread_status_filter) = signal("all".to_string());
    let (comment_status_filter, set_comment_status_filter) = signal("all".to_string());
    let (locale, set_locale) = signal("en".to_string());
    let (mutation_error, set_mutation_error) = signal(Option::<String>::None);
    let (_busy_key, set_busy_key) = signal(Option::<String>::None);

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
        move |(_, _, thread_id, _, locale_value)| async move {
            match thread_id {
                Some(thread_id) => api::fetch_thread_detail(thread_id, locale_value, 1, 100).await,
                None => Err(api::ApiError::ServerFn("Select a thread first".to_string())),
            }
        },
    );

    Effect::new(move |_| {
        if let Some(Ok(payload)) = threads.get() {
            if selected_thread_id.get_untracked().is_none() {
                if let Some(first) = payload.items.first() {
                    set_selected_thread_id.set(Some(first.id.to_string()));
                }
            }
        }
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
                Err(err) => set_mutation_error.set(Some(format!("Failed to update thread: {err}"))),
            }
            set_busy_key.set(None);
        });
    };

    let update_comment_status = move |comment_id: String, status: CommentStatus| {
        let locale_value = locale.get_untracked();
        set_busy_key.set(Some(format!("comment:{comment_id}")));
        spawn_local(async move {
            match api::set_comment_status(comment_id, status, locale_value).await {
                Ok(_) => set_refresh_nonce.update(|value| *value += 1),
                Err(err) => {
                    set_mutation_error.set(Some(format!("Failed to update comment: {err}")))
                }
            }
            set_busy_key.set(None);
        });
    };

    view! {
        <div class="space-y-6">
            <header class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-2">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        "comments"
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">"Comments Moderation"</h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        "Module-owned moderation surface for generic non-forum comments. This UI is native-first and intentionally does not invent a new GraphQL or REST transport."
                    </p>
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
                        placeholder="Target type"
                        prop:value=target_type_filter
                        on:input=move |ev| set_target_type_filter.set(event_target_value(&ev))
                    />
                    <select
                        class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                        on:change=move |ev| set_thread_status_filter.set(event_target_value(&ev))
                    >
                        <option value="all">"All thread statuses"</option>
                        <option value="open">"Open"</option>
                        <option value="closed">"Closed"</option>
                    </select>
                    <select
                        class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                        on:change=move |ev| set_comment_status_filter.set(event_target_value(&ev))
                    >
                        <option value="all">"All comment statuses"</option>
                        <option value="pending">"Pending"</option>
                        <option value="approved">"Approved"</option>
                        <option value="spam">"Spam"</option>
                        <option value="trash">"Trash"</option>
                    </select>
                    <input
                        class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                        placeholder="Locale"
                        prop:value=locale
                        on:input=move |ev| set_locale.set(event_target_value(&ev))
                    />
                </div>
            </section>

            <div class="grid gap-6 xl:grid-cols-[1.1fr_1.3fr]">
                <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="mb-4 flex items-center justify-between gap-4">
                        <h2 class="text-lg font-semibold text-card-foreground">"Threads"</h2>
                        <div class="flex items-center gap-2">
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-2 text-sm disabled:opacity-60"
                                disabled=move || page.get() <= 1
                                on:click=move |_| set_page.update(|value| *value = value.saturating_sub(1).max(1))
                            >
                                "Prev"
                            </button>
                            <span class="text-sm text-muted-foreground">{move || format!("Page {}", page.get())}</span>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-2 text-sm"
                                on:click=move |_| set_page.update(|value| *value += 1)
                            >
                                "Next"
                            </button>
                        </div>
                    </div>
                    <Suspense fallback=move || view! { <div class="h-64 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || {
                            threads.get().map(|result| match result {
                                Ok(payload) => view! {
                                    <div class="space-y-3">
                                        <div class="text-sm text-muted-foreground">
                                            {format!("{} matching threads", payload.total)}
                                        </div>
                                        <div class="space-y-2">
                                            {payload.items.into_iter().map(|thread| {
                                                let thread_id = thread.id.to_string();
                                                let status = format!("{:?}", thread.status).to_lowercase();
                                                view! {
                                                    <button
                                                        type="button"
                                                        class="w-full rounded-xl border border-border px-4 py-3 text-left transition hover:border-primary/50 hover:bg-accent/40"
                                                        on:click=move |_| set_selected_thread_id.set(Some(thread_id.clone()))
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
                                                                {format!("{} comments", thread.comment_count)}
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
                                        {format!("Failed to load threads: {err}")}
                                    </div>
                                }.into_any(),
                            })
                        }}
                    </Suspense>
                </section>

                <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="mb-4 flex items-center justify-between gap-3">
                        <h2 class="text-lg font-semibold text-card-foreground">"Thread Detail"</h2>
                        <div class="flex items-center gap-2">
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-2 text-sm"
                                on:click=move |_| update_thread_status(CommentThreadStatus::Open)
                            >
                                "Open"
                            </button>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-2 text-sm"
                                on:click=move |_| update_thread_status(CommentThreadStatus::Closed)
                            >
                                "Close"
                            </button>
                        </div>
                    </div>
                    <Suspense fallback=move || view! { <div class="h-72 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || {
                            detail.get().map(|result| match result {
                                Ok(detail) => view! {
                                    <div class="space-y-4">
                                        <div class="rounded-xl border border-border bg-background/60 p-4 text-sm">
                                            <div class="text-xs uppercase tracking-wide text-muted-foreground">"Thread"</div>
                                            <div class="mt-2 font-medium text-card-foreground">
                                                {format!("{}:{}", detail.thread.target_type, detail.thread.target_id)}
                                            </div>
                                            <div class="mt-2 text-xs text-muted-foreground">
                                                {format!("{} comments, status {:?}", detail.thread.comment_count, detail.thread.status)}
                                            </div>
                                        </div>
                                        <div class="space-y-3">
                                            {detail.comments.into_iter().map(|comment| {
                                                let comment_id = comment.id.to_string();
                                                view! {
                                                    <div class="rounded-xl border border-border p-4">
                                                        <div class="flex flex-wrap items-center justify-between gap-3">
                                                            <div class="text-xs text-muted-foreground">
                                                                {format!("author {} • {}", comment.author_id, comment.created_at)}
                                                            </div>
                                                            <div class="flex flex-wrap gap-2">
                                                                <StatusButton
                                                                    label="Pending"
                                                                    on_click=Callback::new({
                                                                        let comment_id = comment_id.clone();
                                                                        move |_| update_comment_status(comment_id.clone(), CommentStatus::Pending)
                                                                    })
                                                                />
                                                                <StatusButton
                                                                    label="Approve"
                                                                    on_click=Callback::new({
                                                                        let comment_id = comment_id.clone();
                                                                        move |_| update_comment_status(comment_id.clone(), CommentStatus::Approved)
                                                                    })
                                                                />
                                                                <StatusButton
                                                                    label="Spam"
                                                                    on_click=Callback::new({
                                                                        let comment_id = comment_id.clone();
                                                                        move |_| update_comment_status(comment_id.clone(), CommentStatus::Spam)
                                                                    })
                                                                />
                                                                <StatusButton
                                                                    label="Trash"
                                                                    on_click=Callback::new(move |_| update_comment_status(comment_id.clone(), CommentStatus::Trash))
                                                                />
                                                            </div>
                                                        </div>
                                                        <div class="mt-3 rounded-lg bg-muted/40 px-3 py-2 text-sm text-card-foreground">
                                                            {comment.body}
                                                        </div>
                                                        <div class="mt-2 text-xs text-muted-foreground">
                                                            {format!("locale {} → {}", comment.requested_locale, comment.effective_locale)}
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
fn StatusButton(label: &'static str, on_click: Callback<()>) -> impl IntoView {
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
