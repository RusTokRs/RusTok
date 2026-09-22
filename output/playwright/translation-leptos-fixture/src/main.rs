use leptos::prelude::*;
use leptos_auth::{AuthContext, AuthSession, AuthUser};
use leptos_router::components::Router;
use rustok_translation_admin::TranslationAdmin;
use rustok_ui_core::UiRouteContext;

#[component]
fn TranslationFixture() -> impl IntoView {
    provide_context(AuthContext {
        user: RwSignal::new(Some(AuthUser {
            id: "browser-user".to_string(),
            email: "browser@example.test".to_string(),
            name: Some("Browser Reviewer".to_string()),
            role: "admin".to_string(),
        })),
        session: RwSignal::new(Some(AuthSession {
            token: "browser-token".to_string(),
            refresh_token: "browser-refresh-token".to_string(),
            expires_at: 4_102_444_800,
            tenant: "browser-tenant".to_string(),
        })),
        is_loading: RwSignal::new(false),
        error: RwSignal::new(None),
    });
    provide_context(UiRouteContext {
        locale: Some("en".to_string()),
        route_segment: Some("translation".to_string()),
        subpath: None,
        query: Default::default(),
    });

    view! {
        <main class="mx-auto max-w-7xl p-6">
            <TranslationAdmin />
        </main>
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
