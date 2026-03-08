use leptos::prelude::*;

use super::{register_component, AdminComponentRegistration, AdminSlot};

pub fn register_components() {
    register_component(AdminComponentRegistration {
        id: "core-dashboard-module-status",
        module_slug: None,
        slot: AdminSlot::DashboardSection,
        order: 10,
        render: module_status_card,
    });
}

fn module_status_card() -> AnyView {
    view! {
        <div class="rounded-2xl bg-card border border-border p-6 shadow">
            <h4 class="text-lg font-semibold text-card-foreground">"Modules ready"</h4>
            <p class="mt-2 text-sm text-muted-foreground">
                "Frontend packages can register dashboard widgets dynamically."
            </p>
            <div class="mt-4 flex flex-wrap gap-2 text-xs">
                <span class="rounded-full bg-primary/10 px-3 py-1 font-semibold text-primary">
                    "Leptos"
                </span>
                <span class="rounded-full bg-emerald-50 px-3 py-1 font-semibold text-emerald-600 dark:bg-emerald-900/30 dark:text-emerald-400">
                    "Registry"
                </span>
                <span class="rounded-full bg-secondary px-3 py-1 font-semibold text-secondary-foreground">
                    "Dynamic"
                </span>
            </div>
        </div>
    }
    .into_any()
}
