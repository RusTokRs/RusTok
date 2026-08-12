use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;

use crate::i18n::t;
use crate::topic_fork_model::{
    ForumTopicForkIdentity, ForumTopicForkReceipt, ForumTopicForkReplyPage,
    build_forum_topic_fork_command, forum_topic_fork_candidate_label, forum_topic_fork_reply_label,
    new_forum_topic_fork_identity,
};
use crate::transport;

#[component]
pub fn ForumTopicForkAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let requested_locale = ui_locale.clone().unwrap_or_else(|| "en".to_string());
    let candidates_locale = requested_locale.clone();
    let replies_locale = requested_locale.clone();
    let token = use_token();
    let tenant = use_tenant();
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (source_topic_id, set_source_topic_id) = signal(String::new());
    let (root_reply_id, set_root_reply_id) = signal(String::new());
    let (target_locale, set_target_locale) = signal(requested_locale);
    let (target_title, set_target_title) = signal(String::new());
    let (target_slug, set_target_slug) = signal(String::new());
    let (reason, set_reason) = signal(String::new());
    let (identity, set_identity) = signal(new_forum_topic_fork_identity(""));
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (receipt, set_receipt) = signal(None::<ForumTopicForkReceipt>);

    let title = t(
        ui_locale.as_deref(),
        "forum.fork.title",
        "Fork reply branch",
    );
    let subtitle = t(
        ui_locale.as_deref(),
        "forum.fork.subtitle",
        "Copy one reply branch into a new topic while the Forum owner preserves the source.",
    );
    let source_label = t(ui_locale.as_deref(), "forum.fork.source", "Source topic");
    let choose_label = t(ui_locale.as_deref(), "forum.fork.choose", "Choose a topic");
    let root_label = t(ui_locale.as_deref(), "forum.fork.root", "Branch root reply");
    let no_replies_label = t(
        ui_locale.as_deref(),
        "forum.fork.noReplies",
        "Choose a source topic with at least one reply.",
    );
    let locale_label = t(ui_locale.as_deref(), "forum.fork.locale", "Target locale");
    let target_title_label = t(
        ui_locale.as_deref(),
        "forum.fork.targetTitle",
        "Target title",
    );
    let slug_label = t(
        ui_locale.as_deref(),
        "forum.fork.slug",
        "Target slug (optional)",
    );
    let reason_label = t(ui_locale.as_deref(), "forum.fork.reason", "Reason");
    let warning = t(
        ui_locale.as_deref(),
        "forum.fork.warning",
        "Fork copies the selected root and its descendants. The source topic, replies and solution remain unchanged.",
    );
    let submit_label = t(ui_locale.as_deref(), "forum.fork.submit", "Fork branch");
    let pending_label = t(ui_locale.as_deref(), "forum.fork.pending", "Forking…");
    let operation_label = t(
        ui_locale.as_deref(),
        "forum.fork.operation",
        "Retry identity",
    );
    let target_id_label = t(
        ui_locale.as_deref(),
        "forum.fork.targetId",
        "New topic identity",
    );
    let retry_hint = t(
        ui_locale.as_deref(),
        "forum.fork.retryHint",
        "Exact retries keep both identities. Editing the source, root or target fields rotates both.",
    );
    let complete_label = t(
        ui_locale.as_deref(),
        "forum.fork.complete",
        "Fork committed",
    );

    let candidates = LocalResource::new(move || {
        let _ = refresh_nonce.get();
        let token = token.get();
        let tenant = tenant.get();
        let locale = candidates_locale.clone();
        async move { transport::fetch_topic_fork_candidates(token, tenant, locale).await }
    });

    let replies = LocalResource::new(move || {
        let source_topic_id = source_topic_id.get();
        let token = token.get();
        let tenant = tenant.get();
        let locale = replies_locale.clone();
        async move {
            if source_topic_id.trim().is_empty() {
                Ok(ForumTopicForkReplyPage {
                    total: 0,
                    items: Vec::new(),
                })
            } else {
                transport::fetch_topic_fork_replies(token, tenant, source_topic_id, locale).await
            }
        }
    });

    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        set_error.set(None);
        let Some(Ok(reply_page)) = replies.get_untracked() else {
            set_error.set(Some("Source replies are not available".to_string()));
            return;
        };
        let current_identity = identity.get_untracked();
        let command = match build_forum_topic_fork_command(
            &current_identity,
            source_topic_id.get_untracked().as_str(),
            &reply_page,
            root_reply_id.get_untracked().as_str(),
            target_locale.get_untracked().as_str(),
            target_title.get_untracked().as_str(),
            target_slug.get_untracked().as_str(),
            reason.get_untracked().as_str(),
        ) {
            Ok(command) => command,
            Err(message) => {
                set_error.set(Some(message));
                return;
            }
        };
        let token = token.get_untracked();
        let tenant = tenant.get_untracked();
        set_busy.set(true);
        spawn_local(async move {
            match transport::fork_topic(token, tenant, command).await {
                Ok(result) => {
                    set_receipt.set(Some(result));
                    set_source_topic_id.set(String::new());
                    set_root_reply_id.set(String::new());
                    set_target_title.set(String::new());
                    set_target_slug.set(String::new());
                    set_reason.set(String::new());
                    set_identity.set(new_forum_topic_fork_identity(""));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(message) => set_error.set(Some(message)),
            }
            set_busy.set(false);
        });
    };

    view! {
        <section class="space-y-6">
            <header class="rounded-[2rem] border border-border bg-gradient-to-br from-card via-card to-muted/40 px-6 py-7 shadow-sm lg:px-8">
                <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">"FORUM-21W"</p>
                <h1 class="mt-3 text-3xl font-semibold tracking-tight text-card-foreground">{title}</h1>
                <p class="mt-3 max-w-3xl text-sm leading-6 text-muted-foreground">{subtitle}</p>
            </header>

            {move || error.get().map(|message| view! {
                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{message}</div>
            })}

            {move || receipt.get().map(|receipt| view! {
                <article class="rounded-[1.75rem] border border-emerald-500/30 bg-emerald-500/10 p-5 text-sm">
                    <h2 class="font-semibold text-foreground">{complete_label.clone()}</h2>
                    <dl class="mt-3 grid gap-2 text-muted-foreground sm:grid-cols-2">
                        <div><dt class="font-medium text-foreground">"Operation"</dt><dd class="font-mono text-xs">{receipt.operation_id}</dd></div>
                        <div><dt class="font-medium text-foreground">"Target"</dt><dd class="font-mono text-xs">{receipt.target_topic_id}</dd></div>
                        <div><dt class="font-medium text-foreground">"Copied replies"</dt><dd>{receipt.copied_reply_count}</dd></div>
                        <div><dt class="font-medium text-foreground">"Copied published replies"</dt><dd>{receipt.copied_published_reply_count}</dd></div>
                    </dl>
                </article>
            })}

            <form class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]" on:submit=submit>
                <section class="space-y-6 rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
                    <Suspense fallback=move || view! { <div class="h-20 animate-pulse rounded-2xl bg-muted"></div> }>
                        {move || candidates.get().map(|result| match result {
                            Ok(items) => {
                                let option_items = items.clone();
                                view! {
                                    <label class="block space-y-2 text-sm font-medium text-foreground">
                                        <span class="block">{source_label.clone()}</span>
                                        <select
                                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm"
                                            prop:value=move || source_topic_id.get()
                                            on:change=move |event| {
                                                let value = event_target_value(&event);
                                                let selected_locale = option_items
                                                    .iter()
                                                    .find(|item| item.id == value)
                                                    .map(|item| item.locale.clone());
                                                set_source_topic_id.set(value.clone());
                                                set_root_reply_id.set(String::new());
                                                if let Some(locale) = selected_locale {
                                                    set_target_locale.set(locale);
                                                }
                                                rotate_command_identity(
                                                    set_identity,
                                                    set_receipt,
                                                    set_error,
                                                    value.as_str(),
                                                );
                                            }
                                        >
                                            <option value="">{choose_label.clone()}</option>
                                            {items.into_iter().filter(|item| item.reply_count >= 1).map(|item| {
                                                let label = forum_topic_fork_candidate_label(&item);
                                                view! { <option value=item.id>{label}</option> }
                                            }).collect_view()}
                                        </select>
                                    </label>
                                }.into_any()
                            }
                            Err(message) => view! { <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{message}</div> }.into_any(),
                        })}
                    </Suspense>

                    <section class="space-y-3">
                        <h2 class="text-sm font-semibold text-foreground">{root_label}</h2>
                        <Suspense fallback=move || view! { <div class="h-36 animate-pulse rounded-2xl bg-muted"></div> }>
                            {move || replies.get().map(|result| match result {
                                Ok(page) if !page.items.is_empty() => view! {
                                    <div class="max-h-96 space-y-2 overflow-y-auto rounded-2xl border border-border p-3">
                                        {page.items.into_iter().map(|reply| {
                                            let reply_id = reply.id.clone();
                                            let checked_id = reply_id.clone();
                                            let change_id = reply_id.clone();
                                            let label = forum_topic_fork_reply_label(&reply);
                                            view! {
                                                <label class="flex items-start gap-3 rounded-xl px-3 py-2 text-sm hover:bg-muted/50">
                                                    <input
                                                        class="mt-1"
                                                        type="radio"
                                                        name="forum-topic-fork-root"
                                                        prop:checked=move || root_reply_id.get() == checked_id
                                                        on:change=move |_| {
                                                            set_root_reply_id.set(change_id.clone());
                                                            rotate_command_identity(
                                                                set_identity,
                                                                set_receipt,
                                                                set_error,
                                                                source_topic_id.get_untracked().as_str(),
                                                            );
                                                        }
                                                    />
                                                    <span>
                                                        <span class="block text-foreground">{label}</span>
                                                        <span class="mt-1 block break-all font-mono text-[11px] text-muted-foreground">{reply_id}</span>
                                                    </span>
                                                </label>
                                            }
                                        }).collect_view()}
                                    </div>
                                }.into_any(),
                                Ok(_) => view! { <div class="rounded-2xl border border-dashed border-border p-6 text-sm text-muted-foreground">{no_replies_label.clone()}</div> }.into_any(),
                                Err(message) => view! { <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{message}</div> }.into_any(),
                            })}
                        </Suspense>
                    </section>

                    <div class="grid gap-5 md:grid-cols-2">
                        <label class="space-y-2 text-sm font-medium text-foreground">
                            <span class="block">{locale_label}</span>
                            <input class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm" maxlength=64 prop:value=move || target_locale.get() on:input=move |event| {
                                set_target_locale.set(event_target_value(&event));
                                rotate_command_identity(set_identity, set_receipt, set_error, source_topic_id.get_untracked().as_str());
                            } />
                        </label>
                        <label class="space-y-2 text-sm font-medium text-foreground">
                            <span class="block">{slug_label}</span>
                            <input class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm" maxlength=255 prop:value=move || target_slug.get() on:input=move |event| {
                                set_target_slug.set(event_target_value(&event));
                                rotate_command_identity(set_identity, set_receipt, set_error, source_topic_id.get_untracked().as_str());
                            } />
                        </label>
                    </div>

                    <label class="block space-y-2 text-sm font-medium text-foreground">
                        <span class="block">{target_title_label}</span>
                        <input class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm" maxlength=500 prop:value=move || target_title.get() on:input=move |event| {
                            set_target_title.set(event_target_value(&event));
                            rotate_command_identity(set_identity, set_receipt, set_error, source_topic_id.get_untracked().as_str());
                        } />
                    </label>

                    <label class="block space-y-2 text-sm font-medium text-foreground">
                        <span class="block">{reason_label}</span>
                        <textarea class="min-h-28 w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm" maxlength=500 prop:value=move || reason.get() on:input=move |event| {
                            set_reason.set(event_target_value(&event));
                            rotate_command_identity(set_identity, set_receipt, set_error, source_topic_id.get_untracked().as_str());
                        }></textarea>
                    </label>

                    <p class="rounded-2xl border border-amber-500/20 bg-amber-500/10 px-4 py-3 text-sm text-muted-foreground">{warning}</p>
                </section>

                <aside class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm xl:sticky xl:top-6 xl:self-start">
                    <p class="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">{operation_label}</p>
                    <p class="mt-3 break-all font-mono text-xs text-foreground">{move || identity.get().operation_id}</p>
                    <p class="mt-5 text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">{target_id_label}</p>
                    <p class="mt-3 break-all font-mono text-xs text-foreground">{move || identity.get().target_topic_id}</p>
                    <p class="mt-4 text-xs leading-5 text-muted-foreground">{retry_hint}</p>
                    <button type="submit" class="mt-6 w-full rounded-full bg-primary px-5 py-3 text-sm font-medium text-primary-foreground disabled:opacity-60" disabled=move || busy.get()>
                        {move || if busy.get() { pending_label.clone() } else { submit_label.clone() }}
                    </button>
                </aside>
            </form>
        </section>
    }
}

fn rotate_command_identity(
    set_identity: WriteSignal<ForumTopicForkIdentity>,
    set_receipt: WriteSignal<Option<ForumTopicForkReceipt>>,
    set_error: WriteSignal<Option<String>>,
    source_topic_id: &str,
) {
    set_identity.set(new_forum_topic_fork_identity(source_topic_id));
    set_receipt.set(None);
    set_error.set(None);
}
