use leptos::prelude::*;

use super::{StorefrontComponentRegistration, StorefrontSlot, register_component};

#[cfg(feature = "pages-inline-edit")]
use super::{StorefrontPageRegistration, register_page};

#[cfg(feature = "pages-inline-edit")]
pub const PAGES_AUTHORING_ROUTE_SEGMENT: &str = "pages-authoring";
#[cfg(feature = "pages-inline-edit")]
pub const PAGES_AUTHORING_BOOTSTRAP_ASSET: &str = "/assets/pages-inline-edit-bootstrap.js";

pub fn register_components() {
    register_component(StorefrontComponentRegistration {
        id: "storefront-module-spotlight",
        module_slug: None,
        slot: StorefrontSlot::HomeAfterHero,
        order: 10,
        render: module_spotlight,
    });

    #[cfg(feature = "pages-inline-edit")]
    register_page(StorefrontPageRegistration {
        module_slug: "pages",
        route_segment: PAGES_AUTHORING_ROUTE_SEGMENT,
        title: "Pages inline editor",
        render: pages_authoring_surface,
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

#[cfg(feature = "pages-inline-edit")]
fn pages_authoring_surface() -> AnyView {
    use rustok_ui_core::UiRouteContext;

    let route = use_context::<UiRouteContext>();
    let locale = route
        .as_ref()
        .and_then(|context| context.locale.clone())
        .unwrap_or_else(|| "en".to_string());
    let page_id = route
        .as_ref()
        .and_then(|context| context.query.get("page_id"))
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let Some(page_id) = page_id else {
        return view! {
            <section class="rounded-2xl border border-destructive/30 bg-destructive/10 p-6" role="alert">
                <h2 class="text-xl font-semibold text-destructive">"Page id is required"</h2>
                <p class="mt-2 text-sm text-destructive">
                    "Open this authenticated route with a non-empty `page_id` query parameter."
                </p>
                <a class="mt-4 inline-flex text-sm font-medium underline" href="/admin/pages">
                    "Return to Pages administration"
                </a>
            </section>
        }
        .into_any();
    };

    let admin_page_href = format!("/admin/pages/{page_id}");
    view! {
        <section
            id="pages-inline-edit-client-root"
            class="space-y-5"
            data-pages-page-id=page_id.clone()
            data-pages-locale=locale.clone()
            data-pages-authoring-route="true"
        >
            <nav aria-label="Pages authoring navigation" class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border bg-card px-4 py-3">
                <a class="text-sm font-medium underline" href=admin_page_href>
                    "Back to Pages administration"
                </a>
                <span class="text-xs text-muted-foreground">"Authenticated draft editing · changes save through Pages"</span>
            </nav>
            <rustok_pages_storefront::PagesAuthenticatedInlineEditSurface
                pages_page_id=page_id.clone()
                locale=locale.clone()
                class="min-h-[28rem] rounded-2xl border border-border bg-card p-4 shadow-sm".to_string()
            />
            <script
                type="module"
                src=PAGES_AUTHORING_BOOTSTRAP_ASSET
                data-pages-inline-edit-client="true"
            ></script>
        </section>
    }
    .into_any()
}

#[cfg(all(feature = "pages-inline-edit-hydrate", target_arch = "wasm32"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start_pages_inline_edit_client() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(root) = document.get_element_by_id("pages-inline-edit-client-root") else {
        return;
    };
    let Some(page_id) = root
        .get_attribute("data-pages-page-id")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let locale = root
        .get_attribute("data-pages-locale")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "en".to_string());
    let admin_page_href = format!("/admin/pages/{page_id}");
    let Some(body) = document.body() else {
        return;
    };

    console_error_panic_hook::set_once();
    body.set_inner_html("");
    mount_to_body(move || {
        view! {
            <main class="min-h-screen bg-background px-4 py-6 text-foreground">
                <section class="mx-auto max-w-7xl space-y-5">
                    <nav aria-label="Pages authoring navigation" class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border bg-card px-4 py-3">
                        <a class="text-sm font-medium underline" href=admin_page_href.clone()>
                            "Back to Pages administration"
                        </a>
                        <span class="text-xs text-muted-foreground">"Authenticated draft editing · changes save through Pages"</span>
                    </nav>
                    <rustok_pages_storefront::PagesAuthenticatedInlineEditSurface
                        pages_page_id=page_id.clone()
                        locale=locale.clone()
                        class="min-h-[28rem] rounded-2xl border border-border bg-card p-4 shadow-sm".to_string()
                    />
                </section>
            </main>
        }
    });
}
