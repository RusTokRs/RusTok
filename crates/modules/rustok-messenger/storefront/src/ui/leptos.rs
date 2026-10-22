use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;
use uuid::Uuid;

use crate::core::{
    ChatMessageViewModel, ContextDrawerTab, ConversationFilter, DeliveryStatus, MessengerLabels,
    MessengerWorkspaceState,
};
use crate::transport;

fn local_resource<S, Fut, T>(
    source: impl Fn() -> S + 'static,
    fetcher: impl Fn(S) -> Fut + 'static,
) -> LocalResource<T>
where
    S: 'static,
    Fut: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    LocalResource::new(move || fetcher(source()))
}

/// Full-page 3-column workplace collaboration workstation interface.
#[component]
pub fn MessengerWorkstation() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let labels = StoredValue::new(MessengerLabels::resolve(route_context.locale.as_deref()));

    let (active_filter, set_active_filter) = signal(ConversationFilter::All);
    let (search_query, set_search_query) = signal(String::new());
    let (active_conv_id, set_active_conv_id) = signal(None::<Uuid>);
    let (active_drawer, set_active_drawer) = signal(None::<ContextDrawerTab>);
    let (composer_text, set_composer_text) = signal(String::new());
    let (replying_to, set_replying_to) = signal(None::<ChatMessageViewModel>);
    let (composer_error, set_composer_error) = signal(None::<String>);

    // Conversations resource
    let conversations_resource = local_resource(
        move || (token.get(), tenant.get()),
        move |(tok, ten)| async move { transport::fetch_conversations(tok, ten).await },
    );

    // Messages resource re-triggered when active conversation changes
    let messages_resource = local_resource(
        move || (token.get(), tenant.get(), active_conv_id.get()),
        move |(tok, ten, conv_opt)| async move {
            if let Some(conv_id) = conv_opt {
                transport::fetch_messages(tok, ten, conv_id, None, Some(50)).await
            } else {
                Ok(Vec::new())
            }
        },
    );

    // Auto-select first conversation if none selected
    Effect::new(move |_| {
        if active_conv_id.get().is_none()
            && let Some(first_id) = conversations_resource
                .get()
                .and_then(|r| r.ok())
                .and_then(|convs| convs.first().map(|c| c.id))
        {
            set_active_conv_id.set(Some(first_id));
        }
    });

    let do_send = move || {
        let text = composer_text.get();
        let conv_opt = active_conv_id.get();

        if let Some(conv_id) = conv_opt {
            let mut state = MessengerWorkspaceState::new(Uuid::nil());
            state.active_conversation_id = Some(conv_id);
            state.set_replying_to(replying_to.get());

            match state.submit_optimistic_message(&text) {
                Ok((payload, _)) => {
                    set_composer_error.set(None);
                    set_composer_text.set(String::new());
                    set_replying_to.set(None);

                    let tok = token.get();
                    let ten = tenant.get();
                    leptos::task::spawn_local(async move {
                        if transport::send_message(tok, ten, payload).await.is_ok() {
                            messages_resource.refetch();
                        }
                    });
                }
                Err(err) => {
                    set_composer_error.set(Some(err.to_string()));
                }
            }
        }
    };

    let l = labels.get_value();

    view! {
        <div class="flex h-[calc(100vh-4rem)] w-full overflow-hidden rounded-xl border border-border bg-background shadow-lg">
            // Column 1: Conversations & Folders Sidebar
            <aside class="flex w-80 flex-shrink-0 flex-col border-r border-border bg-muted/20">
                // Sidebar Header & Search
                <div class="border-b border-border p-4 space-y-3">
                    <div class="flex items-center justify-between">
                        <h2 class="text-lg font-bold tracking-tight text-foreground">{l.title.clone()}</h2>
                    </div>
                    <div class="relative">
                        <input
                            type="text"
                            placeholder=l.search_placeholder.clone()
                            class="w-full rounded-lg border border-border bg-background px-3 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary"
                            prop:value=search_query
                            on:input=move |ev| set_search_query.set(event_target_value(&ev))
                        />
                    </div>
                    // Filter Tabs
                    <div class="flex space-x-1 overflow-x-auto text-xs">
                        <button
                            type="button"
                            class=move || {
                                if active_filter.get() == ConversationFilter::All {
                                    "rounded-md bg-primary px-2.5 py-1 text-primary-foreground font-medium"
                                } else {
                                    "rounded-md bg-muted/50 px-2.5 py-1 text-muted-foreground hover:bg-muted font-medium"
                                }
                            }
                            on:click=move |_| set_active_filter.set(ConversationFilter::All)
                        >
                            {labels.get_value().filter_all}
                        </button>
                        <button
                            type="button"
                            class=move || {
                                if active_filter.get() == ConversationFilter::Channels {
                                    "rounded-md bg-primary px-2.5 py-1 text-primary-foreground font-medium"
                                } else {
                                    "rounded-md bg-muted/50 px-2.5 py-1 text-muted-foreground hover:bg-muted font-medium"
                                }
                            }
                            on:click=move |_| set_active_filter.set(ConversationFilter::Channels)
                        >
                            {labels.get_value().filter_channels}
                        </button>
                        <button
                            type="button"
                            class=move || {
                                if active_filter.get() == ConversationFilter::Direct {
                                    "rounded-md bg-primary px-2.5 py-1 text-primary-foreground font-medium"
                                } else {
                                    "rounded-md bg-muted/50 px-2.5 py-1 text-muted-foreground hover:bg-muted font-medium"
                                }
                            }
                            on:click=move |_| set_active_filter.set(ConversationFilter::Direct)
                        >
                            {labels.get_value().filter_direct}
                        </button>
                        <button
                            type="button"
                            class=move || {
                                if active_filter.get() == ConversationFilter::EntityThreads {
                                    "rounded-md bg-primary px-2.5 py-1 text-primary-foreground font-medium"
                                } else {
                                    "rounded-md bg-muted/50 px-2.5 py-1 text-muted-foreground hover:bg-muted font-medium"
                                }
                            }
                            on:click=move |_| set_active_filter.set(ConversationFilter::EntityThreads)
                        >
                            {labels.get_value().filter_entities}
                        </button>
                    </div>
                </div>

                // Conversations List
                <div class="flex-1 overflow-y-auto divide-y divide-border/40">
                    {
                        move || {
                            let raw_convs = conversations_resource.get().and_then(|r| r.ok()).unwrap_or_default();
                            let mut state = MessengerWorkspaceState::new(Uuid::nil());
                            state.set_conversations(&raw_convs);
                            state.set_filter(active_filter.get());
                            state.set_search_query(search_query.get());

                            let filtered = state.filtered_conversations();
                            if filtered.is_empty() {
                                view! {
                                    <div class="p-6 text-center text-xs text-muted-foreground">
                                        {labels.get_value().empty_conversations}
                                    </div>
                                }.into_any()
                            } else {
                                filtered.into_iter().map(|c| {
                                    let conv_id = c.id;
                                    let is_active = active_conv_id.get() == Some(conv_id);
                                    let title = c.title.clone();
                                    let snippet = c.last_message_snippet.clone();
                                    let time = c.last_message_time.clone();
                                    let badge = c.unread_badge.clone();
                                    let is_entity = c.is_entity_thread;
                                    let entity_badge = c.entity_badge.clone();

                                    view! {
                                        <div
                                            class=move || {
                                                if is_active {
                                                    "cursor-pointer p-3 bg-muted/60 transition-colors"
                                                } else {
                                                    "cursor-pointer p-3 hover:bg-muted/30 transition-colors"
                                                }
                                            }
                                            on:click=move |_| set_active_conv_id.set(Some(conv_id))
                                        >
                                            <div class="flex items-center justify-between mb-1">
                                                <span class="font-medium text-xs text-foreground truncate max-w-[170px]">{title}</span>
                                                <span class="text-[10px] text-muted-foreground">{time}</span>
                                            </div>
                                            <div class="flex items-center justify-between">
                                                <p class="text-xs text-muted-foreground truncate max-w-[200px]">{snippet}</p>
                                                {
                                                    if let Some(b) = badge {
                                                        view! {
                                                            <span class="rounded-full bg-primary px-1.5 py-0.5 text-[9px] font-bold text-primary-foreground">
                                                                {b}
                                                            </span>
                                                        }.into_any()
                                                    } else if is_entity {
                                                        view! {
                                                            <span class="rounded bg-secondary/80 px-1 py-0.5 text-[9px] text-secondary-foreground font-mono">
                                                                {entity_badge.unwrap_or_else(|| "UER".into())}
                                                            </span>
                                                        }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }
                                                }
                                            </div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>().into_any()
                            }
                        }
                    }
                </div>
            </aside>

            // Column 2: Active Chat Pane & Message History
            <main class="flex flex-1 flex-col overflow-hidden bg-background">
                // Chat Header
                <header class="flex h-14 items-center justify-between border-b border-border px-6">
                    <div class="flex items-center space-x-3">
                        <div>
                            <h3 class="text-sm font-bold text-foreground">
                                {
                                    move || {
                                        let raw_convs = conversations_resource.get().and_then(|r| r.ok()).unwrap_or_default();
                                        let mut state = MessengerWorkspaceState::new(Uuid::nil());
                                        state.set_conversations(&raw_convs);
                                        state.active_conversation_id = active_conv_id.get();
                                        state.active_conversation().map(|c| c.title.clone()).unwrap_or_else(|| "Select a chat".into())
                                    }
                                }
                            </h3>
                            <p class="text-[11px] text-muted-foreground">
                                {
                                    move || {
                                        let raw_convs = conversations_resource.get().and_then(|r| r.ok()).unwrap_or_default();
                                        let mut state = MessengerWorkspaceState::new(Uuid::nil());
                                        state.set_conversations(&raw_convs);
                                        state.active_conversation_id = active_conv_id.get();
                                        state.active_conversation().and_then(|c| c.topic.clone()).unwrap_or_else(|| labels.get_value().status_online)
                                    }
                                }
                            </p>
                        </div>
                    </div>

                    // Drawer Toggle Buttons
                    <div class="flex items-center space-x-2 text-xs">
                        <button
                            type="button"
                            class="rounded-md border border-border px-2.5 py-1 hover:bg-muted text-muted-foreground"
                            on:click=move |_| set_active_drawer.update(|d| {
                                *d = if *d == Some(ContextDrawerTab::Members) { None } else { Some(ContextDrawerTab::Members) };
                            })
                        >
                            {labels.get_value().drawer_members}
                        </button>
                        <button
                            type="button"
                            class="rounded-md border border-border px-2.5 py-1 hover:bg-muted text-muted-foreground"
                            on:click=move |_| set_active_drawer.update(|d| {
                                *d = if *d == Some(ContextDrawerTab::Media) { None } else { Some(ContextDrawerTab::Media) };
                            })
                        >
                            {labels.get_value().drawer_media}
                        </button>
                    </div>
                </header>

                // Messages Stream
                <div class="flex-1 overflow-y-auto p-6 space-y-4">
                    {
                        move || {
                            let raw_msgs = messages_resource.get().and_then(|r| r.ok()).unwrap_or_default();
                            let mut state = MessengerWorkspaceState::new(Uuid::nil());
                            state.set_messages(&raw_msgs);

                            let groups = state.grouped_messages();
                            if groups.is_empty() {
                                view! {
                                    <div class="flex h-full items-center justify-center text-xs text-muted-foreground">
                                        {labels.get_value().empty_messages}
                                    </div>
                                }.into_any()
                            } else {
                                groups.into_iter().map(|group| {
                                    let date_label = group.date_label;
                                    view! {
                                        <div class="space-y-4">
                                            // Date separator
                                            <div class="flex items-center justify-center my-2">
                                                <span class="rounded-full bg-muted/60 px-3 py-0.5 text-[10px] font-semibold text-muted-foreground">
                                                    {date_label}
                                                </span>
                                            </div>

                                            // Messages in this day
                                            {
                                                group.messages.into_iter().map(|m| {
                                                    let is_out = m.is_outgoing;
                                                    let content = m.content.clone();
                                                    let time = m.formatted_time.clone();
                                                    let sender = m.sender_display_name.clone();
                                                    let status = match m.delivery_status {
                                                        DeliveryStatus::Sending => "⏳",
                                                        DeliveryStatus::Sent => "✓",
                                                        DeliveryStatus::Read => "✓✓",
                                                        DeliveryStatus::Failed => "⚠️",
                                                    };

                                                    let m_clone = m.clone();
                                                    view! {
                                                        <div class=if is_out { "flex justify-end" } else { "flex justify-start" }>
                                                            <div
                                                                class=if is_out {
                                                                    "group relative max-w-md rounded-2xl rounded-tr-sm bg-primary px-4 py-2 text-primary-foreground shadow-sm"
                                                                } else {
                                                                    "group relative max-w-md rounded-2xl rounded-tl-sm bg-muted px-4 py-2 text-foreground shadow-sm"
                                                                }
                                                                on:dblclick=move |_| set_replying_to.set(Some(m_clone.clone()))
                                                            >
                                                                <div class="flex items-center justify-between gap-4 mb-0.5 text-[10px] opacity-75">
                                                                    <span class="font-medium">{sender}</span>
                                                                    <span>{time}</span>
                                                                </div>
                                                                <p class="text-xs whitespace-pre-wrap leading-relaxed">{content}</p>
                                                                {
                                                                    if is_out {
                                                                        view! {
                                                                            <span class="block text-right text-[9px] opacity-70 mt-0.5">{status}</span>
                                                                        }.into_any()
                                                                    } else {
                                                                        ().into_any()
                                                                    }
                                                                }
                                                            </div>
                                                        </div>
                                                    }
                                                }).collect::<Vec<_>>()
                                            }
                                        </div>
                                    }
                                }).collect::<Vec<_>>().into_any()
                            }
                        }
                    }
                </div>

                // Message Composer
                <footer class="border-t border-border p-4 bg-muted/10">
                    // Reply banner preview
                    {
                        move || replying_to.get().map(|rep| {
                            let content = rep.content;
                            let sender = rep.sender_display_name;
                            view! {
                                <div class="mb-2 flex items-center justify-between rounded-lg bg-muted/40 px-3 py-1.5 text-xs">
                                    <div class="truncate">
                                        <span class="font-bold text-primary mr-1">"↳ " {sender}":"</span>
                                        <span class="text-muted-foreground">{content}</span>
                                    </div>
                                    <button
                                        type="button"
                                        class="ml-2 text-muted-foreground hover:text-foreground font-bold"
                                        on:click=move |_| set_replying_to.set(None)
                                    >
                                        "✕"
                                    </button>
                                </div>
                            }
                        })
                    }

                    // Validation error alert
                    {
                        move || composer_error.get().map(|err| {
                            view! {
                                <div class="mb-2 text-xs text-destructive font-medium">{err}</div>
                            }
                        })
                    }

                    // Input & Send Row
                    <div class="flex items-end gap-2">
                        <textarea
                            class="flex-1 resize-none rounded-xl border border-border bg-background p-3 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary"
                            placeholder=l.composer_placeholder.clone()
                            prop:value=move || composer_text.get()
                            on:input=move |ev| set_composer_text.set(event_target_value(&ev))
                            on:keydown=move |ev: leptos::web_sys::KeyboardEvent| {
                                if ev.key() == "Enter" && !ev.shift_key() {
                                    ev.prevent_default();
                                    do_send();
                                }
                            }
                        >{move || composer_text.get()}</textarea>
                        <button
                            type="button"
                            class="rounded-xl bg-primary px-5 py-3 text-xs font-semibold text-primary-foreground shadow hover:bg-primary/90 transition-colors"
                            on:click=move |_| do_send()
                        >
                            {labels.get_value().composer_send}
                        </button>
                    </div>
                </footer>
            </main>

            // Column 3: Collapsible Context Drawer (Members, Media, Threads)
            {
                move || active_drawer.get().map(|tab| {
                    let title = match tab {
                        ContextDrawerTab::Members => labels.get_value().drawer_members,
                        ContextDrawerTab::Media => labels.get_value().drawer_media,
                        ContextDrawerTab::Threads => labels.get_value().drawer_threads,
                        ContextDrawerTab::Search => labels.get_value().search_placeholder,
                    };

                    view! {
                        <aside class="flex w-72 flex-shrink-0 flex-col border-l border-border bg-muted/10">
                            <header class="flex h-14 items-center justify-between border-b border-border px-4">
                                <h4 class="text-xs font-bold text-foreground">{title}</h4>
                                <button
                                    type="button"
                                    class="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground text-xs"
                                    on:click=move |_| set_active_drawer.set(None)
                                >
                                    "✕"
                                </button>
                            </header>
                            <div class="flex-1 p-4 text-xs text-muted-foreground">
                                "Details, participants, and channel media attachments appear here."
                            </div>
                        </aside>
                    }
                })
            }
        </div>
    }
}

/// Contextual floating chat widget dockable on product or order details pages.
#[component]
pub fn MessengerFloatingWidget(
    #[prop(optional)] _entity_type: Option<i16>,
    #[prop(optional)] _entity_id: Option<Uuid>,
    #[prop(optional)] title: Option<String>,
) -> impl IntoView {
    let _token = use_token();
    let _tenant = use_tenant();
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let labels = StoredValue::new(MessengerLabels::resolve(route_context.locale.as_deref()));

    let (is_open, set_is_open) = signal(false);
    let (input_text, set_input_text) = signal(String::new());

    let widget_title = title.unwrap_or_else(|| labels.get_value().widget_title);

    view! {
        <div class="fixed bottom-6 right-6 z-50">
            // Collapsed Launcher Button
            <Show
                when=move || !is_open.get()
                fallback=move || ()
            >
                <button
                    type="button"
                    class="flex h-14 w-14 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-2xl hover:scale-105 transition-transform"
                    on:click=move |_| set_is_open.set(true)
                >
                    <span class="text-xl">"💬"</span>
                </button>
            </Show>

            // Expanded Floating Chat Drawer
            <Show
                when=move || is_open.get()
                fallback=move || ()
            >
                <div class="flex h-[480px] w-88 flex-col rounded-2xl border border-border bg-background shadow-2xl overflow-hidden animate-in slide-in-from-bottom-5">
                    // Widget Header
                    <header class="flex h-12 items-center justify-between border-b border-border bg-primary px-4 text-primary-foreground">
                        <div class="flex items-center space-x-2">
                            <span class="text-sm font-bold">{widget_title.clone()}</span>
                        </div>
                        <div class="flex items-center gap-1">
                            <a
                                href=format!("/{}/messages", route_context.locale.as_deref().unwrap_or("en"))
                                class="rounded-full p-1 text-primary-foreground/80 hover:bg-primary-foreground/20 text-xs inline-flex items-center justify-center"
                                title="Open full screen / Открыть на весь экран"
                            >
                                <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                                    <polyline points="15 3 21 3 21 9"></polyline>
                                    <polyline points="9 21 3 21 3 15"></polyline>
                                    <line x1="21" y1="3" x2="14" y2="10"></line>
                                    <line x1="3" y1="21" x2="10" y2="14"></line>
                                </svg>
                            </a>
                            <button
                                type="button"
                                class="rounded-full p-1 text-primary-foreground/80 hover:bg-primary-foreground/20 text-xs"
                                on:click=move |_| set_is_open.set(false)
                            >
                                "✕"
                            </button>
                        </div>
                    </header>

                    // Message Stream
                    <div class="flex-1 overflow-y-auto p-4 space-y-3 bg-muted/10 text-xs">
                        <div class="rounded-xl bg-muted p-3 text-muted-foreground leading-relaxed">
                            {labels.get_value().empty_messages}
                        </div>
                    </div>

                    // Compact Composer
                    <footer class="border-t border-border p-3 bg-background flex items-center gap-2">
                        <input
                            type="text"
                            placeholder=labels.get_value().composer_placeholder
                            class="flex-1 rounded-lg border border-border bg-background px-3 py-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
                            prop:value=input_text
                            on:input=move |ev| set_input_text.set(event_target_value(&ev))
                        />
                        <button
                            type="button"
                            class="rounded-lg bg-primary px-3 py-2 text-xs font-semibold text-primary-foreground hover:bg-primary/90"
                            on:click=move |_| set_input_text.set(String::new())
                        >
                            {labels.get_value().composer_send}
                        </button>
                    </footer>
                </div>
            </Show>
        </div>
    }
}

/// Storefront page view mounting the workplace collaboration workstation.
#[component]
pub fn MessengerView() -> impl IntoView {
    view! {
        <div class="w-full">
            <MessengerWorkstation />
        </div>
    }
}
