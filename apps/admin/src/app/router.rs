use leptos::prelude::*;
use leptos_auth::components::ProtectedRoute;
use leptos_auth::context::AuthProvider;
use leptos_router::components::{ParentRoute, Route, Router, Routes};
use leptos_router::path;

use crate::pages::{
    dashboard::Dashboard, login::Login, modules::Modules, not_found::NotFound, profile::Profile,
    register::Register, reset::ResetPassword, security::Security, user_details::UserDetails,
    users::Users,
};
use crate::widgets::app_shell::AppLayout;

use super::providers::locale::provide_locale_context;

#[component]
pub fn App() -> impl IntoView {
    provide_locale_context();

    view! {
        <AuthProvider>
            <Router>
                <Routes fallback=|| view! { <NotFound /> }>
                    <Route path=path!("/login") view=Login />
                    <Route path=path!("/register") view=Register />
                    <Route path=path!("/reset") view=ResetPassword />

                    <ParentRoute path=path!("") view=ProtectedRoute>
                        <ParentRoute path=path!("") view=AppLayout>
                            <Route path=path!("/dashboard") view=Dashboard />
                            <Route path=path!("/profile") view=Profile />
                            <Route path=path!("/security") view=Security />
                            <Route path=path!("/modules") view=Modules />
                            <Route path=path!("/users") view=Users />
                            <Route path=path!("/users/:id") view=UserDetails />
                            <Route path=path!("") view=Dashboard />
                        </ParentRoute>
                    </ParentRoute>

                    <Route path=path!("/*") view=NotFound />
                </Routes>
            </Router>
        </AuthProvider>
    }
}
