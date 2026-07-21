use leptos::prelude::*;

use crate::core::NotificationInboxAvailability;
use crate::transport::load_notification_storefront_state;

#[component]
pub fn NotificationsView() -> impl IntoView {
    let state = load_notification_storefront_state();
    let (state_label, message) = match state.availability {
        NotificationInboxAvailability::Unavailable => (
            "unavailable",
            "The notification inbox is not available in this deployment yet.",
        ),
        NotificationInboxAvailability::Available => ("available", "Notification inbox"),
    };

    view! {
        <section
            class="rounded-lg border p-4"
            data-module="notifications"
            data-state=state_label
        >
            <h1 class="text-xl font-semibold">"Notifications"</h1>
            <p class="mt-2 text-sm text-muted-foreground">{message}</p>
        </section>
    }
}
