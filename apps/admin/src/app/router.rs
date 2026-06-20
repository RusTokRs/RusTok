use leptos::prelude::*;
use leptos_auth::components::ProtectedRoute;
use leptos_auth::context::AuthProvider;
use leptos_router::components::{ParentRoute, Route, Router, Routes};
use leptos_router::path;

use crate::pages::{
    cache::CachePage, dashboard::Dashboard, email_settings::EmailSettingsPage, events::EventsPage,
    installer::InstallerPage, module_admin::ModuleAdminPage, modules::Modules,
    not_found::NotFound, roles::RolesPage, workflow_detail::WorkflowDetailPage, workflows::Workflows,
};
use rustok_auth_admin::{
    Login, Register, ResetPassword, Profile, Security, Users, UserDetails, OAuthAppsPage,
};
use crate::shared::ui::LanguageToggle;
use crate::widgets::app_shell::AppLayout;
use crate::I18nContextProvider;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <I18nContextProvider>
            <AuthProvider>
                <Router>
                    <Routes fallback=|| view! { <NotFound /> }>
                        <Route path=path!("/login") view=|| view! { <Login language_toggle=|| view! { <LanguageToggle /> } /> } />
                        <Route path=path!("/register") view=|| view! { <Register language_toggle=|| view! { <LanguageToggle /> } /> } />
                        <Route path=path!("/reset") view=|| view! { <ResetPassword language_toggle=|| view! { <LanguageToggle /> } /> } />
                        <Route path=path!("/install") view=InstallerPage />

                        <ParentRoute path=path!("") view=ProtectedRoute>
                            <ParentRoute path=path!("") view=AppLayout>
                                <Route path=path!("/dashboard") view=Dashboard />
                                <Route path=path!("/profile") view=|| view! { <Profile language_toggle=|| view! { <LanguageToggle /> } /> } />
                                <Route path=path!("/security") view=Security />
                                <Route path=path!("/modules/:module_slug") view=ModuleAdminPage />
                                <Route
                                    path=path!("/modules/:module_slug/*module_path")
                                    view=ModuleAdminPage
                                />
                                <Route path=path!("/modules") view=Modules />
                                <Route path=path!("/users") view=Users />
                                <Route path=path!("/users/:id") view=UserDetails />
                                <Route path=path!("/apps") view=OAuthAppsPage />
                                <Route path=path!("/ai") view=rustok_ai_admin::AiAdmin />
                                <Route path=path!("/ai/diagnostics") view=rustok_ai_admin::AiAdmin />
                                <Route path=path!("/workflows") view=Workflows />
                                <Route path=path!("/workflows/:id") view=WorkflowDetailPage />
                                <Route path=path!("/roles") view=RolesPage />
                                <Route path=path!("/email") view=EmailSettingsPage />
                                <Route path=path!("/cache") view=CachePage />
                                <Route path=path!("/events") view=EventsPage />
                                <Route path=path!("") view=Dashboard />
                            </ParentRoute>
                        </ParentRoute>

                        <Route path=path!("/*") view=NotFound />
                    </Routes>
                </Router>
            </AuthProvider>
        </I18nContextProvider>
    }
}
