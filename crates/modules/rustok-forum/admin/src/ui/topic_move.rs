use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_auth_ui::ui::leptos::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;

use crate::i18n::t;
use crate::topic_move_model::{
    ForumTopicMoveReceipt, build_forum_topic_move_command, forum_topic_move_candidate_label,
    forum_topic_move_category_label, new_forum_topic_move_operation_id,
};
use crate::transport;

#[component]
pub fn ForumTopicMoveAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let requested_locale = ui_locale.clone().unwrap_or_else(|| "en".to_string());
    let token = use_token();
    let tenant = use_tenant();
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (topic_id, set_topic_id) = signal(String::new());
    let (target_category_id, set_target_category_id) = signal(String::new());
    let (reason, set_reason) = signal(String::new());
    let (operation_id, set_operation_id) = signal(new_forum_topic_move_operation_id("", ""));
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (receipt, set_receipt) = signal(None::<ForumTopicMoveReceipt>);

    let title = t(ui_locale.as_deref(), "forum.move.title", "Move topic");
    let subtitle = t(
        ui_locale.as_deref(),
        "forum.move.subtitle",
        "Move one active topic between active categories through the idempotent Forum owner.",
    );
    let topic_label = t(ui_locale.as_deref(), "forum.move.topic", "Topic");
    let category_label = t(
        ui_locale.as_deref(),
        "forum.move.category",
        "Target category",
    );
    let choose_topic_label = t(
        ui_locale.as_deref(),
        "forum.move.chooseTopic",
        "Choose a topic",
    );
    let choose_category_label = t(
        ui_locale.as_deref(),
        "forum.move.chooseCategory",
        "Choose a category",
    );
    let reason_label = t(ui_locale.as_deref(), "forum.move.reason", "Reason");
    let submit_label = t(ui_locale.as_deref(), "forum.move.submit", "Move topic");
    let pending_label = t(ui_locale.as_deref(), "forum.move.pending", "Moving…");
    let complete_label = t(
        ui_locale.as_deref(),
        "forum.move.complete",
        "Move committed",
    );
    let operation_label = t(
        ui_locale.as_deref(),
        "forum.move.operation",
        "Retry identity",
    );
    let retry_hint = t(
        ui_locale.as_deref(),
        "forum.move.retryHint",
        "Retries keep this identity until topic, target category, or reason changes.",
    );
    let error_candidates_label = t(
        ui_locale.as_deref(),
        "forum.move.errorCandidates",
        "Topics and categories are not available.",
    );
    let error_topic_label = t(
        ui_locale.as_deref(),
        "forum.move.errorTopic",
        "Choose the topic to move.",
    );

    let candidates = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                requested_locale.clone(),
                refresh_nonce.get(),
            )
        },
        move |(token, tenant, locale, _generation)| async move {
            transport::fetch_topic_move_candidates(token, tenant, locale).await
        },
    );

    let submit_candidates = candidates;
    let submit = {
        let error_candidates_label = error_candidates_label.clone();
        let error_topic_label = error_topic_label.clone();
        move |event: SubmitEvent| {
            event.prevent_default();
            set_error.set(None);
            let Some(Ok((topics, _categories))) = submit_candidates.get_untracked() else {
                set_error.set(Some(error_candidates_label.clone()));
                return;
            };
            let Some(candidate) = topics
                .iter()
                .find(|item| item.id == topic_id.get_untracked())
                .cloned()
            else {
                set_error.set(Some(error_topic_label.clone()));
                return;
            };
            let command = match build_forum_topic_move_command(
                &candidate,
                target_category_id.get_untracked().as_str(),
                reason.get_untracked().as_str(),
                operation_id.get_untracked().as_str(),
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
                match transport::move_topic(token, tenant, command).await {
                    Ok(result) => {
                        set_receipt.set(Some(result));
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
                <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">"FORUM-21A"</p>
                <h1 class="mt-3 text-3xl font-semibold tracking-tight text-card-foreground">{title}</h1>
                <p class="mt-3 max-w-3xl text-sm leading-6 text-muted-foreground">{subtitle}</p>
            </header>

            {move || error.get().map(|message| view! {
                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{message}</div>
            })}

            {move || receipt.get().map(|receipt| view! {
                <article class="rounded-[1.75rem] border border-emerald-500/30 bg-emerald-500/10 p-5 text-sm">
                    <h2 class="font-semibold text-foreground">{complete_label.clone()}</h2>
                    <dl class="mt-3 grid gap-3 text-muted-foreground sm:grid-cols-2">
                        <div><dt class="font-medium text-foreground">"Topic"</dt><dd dir="ltr" class="break-all font-mono text-xs">{receipt.topic_id}</dd></div>
                        <div><dt class="font-medium text-foreground">"Source category"</dt><dd dir="ltr" class="break-all font-mono text-xs">{receipt.source_category_id}</dd></div>
                        <div><dt class="font-medium text-foreground">"Target category"</dt><dd dir="ltr" class="break-all font-mono text-xs">{receipt.target_category_id}</dd></div>
                        <div><dt class="font-medium text-foreground">"Published replies"</dt><dd>{receipt.published_reply_count}</dd></div>
                    </dl>
                </article>
            })}

            <form class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]" on:submit=submit>
                <section class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
                    <Suspense fallback=move || view! { <div class="h-32 animate-pulse rounded-2xl bg-muted"></div> }>
                        {let view_candidates = candidates;
                        move || view_candidates.get().map(|result| match result {
                            Ok((topics, categories)) => {
                                view! {
                                    <div class="space-y-6">
                                        <label class="space-y-2 text-sm font-medium text-foreground">
                                            <span class="block">{topic_label.clone()}</span>
                                            <select
                                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm"
                                                prop:value=move || topic_id.get()
                                                on:change=move |event| {
                                                    let value = event_target_value(&event);
                                                    set_topic_id.set(value.clone());
                                                    set_operation_id.set(new_forum_topic_move_operation_id(
                                                        value.as_str(),
                                                        target_category_id.get_untracked().as_str(),
                                                    ));
                                                    set_receipt.set(None);
                                                    set_error.set(None);
                                                }
                                            >
                                                <option value="">{choose_topic_label.clone()}</option>
                                                {topics.into_iter().map(|item| {
                                                    let label = forum_topic_move_candidate_label(&item);
                                                    view! { <option value=item.id>{label}</option> }
                                                }).collect_view()}
                                            </select>
                                        </label>
                                        <label class="space-y-2 text-sm font-medium text-foreground">
                                            <span class="block">{category_label.clone()}</span>
                                            <select
                                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm"
                                                prop:value=move || target_category_id.get()
                                                on:change=move |event| {
                                                    let value = event_target_value(&event);
                                                    set_target_category_id.set(value.clone());
                                                    set_operation_id.set(new_forum_topic_move_operation_id(
                                                        topic_id.get_untracked().as_str(),
                                                        value.as_str(),
                                                    ));
                                                    set_receipt.set(None);
                                                    set_error.set(None);
                                                }
                                            >
                                                <option value="">{choose_category_label.clone()}</option>
                                                {categories.into_iter().map(|item| {
                                                    let label = forum_topic_move_category_label(&item);
                                                    view! { <option value=item.id>{label}</option> }
                                                }).collect_view()}
                                            </select>
                                        </label>
                                        <label class="space-y-2 text-sm font-medium text-foreground">
                                            <span class="block">{reason_label.clone()}</span>
                                            <textarea
                                                class="min-h-28 w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm"
                                                maxlength=500
                                                prop:value=move || reason.get()
                                                on:input=move |event| {
                                                    set_reason.set(event_target_value(&event));
                                                    set_receipt.set(None);
                                                    set_error.set(None);
                                                }
                                            ></textarea>
                                        </label>
                                    </div>
                                }.into_any()
                            }
                            Err(message) => view! {
                                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{message}</div>
                            }.into_any(),
                        })}
                    </Suspense>
                </section>

                <aside class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm xl:sticky xl:top-6 xl:self-start">
                    <p class="text-xs font-medium text-foreground">{operation_label}</p>
                    <p dir="ltr" class="mt-2 break-all font-mono text-xs text-muted-foreground">{move || operation_id.get()}</p>
                    <p class="mt-3 text-xs leading-5 text-muted-foreground">{retry_hint}</p>
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
