use leptos::prelude::*;
use leptos_auth::components::ProtectedRoute;
use leptos_auth::context::AuthProvider;
use leptos_router::components::{ParentRoute, Redirect, Route, Router, Routes};
use leptos_router::SsrMode;
use leptos_router::path;

use crate::AdminLocaleProvider;
use crate::app::providers::enabled_modules::ModuleGuard;
use crate::pages::{
    dashboard::Dashboard, design_system::DesignSystemPage, email_settings::EmailSettingsPage,
    installer::InstallerPage, module_admin::ModuleAdminPage, modules::Modules, not_found::NotFound,
};
use crate::shared::ui::LanguageToggle;
use crate::widgets::app_shell::AppLayout;
use rustok_auth_admin::{
    AcceptInvite, Login, OAuthAppsPage, Profile, Register, ResetPassword, Security, UserDetails,
    Users,
};

#[component]
pub fn App() -> impl IntoView {
    view! {
        <AdminLocaleProvider>
            <AuthProvider>
                <Router>
                    <Routes fallback=|| view! { <NotFound /> }>
                        <Route path=path!("/login") view=|| view! { <Login language_toggle=|| view! { <LanguageToggle /> } /> } />
                        <Route path=path!("/register") view=|| view! { <Register language_toggle=|| view! { <LanguageToggle /> } /> } />
                        <Route path=path!("/accept-invite") view=|| view! { <AcceptInvite language_toggle=|| view! { <LanguageToggle /> } /> } />
                        <Route path=path!("/reset") view=|| view! { <ResetPassword language_toggle=|| view! { <LanguageToggle /> } /> } />
                        <Route path=path!("/install") view=InstallerPage />

                        <ParentRoute path=path!("") view=ProtectedRoute>
                            // The layout resolves module availability for every surface it wraps:
                            // the sidebar renders the module navigation and `ModuleGuard` reads the
                            // same decision. The default mode (`OutOfOrder`) streams that
                            // `Suspense` as a fallback plus a `<template>` that a script swaps in,
                            // and this host serves no script, so the decision has to be in the
                            // document. Declaring the mode on the layout route covers every child:
                            // a route listing takes the most restrictive mode of its ancestors.
                            // See DECISIONS/2026-11-13-standalone-admin-editor-delivery.md.
                            <ParentRoute path=path!("") view=AppLayout ssr=SsrMode::Async>
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
                                <Route path=path!("/ai") view=|| view! { <Redirect path="/modules/ai" /> } />
                                <Route path=path!("/ai/diagnostics") view=|| view! { <Redirect path="/modules/ai/diagnostics" /> } />
                                <Route
                                    path=path!("/mcp")
                                    view=|| view! {
                                        <ModuleGuard slug="mcp">
                                            <rustok_mcp_admin::McpAdmin />
                                        </ModuleGuard>
                                    }
                                />
                                <Route
                                    path=path!("/workflows")
                                    view=|| view! { <Redirect path="/modules/workflow" /> }
                                />
                                <Route
                                    path=path!("/workflows/:id")
                                    view=|| view! { <Redirect path="/modules/workflow" /> }
                                />
                                <Route
                                    path=path!("/email")
                                    view=|| view! {
                                        <ModuleGuard slug="email">
                                            <EmailSettingsPage />
                                        </ModuleGuard>
                                    }
                                />
                                <Route path=path!("/cache") view=|| view! { <Redirect path="/modules/cache" /> } />
                                <Route path=path!("/design-system") view=DesignSystemPage />
                                <Route path=path!("/events") view=|| view! { <Redirect path="/modules/events" /> } />
                                <Route path=path!("") view=Dashboard />
                            </ParentRoute>
                        </ParentRoute>

                        <Route path=path!("/*") view=NotFound />
                    </Routes>
                </Router>
            </AuthProvider>
        </AdminLocaleProvider>
    }
}
