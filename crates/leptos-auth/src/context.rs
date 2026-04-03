use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_use::use_interval_fn;

use crate::api;
use crate::storage;
use crate::{AuthError, AuthSession, AuthUser};

fn now_unix_secs() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1000.0) as i64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

#[derive(Clone)]
pub struct AuthContext {
    pub user: RwSignal<Option<AuthUser>>,
    pub session: RwSignal<Option<AuthSession>>,
    pub is_loading: RwSignal<bool>,
    pub error: RwSignal<Option<String>>,
}

impl AuthContext {
    pub fn new() -> Self {
        let user = RwSignal::new(storage::load_user().ok());
        let session = RwSignal::new(storage::load_session().ok());
        let is_loading = RwSignal::new(false);
        let error = RwSignal::new(None);

        Self {
            user,
            session,
            is_loading,
            error,
        }
    }

    pub async fn sign_in(
        &self,
        email: String,
        password: String,
        tenant: String,
    ) -> Result<(), AuthError> {
        self.is_loading.set(true);
        self.error.set(None);

        let result = api::sign_in(email, password, tenant).await;

        match result {
            Ok((user, session)) => {
                let _ = storage::save_user(&user);
                let _ = storage::save_session(&session);
                self.user.set(Some(user));
                self.session.set(Some(session));
                self.is_loading.set(false);
                Ok(())
            }
            Err(e) => {
                self.is_loading.set(false);
                self.error.set(Some(format!("{}", e)));
                Err(e)
            }
        }
    }

    pub async fn sign_up(
        &self,
        email: String,
        password: String,
        name: Option<String>,
        tenant: String,
    ) -> Result<(), AuthError> {
        self.is_loading.set(true);
        self.error.set(None);

        let result = api::sign_up(email, password, name, tenant).await;

        match result {
            Ok((user, session)) => {
                let _ = storage::save_user(&user);
                let _ = storage::save_session(&session);
                self.user.set(Some(user));
                self.session.set(Some(session));
                self.is_loading.set(false);
                Ok(())
            }
            Err(e) => {
                self.is_loading.set(false);
                self.error.set(Some(format!("{}", e)));
                Err(e)
            }
        }
    }

    pub async fn sign_out(&self) -> Result<(), AuthError> {
        self.is_loading.set(true);

        if let Some(session) = self.session.get() {
            let _ = api::sign_out(
                session.token.clone(),
                session.refresh_token.clone(),
                session.tenant.clone(),
            )
            .await;
        }

        storage::clear_session();
        self.user.set(None);
        self.session.set(None);
        self.is_loading.set(false);

        Ok(())
    }

    pub fn is_token_expired(&self) -> bool {
        self.session
            .get()
            .map(|s| now_unix_secs() >= s.expires_at - 60)
            .unwrap_or(true)
    }

    pub fn secs_until_expiry(&self) -> i64 {
        self.session
            .get()
            .map(|s| s.expires_at - now_unix_secs())
            .unwrap_or(0)
    }

    pub async fn refresh_session(&self) -> Result<(), AuthError> {
        if let Some(session) = self.session.get() {
            let (new_session, new_user) =
                api::refresh_token(session.refresh_token.clone(), session.tenant.clone()).await?;
            let _ = storage::save_session(&new_session);
            let _ = storage::save_user(&new_user);
            self.session.set(Some(new_session));
            self.user.set(Some(new_user));
            Ok(())
        } else {
            Err(AuthError::Unauthorized)
        }
    }

    pub async fn fetch_current_user(&self) -> Result<(), AuthError> {
        if let Some(session) = self.session.get() {
            let user =
                api::fetch_current_user(session.token.clone(), session.tenant.clone()).await?;
            if let Some(ref u) = user {
                let _ = storage::save_user(u);
            }
            self.user.set(user);
            Ok(())
        } else {
            Err(AuthError::Unauthorized)
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.user.get().is_some() && self.session.get().is_some() && !self.is_token_expired()
    }

    pub fn get_token(&self) -> Option<String> {
        self.session.get().map(|s| s.token)
    }

    pub fn get_tenant(&self) -> Option<String> {
        self.session.get().map(|s| s.tenant)
    }
}

impl Default for AuthContext {
    fn default() -> Self {
        Self::new()
    }
}

#[component]
pub fn AuthProvider(children: Children) -> impl IntoView {
    let auth_context = AuthContext::new();

    provide_context(auth_context.clone());

    // On mount: fetch current user if a session exists in storage
    let auth_for_init = auth_context.clone();
    Effect::new(move |_| {
        if auth_for_init.session.get().is_some() {
            let auth = auth_for_init.clone();
            spawn_local(async move {
                let _ = auth.fetch_current_user().await;
            });
        }
    });

    // Auto-refresh: every 60 seconds check token expiry
    // - less than 5 minutes remaining → refresh token
    // - already expired → sign out
    let auth_for_interval = auth_context.clone();
    use_interval_fn(
        move || {
            let auth = auth_for_interval.clone();
            spawn_local(async move {
                if auth.session.get().is_none() {
                    return;
                }
                let secs = auth.secs_until_expiry();
                if secs <= 0 {
                    let _ = auth.sign_out().await;
                } else if secs < 300 {
                    let _ = auth.refresh_session().await;
                }
            });
        },
        60_000,
    );

    children()
}
