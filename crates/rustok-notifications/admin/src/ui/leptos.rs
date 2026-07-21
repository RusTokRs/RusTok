use leptos::prelude::*;

use crate::transport::load_notifications_admin_status;

#[component]
pub fn NotificationsAdmin() -> impl IntoView {
    let status = load_notifications_admin_status();
    let source_status = if status.source_registry_ready {
        "Ready"
    } else {
        "Unavailable"
    };
    let persistence_status = if status.persistence_ready {
        "Ready"
    } else {
        "Planned"
    };
    let delivery_status = if status.delivery_ready {
        "Ready"
    } else {
        "Planned"
    };

    view! {
        <section class="space-y-4" data-module="notifications" data-phase="source_registry">
            <header class="space-y-1">
                <h1 class="text-2xl font-semibold">"Notifications"</h1>
                <p class="text-sm text-muted-foreground">
                    "The semantic source registry is available. Inbox persistence and delivery operations are not enabled yet."
                </p>
            </header>
            <dl class="grid gap-3 sm:grid-cols-3">
                <div class="rounded-lg border p-3">
                    <dt class="text-sm text-muted-foreground">"Source registry"</dt>
                    <dd class="font-medium">{source_status}</dd>
                </div>
                <div class="rounded-lg border p-3">
                    <dt class="text-sm text-muted-foreground">"Persistence"</dt>
                    <dd class="font-medium">{persistence_status}</dd>
                </div>
                <div class="rounded-lg border p-3">
                    <dt class="text-sm text-muted-foreground">"Delivery"</dt>
                    <dd class="font-medium">{delivery_status}</dd>
                </div>
            </dl>
        </section>
    }
}
