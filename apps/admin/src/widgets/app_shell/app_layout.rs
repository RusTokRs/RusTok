use leptos::prelude::*;
use leptos_router::components::Outlet;

use crate::app::modules::init_modules;
use crate::app::providers::enabled_modules::EnabledModulesProvider;

use super::header::header;
use super::sidebar::sidebar;

#[component]
pub fn app_layout() -> impl IntoView {
    init_modules();

    view! {
        <EnabledModulesProvider>
            <div class="flex h-screen bg-background text-foreground">
                {sidebar()}
                <div class="flex flex-1 flex-col overflow-hidden">
                    {header()}
                    <main class="flex-1 overflow-y-auto">
                        <Outlet />
                    </main>
                </div>
            </div>
        </EnabledModulesProvider>
    }
}
