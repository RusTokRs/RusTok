use leptos::prelude::*;
use leptos_router::components::Outlet;

use crate::app::modules::init_modules;
use crate::app::providers::enabled_modules::EnabledModulesProvider;

use super::header::Header;
use super::sidebar::Sidebar;

#[component]
pub fn AppLayout() -> impl IntoView {
    init_modules();

    view! {
        <EnabledModulesProvider>
            <div class="flex h-screen bg-background text-foreground">
                <Sidebar />
                <div class="flex flex-1 flex-col overflow-hidden">
                    <Header />
                    <main class="flex-1 overflow-y-auto">
                        <Outlet />
                    </main>
                </div>
            </div>
        </EnabledModulesProvider>
    }
}
