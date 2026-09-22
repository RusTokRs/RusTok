use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;

use crate::i18n::t;
use crate::topic_merge_model::{
    ForumTopicMergeReceipt, ForumTopicMergeWinner, build_forum_topic_merge_command,
    forum_topic_merge_candidate_label, forum_topic_merge_requires_solution_choice,
    new_forum_topic_merge_operation_id,
};
use crate::transport;

#[component]
pub fn ForumTopicMergeAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let requested_locale = ui_locale.clone().unwrap_or_else(|| "en".to_string());
    let token = use_token();
    let tenant = use_tenant();
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (source_topic_id, set_source_topic_id) = signal(String::new());
    let (target_topic_id, set_target_topic_id) = signal(String::new());
    let (reason, set_reason) = signal(String::new());
    let (winner, set_winner) = signal(String::new());
    let (operation_id, set_operation_id) = signal(new_forum_topic_merge_operation_id("", ""));
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (receipt, set_receipt) = signal(None::<ForumTopicMergeReceipt>);

    let title = t(ui_locale.as_deref(), "forum.merge.title", "Merge topics");
    let subtitle = t(
        ui_locale.as_deref(),
        "forum.merge.subtitle",
        "Archive one source thread into a retained target through the idempotent Forum owner.",
    );
    let source_label = t(ui_locale.as_deref(), "forum.merge.source", "Source topic");
    let target_label = t(
        ui_locale.as_deref(),
        "forum.merge.target",
        "Retained target topic",
    );
    let choose_label = t(ui_locale.as_deref(), "forum.merge.choose", "Choose a topic");
    let reason_label = t(ui_locale.as_deref(), "forum.merge.reason", "Reason");
    let winner_label = t(
        ui_locale.as_deref(),
        "forum.merge.winner",
        "Accepted solution to retain",
    );
    let source_winner_label = t(
        ui_locale.as_deref(),
        "forum.merge.sourceWinner",
        "Keep the source topic solution",
    );
    let target_winner_label = t(
        ui_locale.as_deref(),
        "forum.merge.targetWinner",
        "Keep the target topic solution",
    );
    let submit_label = t(ui_locale.as_deref(), "forum.merge.submit", "Merge topics");
    let pending_label = t(ui_locale.as_deref(), "forum.merge.pending", "Merging…");
    let operation_label = t(
        ui_locale.as_deref(),
        "forum.merge.operation",
        "Retry identity",
    );
    let retry_hint = t(
        ui_locale.as_deref(),
        "forum.merge.retryHint",
        "Retries keep this identity until source, target, reason, or solution choice changes.",
    );
    let complete_label = t(
        ui_locale.as_deref(),
        "forum.merge.complete",
        "Merge committed",
    );
    let not_enough_label = t(
        ui_locale.as_deref(),
        "forum.merge.notEnough",
        "At least two active topics are required.",
    );
    let error_candidates_label = t(
        ui_locale.as_deref(),
        "forum.merge.errorCandidates",
        "Topic candidates are not available.",
    );
    let error_source_label = t(
        ui_locale.as_deref(),
        "forum.merge.errorSource",
        "Choose the source topic to archive.",
    );
    let error_target_label = t(
        ui_locale.as_deref(),
        "forum.merge.errorTarget",
        "Choose the retained target topic.",
    );

    let candidates = LocalResource::new(move || {
        let _ = refresh_nonce.get();
        let token = token.get();
        let tenant = tenant.get();
        let locale = requested_locale.clone();
        async move { transport::fetch_topic_merge_candidates(token, tenant, locale).await }
    });

    let solution_choice_required = Signal::derive(move || {
        let Some(Ok(items)) = candidates.get() else {
            return false;
        };
        let source = items.iter().find(|item| item.id == source_topic_id.get());
        let target = items.iter().find(|item| item.id == target_topic_id.get());
        matches!((source, target), (Some(source), Some(target)) if forum_topic_merge_requires_solution_choice(source, target))
    });

    let submit = {
        let error_candidates_label = error_candidates_label.clone();
        let error_source_label = error_source_label.clone();
        let error_target_label = error_target_label.clone();
        move |event: SubmitEvent| {
            event.prevent_default();
            set_error.set(None);
            let Some(Ok(items)) = candidates.get_untracked() else {
                set_error.set(Some(error_candidates_label.clone()));
                return;
            };
            let Some(source) = items
                .iter()
                .find(|item| item.id == source_topic_id.get_untracked())
                .cloned()
            else {
                set_error.set(Some(error_source_label.clone()));
                return;
            };
            let Some(target) = items
                .iter()
                .find(|item| item.id == target_topic_id.get_untracked())
                .cloned()
            else {
                set_error.set(Some(error_target_label.clone()));
                return;
            };
            let selected_winner = match winner.get_untracked().as_str() {
                "source" => Some(ForumTopicMergeWinner::Source),
                "target" => Some(ForumTopicMergeWinner::Target),
                _ => None,
            };
            let command = match build_forum_topic_merge_command(
                operation_id.get_untracked().as_str(),
                &source,
                &target,
                reason.get_untracked().as_str(),
                selected_winner,
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
                match transport::merge_topic(token, tenant, command).await {
                    Ok(result) => {
                        set_receipt.set(Some(result));
                        set_source_topic_id.set(String::new());
                        set_target_topic_id.set(String::new());
                        set_reason.set(String::new());
                        set_winner.set(String::new());
                        set_operation_id.set(new_forum_topic_merge_operation_id("", ""));
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(message) => set_error.set(Some(message)),
                }
                set_busy.set(false);
            });
        }
    };

    view! {
        <section class="space-y-6">
            <header class="rounded-[2rem] border border-border bg-gradient-to-br from-card via-card to-muted/40 px-6 py-7 shadow-sm lg:px-8">
                <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">"FORUM-21N"</p>
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
                        <div><dt class="font-medium text-foreground">"Event"</dt><dd class="font-mono text-xs">{receipt.event_id}</dd></div>
                        <div><dt class="font-medium text-foreground">"Moved replies"</dt><dd>{receipt.moved_reply_count}</dd></div>
                        <div><dt class="font-medium text-foreground">"Resulting published replies"</dt><dd>{receipt.resulting_published_reply_count}</dd></div>
                    </dl>
                </article>
            })}

            <form class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]" on:submit=submit>
                <section class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
                    <Suspense fallback=move || view! { <div class="h-44 animate-pulse rounded-2xl bg-muted"></div> }>
                        {move || candidates.get().map(|result| match result {
                            Ok(items) if items.len() >= 2 => view! {
                                <div class="grid gap-5 md:grid-cols-2">
                                    <label class="space-y-2 text-sm font-medium text-foreground">
                                        <span class="block">{source_label.clone()}</span>
                                        <select
                                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm"
                                            prop:value=move || source_topic_id.get()
                                            on:change=move |event| {
                                                let value = event_target_value(&event);
                                                set_source_topic_id.set(value.clone());
                                                set_winner.set(String::new());
                                                rotate_command_identity(
                                                    set_operation_id,
                                                    set_receipt,
                                                    set_error,
                                                    value.as_str(),
                                                    target_topic_id.get_untracked().as_str(),
                                                );
                                            }
                                        >
                                            <option value="">{choose_label.clone()}</option>
                                            {items.iter().filter(|item| item.id != target_topic_id.get()).cloned().map(|item| {
                                                let label = forum_topic_merge_candidate_label(&item);
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
                                                let value = event_target_value(&event);
                                                set_target_topic_id.set(value.clone());
                                                set_winner.set(String::new());
                                                rotate_command_identity(
                                                    set_operation_id,
                                                    set_receipt,
                                                    set_error,
                                                    source_topic_id.get_untracked().as_str(),
                                                    value.as_str(),
                                                );
                                            }
                                        >
                                            <option value="">{choose_label.clone()}</option>
                                            {items.iter().filter(|item| item.id != source_topic_id.get()).cloned().map(|item| {
                                                let label = forum_topic_merge_candidate_label(&item);
                                                view! { <option value=item.id>{label}</option> }
                                            }).collect_view()}
                                        </select>
                                    </label>
                                </div>
                            }.into_any(),
                            Ok(_) => view! { <div class="rounded-2xl border border-dashed border-border p-6 text-sm text-muted-foreground">{not_enough_label.clone()}</div> }.into_any(),
                            Err(message) => view! { <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{message}</div> }.into_any(),
                        })}
                    </Suspense>

                    <label class="mt-6 block space-y-2 text-sm font-medium text-foreground">
                        <span class="block">{reason_label}</span>
                        <textarea
                            class="min-h-28 w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm"
                            maxlength=500
                            prop:value=move || reason.get()
                            on:input=move |event| {
                                set_reason.set(event_target_value(&event));
                                rotate_command_identity(
                                    set_operation_id,
                                    set_receipt,
                                    set_error,
                                    source_topic_id.get_untracked().as_str(),
                                    target_topic_id.get_untracked().as_str(),
                                );
                            }
                        ></textarea>
                    </label>

                    {move || solution_choice_required.get().then(|| view! {
                        <fieldset class="mt-6 space-y-3 rounded-2xl border border-amber-500/30 bg-amber-500/10 p-4">
                            <legend class="px-1 text-sm font-semibold text-foreground">{winner_label.clone()}</legend>
                            <label class="flex items-center gap-3 text-sm text-foreground">
                                <input type="radio" name="merge-winner" value="source" prop:checked=move || winner.get() == "source" on:change=move |_| {
                                    set_winner.set("source".to_string());
                                    rotate_command_identity(
                                        set_operation_id,
                                        set_receipt,
                                        set_error,
                                        source_topic_id.get_untracked().as_str(),
                                        target_topic_id.get_untracked().as_str(),
                                    );
                                } />
                                {source_winner_label.clone()}
                            </label>
                            <label class="flex items-center gap-3 text-sm text-foreground">
                                <input type="radio" name="merge-winner" value="target" prop:checked=move || winner.get() == "target" on:change=move |_| {
                                    set_winner.set("target".to_string());
                                    rotate_command_identity(
                                        set_operation_id,
                                        set_receipt,
                                        set_error,
                                        source_topic_id.get_untracked().as_str(),
                                        target_topic_id.get_untracked().as_str(),
                                    );
                                } />
                                {target_winner_label.clone()}
                            </label>
                        </fieldset>
                    })}
                </section>

                <aside class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm xl:sticky xl:top-6 xl:self-start">
                    <p class="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">{operation_label}</p>
                    <p class="mt-3 break-all font-mono text-xs text-foreground">{move || operation_id.get()}</p>
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
    set_operation_id: WriteSignal<String>,
    set_receipt: WriteSignal<Option<ForumTopicMergeReceipt>>,
    set_error: WriteSignal<Option<String>>,
    source_topic_id: &str,
    target_topic_id: &str,
) {
    set_operation_id.set(new_forum_topic_merge_operation_id(
        source_topic_id,
        target_topic_id,
    ));
    set_receipt.set(None);
    set_error.set(None);
}
