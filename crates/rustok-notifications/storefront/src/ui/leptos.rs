use leptos::prelude::*;

use crate::core::NotificationInboxAvailability;
use crate::transport::load_notification_storefront_state;

#[component]
pub fn NotificationsView() -> impl IntoView {
    let state = load_notification_storefront_state();
    let unavailable = state.availability == NotificationInboxAvailability::Unavailable;
    view! {
        <section
            class="rounded-lg border p-4"
            data-module="notifications"
            data-state=move || if unavailable { "unavailable" } else { "available" }
        >
            <h1 class="text-xl font-semibold">"Notifications"</h1>
            <p class="mt-2 text-sm text-muted-foreground">
                {move || if unavailable {
                    "The notification inbox is not available in this deployment yet."
                } else {
                    "Notification inbox"
                }}
            </p>
        </section>
    }
}
