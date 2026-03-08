use leptos::prelude::*;

use super::{register_component, StorefrontComponentRegistration, StorefrontSlot};

pub fn register_components() {
    register_component(StorefrontComponentRegistration {
        id: "storefront-module-spotlight",
        module_slug: None,
        slot: StorefrontSlot::HomeAfterHero,
        order: 10,
        render: module_spotlight,
    });
}

fn module_spotlight() -> AnyView {
    view! {
        <section class="container-app">
            <div class="rounded-2xl border border-border bg-card p-6 shadow">
                <div class="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
                    <div>
                        <h3 class="text-2xl font-bold text-card-foreground">"Composable storefront modules"</h3>
                        <p class="mt-2 text-sm text-muted-foreground">
                            "Ship curated sections from optional packages without touching core."
                        </p>
                    </div>
                    <div class="flex flex-wrap gap-2 text-xs">
                        <span class="inline-flex items-center rounded-full bg-primary/10 px-2.5 py-0.5 font-medium text-primary">"Leptos"</span>
                        <span class="inline-flex items-center rounded-full bg-emerald-100 px-2.5 py-0.5 font-medium text-emerald-700">"Registry"</span>
                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-foreground">"Extensions"</span>
                    </div>
                </div>
            </div>
        </section>
    }
    .into_any()
}
