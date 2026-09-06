use leptos::prelude::*;
use leptos_router::components::Outlet;
use leptos_router::hooks::use_navigate;

use crate::hooks::{use_is_authenticated, use_is_loading};

#[component]
pub fn ProtectedRoute() -> impl IntoView {
    let is_authenticated = use_is_authenticated();
    let is_loading = use_is_loading();
    let redirect_to = "/login".to_string();
    let navigate = use_navigate();

    Effect::new(move |_| {
        if !is_loading.get() && !is_authenticated.get() {
            navigate(&redirect_to, Default::default());
        }
    });

    view! {
        <Show
            when=move || is_authenticated.get()
            fallback=|| view! {
                <div class="flex items-center justify-center min-h-screen">
                    <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-gray-900"></div>
                </div>
            }
        >
            <Outlet />
        </Show>
    }
}

#[component]
pub fn GuestRoute() -> impl IntoView {
    let is_authenticated = use_is_authenticated();
    let is_loading = use_is_loading();
    let redirect_to = "/dashboard".to_string();
    let navigate = use_navigate();

    Effect::new(move |_| {
        if !is_loading.get() && is_authenticated.get() {
            navigate(&redirect_to, Default::default());
        }
    });

    view! {
        <Show
            when=move || !is_authenticated.get()
            fallback=|| view! {
                <div class="flex items-center justify-center min-h-screen">
                    <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-gray-900"></div>
                </div>
            }
        >
            <Outlet />
        </Show>
    }
}

#[component]
pub fn RequireAuth(children: ChildrenFn) -> impl IntoView {
    let is_authenticated = use_is_authenticated();

    view! {
        <Show
            when=move || is_authenticated.get()
            fallback=|| view! { <p>"Please sign in to view this content"</p> }
        >
            {children()}
        </Show>
    }
}
