use leptos::prelude::*;

use crate::model::{MarketplaceSellerAdminEvent, MarketplaceSellerAdminEventHistory};

#[component]
pub fn MarketplaceSellerEventTimeline(
    history: MarketplaceSellerAdminEventHistory,
) -> impl IntoView {
    let seller_id = history.seller_id;
    let items = history.items;
    view! {
        <section class="space-y-3" data-seller-id=seller_id>
            <header class="flex items-center justify-between gap-3">
                <div>
                    <h3 class="text-sm font-semibold text-foreground">"Seller timeline"</h3>
                    <p class="text-xs text-muted-foreground">
                        "Immutable lifecycle, moderation, and membership history."
                    </p>
                </div>
                <span class="rounded border px-2 py-1 text-xs text-muted-foreground">
                    {items.len()} " events"
                </span>
            </header>
            {if items.is_empty() {
                view! {
                    <div class="rounded border border-dashed p-4 text-sm text-muted-foreground">
                        "No seller events recorded."
                    </div>
                }
                    .into_any()
            } else {
                view! {
                    <ol class="space-y-2">
                        {items.into_iter().map(event_row).collect_view()}
                    </ol>
                }
                    .into_any()
            }}
        </section>
    }
}

fn event_row(event: MarketplaceSellerAdminEvent) -> impl IntoView {
    let actor = event.actor_id.unwrap_or_else(|| "unknown actor".to_string());
    let locale = event.locale.unwrap_or_else(|| "unknown locale".to_string());
    let note = event.note.filter(|value| !value.trim().is_empty());
    let legacy = event.provenance == "legacy_snapshot";
    view! {
        <li class="rounded border bg-card p-3">
            <div class="flex flex-wrap items-center justify-between gap-2">
                <div class="flex flex-wrap items-center gap-2">
                    <span class="font-mono text-xs font-semibold text-foreground">
                        {event.event_kind}
                    </span>
                    <span class="rounded bg-muted px-1.5 py-0.5 text-[11px] text-muted-foreground">
                        {event.provenance}
                    </span>
                    {legacy.then(|| view! {
                        <span class="rounded bg-muted px-1.5 py-0.5 text-[11px] text-muted-foreground">
                            "imported snapshot"
                        </span>
                    })}
                </div>
                <time class="text-xs text-muted-foreground">{event.created_at}</time>
            </div>
            <div class="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
                <span>{actor}</span>
                <span>{locale}</span>
                <span class="font-mono">{event.id}</span>
            </div>
            {note.map(|value| view! {
                <p class="mt-2 whitespace-pre-wrap text-sm text-foreground">{value}</p>
            })}
        </li>
    }
}
