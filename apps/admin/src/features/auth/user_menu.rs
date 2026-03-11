use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;

use leptos_auth::hooks::{use_auth, use_current_user};

use crate::{t_string, use_i18n};

#[component]
pub fn UserMenu() -> impl IntoView {
    let i18n = use_i18n();
    let auth = use_auth();
    let current_user = use_current_user();

    let (open, set_open) = signal(false);

    let handle_logout = Callback::new(move |_| {
        let auth = auth.clone();
        spawn_local(async move {
            let _ = auth.sign_out().await;
        });
    });

    let toggle_menu = move |_| {
        set_open.update(|v| *v = !*v);
    };

    view! {
        <div class="relative">
            <button
                on:click=toggle_menu
                class="flex items-center gap-2 rounded-lg p-2 transition-colors hover:bg-accent"
            >
                <div class="flex h-8 w-8 items-center justify-center rounded-full bg-primary">
                    <span class="text-sm font-semibold text-primary-foreground">
                        {move || {
                            current_user
                                .get()
                                .and_then(|u| u.name.clone())
                                .and_then(|n| n.chars().next())
                                .map(|c| c.to_string())
                                .unwrap_or_else(|| t_string!(i18n, app.menu.userInitial).to_string())
                        }}
                    </span>
                </div>
                <div class="hidden text-left md:block">
                    <p class="text-sm font-medium text-foreground">
                        {move || {
                            current_user
                                .get()
                                .and_then(|u| u.name.clone())
                                .unwrap_or_else(|| t_string!(i18n, app.menu.defaultUser).to_string())
                        }}
                    </p>
                    <p class="text-xs text-muted-foreground">
                        {move || {
                            current_user
                                .get()
                                .map(|u| u.role.clone())
                                .unwrap_or_else(|| "user".to_string())
                        }}
                    </p>
                </div>
                <svg
                    class="h-4 w-4 text-muted-foreground"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    stroke-width="2"
                >
                    {move || if open.get() {
                        view! { <path d="M18 15l-6-6-6 6" /> }.into_any()
                    } else {
                        view! { <path d="M6 9l6 6 6-6" /> }.into_any()
                    }}
                </svg>
            </button>

            <Show when=move || open.get()>
                <div class="absolute right-0 z-50 mt-2 w-56 rounded-lg border border-border bg-popover py-1 shadow-md">
                    <div class="border-b border-border px-4 py-3">
                        <p class="text-sm font-medium text-popover-foreground">
                            {move || {
                                current_user
                                    .get()
                                    .and_then(|u| u.name.clone())
                                    .unwrap_or_else(|| t_string!(i18n, app.menu.defaultUser).to_string())
                            }}
                        </p>
                        <p class="truncate text-xs text-muted-foreground">
                            {move || {
                                current_user
                                    .get()
                                    .map(|u| u.email.clone())
                                    .unwrap_or_default()
                            }}
                        </p>
                    </div>

                    <div class="py-1">
                        <DropdownLink href="/profile" icon="user">
                            {move || t_string!(i18n, app.menu.profile)}
                        </DropdownLink>
                        <DropdownLink href="/security" icon="lock">
                            {move || t_string!(i18n, app.menu.security)}
                        </DropdownLink>
                    </div>

                    <div class="border-t border-border py-1">
                        <button
                            on:click=move |ev| handle_logout.run(ev)
                            class="flex w-full items-center gap-3 px-4 py-2 text-sm text-destructive transition-colors hover:bg-destructive/10"
                        >
                            <MenuIcon icon="logout" />
                            <span>{move || t_string!(i18n, app.menu.signOut)}</span>
                        </button>
                    </div>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn dropdown_link(href: &'static str, icon: &'static str, children: Children) -> impl IntoView {
    view! {
        <A
            href=href
            attr:class="flex items-center gap-3 px-4 py-2 text-sm text-popover-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
        >
            <MenuIcon icon=icon />
            <span>{children()}</span>
        </A>
    }
}

#[component]
fn menu_icon(icon: &'static str) -> impl IntoView {
    let path = match icon {
        "user" => "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8z",
        "lock" => "M19 11H5a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7a2 2 0 0 0-2-2zM7 11V7a5 5 0 0 1 10 0v4",
        "logout" => "M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9",
        _ => "M12 12m-10 0a10 10 0 1 0 20 0a10 10 0 1 0-20 0",
    };

    view! {
        <svg
            class="h-4 w-4 shrink-0"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <path d=path />
        </svg>
    }
}
