use leptos::prelude::*;

use crate::context::AuthContext;
use crate::{AuthSession, AuthUser};

pub fn use_auth() -> AuthContext {
    use_context::<AuthContext>()
        .expect("AuthContext not found. Make sure to wrap your app with <AuthProvider>")
}

pub fn use_current_user() -> Signal<Option<AuthUser>> {
    let auth = use_auth();
    Signal::derive(move || auth.user.get())
}

pub fn use_session() -> Signal<Option<AuthSession>> {
    let auth = use_auth();
    Signal::derive(move || auth.session.get())
}

pub fn use_is_authenticated() -> Signal<bool> {
    let auth = use_auth();
    Signal::derive(move || auth.is_authenticated())
}

pub fn use_is_loading() -> Signal<bool> {
    let auth = use_auth();
    Signal::derive(move || auth.is_loading.get())
}

pub fn use_auth_error() -> Signal<Option<String>> {
    let auth = use_auth();
    Signal::derive(move || auth.error.get())
}

pub fn use_token() -> Signal<Option<String>> {
    let auth = use_auth();
    Signal::derive(move || auth.get_token())
}

pub fn use_tenant() -> Signal<Option<String>> {
    let auth = use_auth();
    Signal::derive(move || auth.get_tenant())
}

pub fn use_is_token_valid() -> Signal<bool> {
    let auth = use_auth();
    Signal::derive(move || !auth.is_token_expired())
}
