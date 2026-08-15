use leptos::prelude::*;
use leptos::task::spawn_local;
#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use uuid::Uuid;

use crate::core::{
    NotificationStorefrontGroupItemsRequest, NotificationStorefrontGroupItemsSnapshot,
    NotificationStorefrontGroupStateAction, NotificationStorefrontGroupStateCommand,
    NotificationStorefrontGroupSummary, NotificationStorefrontGroupSummaryRequest,
    NotificationStorefrontInboxSnapshot, NotificationStorefrontItem,
    NotificationStorefrontItemState, NotificationStorefrontOpenDecision,
    NotificationStorefrontOpenRequest, NotificationStorefrontPriority,
};
use crate::transport::{
    NativeNotificationStorefrontError, NotificationStorefrontTransportContext,
    apply_notification_group_state, authorize_notification_open,
    current_notification_storefront_transport_context, load_notification_group_items,
    load_notification_group_summaries, load_notification_group_summaries_selected,
    load_notification_unread_count_selected,
};

const SUMMARY_PAGE_SIZE: u16 = 20;
const ITEM_PAGE_SIZE: u16 = 20;
const GROUP_ACTION_PAGE_SIZE: u16 = 64;

#[component]
pub fn NotificationsView() -> impl IntoView {
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (refresh_feedback, set_refresh_feedback) = signal(Option::<String>::None);
    let transport_context = Memo::new(move |_| current_notification_storefront_transport_context());
    Effect::new(move |_| {
        let _ = transport_context.get();
        set_refresh_feedback.set(None);
    });
    let bootstrap = Resource::new_blocking(
        move || (refresh_nonce.get(), transport_context.get()),
        move |(_, context)| async move { load_inbox_snapshot(context).await },
    );
    let on_refresh = Callback::new(move |feedback: String| {
        set_refresh_feedback.set(Some(feedback));
        set_refresh_nonce.update(|value| *value = (*value).saturating_add(1));
    });

    view! {
        <section
            class="rounded-[2rem] border border-border bg-card p-6 shadow-sm md:p-8"
            data-module="notifications"
            data-state="available"
        >
            <Suspense fallback=|| view! { <NotificationInboxSkeleton /> }>
                {move || {
                    let bootstrap = bootstrap;
                    let on_refresh = on_refresh;
                    let initial_feedback = refresh_feedback.get();
                    Suspend::new(async move {
                        match bootstrap.await {
                            Ok(snapshot) => view! {
                                <NotificationInboxWorkspace
                                    initial=snapshot
                                    initial_feedback=initial_feedback
                                    on_refresh=on_refresh
                                />
                            }
                            .into_any(),
                            Err(error) => view! {
                                <NotificationInboxLoadError message=error.to_string() />
                            }
                            .into_any(),
                        }
                    })
                }}
            </Suspense>
        </section>
    }
}

#[component]
pub fn NotificationUnreadBadge(unread_count: u64) -> impl IntoView {
    let label = if unread_count == 0 {
        "No unread notifications".to_string()
    } else {
        format!("{unread_count} unread")
    };

    view! {
        <span
            class="inline-flex items-center rounded-full border border-border bg-background px-3 py-1 text-xs font-semibold text-card-foreground"
            data-notification-unread-count=unread_count.to_string()
        >
            {label}
        </span>
    }
}

#[component]
fn NotificationInboxWorkspace(
    initial: NotificationStorefrontInboxSnapshot,
    initial_feedback: Option<String>,
    on_refresh: Callback<String>,
) -> impl IntoView {
    let (snapshot, set_snapshot) = signal(initial);
    let (summary_busy, set_summary_busy) = signal(false);
    let (summary_error, set_summary_error) = signal(Option::<String>::None);
    let (expanded_group, set_expanded_group) = signal(Option::<String>::None);
    let (group_items, set_group_items) =
        signal(Option::<NotificationStorefrontGroupItemsSnapshot>::None);
    let (items_busy, set_items_busy) = signal(false);
    let (items_error, set_items_error) = signal(Option::<String>::None);
    let (items_request_nonce, set_items_request_nonce) = signal(0_u64);
    let (action_busy_group, set_action_busy_group) = signal(Option::<String>::None);
    let (open_busy_item, set_open_busy_item) = signal(Option::<String>::None);
    let (interaction_error, set_interaction_error) = signal(Option::<String>::None);
    let (interaction_feedback, set_interaction_feedback) = signal(initial_feedback);

    let load_more_groups = Callback::new(move |_: ()| {
        if summary_busy.get() {
            return;
        }
        let current = snapshot.get();
        if !current.has_more {
            return;
        }
        let Some(cursor) = current.next_cursor else {
            return;
        };

        set_summary_busy.set(true);
        set_summary_error.set(None);
        spawn_local(async move {
            match load_notification_group_summaries(NotificationStorefrontGroupSummaryRequest {
                cursor: Some(cursor),
                limit: SUMMARY_PAGE_SIZE,
            })
            .await
            {
                Ok(page) => {
                    set_snapshot.update(|state| {
                        state.append_page(page);
                    });
                }
                Err(error) => set_summary_error.set(Some(error.to_string())),
            }
            set_summary_busy.set(false);
        });
    });

    let toggle_group = Callback::new(move |group_key: String| {
        let request_nonce = items_request_nonce.get().saturating_add(1);
        set_items_request_nonce.set(request_nonce);
        if expanded_group.get().as_deref() == Some(group_key.as_str()) {
            set_expanded_group.set(None);
            set_group_items.set(None);
            set_items_error.set(None);
            set_items_busy.set(false);
            return;
        }

        set_expanded_group.set(Some(group_key.clone()));
        set_group_items.set(None);
        set_items_error.set(None);
        set_items_busy.set(true);
        let requested_group = group_key.clone();
        spawn_local(async move {
            let result = load_notification_group_items(NotificationStorefrontGroupItemsRequest {
                group_key: requested_group.clone(),
                state: None,
                cursor: None,
                limit: ITEM_PAGE_SIZE,
            })
            .await;
            if items_request_nonce.get() == request_nonce
                && expanded_group.get().as_deref() == Some(requested_group.as_str())
            {
                match result {
                    Ok(page) => set_group_items.set(Some(
                        NotificationStorefrontGroupItemsSnapshot::from_page(requested_group, page),
                    )),
                    Err(error) => set_items_error.set(Some(error.to_string())),
                }
                set_items_busy.set(false);
            }
        });
    });

    let load_more_items = Callback::new(move |_: ()| {
        if items_busy.get() {
            return;
        }
        let Some(current) = group_items.get() else {
            return;
        };
        if !current.has_more {
            return;
        }
        let Some(cursor) = current.next_cursor else {
            return;
        };
        let group_key = current.group_key.clone();
        let request_nonce = items_request_nonce.get().saturating_add(1);
        set_items_request_nonce.set(request_nonce);

        set_items_busy.set(true);
        set_items_error.set(None);
        spawn_local(async move {
            let result = load_notification_group_items(NotificationStorefrontGroupItemsRequest {
                group_key: group_key.clone(),
                state: None,
                cursor: Some(cursor),
                limit: ITEM_PAGE_SIZE,
            })
            .await;
            if items_request_nonce.get() == request_nonce
                && expanded_group.get().as_deref() == Some(group_key.as_str())
            {
                match result {
                    Ok(page) => set_group_items.update(|state| {
                        if let Some(state) =
                            state.as_mut().filter(|state| state.group_key == group_key)
                        {
                            state.append_page(page);
                        }
                    }),
                    Err(error) => set_items_error.set(Some(error.to_string())),
                }
                set_items_busy.set(false);
            }
        });
    });

    let apply_group_action = Callback::new(
        move |(group_key, action): (String, NotificationStorefrontGroupStateAction)| {
            if action_busy_group.get().is_some() {
                return;
            }
            set_action_busy_group.set(Some(group_key.clone()));
            set_interaction_error.set(None);
            set_interaction_feedback.set(None);
            let idempotency_key = format!("notifications-storefront-ui-{}", Uuid::new_v4());
            spawn_local(async move {
                match apply_notification_group_state(NotificationStorefrontGroupStateCommand {
                    group_key: group_key.clone(),
                    action,
                    cursor: None,
                    limit: GROUP_ACTION_PAGE_SIZE,
                    idempotency_key,
                })
                .await
                {
                    Ok(page) => {
                        let continuation = if page.has_more {
                            " More matching items remain; repeat the action after refresh."
                        } else {
                            ""
                        };
                        let feedback = format!(
                            "Updated {} of {} scanned notifications.{continuation}",
                            page.changed, page.scanned
                        );
                        set_interaction_feedback.set(Some(feedback.clone()));
                        set_items_request_nonce.update(|value| *value = (*value).saturating_add(1));
                        set_expanded_group.set(None);
                        set_group_items.set(None);
                        set_items_busy.set(false);
                        on_refresh.run(feedback);
                    }
                    Err(error) => set_interaction_error.set(Some(error.to_string())),
                }
                set_action_busy_group.set(None);
            });
        },
    );

    let open_notification = Callback::new(move |notification_id: String| {
        if open_busy_item.get().is_some() {
            return;
        }
        set_open_busy_item.set(Some(notification_id.clone()));
        set_interaction_error.set(None);
        spawn_local(async move {
            match authorize_notification_open(NotificationStorefrontOpenRequest { notification_id })
                .await
            {
                Ok(NotificationStorefrontOpenDecision::Allowed { route }) => {
                    if let Err(error) = navigate_to_route(route.as_str()) {
                        set_interaction_error.set(Some(error));
                    }
                }
                Ok(NotificationStorefrontOpenDecision::Unavailable) => {
                    set_interaction_error.set(Some(
                        "This notification target is no longer available.".to_string(),
                    ));
                }
                Err(error) => set_interaction_error.set(Some(error.to_string())),
            }
            set_open_busy_item.set(None);
        });
    });

    view! {
        <div class="space-y-6">
            <header class="flex flex-col gap-4 border-b border-border pb-6 sm:flex-row sm:items-start sm:justify-between">
                <div class="max-w-3xl space-y-2">
                    <span class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                        "Inbox"
                    </span>
                    <h1 class="text-3xl font-semibold text-card-foreground">"Notifications"</h1>
                    <p class="text-sm leading-6 text-muted-foreground">
                        "Grouped activity from sources you can still access. Counts and routes are revalidated by the notification owner."
                    </p>
                </div>
                {move || view! {
                    <NotificationUnreadBadge unread_count=snapshot.get().unread_count />
                }}
            </header>

            {move || interaction_error.get().map(|message| view! {
                <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">
                    {message}
                </div>
            })}
            {move || interaction_feedback.get().map(|message| view! {
                <div class="rounded-2xl border border-border bg-muted/40 px-4 py-3 text-sm text-card-foreground" role="status">
                    {message}
                </div>
            })}
            {move || summary_error.get().map(|message| view! {
                <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">
                    {format!("Could not load more groups: {message}")}
                </div>
            })}

            {move || {
                let current = snapshot.get();
                if current.groups.is_empty() {
                    view! {
                        <div class="rounded-3xl border border-dashed border-border p-8 text-center">
                            <h2 class="text-lg font-semibold text-card-foreground">"Your inbox is clear"</h2>
                            <p class="mt-2 text-sm text-muted-foreground">
                                "No currently available notification groups were returned."
                            </p>
                        </div>
                    }
                    .into_any()
                } else {
                    let cards = current
                        .groups
                        .into_iter()
                        .map(|group| view! {
                            <NotificationGroupCard
                                group=group
                                expanded_group=expanded_group
                                items=group_items
                                items_busy=items_busy
                                items_error=items_error
                                action_busy_group=action_busy_group
                                open_busy_item=open_busy_item
                                on_toggle=toggle_group
                                on_load_more_items=load_more_items
                                on_action=apply_group_action
                                on_open=open_notification
                            />
                        })
                        .collect_view();
                    view! {
                        <div class="space-y-4">{cards}</div>
                        <Show when=move || snapshot.get().has_more>
                            <div class="flex justify-center pt-2">
                                <button
                                    class="inline-flex items-center rounded-xl border border-border px-4 py-2 text-sm font-medium text-card-foreground disabled:cursor-not-allowed disabled:opacity-50"
                                    type="button"
                                    disabled=move || summary_busy.get()
                                    on:click=move |_| load_more_groups.run(())
                                >
                                    {move || if summary_busy.get() { "Loading..." } else { "Load more groups" }}
                                </button>
                            </div>
                        </Show>
                    }
                    .into_any()
                }
            }}
        </div>
    }
}

#[component]
fn NotificationGroupCard(
    group: NotificationStorefrontGroupSummary,
    expanded_group: ReadSignal<Option<String>>,
    items: ReadSignal<Option<NotificationStorefrontGroupItemsSnapshot>>,
    items_busy: ReadSignal<bool>,
    items_error: ReadSignal<Option<String>>,
    action_busy_group: ReadSignal<Option<String>>,
    open_busy_item: ReadSignal<Option<String>>,
    on_toggle: Callback<String>,
    on_load_more_items: Callback<()>,
    on_action: Callback<(String, NotificationStorefrontGroupStateAction)>,
    on_open: Callback<String>,
) -> impl IntoView {
    let group_key = group.group_key.clone();
    let title = group.latest_item.display_title();
    let body = group.latest_item.display_body();
    let item_count = group.item_count;
    let unread_count = group.unread_count;
    let expanded_key = group_key.clone();
    let toggle_key = group_key.clone();
    let toggle_label_key = group_key.clone();
    let items_group_key = StoredValue::new(group_key.clone());
    let items_key = group_key.clone();

    view! {
        <article class="rounded-3xl border border-border bg-background p-5 shadow-sm" data-group-key=group_key.clone()>
            <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                <button
                    class="min-w-0 flex-1 text-left"
                    type="button"
                    aria-expanded=move || expanded_group.get().as_deref() == Some(expanded_key.as_str())
                    on:click=move |_| on_toggle.run(toggle_key.clone())
                >
                    <div class="flex flex-wrap items-center gap-2">
                        <h2 class="text-lg font-semibold text-card-foreground">{title}</h2>
                        <Show when=move || { unread_count > 0 }>
                            <span class="rounded-full bg-primary px-2.5 py-1 text-xs font-semibold text-primary-foreground">
                                {format!("{unread_count} unread")}
                            </span>
                        </Show>
                    </div>
                    <p class="mt-2 text-sm leading-6 text-muted-foreground">{body}</p>
                    <p class="mt-3 text-xs text-muted-foreground">
                        {format!("{item_count} notifications · {}", group.latest_item.created_at)}
                    </p>
                    <span class="mt-3 inline-flex text-sm font-medium text-primary">
                        {move || if expanded_group.get().as_deref() == Some(toggle_label_key.as_str()) {
                            "Hide notifications"
                        } else {
                            "View notifications"
                        }}
                    </span>
                </button>

                <div class="flex flex-wrap gap-2">
                    <GroupActionButton
                        label="Mark read"
                        group_key=group_key.clone()
                        action=NotificationStorefrontGroupStateAction::MarkRead
                        busy_group=action_busy_group
                        on_action=on_action
                    />
                    <GroupActionButton
                        label="Mark unread"
                        group_key=group_key.clone()
                        action=NotificationStorefrontGroupStateAction::MarkUnread
                        busy_group=action_busy_group
                        on_action=on_action
                    />
                    <GroupActionButton
                        label="Archive"
                        group_key=group_key.clone()
                        action=NotificationStorefrontGroupStateAction::Archive
                        busy_group=action_busy_group
                        on_action=on_action
                    />
                </div>
            </div>

            <Show when=move || expanded_group.get().as_deref() == Some(items_key.as_str())>
                <div class="mt-5 border-t border-border pt-5">
                    {move || {
                        if items_busy.get() && items.get().is_none() {
                            return view! { <p class="text-sm text-muted-foreground">"Loading notifications..."</p> }.into_any();
                        }
                        if let Some(error) = items_error.get() {
                            return view! {
                                <p class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">
                                    {format!("Could not load this group: {error}")}
                                </p>
                            }.into_any();
                        }
                        let Some(current) = items.get() else {
                            return view! { <p class="text-sm text-muted-foreground">"No notifications loaded."</p> }.into_any();
                        };
                        if items_group_key.with_value(|key| current.group_key != *key) {
                            return view! { <p class="text-sm text-muted-foreground">"Loading notifications..."</p> }.into_any();
                        }
                        if current.items.is_empty() {
                            return view! { <p class="text-sm text-muted-foreground">"No currently available notifications remain in this group."</p> }.into_any();
                        }

                        let rows = current.items.into_iter().map(|item| view! {
                            <NotificationItemRow
                                item=item
                                open_busy_item=open_busy_item
                                on_open=on_open
                            />
                        }).collect_view();
                        let has_more = current.has_more;
                        view! {
                            <div class="space-y-3">{rows}</div>
                            <Show when=move || has_more>
                                <div class="pt-4">
                                    <button
                                        class="inline-flex items-center rounded-xl border border-border px-4 py-2 text-sm font-medium text-card-foreground disabled:cursor-not-allowed disabled:opacity-50"
                                        type="button"
                                        disabled=move || items_busy.get()
                                        on:click=move |_| on_load_more_items.run(())
                                    >
                                        {move || if items_busy.get() { "Loading..." } else { "Load more notifications" }}
                                    </button>
                                </div>
                            </Show>
                        }.into_any()
                    }}
                </div>
            </Show>
        </article>
    }
}

#[component]
fn GroupActionButton(
    label: &'static str,
    group_key: String,
    action: NotificationStorefrontGroupStateAction,
    busy_group: ReadSignal<Option<String>>,
    on_action: Callback<(String, NotificationStorefrontGroupStateAction)>,
) -> impl IntoView {
    let busy_key = group_key.clone();
    let command_key = group_key.clone();
    view! {
        <button
            class="inline-flex items-center rounded-xl border border-border px-3 py-2 text-xs font-medium text-card-foreground disabled:cursor-not-allowed disabled:opacity-50"
            type="button"
            data-action=action.as_str()
            disabled=move || busy_group.get().is_some()
            on:click=move |_| on_action.run((command_key.clone(), action))
        >
            {move || if busy_group.get().as_deref() == Some(busy_key.as_str()) {
                "Updating..."
            } else {
                label
            }}
        </button>
    }
}

#[component]
fn NotificationItemRow(
    item: NotificationStorefrontItem,
    open_busy_item: ReadSignal<Option<String>>,
    on_open: Callback<String>,
) -> impl IntoView {
    let item_id = item.id.clone();
    let busy_id = item_id.clone();
    let open_id = item_id.clone();
    let title = item.display_title();
    let body = item.display_body();
    let state = item_state_label(item.state);
    let priority = priority_label(item.priority);

    view! {
        <article class="rounded-2xl border border-border bg-card p-4" data-notification-id=item_id>
            <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                <div class="min-w-0">
                    <div class="flex flex-wrap items-center gap-2">
                        <h3 class="font-medium text-card-foreground">{title}</h3>
                        <span class="rounded-full border border-border px-2 py-0.5 text-[0.7rem] font-medium text-muted-foreground">
                            {state}
                        </span>
                        <span class="rounded-full border border-border px-2 py-0.5 text-[0.7rem] font-medium text-muted-foreground">
                            {priority}
                        </span>
                    </div>
                    <p class="mt-2 text-sm leading-6 text-muted-foreground">{body}</p>
                    <p class="mt-3 text-xs text-muted-foreground">
                        {format!("{} · {}", item.source, item.created_at)}
                    </p>
                </div>
                <button
                    class="inline-flex shrink-0 items-center rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:cursor-not-allowed disabled:opacity-50"
                    type="button"
                    disabled=move || open_busy_item.get().is_some()
                    on:click=move |_| on_open.run(open_id.clone())
                >
                    {move || if open_busy_item.get().as_deref() == Some(busy_id.as_str()) {
                        "Opening..."
                    } else {
                        "Open"
                    }}
                </button>
            </div>
        </article>
    }
}

#[component]
fn NotificationInboxSkeleton() -> impl IntoView {
    view! {
        <div class="space-y-6" aria-busy="true">
            <div class="space-y-3 border-b border-border pb-6">
                <div class="h-3 w-20 animate-pulse rounded bg-muted"></div>
                <div class="h-9 w-56 animate-pulse rounded bg-muted"></div>
                <div class="h-4 max-w-2xl animate-pulse rounded bg-muted"></div>
            </div>
            <div class="space-y-4">
                <div class="h-40 animate-pulse rounded-3xl bg-muted"></div>
                <div class="h-40 animate-pulse rounded-3xl bg-muted"></div>
            </div>
        </div>
    }
}

#[component]
fn NotificationInboxLoadError(message: String) -> impl IntoView {
    view! {
        <div class="rounded-3xl border border-destructive/30 bg-destructive/10 p-6" role="alert" data-state="unavailable">
            <h1 class="text-xl font-semibold text-destructive">"Notification inbox unavailable"</h1>
            <p class="mt-2 text-sm text-destructive">{message}</p>
        </div>
    }
}

async fn load_inbox_snapshot(
    context: NotificationStorefrontTransportContext,
) -> Result<NotificationStorefrontInboxSnapshot, NativeNotificationStorefrontError> {
    let unread = load_notification_unread_count_selected(context.clone())
        .await
        .map_err(|error| NativeNotificationStorefrontError(error.to_string()))?;
    let summaries = load_notification_group_summaries_selected(
        context,
        NotificationStorefrontGroupSummaryRequest {
            cursor: None,
            limit: SUMMARY_PAGE_SIZE,
        },
    )
    .await
    .map_err(|error| NativeNotificationStorefrontError(error.to_string()))?;
    Ok(NotificationStorefrontInboxSnapshot::new(
        unread.unread_count,
        summaries,
    ))
}

fn item_state_label(state: NotificationStorefrontItemState) -> &'static str {
    match state {
        NotificationStorefrontItemState::Unread => "Unread",
        NotificationStorefrontItemState::Seen => "Seen",
        NotificationStorefrontItemState::Read => "Read",
        NotificationStorefrontItemState::Archived => "Archived",
    }
}

fn priority_label(priority: NotificationStorefrontPriority) -> &'static str {
    match priority {
        NotificationStorefrontPriority::Low => "Low priority",
        NotificationStorefrontPriority::Normal => "Normal priority",
        NotificationStorefrontPriority::High => "High priority",
        NotificationStorefrontPriority::Urgent => "Urgent",
    }
}

fn navigate_to_route(route: &str) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        let window =
            web_sys::window().ok_or_else(|| "Browser navigation is unavailable.".to_string())?;
        window
            .location()
            .set_href(route)
            .map_err(|_| "The authorized notification route could not be opened.".to_string())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = route;
        Err("Browser navigation is unavailable.".to_string())
    }
}
