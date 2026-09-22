use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;

use crate::i18n::t;
use crate::topic_reply_range_model::{
    ForumReplyRangeMoveIdentity, ForumReplyRangeMoveReceipt, build_forum_reply_range_move_command,
    forum_reply_range_move_candidate_label, new_forum_reply_range_move_identity,
};
use crate::transport;

#[component]
pub fn ForumTopicReplyRangeAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let requested_locale = ui_locale.clone().unwrap_or_else(|| "en".to_string());
    let token = use_token();
    let tenant = use_tenant();
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (source_topic_id, set_source_topic_id) = signal(String::new());
    let (target_topic_id, set_target_topic_id) = signal(String::new());
    let (start_position, set_start_position) = signal("1".to_string());
    let (end_position, set_end_position) = signal("1".to_string());
    let (reason, set_reason) = signal(String::new());
    let (identity, set_identity) = signal(new_forum_reply_range_move_identity(""));
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (receipt, set_receipt) = signal(None::<ForumReplyRangeMoveReceipt>);

    let title = t(
        ui_locale.as_deref(),
        "forum.replyRange.title",
        "Move reply range",
    );
    let subtitle = t(
        ui_locale.as_deref(),
        "forum.replyRange.subtitle",
        "Move one inclusive owner-position range into an existing topic.",
    );
    let source_label = t(
        ui_locale.as_deref(),
        "forum.replyRange.source",
        "Source topic",
    );
    let target_label = t(
        ui_locale.as_deref(),
        "forum.replyRange.target",
        "Target topic",
    );
    let choose_label = t(
        ui_locale.as_deref(),
        "forum.replyRange.choose",
        "Choose a topic",
    );
    let start_label = t(
        ui_locale.as_deref(),
        "forum.replyRange.start",
        "Inclusive start position",
    );
    let end_label = t(
        ui_locale.as_deref(),
        "forum.replyRange.end",
        "Inclusive end position",
    );
    let reason_label = t(ui_locale.as_deref(), "forum.replyRange.reason", "Reason");
    let warning = t(
        ui_locale.as_deref(),
        "forum.replyRange.warning",
        "Use canonical owner positions, not row numbers. The owner validates occupied endpoints, bounds, parent edges, ACL, solutions and counters.",
    );
    let operation_label = t(
        ui_locale.as_deref(),
        "forum.replyRange.operation",
        "Retry identity",
    );
    let retry_hint = t(
        ui_locale.as_deref(),
        "forum.replyRange.retryHint",
        "Exact retries keep this identity. Editing source, target, either endpoint or reason rotates it.",
    );
    let submit_label = t(
        ui_locale.as_deref(),
        "forum.replyRange.submit",
        "Move reply range",
    );
    let pending_label = t(ui_locale.as_deref(), "forum.replyRange.pending", "Moving…");
    let complete_label = t(
        ui_locale.as_deref(),
        "forum.replyRange.complete",
        "Move committed",
    );

    let candidates = LocalResource::new(move || {
        let _ = refresh_nonce.get();
        let token = token.get();
        let tenant = tenant.get();
        let locale = requested_locale.clone();
        async move { transport::fetch_reply_range_move_candidates(token, tenant, locale).await }
    });

    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        set_error.set(None);
        let current_identity = identity.get_untracked();
        let command = match build_forum_reply_range_move_command(
            &current_identity,
            source_topic_id.get_untracked().as_str(),
            target_topic_id.get_untracked().as_str(),
            start_position.get_untracked().as_str(),
            end_position.get_untracked().as_str(),
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
            match transport::move_reply_range(token, tenant, command).await {
                Ok(result) => {
                    set_receipt.set(Some(result));
                    set_source_topic_id.set(String::new());
                    set_target_topic_id.set(String::new());
                    set_start_position.set("1".to_string());
                    set_end_position.set("1".to_string());
                    set_reason.set(String::new());
                    set_identity.set(new_forum_reply_range_move_identity(""));
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
                <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">"FORUM-21X"</p>
                <h1 class="mt-3 text-3xl font-semibold tracking-tight text-card-foreground">{title}</h1>
                <p class="mt-3 max-w-3xl text-sm leading-6 text-muted-foreground">{subtitle}</p>
            </header>

            {move || error.get().map(|message| view! {
                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{message}</div>
            })}

            {move || receipt.get().map(|receipt| view! {
                <article class="rounded-[1.75rem] border border-emerald-500/30 bg-emerald-500/10 p-5 text-sm">
                    <h2 class="font-semibold text-foreground">{complete_label.clone()}</h2>
                    <dl class="mt-3 grid gap-2 text-muted-foreground sm:grid-cols-2 lg:grid-cols-3">
                        <div><dt class="font-medium text-foreground">"Operation"</dt><dd class="font-mono text-xs">{receipt.operation_id}</dd></div>
                        <div><dt class="font-medium text-foreground">"Source range"</dt><dd>{format!("{}–{}", receipt.source_start_position, receipt.source_end_position)}</dd></div>
                        <div><dt class="font-medium text-foreground">"Target range"</dt><dd>{format!("{}–{}", receipt.target_start_position, receipt.target_end_position)}</dd></div>
                        <div><dt class="font-medium text-foreground">"Moved replies"</dt><dd>{receipt.moved_reply_count}</dd></div>
                        <div><dt class="font-medium text-foreground">"Moved published replies"</dt><dd>{receipt.moved_published_reply_count}</dd></div>
                        <div><dt class="font-medium text-foreground">"Event"</dt><dd class="font-mono text-xs">{receipt.event_id}</dd></div>
                    </dl>
                </article>
            })}

            <form class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]" on:submit=submit>
                <section class="space-y-6 rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
                    <Suspense fallback=move || view! { <div class="h-20 animate-pulse rounded-2xl bg-muted"></div> }>
                        {move || candidates.get().map(|result| match result {
                            Ok(items) => {
                                let source_items = items.clone();
                                let target_items = items.clone();
                                view! {
                                    <div class="grid gap-5 md:grid-cols-2">
                                        <label class="space-y-2 text-sm font-medium text-foreground">
                                            <span class="block">{source_label.clone()}</span>
                                            <select
                                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm"
                                                prop:value=move || source_topic_id.get()
                                                on:change=move |event| {
                                                    let value = event_target_value(&event);
                                                    set_source_topic_id.set(value.clone());
                                                    if target_topic_id.get_untracked() == value {
                                                        set_target_topic_id.set(String::new());
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
                                                {source_items.into_iter().map(|item| {
                                                    let label = forum_reply_range_move_candidate_label(&item);
                                                    view! { <option value=item.id>{label}</option> }
                                                }).collect_view()}
                                            </select>
                                        </label>
                                        <label class="space-y-2 text-sm font-medium text-foreground">
                                            <span class="block">{target_label.clone()}</span>
                                            <select
                                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm"
                                                prop:value=move || target_topic_id.get()
                                                on:change=move |event| {
                                                    set_target_topic_id.set(event_target_value(&event));
                                                    rotate_command_identity(
                                                        set_identity,
                                                        set_receipt,
                                                        set_error,
                                                        source_topic_id.get_untracked().as_str(),
                                                    );
                                                }
                                            >
                                                <option value="">{choose_label.clone()}</option>
                                                {target_items.into_iter().map(|item| {
                                                    let item_id = item.id.clone();
                                                    let label = forum_reply_range_move_candidate_label(&item);
                                                    view! {
                                                        <option
                                                            value=item.id
                                                            disabled=move || source_topic_id.get() == item_id
                                                        >
                                                            {label}
                                                        </option>
                                                    }
                                                }).collect_view()}
                                            </select>
                                        </label>
                                    </div>
                                }.into_any()
                            }
                            Err(message) => view! {
                                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{message}</div>
                            }.into_any(),
                        })}
                    </Suspense>

                    <div class="grid gap-5 md:grid-cols-2">
                        <label class="space-y-2 text-sm font-medium text-foreground">
                            <span class="block">{start_label}</span>
                            <input
                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm"
                                type="number"
                                min="1"
                                step="1"
                                prop:value=move || start_position.get()
                                on:input=move |event| {
                                    set_start_position.set(event_target_value(&event));
                                    rotate_command_identity(set_identity, set_receipt, set_error, source_topic_id.get_untracked().as_str());
                                }
                            />
                        </label>
                        <label class="space-y-2 text-sm font-medium text-foreground">
                            <span class="block">{end_label}</span>
                            <input
                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm"
                                type="number"
                                min="1"
                                step="1"
                                prop:value=move || end_position.get()
                                on:input=move |event| {
                                    set_end_position.set(event_target_value(&event));
                                    rotate_command_identity(set_identity, set_receipt, set_error, source_topic_id.get_untracked().as_str());
                                }
                            />
                        </label>
                    </div>

                    <label class="block space-y-2 text-sm font-medium text-foreground">
                        <span class="block">{reason_label}</span>
                        <textarea
                            class="min-h-28 w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm"
                            maxlength=500
                            prop:value=move || reason.get()
                            on:input=move |event| {
                                set_reason.set(event_target_value(&event));
                                rotate_command_identity(set_identity, set_receipt, set_error, source_topic_id.get_untracked().as_str());
                            }
                        ></textarea>
                    </label>

                    <p class="rounded-2xl border border-amber-500/20 bg-amber-500/10 px-4 py-3 text-sm text-muted-foreground">{warning}</p>
                </section>

                <aside class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm xl:sticky xl:top-6 xl:self-start">
                    <p class="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">{operation_label}</p>
                    <p class="mt-3 break-all font-mono text-xs text-foreground">{move || identity.get().operation_id}</p>
                    <p class="mt-4 text-xs leading-5 text-muted-foreground">{retry_hint}</p>
                    <button
                        type="submit"
                        class="mt-6 w-full rounded-full bg-primary px-5 py-3 text-sm font-medium text-primary-foreground disabled:opacity-60"
                        disabled=move || busy.get()
                    >
                        {move || if busy.get() { pending_label.clone() } else { submit_label.clone() }}
                    </button>
                </aside>
            </form>
        </section>
    }
}

fn rotate_command_identity(
    set_identity: WriteSignal<ForumReplyRangeMoveIdentity>,
    set_receipt: WriteSignal<Option<ForumReplyRangeMoveReceipt>>,
    set_error: WriteSignal<Option<String>>,
    seed: &str,
) {
    set_identity.set(new_forum_reply_range_move_identity(seed));
    set_receipt.set(None);
    set_error.set(None);
}
