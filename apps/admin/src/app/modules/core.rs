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
    register_component(AdminComponentRegistration {
        id: "workflow-nav",
        module_slug: Some("workflow"),
        slot: AdminSlot::NavItem,
        order: 20,
        render: workflow_nav_item,
    });
}

fn workflow_nav_item() -> AnyView {
    use leptos_router::components::A;
    use leptos_router::hooks::use_location;

    let location = use_location();
    let is_active = move || location.pathname.get().starts_with("/workflows");

    view! {
        <A
            href="/workflows"
            attr:class=move || format!(
                "flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-all hover:bg-accent hover:text-accent-foreground {}",
                if is_active() { "bg-accent text-accent-foreground shadow-sm" } else { "text-muted-foreground" }
            )
        >
            <svg class="h-4 w-4 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
            </svg>
            "Workflows"
        </A>
    }
    .into_any()
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
