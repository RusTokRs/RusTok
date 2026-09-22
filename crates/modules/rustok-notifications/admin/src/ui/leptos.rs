use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::i18n::t;
use crate::transport::load_notifications_admin_status;

#[component]
pub fn NotificationsAdmin() -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let loc = locale.as_deref();

    let title = t(loc, "notifications.title", "Notifications");
    let subtitle = t(
        loc,
        "notifications.subtitle",
        "The semantic source registry is available. Inbox persistence and delivery operations are not enabled yet.",
    );
    let source_registry_label = t(loc, "notifications.sourceRegistry", "Source registry");
    let persistence_label = t(loc, "notifications.persistence", "Persistence");
    let delivery_label = t(loc, "notifications.delivery", "Delivery");

    let status = load_notifications_admin_status();
    let source_status = if status.source_registry_ready {
        t(loc, "notifications.status.ready", "Ready")
    } else {
        t(loc, "notifications.status.unavailable", "Unavailable")
    };
    let persistence_status = if status.persistence_ready {
        t(loc, "notifications.status.ready", "Ready")
    } else {
        t(loc, "notifications.status.planned", "Planned")
    };
    let delivery_status = if status.delivery_ready {
        t(loc, "notifications.status.ready", "Ready")
    } else {
        t(loc, "notifications.status.planned", "Planned")
    };

    view! {
        <section class="space-y-4" data-module="notifications" data-phase="source_registry">
            <header class="space-y-1">
                <h1 class="text-2xl font-semibold">{title}</h1>
                <p class="text-sm text-muted-foreground">{subtitle}</p>
            </header>
            <dl class="grid gap-3 sm:grid-cols-3">
                <div class="rounded-lg border p-3">
                    <dt class="text-sm text-muted-foreground">{source_registry_label}</dt>
                    <dd class="font-medium">{source_status}</dd>
                </div>
                <div class="rounded-lg border p-3">
                    <dt class="text-sm text-muted-foreground">{persistence_label}</dt>
                    <dd class="font-medium">{persistence_status}</dd>
                </div>
                <div class="rounded-lg border p-3">
                    <dt class="text-sm text-muted-foreground">{delivery_label}</dt>
                    <dd class="font-medium">{delivery_status}</dd>
                </div>
            </dl>
        </section>
    }
}
