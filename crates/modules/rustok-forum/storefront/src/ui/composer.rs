use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    forum_storefront_content_locale, ComposerMode, ComposerState, ComposerSubmission,
};
use crate::i18n::t;
use crate::model::ForumCategoryListItem;
use crate::transport;

pub type ComposerSignal = RwSignal<ComposerState>;

#[component]
pub fn ForumComposer(
    #[prop(default = Vec::new())] categories: Vec<ForumCategoryListItem>,
    #[prop(optional)] on_submitted: Option<Callback<String>>,
) -> AnyView {
    let composer = use_context::<ComposerSignal>();

    let Some(composer) = composer else {
        return view! { <div class="hidden"></div> }.into_any();
    };

    let (active_tab, set_active_tab) = signal("write");
    let route_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;

    let draft_label = t(route_locale.as_deref(), "forum.composer.draft", "Draft");
    let expand_label = t(route_locale.as_deref(), "forum.composer.expand", "Expand");
    let minimize_label = t(
        route_locale.as_deref(),
        "forum.composer.minimize",
        "Minimize",
    );
    let close_label = t(route_locale.as_deref(), "forum.composer.close", "Close");
    let reply_label = t(
        route_locale.as_deref(),
        "forum.composer.replyTitle",
        "Reply",
    );
    let new_topic_label = t(
        route_locale.as_deref(),
        "forum.composer.newTopicTitle",
        "Create new topic",
    );
    let topic_title_placeholder = t(
        route_locale.as_deref(),
        "forum.composer.topicTitlePlaceholder",
        "Topic title...",
    );
    let category_placeholder = t(
        route_locale.as_deref(),
        "forum.composer.categoryPlaceholder",
        "Select a category",
    );
    let quotes_label = t(
        route_locale.as_deref(),
        "forum.composer.quotesLabel",
        "Quotes",
    );
    let write_tab_label = t(route_locale.as_deref(), "forum.composer.tabWrite", "Write");
    let preview_tab_label = t(
        route_locale.as_deref(),
        "forum.composer.tabPreview",
        "Preview",
    );
    let body_placeholder = t(
        route_locale.as_deref(),
        "forum.composer.bodyPlaceholder",
        "Write your post here. A blank line starts a new paragraph.",
    );
    let preview_hint_label = t(
        route_locale.as_deref(),
        "forum.composer.previewHint",
        "Live preview",
    );
    let preview_empty_label = t(
        route_locale.as_deref(),
        "forum.composer.previewEmpty",
        "Nothing to preview yet.",
    );
    let draft_active_label = t(
        route_locale.as_deref(),
        "forum.composer.draftActive",
        "Draft active",
    );
    let cancel_label = t(route_locale.as_deref(), "forum.composer.cancel", "Cancel");
    let submit_topic_label = t(
        route_locale.as_deref(),
        "forum.composer.submitTopic",
        "Create topic",
    );
    let submit_reply_label = t(
        route_locale.as_deref(),
        "forum.composer.submitReply",
        "Post reply",
    );
    let submitting_label = t(
        route_locale.as_deref(),
        "forum.composer.submitting",
        "Posting...",
    );
    let submit_failed_label = t(
        route_locale.as_deref(),
        "forum.composer.submitFailed",
        "The forum rejected this post",
    );

    view! {
        {move || {
            let state = composer.get();
            if !state.is_open {
                return view! { <div class="hidden"></div> }.into_any();
            }

            if state.is_minimized {
                let draft_title = match state.mode {
                    ComposerMode::Reply => state
                        .topic_title
                        .clone()
                        .unwrap_or_else(|| reply_label.clone()),
                    ComposerMode::Topic => {
                        if state.title.trim().is_empty() {
                            new_topic_label.clone()
                        } else {
                            state.title.clone()
                        }
                    }
                };

                return view! {
                    <div class="fixed bottom-4 right-4 z-50 flex items-center gap-2 rounded-full border border-border bg-card px-4 py-2.5 shadow-xl transition-all hover:bg-muted/80">
                        <span class="h-2 w-2 rounded-full bg-primary animate-pulse" />
                        <button
                            type="button"
                            class="text-xs font-medium text-foreground hover:underline"
                            on:click=move |_| composer.update(|c| c.expand())
                        >
                            {format!("{}: {draft_title}", draft_label.clone())}
                        </button>
                        <button
                            type="button"
                            class="rounded-full p-1 text-muted-foreground hover:bg-background hover:text-foreground"
                            title=expand_label.clone()
                            on:click=move |_| composer.update(|c| c.expand())
                        >
                            "↗"
                        </button>
                        <button
                            type="button"
                            class="rounded-full p-1 text-muted-foreground hover:bg-background hover:text-foreground"
                            title=close_label.clone()
                            on:click=move |_| composer.update(|c| c.close())
                        >
                            "✕"
                        </button>
                    </div>
                }.into_any();
            }

            let mode_title = match state.mode {
                ComposerMode::Reply => match state.topic_title.as_deref() {
                    Some(topic_title) => format!("{}: {topic_title}", reply_label.clone()),
                    None => reply_label.clone(),
                },
                ComposerMode::Topic => new_topic_label.clone(),
            };

            let is_topic = state.mode == ComposerMode::Topic;
            let categories_clone = categories.clone();
            let quotes = state.quotes.clone();

            view! {
                <div class="fixed inset-x-0 bottom-0 z-50 flex justify-center p-2 sm:p-4 pointer-events-none">
                    <div class="pointer-events-auto flex max-h-[85vh] w-full max-w-4xl flex-col overflow-hidden rounded-t-2xl sm:rounded-2xl border border-border bg-card shadow-2xl transition-all">
                        // Header bar
                        <div class="flex items-center justify-between border-b border-border bg-muted/40 px-4 py-3">
                            <div class="flex items-center gap-2">
                                <span class="flex h-6 w-6 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary">
                                    "✎"
                                </span>
                                <span class="text-sm font-semibold text-foreground">
                                    {mode_title}
                                </span>
                            </div>

                            <div class="flex items-center gap-1">
                                <button
                                    type="button"
                                    class="rounded-lg p-1.5 text-xs text-muted-foreground transition hover:bg-background hover:text-foreground"
                                    title=minimize_label.clone()
                                    on:click=move |_| composer.update(|c| c.minimize())
                                >
                                    "−"
                                </button>
                                <button
                                    type="button"
                                    class="rounded-lg p-1.5 text-xs text-muted-foreground transition hover:bg-background hover:text-foreground"
                                    title=close_label.clone()
                                    on:click=move |_| composer.update(|c| c.close())
                                >
                                    "✕"
                                </button>
                            </div>
                        </div>

                        // Form body
                        <div class="flex flex-1 flex-col overflow-hidden">
                            {is_topic.then(|| {
                                let cats = categories_clone.clone();
                                view! {
                                    <div class="grid gap-3 border-b border-border p-4 sm:grid-cols-[1fr_200px]">
                                        <input
                                            type="text"
                                            placeholder=topic_title_placeholder.clone()
                                            disabled=move || composer.get().is_submitting
                                            prop:value=move || composer.get().title
                                            on:input=move |ev| {
                                                let val = event_target_value(&ev);
                                                composer.update(|c| c.title = val);
                                            }
                                            class="rounded-xl border border-border bg-background px-3.5 py-2 text-sm font-medium text-foreground outline-none transition focus:border-primary focus:ring-1 focus:ring-primary"
                                        />
                                        <select
                                            disabled=move || composer.get().is_submitting
                                            prop:value=move || {
                                                composer
                                                    .get()
                                                    .category_id
                                                    .clone()
                                                    .unwrap_or_default()
                                            }
                                            on:change=move |ev| {
                                                let val = event_target_value(&ev);
                                                composer.update(|c| c.select_category(if val.is_empty() { None } else { Some(val) }));
                                            }
                                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary focus:ring-1 focus:ring-primary"
                                        >
                                            <option value="">
                                                {category_placeholder.clone()}
                                            </option>
                                            {cats.into_iter().map(|cat| view! {
                                                <option value=cat.id.clone()>{cat.name}</option>
                                            }).collect_view()}
                                        </select>
                                    </div>
                                }
                            })}

                            // Quotes bar
                            {(!quotes.is_empty()).then(|| {
                                let qs = quotes.clone();
                                view! {
                                    <div class="flex flex-wrap gap-2 border-b border-border bg-muted/20 px-4 py-2">
                                        <span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                                            {format!("{}:", quotes_label.clone())}
                                        </span>
                                        {qs.into_iter().enumerate().map(|(idx, q)| view! {
                                            <span class="inline-flex items-center gap-1.5 rounded-full border border-border bg-background px-2.5 py-0.5 text-xs text-foreground">
                                                <span class="max-w-[150px] truncate text-muted-foreground">
                                                    {q.author_handle.map(|handle| {
                                                        format!("@{handle}")
                                                    }).unwrap_or_else(|| {
                                                        let short_id: String = q
                                                            .target_id
                                                            .chars()
                                                            .take(6)
                                                            .collect();
                                                        format!("#{short_id}")
                                                    })}
                                                </span>
                                                <button
                                                    type="button"
                                                    class="text-muted-foreground hover:text-destructive"
                                                    on:click=move |_| composer.update(|c| c.remove_quote(idx))
                                                >
                                                    "✕"
                                                </button>
                                            </span>
                                        }).collect_view()}
                                    </div>
                                }
                            })}

                            // Tab switches
                            <div class="flex items-center gap-2 border-b border-border px-4 py-1.5 text-xs">
                                <button
                                    type="button"
                                    class=move || {
                                        let base = "rounded-lg px-3 py-1 font-medium transition ";
                                        if active_tab.get() == "write" {
                                            format!("{base}bg-primary/10 text-primary")
                                        } else {
                                            format!("{base}text-muted-foreground hover:text-foreground")
                                        }
                                    }
                                    on:click=move |_| set_active_tab.set("write")
                                >
                                    {write_tab_label.clone()}
                                </button>
                                <button
                                    type="button"
                                    class=move || {
                                        let base = "rounded-lg px-3 py-1 font-medium transition ";
                                        if active_tab.get() == "preview" {
                                            format!("{base}bg-primary/10 text-primary")
                                        } else {
                                            format!("{base}text-muted-foreground hover:text-foreground")
                                        }
                                    }
                                    on:click=move |_| set_active_tab.set("preview")
                                >
                                    {preview_tab_label.clone()}
                                </button>
                            </div>

                            // Editor / Preview body
                            <div class="flex-1 overflow-y-auto p-4 min-h-[180px]">
                                {
                                    let body_placeholder = body_placeholder.clone();
                                    let preview_hint_label = preview_hint_label.clone();
                                    let preview_empty_label = preview_empty_label.clone();
                                    move || if active_tab.get() == "write" {
                                    view! {
                                        <textarea
                                            placeholder=body_placeholder.clone()
                                            disabled=move || composer.get().is_submitting
                                            prop:value=move || composer.get().content
                                            on:input=move |ev| {
                                                let val = event_target_value(&ev);
                                                composer.update(|c| c.content = val);
                                            }
                                            class="w-full min-h-[160px] rounded-xl border border-border bg-background p-3 text-sm text-foreground outline-none transition focus:border-primary focus:ring-1 focus:ring-primary font-mono resize-y"
                                        />
                                    }.into_any()
                                } else {
                                    let content = composer.get().content;
                                    view! {
                                        <div class="prose dark:prose-invert max-w-none text-sm text-foreground">
                                            <p class="italic text-muted-foreground">
                                                {format!("{}:", preview_hint_label.clone())}
                                            </p>
                                            <div class="mt-2 rounded-xl border border-dashed border-border p-4 whitespace-pre-wrap">
                                                {if content.trim().is_empty() {
                                                    preview_empty_label.clone()
                                                } else {
                                                    content
                                                }}
                                            </div>
                                        </div>
                                    }.into_any()
                                }}
                            </div>

                            // A rejected write stays visible: the composer does not
                            // reset or fake success, and the owner keeps the detailed
                            // reason server-side because transport errors are redacted.
                            {state.error.clone().map(|error| view! {
                                <div class="border-t border-destructive/30 bg-destructive/10 px-4 py-2 text-xs text-destructive">
                                    {format!("{}: {error}", submit_failed_label.clone())}
                                </div>
                            })}

                            // Footer actions
                            <div class="flex items-center justify-between border-t border-border bg-muted/20 px-4 py-3">
                                <span class="text-xs text-muted-foreground">
                                    {draft_active_label.clone()}
                                </span>

                                <div class="flex items-center gap-2">
                                    <button
                                        type="button"
                                        class="rounded-xl px-4 py-2 text-xs font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
                                        on:click=move |_| composer.update(|c| c.close())
                                    >
                                        {cancel_label.clone()}
                                    </button>
                                    <button
                                        type="button"
                                        disabled=move || !composer.get().can_submit()
                                        class="inline-flex items-center gap-2 rounded-xl bg-primary px-5 py-2 text-xs font-semibold text-primary-foreground shadow transition hover:bg-primary/90 disabled:opacity-60"
                                        on:click={
                                            let route_locale = route_locale.clone();
                                            move |_| {
                                                let locale = forum_storefront_content_locale(route_locale.as_deref());
                                                let mut submission = None;
                                                composer.update(|c| submission = c.begin_submission(locale));
                                                let Some(submission) = submission else { return; };
                                                spawn_local(async move {
                                                    let outcome = match submission {
                                                        ComposerSubmission::Topic(draft) => transport::create_storefront_forum_topic(draft)
                                                            .await.map(|created| created.topic_id),
                                                        ComposerSubmission::Reply(draft) => transport::create_storefront_forum_reply(draft)
                                                            .await.map(|created| created.topic_id),
                                                    };
                                                    match outcome {
                                                        Ok(target_topic_id) => {
                                                            composer.update(|c| c.reset());
                                                            if let Some(callback) = on_submitted {
                                                                callback.run(target_topic_id);
                                                            }
                                                        }
                                                        Err(error) => composer.update(|c| c.fail_submission(error.to_string())),
                                                    }
                                                });
                                            }
                                        }
                                    >
                                        {if state.is_submitting {
                                            submitting_label.clone()
                                        } else if is_topic {
                                            submit_topic_label.clone()
                                        } else {
                                            submit_reply_label.clone()
                                        }}
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            }.into_any()
        }}
    }
    .into_any()
}
