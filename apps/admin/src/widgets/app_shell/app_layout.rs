use leptos::prelude::*;
use leptos_router::components::Outlet;

use crate::app::modules::init_modules;
use crate::app::providers::enabled_modules::EnabledModulesProvider;

use super::header::Header;
use super::sidebar::Sidebar;

#[component]
pub fn app_layout() -> impl IntoView {
    init_modules();

    view! {
        <EnabledModulesProvider>
            <div class="min-h-svh bg-background text-foreground md:flex">
                <Sidebar />
                <div class="flex min-w-0 flex-1 flex-col">
                    <Header />
                    <main class="flex-1 overflow-y-auto">
                        <Outlet />
                    </main>
                </div>
            </div>
        </EnabledModulesProvider>
    }
}
