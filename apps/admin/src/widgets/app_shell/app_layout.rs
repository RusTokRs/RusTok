use leptos::prelude::*;
use leptos_router::components::Outlet;

use crate::app::modules::init_modules;
use crate::app::providers::enabled_modules::EnabledModulesProvider;

use super::header::Header;
use super::sidebar::Sidebar;

#[component]
pub fn app_layout() -> impl IntoView {
    init_modules();
    let (sidebar_open, set_sidebar_open) = signal(true);

    view! {
        <EnabledModulesProvider>
            <div class="h-svh overflow-hidden bg-background text-foreground md:flex">
                <Sidebar sidebar_open=sidebar_open />
                <div class="flex min-h-0 min-w-0 flex-1 flex-col">
                    <Header sidebar_open=sidebar_open set_sidebar_open=set_sidebar_open />
                    <main class="min-h-0 flex-1 overflow-y-auto">
                        <Outlet />
                    </main>
                </div>
            </div>
        </EnabledModulesProvider>
    }
}
