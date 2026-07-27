use leptos::prelude::*;
use leptos_auth::AuthContext;
use rustok_ui_core::UiRouteContext;

use super::leptos::NotificationUnreadBadge;
use crate::i18n::{t, with_count};
use crate::transport::{
    NotificationNavigationTransportContext, load_notification_navigation_unread_count,
};

#[component]
pub fn NotificationNavigation() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let href = route_context.module_route_base("notifications");
    let auth = use_context::<AuthContext>();
    let access_token = auth.as_ref().and_then(AuthContext::get_token);
    let tenant_slug = auth
        .as_ref()
        .and_then(AuthContext::get_tenant)
        .or_else(|| option_env!("RUSTOK_TENANT_SLUG").map(str::to_string));
    let context = NotificationNavigationTransportContext::new(access_token, tenant_slug);
    let unread = Resource::new_blocking(
        || (),
        move |_| {
            let context = context.clone();
            async move { load_notification_navigation_unread_count(context).await }
        },
    );
    let link_label = t(
        locale.as_deref(),
        "notifications.navigation.label",
        "Notifications",
    );

    view! {
        <Suspense fallback=|| ()>
            {move || {
                let unread = unread;
                let href = href.clone();
                let link_label = link_label.clone();
                let locale = locale.clone();
                Suspend::new(async move {
                    match unread.await {
                        Ok(count) => {
                            let unread_label = with_count(
                                t(
                                    locale.as_deref(),
                                    "notifications.navigation.unread",
                                    "{count} unread notifications",
                                ),
                                count.unread_count,
                            );
                            view! {
                                <a
                                    class="inline-flex items-center gap-2 rounded-md px-2 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                                    href=href
                                    aria-label=if count.unread_count > 0 {
                                        format!("{link_label}. {unread_label}")
                                    } else {
                                        link_label.clone()
                                    }
                                    data-notification-navigation="true"
                                >
                                    <span>{link_label}</span>
                                    <Show when=move || count.unread_count > 0>
                                        <NotificationUnreadBadge unread_count=count.unread_count />
                                    </Show>
                                </a>
                            }
                            .into_any()
                        }
                        Err(_) => view! { <span class="hidden" data-notification-navigation="unavailable"></span> }
                            .into_any(),
                    }
                })
            }}
        </Suspense>
    }
}
