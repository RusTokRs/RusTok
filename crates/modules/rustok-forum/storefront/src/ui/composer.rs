use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    forum_storefront_composer_has_text, forum_storefront_content_locale,
    forum_storefront_quote_revision,
};
use crate::i18n::t;
use crate::model::{
    CreateStorefrontForumReplyDraft, CreateStorefrontForumTopicDraft, ForumCategoryListItem,
    StorefrontForumComposerQuote,
};
use crate::transport;

/// A composer quote. `revision_id` pins the exact quoted source revision when the
/// caller already knows it; `None` keeps the quote as quoted text only, because
/// the owner contract rejects a fabricated revision.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComposerQuote {
    pub target_kind: String,
    pub target_id: String,
    pub revision_id: Option<i64>,
    pub author_handle: Option<String>,
    pub snippet: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ComposerMode {
    #[default]
    Reply,
    Topic,
}

#[derive(Clone, Debug, Default)]
pub struct ComposerState {
    pub is_open: bool,
    pub is_minimized: bool,
    pub mode: ComposerMode,
    pub topic_id: Option<String>,
    pub topic_title: Option<String>,
    pub category_id: Option<String>,
    pub parent_reply_id: Option<String>,
    pub title: String,
    pub content: String,
    pub quotes: Vec<ComposerQuote>,
    pub is_submitting: bool,
    pub error: Option<String>,
}

impl ComposerState {
    pub fn open_reply(
        &mut self,
        topic_id: String,
        topic_title: String,
        parent_reply_id: Option<String>,
    ) {
        self.mode = ComposerMode::Reply;
        self.topic_id = Some(topic_id);
        self.topic_title = Some(topic_title);
        self.parent_reply_id = parent_reply_id;
        self.title.clear();
        self.quotes.clear();
        self.error = None;
        self.is_open = true;
        self.is_minimized = false;
    }

    pub fn open_topic(&mut self, category_id: Option<String>) {
        self.mode = ComposerMode::Topic;
        self.topic_id = None;
        self.topic_title = None;
        self.category_id = category_id;
        self.parent_reply_id = None;
        self.title.clear();
        self.quotes.clear();
        self.error = None;
        self.is_open = true;
        self.is_minimized = false;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.is_minimized = false;
    }

    pub fn minimize(&mut self) {
        self.is_minimized = true;
    }

    pub fn expand(&mut self) {
        self.is_minimized = false;
    }

    pub fn append_quote(&mut self, quote: ComposerQuote, quote_text: Option<String>) {
        if !self
            .quotes
            .iter()
            .any(|q| q.target_id == quote.target_id && q.target_kind == quote.target_kind)
        {
            self.quotes.push(quote.clone());
        }

        if let Some(text) = quote_text {
            let quote_header = match &quote.author_handle {
                Some(handle) => format!("> @{handle}:\n"),
                None => "> Quote:\n".to_string(),
            };
            let quoted_lines = text
                .lines()
                .map(|l| format!("> {l}"))
                .collect::<Vec<_>>()
                .join("\n");
            let quote_block = format!("{quote_header}{quoted_lines}\n\n");
            self.content.push_str(&quote_block);
        }

        self.is_open = true;
        self.is_minimized = false;
    }

    pub fn remove_quote(&mut self, index: usize) {
        if index < self.quotes.len() {
            self.quotes.remove(index);
        }
    }

    pub fn reset(&mut self) {
        self.title.clear();
        self.content.clear();
        self.quotes.clear();
        self.error = None;
        self.is_submitting = false;
        self.is_open = false;
        self.is_minimized = false;
    }

    /// Whether the owner contract can accept the current draft.
    ///
    /// A topic needs a title and a category and a reply needs its topic. The owner
    /// rejects an empty discussion document, so the submit action stays disabled
    /// instead of sending a doomed write.
    pub fn can_submit(&self) -> bool {
        if self.is_submitting || !forum_storefront_composer_has_text(self.content.as_str()) {
            return false;
        }
        match self.mode {
            ComposerMode::Topic => {
                !self.title.trim().is_empty()
                    && self
                        .category_id
                        .as_deref()
                        .is_some_and(|category_id| !category_id.trim().is_empty())
            }
            ComposerMode::Reply => self
                .topic_id
                .as_deref()
                .is_some_and(|topic_id| !topic_id.trim().is_empty()),
        }
    }

    /// Quotes that may be sent as owner relations, i.e. only the ones pinned to a
    /// positive source revision. Unpinned quotes stay in the post body as text.
    fn transport_quotes(&self) -> Vec<StorefrontForumComposerQuote> {
        self.quotes
            .iter()
            .filter_map(|quote| {
                forum_storefront_quote_revision(quote.revision_id).map(|revision_id| {
                    StorefrontForumComposerQuote {
                        target_kind: quote.target_kind.clone(),
                        target_id: quote.target_id.clone(),
                        revision_id,
                    }
                })
            })
            .collect()
    }
}

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
                                            prop:value=move || composer.get().title
                                            on:input=move |ev| {
                                                let val = event_target_value(&ev);
                                                composer.update(|c| c.title = val);
                                            }
                                            class="rounded-xl border border-border bg-background px-3.5 py-2 text-sm font-medium text-foreground outline-none transition focus:border-primary focus:ring-1 focus:ring-primary"
                                        />
                                        <select
                                            prop:value=move || {
                                                composer
                                                    .get()
                                                    .category_id
                                                    .clone()
                                                    .unwrap_or_default()
                                            }
                                            on:change=move |ev| {
                                                let val = event_target_value(&ev);
                                                composer.update(|c| c.category_id = Some(val));
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
                                                let draft = composer.get();
                                                if !draft.can_submit() {
                                                    return;
                                                }
                                                let is_topic = matches!(draft.mode, ComposerMode::Topic);
                                                let locale = forum_storefront_content_locale(
                                                    route_locale.as_deref(),
                                                );
                                            let quotes = draft.transport_quotes();
                                            let body_text = draft.content.clone();
                                            let category_id = draft
                                                .category_id
                                                .clone()
                                                .unwrap_or_default();
                                            let title = draft.title.trim().to_string();
                                            let topic_id = draft
                                                .topic_id
                                                .clone()
                                                .unwrap_or_default();
                                            let parent_reply_id = draft.parent_reply_id.clone();
                                            composer.update(|c| {
                                                c.is_submitting = true;
                                                c.error = None;
                                            });
                                            spawn_local(async move {
                                                let outcome = if is_topic {
                                                    transport::create_storefront_forum_topic(
                                                        CreateStorefrontForumTopicDraft {
                                                            category_id,
                                                            title,
                                                            body_text,
                                                            locale,
                                                            quotes,
                                                        },
                                                    )
                                                    .await
                                                    .map(|created| created.topic_id)
                                                } else {
                                                    transport::create_storefront_forum_reply(
                                                        CreateStorefrontForumReplyDraft {
                                                            topic_id,
                                                            parent_reply_id,
                                                            body_text,
                                                            locale,
                                                            quotes,
                                                        },
                                                    )
                                                    .await
                                                    .map(|created| created.topic_id)
                                                };
                                                match outcome {
                                                    Ok(target_topic_id) => {
                                                        composer.update(|c| c.reset());
                                                        if let Some(callback) = on_submitted {
                                                            callback.run(target_topic_id);
                                                        }
                                                    }
                                                    Err(error) => composer.update(|c| {
                                                        c.is_submitting = false;
                                                        c.error = Some(error.to_string());
                                                    }),
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
