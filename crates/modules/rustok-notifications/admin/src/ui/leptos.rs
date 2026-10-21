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
    let source_item = status.source_status();
    let source_status = t(loc, source_item.i18n_key(), source_item.default_label());
    let persistence_item = status.persistence_status();
    let persistence_status = t(
        loc,
        persistence_item.i18n_key(),
        persistence_item.default_label(),
    );
    let delivery_item = status.delivery_status();
    let delivery_status = t(loc, delivery_item.i18n_key(), delivery_item.default_label());

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
