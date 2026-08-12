use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;

use crate::i18n::t;
use crate::topic_slug_rename_model::{
    ForumTopicSlugRenameReceipt, build_forum_topic_slug_rename_command,
    forum_topic_slug_rename_candidate_label,
};
use crate::transport;

#[component]
pub fn ForumTopicSlugRenameAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let requested_locale = ui_locale.clone().unwrap_or_else(|| "en".to_string());
    let token = use_token();
    let tenant = use_tenant();
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (topic_id, set_topic_id) = signal(String::new());
    let (slug, set_slug) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (receipt, set_receipt) = signal(None::<ForumTopicSlugRenameReceipt>);

    let title = t(
        ui_locale.as_deref(),
        "forum.slugRename.title",
        "Rename topic route",
    );
    let subtitle = t(
        ui_locale.as_deref(),
        "forum.slugRename.subtitle",
        "Change one existing localized topic slug while the Forum owner records the old route as an immutable redirect.",
    );
    let topic_label = t(
        ui_locale.as_deref(),
        "forum.slugRename.topic",
        "Localized topic route",
    );
    let choose_label = t(
        ui_locale.as_deref(),
        "forum.slugRename.choose",
        "Choose a topic",
    );
    let slug_label = t(ui_locale.as_deref(), "forum.slugRename.slug", "New slug");
    let slug_hint = t(
        ui_locale.as_deref(),
        "forum.slugRename.slugHint",
        "The owner normalizes the route segment and preserves the previous path.",
    );
    let submit_label = t(
        ui_locale.as_deref(),
        "forum.slugRename.submit",
        "Rename route",
    );
    let pending_label = t(
        ui_locale.as_deref(),
        "forum.slugRename.pending",
        "Renaming…",
    );
    let complete_label = t(
        ui_locale.as_deref(),
        "forum.slugRename.complete",
        "Route rename committed",
    );
    let replay_label = t(
        ui_locale.as_deref(),
        "forum.slugRename.replay",
        "The canonical slug was already current; no new alias was written.",
    );
    let warning_label = t(
        ui_locale.as_deref(),
        "forum.slugRename.warning",
        "This changes only an existing localized route. Locale creation, route deletion and canonical merge policy remain owner-controlled.",
    );
    let error_candidates_label = t(
        ui_locale.as_deref(),
        "forum.slugRename.errorCandidates",
        "Topic routes are not available.",
    );
    let error_topic_label = t(
        ui_locale.as_deref(),
        "forum.slugRename.errorTopic",
        "Choose the topic route to rename.",
    );

    let candidates = LocalResource::new(move || {
        let _ = refresh_nonce.get();
        let token = token.get();
        let tenant = tenant.get();
        let locale = requested_locale.clone();
        async move { transport::fetch_topic_slug_rename_candidates(token, tenant, locale).await }
    });

    let submit = {
        let error_candidates_label = error_candidates_label.clone();
        let error_topic_label = error_topic_label.clone();
        move |event: SubmitEvent| {
            event.prevent_default();
            set_error.set(None);
            let Some(Ok(items)) = candidates.get_untracked() else {
                set_error.set(Some(error_candidates_label.clone()));
                return;
            };
            let Some(candidate) = items
                .iter()
                .find(|item| item.id == topic_id.get_untracked())
                .cloned()
            else {
                set_error.set(Some(error_topic_label.clone()));
                return;
            };
            let command = match build_forum_topic_slug_rename_command(
                &candidate,
                slug.get_untracked().as_str(),
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
                match transport::rename_topic_slug(token, tenant, command).await {
                    Ok(result) => {
                        set_slug.set(result.slug.clone());
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
                <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">"FORUM-24G"</p>
                <h1 class="mt-3 text-3xl font-semibold tracking-tight text-card-foreground">{title}</h1>
                <p class="mt-3 max-w-3xl text-sm leading-6 text-muted-foreground">{subtitle}</p>
            </header>

            {move || error.get().map(|message| view! {
                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{message}</div>
            })}

            {move || receipt.get().map(|receipt| {
                let status = if receipt.changed {
                    complete_label.clone()
                } else {
                    replay_label.clone()
                };
                view! {
                    <article class="rounded-[1.75rem] border border-emerald-500/30 bg-emerald-500/10 p-5 text-sm">
                        <h2 class="font-semibold text-foreground">{status}</h2>
                        <dl class="mt-3 grid gap-3 text-muted-foreground sm:grid-cols-2">
                            <div><dt class="font-medium text-foreground">"Previous path"</dt><dd class="break-all font-mono text-xs">{receipt.previous_path}</dd></div>
                            <div><dt class="font-medium text-foreground">"Canonical path"</dt><dd class="break-all font-mono text-xs">{receipt.canonical.path}</dd></div>
                            <div><dt class="font-medium text-foreground">"Locale"</dt><dd>{receipt.locale}</dd></div>
                            <div><dt class="font-medium text-foreground">"Alias"</dt><dd class="break-all font-mono text-xs">{receipt.alias_id.unwrap_or_else(|| "—".to_string())}</dd></div>
                        </dl>
                    </article>
                }
            })}

            <form class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]" on:submit=submit>
                <section class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
                    <Suspense fallback=move || view! { <div class="h-32 animate-pulse rounded-2xl bg-muted"></div> }>
                        {move || candidates.get().map(|result| match result {
                            Ok(items) => {
                                let selection_items = items.clone();
                                view! {
                                    <label class="space-y-2 text-sm font-medium text-foreground">
                                        <span class="block">{topic_label.clone()}</span>
                                        <select
                                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm"
                                            prop:value=move || topic_id.get()
                                            on:change=move |event| {
                                                let value = event_target_value(&event);
                                                let current_slug = selection_items
                                                    .iter()
                                                    .find(|item| item.id == value)
                                                    .map(|item| item.slug.clone())
                                                    .unwrap_or_default();
                                                set_topic_id.set(value);
                                                set_slug.set(current_slug);
                                                set_receipt.set(None);
                                                set_error.set(None);
                                            }
                                        >
                                            <option value="">{choose_label.clone()}</option>
                                            {items.into_iter().map(|item| {
                                                let label = forum_topic_slug_rename_candidate_label(&item);
                                                view! { <option value=item.id>{label}</option> }
                                            }).collect_view()}
                                        </select>
                                    </label>
                                }.into_any()
                            }
                            Err(message) => view! {
                                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{message}</div>
                            }.into_any(),
                        })}
                    </Suspense>

                    <label class="mt-6 block space-y-2 text-sm font-medium text-foreground">
                        <span class="block">{slug_label}</span>
                        <input
                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm"
                            maxlength=255
                            prop:value=move || slug.get()
                            on:input=move |event| {
                                set_slug.set(event_target_value(&event));
                                set_receipt.set(None);
                                set_error.set(None);
                            }
                        />
                        <span class="block text-xs font-normal leading-5 text-muted-foreground">{slug_hint}</span>
                    </label>
                </section>

                <aside class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm xl:sticky xl:top-6 xl:self-start">
                    <p class="text-xs leading-5 text-muted-foreground">{warning_label}</p>
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
