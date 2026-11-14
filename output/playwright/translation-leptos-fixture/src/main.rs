use leptos::prelude::*;
use leptos_router::components::Router;
use rustok_auth_ui::ui::leptos::AuthProvider;
use rustok_translation_admin::TranslationAdmin;
use rustok_ui_core::UiRouteContext;

#[component]
fn TranslationFixture() -> impl IntoView {
    provide_context(UiRouteContext {
        locale: Some("en".to_string()),
        route_segment: Some("translation".to_string()),
        subpath: None,
        query: Default::default(),
    });

    view! {
        <AuthProvider tenant_slug="browser-tenant".to_string()>
            <main class="mx-auto max-w-7xl p-6">
                <TranslationAdmin />
            </main>
        </AuthProvider>
    }
}

fn main() {
    mount_to_body(|| {
        view! {
            <Router>
                <TranslationFixture />
            </Router>
        }
    });
}
