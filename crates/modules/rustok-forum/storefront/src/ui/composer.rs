use leptos::prelude::*;

use crate::model::ForumCategoryListItem;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComposerQuote {
    pub target_kind: String,
    pub target_id: String,
    pub revision_id: i64,
    pub author_handle: Option<String>,
    pub snippet: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ComposerMode {
    Reply,
    Topic,
}

impl Default for ComposerMode {
    fn default() -> Self {
        Self::Reply
    }
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
        self.is_open = false;
        self.is_minimized = false;
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

    view! {
        {move || {
            let state = composer.get();
            if !state.is_open {
                return view! { <div class="hidden"></div> }.into_any();
            }

            if state.is_minimized {
                let draft_title = match state.mode {
                    ComposerMode::Reply => format!("Draft: {}", state.topic_title.as_deref().unwrap_or("Reply")),
                    ComposerMode::Topic => format!("Draft: {}", if state.title.is_empty() { "New Topic" } else { state.title.as_str() }),
                };

                return view! {
                    <div class="fixed bottom-4 right-4 z-50 flex items-center gap-2 rounded-full border border-border bg-card px-4 py-2.5 shadow-xl transition-all hover:bg-muted/80">
                        <span class="h-2 w-2 rounded-full bg-primary animate-pulse" />
                        <button
                            type="button"
                            class="text-xs font-medium text-foreground hover:underline"
                            on:click=move |_| composer.update(|c| c.expand())
                        >
                            {draft_title}
                        </button>
                        <button
                            type="button"
                            class="rounded-full p-1 text-muted-foreground hover:bg-background hover:text-foreground"
                            title="Expand"
                            on:click=move |_| composer.update(|c| c.expand())
                        >
                            "↗"
                        </button>
                        <button
                            type="button"
                            class="rounded-full p-1 text-muted-foreground hover:bg-background hover:text-foreground"
                            title="Close"
                            on:click=move |_| composer.update(|c| c.close())
                        >
                            "✕"
                        </button>
                    </div>
                }.into_any();
            }

            let mode_title = match state.mode {
                ComposerMode::Reply => format!("Replying to: {}", state.topic_title.as_deref().unwrap_or("Topic")),
                ComposerMode::Topic => "Create New Topic".to_string(),
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
                                    title="Minimize"
                                    on:click=move |_| composer.update(|c| c.minimize())
                                >
                                    "−"
                                </button>
                                <button
                                    type="button"
                                    class="rounded-lg p-1.5 text-xs text-muted-foreground transition hover:bg-background hover:text-foreground"
                                    title="Close"
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
                                            placeholder="Topic title..."
                                            prop:value=move || composer.get().title
                                            on:input=move |ev| {
                                                let val = event_target_value(&ev);
                                                composer.update(|c| c.title = val);
                                            }
                                            class="rounded-xl border border-border bg-background px-3.5 py-2 text-sm font-medium text-foreground outline-none transition focus:border-primary focus:ring-1 focus:ring-primary"
                                        />
                                        <select
                                            on:change=move |ev| {
                                                let val = event_target_value(&ev);
                                                composer.update(|c| c.category_id = Some(val));
                                            }
                                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary focus:ring-1 focus:ring-primary"
                                        >
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
                                            "Quotes:"
                                        </span>
                                        {qs.into_iter().enumerate().map(|(idx, q)| view! {
                                            <span class="inline-flex items-center gap-1.5 rounded-full border border-border bg-background px-2.5 py-0.5 text-xs text-foreground">
                                                <span class="max-w-[150px] truncate text-muted-foreground">
                                                    {q.author_handle.map(|h| format!("@{h}")).unwrap_or_else(|| format!("#{}", &q.target_id[..6.min(q.target_id.len())]))}
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
                                    "Write"
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
                                    "Preview"
                                </button>
                            </div>

                            // Editor / Preview body
                            <div class="flex-1 overflow-y-auto p-4 min-h-[180px]">
                                {move || if active_tab.get() == "write" {
                                    view! {
                                        <textarea
                                            placeholder="Write your post here in Markdown..."
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
                                            <p class="italic text-muted-foreground">"Live preview:"</p>
                                            <div class="mt-2 rounded-xl border border-dashed border-border p-4 whitespace-pre-wrap">
                                                {if content.is_empty() {
                                                    "Nothing to preview yet.".to_string()
                                                } else {
                                                    content
                                                }}
                                            </div>
                                        </div>
                                    }.into_any()
                                }}
                            </div>

                            // Footer actions
                            <div class="flex items-center justify-between border-t border-border bg-muted/20 px-4 py-3">
                                <span class="text-xs text-muted-foreground">
                                    "Draft active"
                                </span>

                                <div class="flex items-center gap-2">
                                    <button
                                        type="button"
                                        class="rounded-xl px-4 py-2 text-xs font-semibold text-muted-foreground transition hover:bg-muted hover:text-foreground"
                                        on:click=move |_| composer.update(|c| c.close())
                                    >
                                        "Cancel"
                                    </button>
                                    <button
                                        type="button"
                                        disabled=state.is_submitting
                                        class="inline-flex items-center gap-2 rounded-xl bg-primary px-5 py-2 text-xs font-semibold text-primary-foreground shadow transition hover:bg-primary/90 disabled:opacity-60"
                                        on:click=move |_| {
                                            let st = composer.get();
                                            if let Some(ref cb) = on_submitted {
                                                if let Some(t_id) = st.topic_id {
                                                    cb.run(t_id);
                                                }
                                            }
                                            composer.update(|c| c.reset());
                                        }
                                    >
                                        {if state.is_submitting {
                                            "Posting…"
                                        } else if is_topic {
                                            "Create Topic"
                                        } else {
                                            "Post Reply"
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
